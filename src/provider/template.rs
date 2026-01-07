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

/// 构建 OAuth 模板上下文（仅暴露白名单字段）。
///
/// - `client_*`/`redirect_uri`/`scopes`：来自 provider 配置（数据库）
/// - `session.*`：来自 `oauth_client_sessions`（数据库），并按白名单过滤
/// - `request.*`：来自请求入参（如 exchange 的授权码）
pub fn build_oauth_template_context(
    config: &OAuthProviderConfig,
    session: &oauth_client_sessions::Model,
    authorization_code: Option<&str>,
) -> Value {
    // 直接将数据库配置反序列化后的结构作为模板上下文根对象，避免代码侧硬编码 `client_id` 等字段名。
    // 注意：会覆盖/移除保留命名空间 `session`/`request`，再由下方按白名单注入运行时字段。
    let mut root: serde_json::Map<String, Value> = serde_json::to_value(config)
        .map_err(|e| ConversionError::message(format!("序列化 OAuthProviderConfig 失败: {e}")))
        .and_then(|v| match v {
            Value::Object(map) => Ok(map),
            _ => Err(ConversionError::message(
                "OAuthProviderConfig 序列化结果不是 object",
            )),
        })
        .unwrap_or_default();

    // 删除 `null` 字段：在 Strict 模式下，缺失变量会报错；而 `null` 会被当成已定义变量，容易掩盖配置问题。
    strip_null_fields(&mut root);

    // 保护保留命名空间：不允许数据库配置覆盖运行时注入的 `session`/`request`。
    root.remove("session");
    root.remove("request");

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
    if let Some(refresh_token) = &session.refresh_token {
        session_obj.insert(
            "refresh_token".to_string(),
            Value::String(refresh_token.clone()),
        );
    }
    session_obj.insert(
        "session_id".to_string(),
        Value::String(session.session_id.clone()),
    );
    root.insert("session".to_string(), Value::Object(session_obj));

    let mut request_obj: serde_json::Map<String, Value> = serde_json::Map::new();
    if let Some(code) = authorization_code {
        request_obj.insert(
            "authorization_code".to_string(),
            Value::String(code.to_string()),
        );
    }
    root.insert("request".to_string(), Value::Object(request_obj));

    Value::Object(root)
}

fn strip_null_fields(map: &mut serde_json::Map<String, Value>) {
    let keys_to_remove: Vec<String> = map
        .iter()
        .filter(|(_, v)| matches!(v, Value::Null))
        .map(|(k, _)| k.clone())
        .collect();

    for k in keys_to_remove {
        map.remove(&k);
    }
}
