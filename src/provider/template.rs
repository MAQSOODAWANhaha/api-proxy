use crate::auth::types::OAuthProviderConfig;
use crate::error::Result;
use crate::error::conversion::ConversionError;
use entity::oauth_client_sessions;
use minijinja::{AutoEscape, Environment, UndefinedBehavior};
use serde_json::Value;
use std::sync::OnceLock;

/// 模板渲染器（基于 minijinja）。
///
/// 说明：
/// - 语法：Jinja2 风格（兼容 `{{client_id}}` / `{{session.state}}` 这类占位符）
/// - 未定义变量：严格报错（避免静默生成错误请求）
/// - 自动转义：关闭（OAuth 参数通常是 URL/form 参数，不做 HTML 转义）
pub fn render_template(input: &str, context: &Value) -> Result<String> {
    if !input.contains("{{") && !input.contains("{%") && !input.contains("{#") {
        return Ok(input.to_string());
    }

    template_env()
        .render_str(input, context)
        .map_err(|e| ConversionError::message(format!("模板渲染失败: {e}")).into())
}

fn template_env() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        env.set_undefined_behavior(UndefinedBehavior::Strict);
        env.set_auto_escape_callback(|_| AutoEscape::None);
        env
    })
}

/// 将 JSON 值渲染为字符串（用于 query/body）。
///
/// - `null` 返回 `Ok(None)`（表示跳过该参数）
/// - `string` 会进行模板替换
/// - `bool/number` 直接转字符串
/// - `object/array` 会递归渲染内部字符串，然后序列化为字符串
pub fn render_json_value(value: &Value, context: &Value) -> Result<Option<String>> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(render_template(s, context)?)),
        Value::Bool(b) => Ok(Some(b.to_string())),
        Value::Number(n) => Ok(Some(n.to_string())),
        Value::Array(_) | Value::Object(_) => {
            let rendered = render_json_deep(value, context)?;
            Ok(Some(serde_json::to_string(&rendered).map_err(|e| {
                ConversionError::message(format!("序列化 JSON 参数失败: {e}"))
            })?))
        }
    }
}

fn render_json_deep(value: &Value, context: &Value) -> Result<Value> {
    match value {
        Value::Null => Ok(Value::Null),
        Value::Bool(b) => Ok(Value::Bool(*b)),
        Value::Number(n) => Ok(Value::Number(n.clone())),
        Value::String(s) => Ok(Value::String(render_template(s, context)?)),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(render_json_deep(item, context)?);
            }
            Ok(Value::Array(out))
        }
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                out.insert(k.clone(), render_json_deep(v, context)?);
            }
            Ok(Value::Object(out))
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OAuthTemplateRequest<'a> {
    pub authorization_code: Option<&'a str>,
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
    // 仅注入白名单字段：`session.*` 与数据库表字段绑定，避免配置方意外获得更多会话字段。
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
    root.insert("request".to_string(), Value::Object(request_obj));

    Value::Object(root)
}
