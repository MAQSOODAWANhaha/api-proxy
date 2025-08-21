//! # API 响应结构
//!
//! 定义了标准的 JSON API 响应格式，包括成功、失败和分页响应。

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::error::ProxyError;

/// # 分页信息
#[derive(Debug, Serialize, Deserialize)]
pub struct Pagination {
    pub page: u64,
    pub limit: u64,
    pub total: u64,
    pub pages: u64,
}

/// # 标准成功响应
#[derive(Debug, Serialize)]
pub struct SuccessResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// # 分页成功响应
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub success: bool,
    pub data: Vec<T>,
    pub pagination: Pagination,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// # 标准错误信息
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: String,
    pub message: String,
}

/// # 标准错误响应
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: ErrorInfo,
    pub timestamp: DateTime<Utc>,
}

/// # API响应枚举
///
/// 统一所有API出口，方便转换为 `axum::response::Response`
#[derive(Debug)]
pub enum ApiResponse<T: Serialize> {
    Success(T),
    SuccessWithMessage(T, String),
    SuccessWithoutData(String),
    Paginated(Vec<T>, Pagination),
    Error(StatusCode, String, String),
    AppError(ProxyError),
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        match self {
            ApiResponse::Success(data) => (
                StatusCode::OK,
                Json(SuccessResponse {
                    success: true,
                    data: Some(data),
                    message: Some("操作成功".to_string()),
                    timestamp: Utc::now(),
                }),
            )
                .into_response(),
            ApiResponse::SuccessWithMessage(data, message) => (
                StatusCode::OK,
                Json(SuccessResponse {
                    success: true,
                    data: Some(data),
                    message: Some(message),
                    timestamp: Utc::now(),
                }),
            )
                .into_response(),
            ApiResponse::SuccessWithoutData(message) => (
                StatusCode::OK,
                Json(SuccessResponse::<()> {
                    success: true,
                    data: None,
                    message: Some(message),
                    timestamp: Utc::now(),
                }),
            )
                .into_response(),
            ApiResponse::Paginated(data, pagination) => (
                StatusCode::OK,
                Json(PaginatedResponse {
                    success: true,
                    data,
                    pagination,
                    message: Some("获取成功".to_string()),
                    timestamp: Utc::now(),
                }),
            )
                .into_response(),
            ApiResponse::Error(status, code, message) => {
                let error_response = ErrorResponse {
                    success: false,
                    error: ErrorInfo {
                        code,
                        message,
                    },
                    timestamp: Utc::now(),
                };
                (status, Json(error_response)).into_response()
            }
            ApiResponse::AppError(error) => {
                // 将ProxyError转换为相应的HTTP状态码和错误信息
                let (status, code) = match &error {
                    ProxyError::Config { .. } => (StatusCode::BAD_REQUEST, "CONFIG_ERROR"),
                    ProxyError::Database { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR"),
                    ProxyError::Network { .. } => (StatusCode::BAD_GATEWAY, "NETWORK_ERROR"),
                    ProxyError::Auth { .. } => (StatusCode::UNAUTHORIZED, "AUTH_ERROR"),
                    ProxyError::AiProvider { .. } => (StatusCode::BAD_GATEWAY, "AI_PROVIDER_ERROR"),
                    ProxyError::Tls { .. } => (StatusCode::BAD_REQUEST, "TLS_ERROR"),
                    ProxyError::Business { .. } => (StatusCode::BAD_REQUEST, "BUSINESS_ERROR"),
                    ProxyError::Internal { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
                    ProxyError::Io { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "IO_ERROR"),
                    ProxyError::Serialization { .. } => (StatusCode::BAD_REQUEST, "SERIALIZATION_ERROR"),
                    ProxyError::Cache { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "CACHE_ERROR"),
                    ProxyError::ServerInit { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "SERVER_INIT_ERROR"),
                    ProxyError::ServerStart { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "SERVER_START_ERROR"),
                    ProxyError::Authentication { .. } => (StatusCode::UNAUTHORIZED, "AUTHENTICATION_ERROR"),
                    ProxyError::UpstreamNotFound { .. } => (StatusCode::NOT_FOUND, "UPSTREAM_NOT_FOUND"),
                    ProxyError::UpstreamNotAvailable { .. } => (StatusCode::SERVICE_UNAVAILABLE, "UPSTREAM_NOT_AVAILABLE"),
                    ProxyError::RateLimit { .. } => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMIT_ERROR"),
                    ProxyError::BadGateway { .. } => (StatusCode::BAD_GATEWAY, "BAD_GATEWAY_ERROR"),
                    ProxyError::ConnectionTimeout { .. } => (StatusCode::GATEWAY_TIMEOUT, "CONNECTION_TIMEOUT"),
                    ProxyError::ReadTimeout { .. } => (StatusCode::GATEWAY_TIMEOUT, "READ_TIMEOUT"),
                    ProxyError::WriteTimeout { .. } => (StatusCode::GATEWAY_TIMEOUT, "WRITE_TIMEOUT"),
                    ProxyError::LoadBalancer { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "LOAD_BALANCER_ERROR"),
                    ProxyError::HealthCheck { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "HEALTH_CHECK_ERROR"),
                    ProxyError::Statistics { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "STATISTICS_ERROR"),
                    ProxyError::Tracing { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "TRACING_ERROR"),
                    ProxyError::ManagementAuth { .. } => (StatusCode::UNAUTHORIZED, "AUTH_ERROR"),
                    ProxyError::ManagementPermission { .. } => (StatusCode::FORBIDDEN, "PERMISSION_ERROR"),
                    ProxyError::ManagementValidation { .. } => (StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
                    ProxyError::ManagementBusiness { .. } => (StatusCode::BAD_REQUEST, "BUSINESS_ERROR"),
                    ProxyError::ManagementNotFound { .. } => (StatusCode::NOT_FOUND, "RESOURCE_NOT_FOUND"),
                    ProxyError::ManagementConflict { .. } => (StatusCode::CONFLICT, "RESOURCE_CONFLICT"),
                    ProxyError::ManagementRateLimit { .. } => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMIT_EXCEEDED"),
                };
                
                let error_response = ErrorResponse {
                    success: false,
                    error: ErrorInfo {
                        code: code.to_string(),
                        message: error.to_string(),
                    },
                    timestamp: Utc::now(),
                };
                (status, Json(error_response)).into_response()
            },
        }
    }
}

/// # 便捷函数：成功响应
pub fn success<T: Serialize>(data: T) -> axum::response::Response {
    ApiResponse::Success(data).into_response()
}

/// # 便捷函数：带消息的成功响应
pub fn success_with_message<T: Serialize>(data: T, message: &str) -> axum::response::Response {
    ApiResponse::SuccessWithMessage(data, message.to_string()).into_response()
}

/// # 便捷函数：无数据体的成功响应
pub fn success_without_data(message: &str) -> axum::response::Response {
    ApiResponse::<()>::SuccessWithoutData(message.to_string()).into_response()
}

/// # 便捷函数：分页响应
pub fn paginated<T: Serialize>(data: Vec<T>, pagination: Pagination) -> axum::response::Response {
    ApiResponse::Paginated(data, pagination).into_response()
}

/// # 便捷函数：HTTP错误响应
pub fn error(status: StatusCode, code: &str, message: &str) -> axum::response::Response {
    ApiResponse::<()>::Error(status, code.to_string(), message.to_string()).into_response()
}

/// # 便捷函数：应用错误响应
pub fn app_error(error: ProxyError) -> axum::response::Response {
    ApiResponse::<()>::AppError(error).into_response()
}
