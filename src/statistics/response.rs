//! 响应侧统计采集

use pingora_http::ResponseHeader;

use crate::proxy::ProxyContext;
use crate::statistics::types::ResponseStats;

/// 收集响应详情并同步关键字段到上下文
#[must_use]
pub fn collect_details(
    upstream_response: &ResponseHeader,
    ctx: &mut ProxyContext,
) -> ResponseStats {
    // 收集响应头
    let mut headers = std::collections::HashMap::new();
    for (name, value) in &upstream_response.headers {
        if let Ok(value_str) = std::str::from_utf8(value.as_bytes()) {
            headers.insert(name.as_str().to_string(), value_str.to_string());
        }
    }

    // 获取Content-Type和Content-Length
    let content_type = upstream_response
        .headers
        .get("content-type")
        .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
        .map(std::string::ToString::to_string);

    let content_length = upstream_response
        .headers
        .get("content-length")
        .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
        .and_then(|s| s.parse::<i64>().ok());

    // 同步关键字段到上下文
    if let Some(ct) = &content_type {
        ctx.response_details.content_type = Some(ct.clone());
    }
    let content_encoding = upstream_response
        .headers
        .get("content-encoding")
        .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
        .map(str::to_lowercase);
    ctx.response_details.content_encoding = content_encoding;
    ctx.response_details.status_code = Some(upstream_response.status.as_u16());

    ResponseStats {
        status_code: upstream_response.status.as_u16(),
        headers,
        content_type,
        content_length,
    }
}
