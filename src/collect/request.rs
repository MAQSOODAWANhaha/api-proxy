//! 请求侧统计采集

use pingora_proxy::Session;

use crate::auth::AuthUtils;
use crate::collect::types::RequestDetails;
use crate::collect::types::RequestStats;

/// 收集请求统计信息（方法、路径、客户端信息）
#[must_use]
pub fn collect_stats(session: &Session) -> RequestStats {
    // 将Pingora headers转换为标准HeaderMap以便使用AuthUtils
    let mut headers = axum::http::HeaderMap::new();
    for (name, value) in &session.req_header().headers {
        if let Ok(header_name) = axum::http::HeaderName::from_bytes(name.as_str().as_bytes())
            && let Ok(header_value) = axum::http::HeaderValue::from_bytes(value.as_bytes())
        {
            headers.insert(header_name, header_value);
        }
    }

    // 使用AuthUtils提取客户端信息
    let client_ip = AuthUtils::extract_real_client_ip(
        &headers,
        session.client_addr().map(std::string::ToString::to_string),
    );
    let user_agent = AuthUtils::extract_user_agent(&headers);
    let referer = AuthUtils::extract_referer(&headers);

    let req_header = session.req_header();
    RequestStats {
        method: req_header.method.to_string(),
        path: req_header.uri.path().to_string(),
        client_ip,
        user_agent,
        referer,
    }
}

/// 收集请求详情（头、大小、类型等）
#[must_use]
pub fn collect_details(session: &Session, request_stats: &RequestStats) -> RequestDetails {
    let req_header = session.req_header();

    // 收集请求头
    let mut headers = std::collections::HashMap::new();
    for (name, value) in &req_header.headers {
        if let Ok(value_str) = std::str::from_utf8(value.as_bytes()) {
            headers.insert(name.as_str().to_string(), value_str.to_string());
        }
    }

    // 获取Content-Type
    let content_type = req_header
        .headers
        .get("content-type")
        .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
        .map(std::string::ToString::to_string);

    // 获取Content-Length
    let body_size = req_header
        .headers
        .get("content-length")
        .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
        .and_then(|s| s.parse::<u64>().ok());

    RequestDetails {
        headers,
        body_size,
        content_type,
        client_ip: request_stats.client_ip.clone(),
        user_agent: request_stats.user_agent.clone(),
        referer: request_stats.referer.clone(),
        method: request_stats.method.clone(),
        path: request_stats.path.clone(),
        protocol_version: Some("HTTP/1.1".to_string()),
    }
}
