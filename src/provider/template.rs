use crate::auth::types::OAuthProviderConfig;
use crate::error::Result;
use crate::error::conversion::ConversionError;
use entity::oauth_client_sessions;
use serde_json::Value;

const ALLOWED_SESSION_KEYS: &[&str] = &[
    "state",
    "code_verifier",
    "code_challenge",
    "refresh_token",
    "session_id",
];

const ALLOWED_REQUEST_KEYS: &[&str] = &["authorization_code", "refresh_token"];

/// 简易模板渲染器：将 `{{var}}` 替换为运行时值。
///
/// 说明：
/// - 仅支持字符串内的占位符替换，不支持表达式/函数。
/// - 未知变量会返回错误，避免静默生成错误请求。
pub fn render_template<F>(input: &str, mut lookup: F) -> Result<String>
where
    F: FnMut(&str) -> Option<String>,
{
    if !input.contains("{{") {
        return Ok(input.to_string());
    }

    let mut out = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(start) = rest.find("{{") {
        let (prefix, after_start) = rest.split_at(start);
        out.push_str(prefix);

        let after_start = &after_start[2..];
        let Some(end) = after_start.find("}}") else {
            return Err(ConversionError::message("模板占位符缺少结束标记 '}}'").into());
        };

        let (raw_key, after_end) = after_start.split_at(end);
        let key = raw_key.trim();
        let Some(value) = lookup(key) else {
            return Err(ConversionError::message(format!("未知模板变量: {key}")).into());
        };
        out.push_str(&value);
        rest = &after_end[2..];
    }

    out.push_str(rest);
    Ok(out)
}

/// 将 JSON 值渲染为字符串（用于 query/body）。
///
/// - `null` 返回 `Ok(None)`（表示跳过该参数）
/// - `string` 会进行模板替换
/// - `bool/number/object/array` 会序列化为字符串（不做模板替换）
pub fn render_json_value<F>(value: &Value, lookup: F) -> Result<Option<String>>
where
    F: FnMut(&str) -> Option<String>,
{
    match value {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(render_template(s, lookup)?)),
        Value::Bool(b) => Ok(Some(b.to_string())),
        Value::Number(n) => Ok(Some(n.to_string())),
        Value::Array(_) | Value::Object(_) => {
            Ok(Some(serde_json::to_string(value).map_err(|e| {
                ConversionError::message(format!("序列化 JSON 参数失败: {e}"))
            })?))
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OAuthTemplateRequest<'a> {
    pub authorization_code: Option<&'a str>,
    pub refresh_token: Option<&'a str>,
}

/// 构建 OAuth 模板上下文（仅暴露白名单字段）。
///
/// - `client_*`/`redirect_uri`/`scopes`：来自 provider 配置（数据库）
/// - `session.*`：来自 `oauth_client_sessions`（数据库），并按白名单过滤
/// - `request.*`：来自请求入参（如 exchange 的授权码）
pub fn build_oauth_template_context(
    config: &OAuthProviderConfig,
    session: &oauth_client_sessions::Model,
    request: OAuthTemplateRequest<'_>,
) -> Value {
    let mut root: serde_json::Map<String, Value> = serde_json::Map::new();
    root.insert(
        "client_id".to_string(),
        Value::String(config.client_id.clone()),
    );
    if let Some(secret) = &config.client_secret {
        root.insert("client_secret".to_string(), Value::String(secret.clone()));
    }
    root.insert(
        "redirect_uri".to_string(),
        Value::String(config.redirect_uri.clone()),
    );
    root.insert("scopes".to_string(), Value::String(config.scopes.clone()));
    root.insert(
        "pkce_required".to_string(),
        Value::Bool(config.pkce_required),
    );

    // 注入数据库可扩展字段（除保留命名空间外）
    for (k, v) in &config.extra {
        if matches!(k.as_str(), "session" | "request") {
            continue;
        }
        root.entry(k.clone()).or_insert_with(|| v.clone());
    }

    let mut session_obj: serde_json::Map<String, Value> = serde_json::Map::new();
    // 仅添加白名单字段
    session_obj.insert("state".to_string(), Value::String(session.state.clone()));
    session_obj.insert(
        "code_verifier".to_string(),
        Value::String(session.code_verifier.clone()),
    );
    session_obj.insert(
        "code_challenge".to_string(),
        Value::String(session.code_challenge.clone()),
    );
    session_obj.insert(
        "refresh_token".to_string(),
        session
            .refresh_token
            .clone()
            .map_or(Value::Null, Value::String),
    );
    session_obj.insert(
        "session_id".to_string(),
        Value::String(session.session_id.clone()),
    );
    root.insert("session".to_string(), Value::Object(session_obj));

    let mut request_obj: serde_json::Map<String, Value> = serde_json::Map::new();
    request_obj.insert(
        "authorization_code".to_string(),
        request
            .authorization_code
            .map_or(Value::Null, |v| Value::String(v.to_string())),
    );
    request_obj.insert(
        "refresh_token".to_string(),
        request
            .refresh_token
            .map_or(Value::Null, |v| Value::String(v.to_string())),
    );
    root.insert("request".to_string(), Value::Object(request_obj));

    Value::Object(root)
}

/// 从上下文中解析模板变量（支持 `a.b` 点路径）。
///
/// - `session.*`/`request.*` 会进行白名单校验
/// - 解析到 `null` 视为不可用（返回 `None`）
pub fn lookup_oauth_template(context: &Value, key: &str) -> Option<String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return None;
    }

    // `session.*`/`request.*` 必须白名单校验；其他字段完全由数据库配置驱动，不做额外限制。
    if trimmed == "session" || trimmed == "request" {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("session.") {
        // 仅允许 `session.<key>`（不允许更深层路径）
        if rest.contains('.') || !ALLOWED_SESSION_KEYS.contains(&rest) {
            return None;
        }
    }
    if let Some(rest) = trimmed.strip_prefix("request.")
        && (rest.contains('.') || !ALLOWED_REQUEST_KEYS.contains(&rest))
    {
        return None;
    }

    let resolved = resolve_dot_path(context, trimmed)?;
    json_value_to_string(resolved)
}

fn resolve_dot_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = root;
    for seg in path.split('.') {
        let Value::Object(map) = cur else {
            return None;
        };
        cur = map.get(seg)?;
    }
    Some(cur)
}

fn json_value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).ok(),
    }
}
