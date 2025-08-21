//! # 管理模块错误处理优化
//!
//! 优化管理模块中的错误处理，提高一致性和可维护性

use crate::error::{ProxyError, Result};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use chrono::Utc;

/// 管理模块专用错误类型别名
pub type ManagementError = ProxyError;

/// 创建管理模块认证错误
pub fn management_auth<T: Into<String>>(message: T) -> ProxyError {
    ProxyError::management_auth(message)
}

/// 创建管理模块权限错误
pub fn management_permission<T: Into<String>>(message: T) -> ProxyError {
    ProxyError::management_permission(message)
}

/// 创建管理模块验证错误
pub fn management_validation<T: Into<String>>(message: T, field: Option<String>) -> ProxyError {
    ProxyError::management_validation(message, field)
}

/// 创建管理模块业务错误
pub fn management_business<T: Into<String>>(message: T) -> ProxyError {
    ProxyError::management_business(message)
}

/// 创建管理模块资源未找到错误
pub fn management_not_found<T: Into<String>, I: Into<String>>(resource_type: T, identifier: I) -> ProxyError {
    ProxyError::management_not_found(resource_type, identifier)
}

/// 创建管理模块资源冲突错误
pub fn management_conflict<T: Into<String>, I: Into<String>>(resource_type: T, identifier: I) -> ProxyError {
    ProxyError::management_conflict(resource_type, identifier)
}

/// 创建管理模块速率限制错误
pub fn management_rate_limit<T: Into<String>>(message: T) -> ProxyError {
    ProxyError::management_rate_limit(message)
}

// 实现 IntoResponse trait，将管理错误转换为标准HTTP响应
impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        // 检查是否为管理模块错误
        let (status, error_code) = match &self {
            ProxyError::ManagementAuth { .. } => (StatusCode::UNAUTHORIZED, "AUTH_ERROR"),
            ProxyError::ManagementPermission { .. } => (StatusCode::FORBIDDEN, "PERMISSION_ERROR"),
            ProxyError::ManagementValidation { .. } => (StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
            ProxyError::ManagementBusiness { .. } => (StatusCode::BAD_REQUEST, "BUSINESS_ERROR"),
            ProxyError::ManagementNotFound { .. } => (StatusCode::NOT_FOUND, "RESOURCE_NOT_FOUND"),
            ProxyError::ManagementConflict { .. } => (StatusCode::CONFLICT, "RESOURCE_CONFLICT"),
            ProxyError::ManagementRateLimit { .. } => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMIT_EXCEEDED"),
            // 其他错误类型保持原样
            _ => {
                let (status, code) = match &self {
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
                return (status, axum::Json(crate::management::response::ErrorResponse {
                    success: false,
                    error: crate::management::response::ErrorInfo {
                        code: code.to_string(),
                        message: self.to_string(),
                    },
                    timestamp: Utc::now(),
                })).into_response();
            }
        };

        let error_response = crate::management::response::ErrorResponse {
            success: false,
            error: crate::management::response::ErrorInfo {
                code: error_code.to_string(),
                message: self.to_string(),
            },
            timestamp: Utc::now(),
        };

        (status, axum::Json(error_response)).into_response()
    }
}

/// 管理模块结果类型
pub type ManagementResult<T> = std::result::Result<T, ProxyError>;

/// 管理模块错误上下文扩展trait
pub trait ManagementErrorContext<T> {
    /// 添加认证错误上下文
    fn with_auth_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String;

    /// 添加权限错误上下文
    fn with_permission_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String;

    /// 添加验证错误上下文
    fn with_validation_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String;

    /// 添加业务错误上下文
    fn with_business_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ManagementErrorContext<T> for std::result::Result<T, E>
where
    E: Into<anyhow::Error>,
{
    fn with_auth_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| ProxyError::management_auth_with_source(f(), e.into()))
    }

    fn with_permission_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| ProxyError::management_permission_with_source(f(), e.into()))
    }

    fn with_validation_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| ProxyError::management_validation_with_source(f(), None, e.into()))
    }

    fn with_business_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| ProxyError::management_business_with_source(f(), e.into()))
    }
}

impl<T> ManagementErrorContext<T> for Option<T> {
    fn with_auth_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| ProxyError::management_auth(f()))
    }

    fn with_permission_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| ProxyError::management_permission(f()))
    }

    fn with_validation_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| ProxyError::management_validation(f(), None))
    }

    fn with_business_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| ProxyError::management_business(f()))
    }
}

