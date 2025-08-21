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

/// # 错误详情
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub reason: String,
}

/// # 标准错误信息
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<ErrorDetails>,
}

/// # 标准错误响应
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: ErrorInfo,
}

/// # API响应枚举
///
/// 统一所有API出口，方便转换为 `axum::response::Response`
pub enum ApiResponse<T: Serialize> {
    Success(T),
    SuccessWithMessage(T, String),
    SuccessWithoutData(String),
    Paginated(Vec<T>, Pagination),
    Error(StatusCode, String, String),
    DetailedError(StatusCode, String, String, Option<ErrorDetails>),
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
                        details: None,
                    },
                };
                (status, Json(error_response)).into_response()
            }
            ApiResponse::DetailedError(status, code, message, details) => {
                let error_response = ErrorResponse {
                    success: false,
                    error: ErrorInfo {
                        code,
                        message,
                        details,
                    },
                };
                (status, Json(error_response)).into_response()
            }
        }
    }
}

/// # 便捷函数：成功响应
pub fn success<T: Serialize>(data: T) -> ApiResponse<T> {
    ApiResponse::Success(data)
}

/// # 便捷函数：带消息的成功响应
pub fn success_with_message<T: Serialize>(data: T, message: &str) -> ApiResponse<T> {
    ApiResponse::SuccessWithMessage(data, message.to_string())
}

/// # 便捷函数：无数据体的成功响应
pub fn success_without_data(message: &str) -> ApiResponse<()> {
    ApiResponse::SuccessWithoutData(message.to_string())
}

/// # 便捷函数：分页响应
pub fn paginated<T: Serialize>(data: Vec<T>, pagination: Pagination) -> ApiResponse<T> {
    ApiResponse::Paginated(data, pagination)
}

/// # 便捷函数：简单错误响应（泛型，占位 T 以便与成功分支统一）
/// 这样在 handler 中 `Ok` 分支返回 `ApiResponse<T>` 而错误分支返回 `ApiResponse<T>` 也能类型统一，
/// 避免之前因为错误分支固定为 `ApiResponse<()>` 导致的 E0308 类型不匹配。
pub fn error<T: Serialize>(status: StatusCode, code: &str, message: &str) -> ApiResponse<T> {
    ApiResponse::Error(status, code.to_string(), message.to_string())
}

/// # 便捷函数：带详情的错误响应（泛型版）
pub fn detailed_error<T: Serialize>(
    status: StatusCode,
    code: &str,
    message: &str,
    details: Option<ErrorDetails>,
) -> ApiResponse<T> {
    ApiResponse::DetailedError(status, code.to_string(), message.to_string(), details)
}
