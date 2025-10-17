use crate::error::auth::{AuthError, RateLimitInfo, RateLimitKind};
use bytes::Bytes;
use pingora_core::{Error as PingoraError, ErrorType, Result as PingoraResult};
use pingora_http::ResponseHeader;
use pingora_proxy::Session;
use serde_json::{Value, json};
use std::fmt::Write;

/// 统一的 JSON 错误响应结构
pub struct JsonError {
    pub status: u16,
    pub payload: Value,
    pub message: String,
}

#[must_use]
pub fn format_rate_limit_message(info: &RateLimitInfo) -> String {
    let kind_label = match info.kind {
        RateLimitKind::PerMinute => "每分钟请求",
        RateLimitKind::DailyRequests => "每日请求次数",
        RateLimitKind::DailyTokens => "每日 Token 用量",
        RateLimitKind::DailyCost => "每日成本",
    };

    let mut message = format!("已达到{kind_label}上限");

    match (info.limit, info.current) {
        (Some(limit), Some(current)) => {
            let _ = write!(
                message,
                "（限制 {}，当前 {}）",
                format_quantity(limit),
                format_quantity(current)
            );
        }
        (Some(limit), None) => {
            let _ = write!(message, "（限制 {}）", format_quantity(limit));
        }
        (None, Some(current)) => {
            let _ = write!(message, "（当前 {}）", format_quantity(current));
        }
        _ => {}
    }

    let _ = write!(message, "，套餐：{}", info.plan_type);

    if let Some(resets) = info.resets_in {
        let _ = write!(message, "，预计 {} 秒后重置", resets.as_secs());
    }

    message.push('。');
    message
}

#[must_use]
pub fn build_auth_error_response(err: &AuthError) -> JsonError {
    match err {
        AuthError::RateLimitExceeded(info) => {
            let message = format_rate_limit_message(info);
            let payload = json!({
                "error": {
                    "type": "usage_limit_reached",
                    "message": message,
                    "plan_type": info.plan_type
                }
            });
            JsonError {
                status: 429,
                payload,
                message,
            }
        }
        other => {
            let message = build_auth_failure_message(other);
            let payload = json!({
                "error": {
                    "type": "authentication_failed",
                    "message": message
                }
            });
            JsonError {
                status: 401,
                payload,
                message,
            }
        }
    }
}

pub async fn write_json_error(
    session: &mut Session,
    status: u16,
    payload: Value,
) -> PingoraResult<()> {
    let body = serde_json::to_vec(&payload).map_err(|err| {
        PingoraError::explain(
            ErrorType::InternalError,
            format!("Failed to serialize error payload: {err}"),
        )
    })?;

    let mut resp = ResponseHeader::build(status, Some(4)).map_err(|err| {
        PingoraError::explain(
            ErrorType::InternalError,
            format!("Failed to build error response header: {err}"),
        )
    })?;

    resp.insert_header("content-type", "application/json; charset=utf-8")
        .map_err(|err| {
            PingoraError::explain(
                ErrorType::InternalError,
                format!("Failed to set content-type header: {err}"),
            )
        })?;
    resp.insert_header("cache-control", "private, no-store")
        .map_err(|err| {
            PingoraError::explain(
                ErrorType::InternalError,
                format!("Failed to set cache-control header: {err}"),
            )
        })?;
    resp.set_content_length(body.len()).map_err(|err| {
        PingoraError::explain(
            ErrorType::InternalError,
            format!("Failed to set content-length: {err}"),
        )
    })?;

    session.write_response_header(Box::new(resp), false).await?;
    session
        .write_response_body(Some(Bytes::from(body)), true)
        .await?;
    Ok(())
}

fn format_quantity(value: f64) -> String {
    if (value.fract()).abs() < 1e-6 {
        format!("{:.0}", value.round())
    } else {
        format!("{value:.2}")
    }
}

fn build_auth_failure_message(err: &AuthError) -> String {
    match err {
        AuthError::ApiKeyMissing => "缺少认证信息，请在请求头或查询参数中提供 API Key".to_string(),
        AuthError::ApiKeyInvalid(reason) => {
            if reason.is_empty() {
                "提供的 API Key 无效".to_string()
            } else {
                format!("提供的 API Key 无效：{reason}")
            }
        }
        AuthError::ApiKeyMalformed => {
            "认证信息格式不正确，请确认使用 Bearer 或 X-API-Key 头".to_string()
        }
        AuthError::ApiKeyInactive => "该 API Key 已被禁用，请联系管理员启用后再试".to_string(),
        AuthError::NotAuthenticated => "尚未完成认证，请提供有效的凭据".to_string(),
        AuthError::PermissionDenied { required, actual } => {
            format!("权限不足，操作需要权限 {required}，当前权限 {actual}")
        }
        AuthError::HeaderParse(e) => format!("认证头解析失败：{e}"),
        AuthError::OAuth(e) => format!("OAuth 流程发生异常：{e}"),
        AuthError::Pkce(e) => format!("PKCE 验证失败：{e}"),
        AuthError::Message(msg) => msg.clone(),
        AuthError::RateLimitExceeded(info) => format_rate_limit_message(info),
    }
}