/// 管理模块错误处理宏
#[macro_export]
macro_rules! management_error {
    ($variant:ident, $msg:expr) => {
        $crate::error::ProxyError::$variant {
            message: $msg.into(),
            source: None,
        }
    };
    ($variant:ident, $msg:expr, $source:expr) => {
        $crate::error::ProxyError::$variant {
            message: $msg.into(),
            source: Some($source.into()),
        }
    };
}

/// 管理模块验证错误宏
#[macro_export]
macro_rules! validation_error {
    ($msg:expr) => {
        $crate::error::ProxyError::management_validation($msg, None)
    };
    ($msg:expr, $field:expr) => {
        $crate::error::ProxyError::management_validation($msg, Some($field.into()))
    };
    ($msg:expr, $field:expr, $source:expr) => {
        $crate::error::ProxyError::management_validation_with_source($msg, Some($field.into()), $source)
    };
}

/// 管理模块确保宏
#[macro_export]
macro_rules! ensure_auth {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err($crate::error::management_auth($msg));
        }
    };
}

/// 管理模块权限确保宏
#[macro_export]
macro_rules! ensure_permission {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err($crate::error::management_permission($msg));
        }
    };
}

/// 管理模块验证确保宏
#[macro_export]
macro_rules! ensure_validation {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err($crate::error::management_validation($msg, None));
        }
    };
    ($cond:expr, $msg:expr, $field:expr) => {
        if !($cond) {
            return Err($crate::error::management_validation($msg, Some($field.into())));
        }
    };
}

