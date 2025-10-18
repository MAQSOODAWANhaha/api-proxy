//! # 时区中间件
//!
//! 用于解析 X-Timezone 头，并在请求上下文中提供时区信息。

use crate::types::TimezoneContext;
use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use chrono_tz::Tz;
use std::sync::Arc;

/// 时区中间件
pub async fn timezone_middleware(mut request: Request, next: Next) -> Result<Response, StatusCode> {
    // 从请求头中获取 X-Timezone
    let timezone_header = request
        .headers()
        .get("X-Timezone")
        .and_then(|header| header.to_str().ok());

    // 解析时区，如果无效则默认使用 UTC
    let timezone =
        timezone_header.map_or(Tz::UTC, |tz_str| tz_str.parse::<Tz>().unwrap_or(Tz::UTC));

    // 创建时区上下文
    let tz_context = TimezoneContext { timezone };

    // 将时区上下文注入到请求扩展中
    request.extensions_mut().insert(Arc::new(tz_context));

    // 继续处理请求
    Ok(next.run(request).await)
}

/// 获取请求中的时区上下文
pub fn get_timezone_from_request(request: &Request) -> Option<Arc<TimezoneContext>> {
    request.extensions().get::<Arc<TimezoneContext>>().cloned()
}
