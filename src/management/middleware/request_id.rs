//! # Request ID 中间件
//!
//! 为每个请求生成唯一 `request_id`，并注入到请求扩展中。

use axum::{extract::Request, middleware::Next, response::Response};
use http::header::HeaderValue;
use std::fmt;
use std::ops::Deref;
use uuid::Uuid;

/// 请求ID类型
#[derive(Debug, Clone)]
pub struct RequestId(String);

impl RequestId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<str> for RequestId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for RequestId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

/// 请求ID中间件
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    let request_id = RequestId::new();
    request.extensions_mut().insert(request_id.clone());

    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}