/// 管理模块业务确保宏
#[macro_export]
macro_rules! ensure_business {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err($crate::error::management_business($msg));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_management_error_creation() {
        let err = management_auth("认证失败");
        assert!(matches!(err, ProxyError::ManagementAuth { .. }));
        assert_eq!(err.to_string(), "管理认证错误: 认证失败");

        let err = management_permission("权限不足");
        assert!(matches!(err, ProxyError::ManagementPermission { .. }));
        assert_eq!(err.to_string(), "管理权限错误: 权限不足");

        let err = management_validation("字段验证失败", Some("username".to_string()));
        assert!(matches!(err, ProxyError::ManagementValidation { .. }));
        assert_eq!(err.to_string(), "管理验证错误: 字段验证失败");

        let err = management_business("业务逻辑错误");
        assert!(matches!(err, ProxyError::ManagementBusiness { .. }));
        assert_eq!(err.to_string(), "管理业务错误: 业务逻辑错误");

        let err = management_not_found("用户", "123");
        assert!(matches!(err, ProxyError::ManagementNotFound { .. }));
        assert_eq!(err.to_string(), "管理资源未找到: 用户 123");

        let err = management_conflict("用户", "admin");
        assert!(matches!(err, ProxyError::ManagementConflict { .. }));
        assert_eq!(err.to_string(), "管理资源冲突: 用户 admin");

        let err = management_rate_limit("请求过于频繁");
        assert!(matches!(err, ProxyError::ManagementRateLimit { .. }));
        assert_eq!(err.to_string(), "管理速率限制: 请求过于频繁");
    }

    #[test]
    fn test_management_error_with_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件不存在");
        let err = management_auth_with_source("认证失败", io_err);
        
        assert!(matches!(err, ProxyError::ManagementAuth { .. }));
        assert!(err.to_string().contains("管理认证错误: 认证失败"));
        assert!(err.source().is_some());

        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "权限不足");
        let err = management_permission_with_source("权限验证失败", io_err);
        
        assert!(matches!(err, ProxyError::ManagementPermission { .. }));
        assert!(err.to_string().contains("管理权限错误: 权限验证失败"));
        assert!(err.source().is_some());

        let io_err = std::io::Error::new(std::io::ErrorKind::InvalidInput, "无效输入");
        let err = ProxyError::management_validation_with_source("字段验证失败", Some("username".to_string()), io_err);
        
        assert!(matches!(err, ProxyError::ManagementValidation { .. }));
        assert!(err.to_string().contains("管理验证错误: 字段验证失败"));
        assert!(err.source().is_some());

        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "业务异常");
        let err = management_business_with_source("业务处理失败", io_err);
        
        assert!(matches!(err, ProxyError::ManagementBusiness { .. }));
        assert!(err.to_string().contains("管理业务错误: 业务处理失败"));
        assert!(err.source().is_some());

        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "资源不存在");
        let err = management_not_found_with_source("用户", "123", io_err);
        
        assert!(matches!(err, ProxyError::ManagementNotFound { .. }));
        assert!(err.to_string().contains("管理资源未找到: 用户 123"));
        assert!(err.source().is_some());

        let io_err = std::io::Error::new(std::io::ErrorKind::AlreadyExists, "资源已存在");
        let err = management_conflict_with_source("用户", "admin", io_err);
        
        assert!(matches!(err, ProxyError::ManagementConflict { .. }));
        assert!(err.to_string().contains("管理资源冲突: 用户 admin"));
        assert!(err.source().is_some());

        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "速率限制");
        let err = management_rate_limit_with_source("请求过于频繁", io_err);
        
        assert!(matches!(err, ProxyError::ManagementRateLimit { .. }));
        assert!(err.to_string().contains("管理速率限制: 请求过于频繁"));
        assert!(err.source().is_some());
    }

    #[test]
    fn test_management_error_context_trait() {
        let result: Result<(), std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "权限不足"
        ));
        
        let err = result.with_auth_context(|| "认证失败".to_string()).unwrap_err();
        assert!(matches!(err, ProxyError::ManagementAuth { .. }));
        assert!(err.to_string().contains("管理认证错误: 认证失败"));

        let result: Result<(), std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "权限不足"
        ));
        
        let err = result.with_permission_context(|| "权限不足".to_string()).unwrap_err();
        assert!(matches!(err, ProxyError::ManagementPermission { .. }));
        assert!(err.to_string().contains("管理权限错误: 权限不足"));

        let result: Result<(), std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "无效输入"
        ));
        
        let err = result.with_validation_context(|| "字段验证失败".to_string()).unwrap_err();
        assert!(matches!(err, ProxyError::ManagementValidation { .. }));
        assert!(err.to_string().contains("管理验证错误: 字段验证失败"));

        let result: Result<(), std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "业务异常"
        ));
        
        let err = result.with_business_context(|| "业务处理失败".to_string()).unwrap_err();
        assert!(matches!(err, ProxyError::ManagementBusiness { .. }));
        assert!(err.to_string().contains("管理业务错误: 业务处理失败"));
    }

    #[test]
    fn test_option_management_error_context() {
        let option: Option<String> = None;
        let err = option.with_auth_context(|| "认证信息缺失".to_string()).unwrap_err();
        
        assert!(matches!(err, ProxyError::ManagementAuth { .. }));
        assert_eq!(err.to_string(), "管理认证错误: 认证信息缺失");

        let option: Option<String> = None;
        let err = option.with_permission_context(|| "权限信息缺失".to_string()).unwrap_err();
        
        assert!(matches!(err, ProxyError::ManagementPermission { .. }));
        assert_eq!(err.to_string(), "管理权限错误: 权限信息缺失");

        let option: Option<String> = None;
        let err = option.with_validation_context(|| "验证信息缺失".to_string()).unwrap_err();
        
        assert!(matches!(err, ProxyError::ManagementValidation { .. }));
        assert_eq!(err.to_string(), "管理验证错误: 验证信息缺失");

        let option: Option<String> = None;
        let err = option.with_business_context(|| "业务信息缺失".to_string()).unwrap_err();
        
        assert!(matches!(err, ProxyError::ManagementBusiness { .. }));
        assert_eq!(err.to_string(), "管理业务错误: 业务信息缺失");
    }

    #[test]
    fn test_management_error_macros() {
        let err = crate::management_error!(ManagementAuth, "认证错误");
        assert!(matches!(err, ProxyError::ManagementAuth { .. }));

        let err = crate::management_error!(ManagementPermission, "权限错误");
        assert!(matches!(err, ProxyError::ManagementPermission { .. }));

        let err = crate::validation_error!("验证错误");
        assert!(matches!(err, ProxyError::ManagementValidation { .. }));

        let err = crate::validation_error!("验证错误", "字段名");
        assert!(matches!(err, ProxyError::ManagementValidation { .. }));

        // 测试确保宏
        let condition = false;
        crate::ensure_auth!(condition, "这不应该触发");

        let condition = false;
        crate::ensure_permission!(condition, "这不应该触发");

        let condition = false;
        crate::ensure_validation!(condition, "这不应该触发");

        let condition = false;
        crate::ensure_business!(condition, "这不应该触发");
    }

    #[test]
    fn test_into_response() {
        let err = management_auth("认证失败");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let err = management_permission("权限不足");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let err = management_validation("验证失败", None);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let err = management_business("业务错误");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let err = management_not_found("用户", "123");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let err = management_conflict("用户", "admin");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);

        let err = management_rate_limit("请求过于频繁");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}