//! # 管理模块错误处理优化
//!
//! 优化管理模块中的错误处理，提高一致性和可维护性

use crate::error::{ProxyError, Result};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// 管理模块专用错误类型
#[derive(Debug, thiserror::Error)]
pub enum ManagementError {
    /// 认证错误
    #[error("认证错误: {message}")]
    Auth {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 权限错误
    #[error("权限错误: {message}")]
    Permission {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 验证错误
    #[error("验证错误: {message}")]
    Validation {
        message: String,
        field: Option<String>,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 业务逻辑错误
    #[error("业务错误: {message}")]
    Business {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 资源未找到错误
    #[error("资源未找到: {resource_type} {identifier}")]
    NotFound {
        resource_type: String,
        identifier: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 资源冲突错误
    #[error("资源冲突: {resource_type} {identifier}")]
    Conflict {
        resource_type: String,
        identifier: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 速率限制错误
    #[error("速率限制: {message}")]
    RateLimit {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },
}

impl ManagementError {
    /// 创建认证错误
    pub fn auth<T: Into<String>>(message: T) -> Self {
        Self::Auth {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的认证错误
    pub fn auth_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Auth {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建权限错误
    pub fn permission<T: Into<String>>(message: T) -> Self {
        Self::Permission {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的权限错误
    pub fn permission_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Permission {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建验证错误
    pub fn validation<T: Into<String>>(message: T, field: Option<String>) -> Self {
        Self::Validation {
            message: message.into(),
            field,
            source: None,
        }
    }

    /// 创建带来源的验证错误
    pub fn validation_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        field: Option<String>,
        source: E,
    ) -> Self {
        Self::Validation {
            message: message.into(),
            field,
            source: Some(source.into()),
        }
    }

    /// 创建业务错误
    pub fn business<T: Into<String>>(message: T) -> Self {
        Self::Business {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的业务错误
    pub fn business_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Business {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建资源未找到错误
    pub fn not_found<T: Into<String>, I: Into<String>>(resource_type: T, identifier: I) -> Self {
        Self::NotFound {
            resource_type: resource_type.into(),
            identifier: identifier.into(),
            source: None,
        }
    }

    /// 创建带来源的资源未找到错误
    pub fn not_found_with_source<T: Into<String>, I: Into<String>, E: Into<anyhow::Error>>(
        resource_type: T,
        identifier: I,
        source: E,
    ) -> Self {
        Self::NotFound {
            resource_type: resource_type.into(),
            identifier: identifier.into(),
            source: Some(source.into()),
        }
    }

    /// 创建资源冲突错误
    pub fn conflict<T: Into<String>, I: Into<String>>(resource_type: T, identifier: I) -> Self {
        Self::Conflict {
            resource_type: resource_type.into(),
            identifier: identifier.into(),
            source: None,
        }
    }

    /// 创建带来源的资源冲突错误
    pub fn conflict_with_source<T: Into<String>, I: Into<String>, E: Into<anyhow::Error>>(
        resource_type: T,
        identifier: I,
        source: E,
    ) -> Self {
        Self::Conflict {
            resource_type: resource_type.into(),
            identifier: identifier.into(),
            source: Some(source.into()),
        }
    }

    /// 创建速率限制错误
    pub fn rate_limit<T: Into<String>>(message: T) -> Self {
        Self::RateLimit {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的速率限制错误
    pub fn rate_limit_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::RateLimit {
            message: message.into(),
            source: Some(source.into()),
        }
    }
}

// 实现 IntoResponse trait，将管理错误转换为标准HTTP响应
impl IntoResponse for ManagementError {
    fn into_response(self) -> Response {
        let (status, error_code) = match &self {
            ManagementError::Auth { .. } => (StatusCode::UNAUTHORIZED, "AUTH_ERROR"),
            ManagementError::Permission { .. } => (StatusCode::FORBIDDEN, "PERMISSION_ERROR"),
            ManagementError::Validation { .. } => (StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
            ManagementError::Business { .. } => (StatusCode::BAD_REQUEST, "BUSINESS_ERROR"),
            ManagementError::NotFound { .. } => (StatusCode::NOT_FOUND, "RESOURCE_NOT_FOUND"),
            ManagementError::Conflict { .. } => (StatusCode::CONFLICT, "RESOURCE_CONFLICT"),
            ManagementError::RateLimit { .. } => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMIT_EXCEEDED"),
        };

        let error_response = crate::management::response::ApiErrorResponse {
            success: false,
            error: crate::management::response::ErrorDetails {
                code: error_code.to_string(),
                message: self.to_string(),
            },
        };

        (status, axum::Json(error_response)).into_response()
    }
}

/// 管理模块结果类型
pub type ManagementResult<T> = std::result::Result<T, ManagementError>;

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
        self.map_err(|e| ManagementError::auth_with_source(f(), e.into()))
    }

    fn with_permission_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| ManagementError::permission_with_source(f(), e.into()))
    }

    fn with_validation_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| ManagementError::validation_with_source(f(), None, e.into()))
    }

    fn with_business_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| ManagementError::business_with_source(f(), e.into()))
    }
}

impl<T> ManagementErrorContext<T> for Option<T> {
    fn with_auth_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| ManagementError::auth(f()))
    }

    fn with_permission_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| ManagementError::permission(f()))
    }

    fn with_validation_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| ManagementError::validation(f(), None))
    }

    fn with_business_context<F>(self, f: F) -> ManagementResult<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| ManagementError::business(f()))
    }
}

/// 管理模块错误处理宏
#[macro_export]
macro_rules! management_error {
    ($variant:ident, $msg:expr) => {
        $crate::management::error::ManagementError::$variant {
            message: $msg.into(),
            source: None,
        }
    };
    ($variant:ident, $msg:expr, $source:expr) => {
        $crate::management::error::ManagementError::$variant {
            message: $msg.into(),
            source: Some($source.into()),
        }
    };
}

/// 管理模块验证错误宏
#[macro_export]
macro_rules! validation_error {
    ($msg:expr) => {
        $crate::management::error::ManagementError::validation($msg, None)
    };
    ($msg:expr, $field:expr) => {
        $crate::management::error::ManagementError::validation($msg, Some($field.into()))
    };
    ($msg:expr, $field:expr, $source:expr) => {
        $crate::management::error::ManagementError::validation_with_source($msg, Some($field.into()), $source)
    };
}

/// 管理模块确保宏
#[macro_export]
macro_rules! ensure_auth {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err($crate::management::error::ManagementError::auth($msg));
        }
    };
}

/// 管理模块权限确保宏
#[macro_export]
macro_rules! ensure_permission {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err($crate::management::error::ManagementError::permission($msg));
        }
    };
}

/// 管理模块验证确保宏
#[macro_export]
macro_rules! ensure_validation {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err($crate::management::error::ManagementError::validation($msg, None));
        }
    };
    ($cond:expr, $msg:expr, $field:expr) => {
        if !($cond) {
            return Err($crate::management::error::ManagementError::validation($msg, Some($field.into())));
        }
    };
}

/// 管理模块业务确保宏
#[macro_export]
macro_rules! ensure_business {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err($crate::management::error::ManagementError::business($msg));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_management_error_creation() {
        let err = ManagementError::auth("认证失败");
        assert!(matches!(err, ManagementError::Auth { .. }));
        assert_eq!(err.to_string(), "认证错误: 认证失败");

        let err = ManagementError::permission("权限不足");
        assert!(matches!(err, ManagementError::Permission { .. }));
        assert_eq!(err.to_string(), "权限错误: 权限不足");

        let err = ManagementError::validation("字段验证失败", Some("username".to_string()));
        assert!(matches!(err, ManagementError::Validation { .. }));
        assert_eq!(err.to_string(), "验证错误: 字段验证失败");

        let err = ManagementError::business("业务逻辑错误");
        assert!(matches!(err, ManagementError::Business { .. }));
        assert_eq!(err.to_string(), "业务错误: 业务逻辑错误");

        let err = ManagementError::not_found("用户", "123");
        assert!(matches!(err, ManagementError::NotFound { .. }));
        assert_eq!(err.to_string(), "资源未找到: 用户 123");

        let err = ManagementError::conflict("用户", "admin");
        assert!(matches!(err, ManagementError::Conflict { .. }));
        assert_eq!(err.to_string(), "资源冲突: 用户 admin");

        let err = ManagementError::rate_limit("请求过于频繁");
        assert!(matches!(err, ManagementError::RateLimit { .. }));
        assert_eq!(err.to_string(), "速率限制: 请求过于频繁");
    }

    #[test]
    fn test_management_error_with_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件不存在");
        let err = ManagementError::auth_with_source("认证失败", io_err);
        
        assert!(matches!(err, ManagementError::Auth { .. }));
        assert!(err.to_string().contains("认证错误: 认证失败"));
        assert!(err.source().is_some());

        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "权限不足");
        let err = ManagementError::permission_with_source("权限验证失败", io_err);
        
        assert!(matches!(err, ManagementError::Permission { .. }));
        assert!(err.to_string().contains("权限错误: 权限验证失败"));
        assert!(err.source().is_some());

        let io_err = std::io::Error::new(std::io::ErrorKind::InvalidInput, "无效输入");
        let err = ManagementError::validation_with_source("字段验证失败", Some("username".to_string()), io_err);
        
        assert!(matches!(err, ManagementError::Validation { .. }));
        assert!(err.to_string().contains("验证错误: 字段验证失败"));
        assert!(err.source().is_some());

        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "业务异常");
        let err = ManagementError::business_with_source("业务处理失败", io_err);
        
        assert!(matches!(err, ManagementError::Business { .. }));
        assert!(err.to_string().contains("业务错误: 业务处理失败"));
        assert!(err.source().is_some());

        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "资源不存在");
        let err = ManagementError::not_found_with_source("用户", "123", io_err);
        
        assert!(matches!(err, ManagementError::NotFound { .. }));
        assert!(err.to_string().contains("资源未找到: 用户 123"));
        assert!(err.source().is_some());

        let io_err = std::io::Error::new(std::io::ErrorKind::AlreadyExists, "资源已存在");
        let err = ManagementError::conflict_with_source("用户", "admin", io_err);
        
        assert!(matches!(err, ManagementError::Conflict { .. }));
        assert!(err.to_string().contains("资源冲突: 用户 admin"));
        assert!(err.source().is_some());

        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "速率限制");
        let err = ManagementError::rate_limit_with_source("请求过于频繁", io_err);
        
        assert!(matches!(err, ManagementError::RateLimit { .. }));
        assert!(err.to_string().contains("速率限制: 请求过于频繁"));
        assert!(err.source().is_some());
    }

    #[test]
    fn test_management_error_context_trait() {
        let result: Result<(), std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "权限不足"
        ));
        
        let err = result.with_auth_context(|| "认证失败".to_string()).unwrap_err();
        assert!(matches!(err, ManagementError::Auth { .. }));
        assert!(err.to_string().contains("认证错误: 认证失败"));

        let result: Result<(), std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "权限不足"
        ));
        
        let err = result.with_permission_context(|| "权限不足".to_string()).unwrap_err();
        assert!(matches!(err, ManagementError::Permission { .. }));
        assert!(err.to_string().contains("权限错误: 权限不足"));

        let result: Result<(), std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "无效输入"
        ));
        
        let err = result.with_validation_context(|| "字段验证失败".to_string()).unwrap_err();
        assert!(matches!(err, ManagementError::Validation { .. }));
        assert!(err.to_string().contains("验证错误: 字段验证失败"));

        let result: Result<(), std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "业务异常"
        ));
        
        let err = result.with_business_context(|| "业务处理失败".to_string()).unwrap_err();
        assert!(matches!(err, ManagementError::Business { .. }));
        assert!(err.to_string().contains("业务错误: 业务处理失败"));
    }

    #[test]
    fn test_option_management_error_context() {
        let option: Option<String> = None;
        let err = option.with_auth_context(|| "认证信息缺失".to_string()).unwrap_err();
        
        assert!(matches!(err, ManagementError::Auth { .. }));
        assert_eq!(err.to_string(), "认证错误: 认证信息缺失");

        let option: Option<String> = None;
        let err = option.with_permission_context(|| "权限信息缺失".to_string()).unwrap_err();
        
        assert!(matches!(err, ManagementError::Permission { .. }));
        assert_eq!(err.to_string(), "权限错误: 权限信息缺失");

        let option: Option<String> = None;
        let err = option.with_validation_context(|| "验证信息缺失".to_string()).unwrap_err();
        
        assert!(matches!(err, ManagementError::Validation { .. }));
        assert_eq!(err.to_string(), "验证错误: 验证信息缺失");

        let option: Option<String> = None;
        let err = option.with_business_context(|| "业务信息缺失".to_string()).unwrap_err();
        
        assert!(matches!(err, ManagementError::Business { .. }));
        assert_eq!(err.to_string(), "业务错误: 业务信息缺失");
    }

    #[test]
    fn test_management_error_macros() {
        let err = crate::management_error!(Auth, "认证错误");
        assert!(matches!(err, ManagementError::Auth { .. }));

        let err = crate::management_error!(Permission, "权限错误");
        assert!(matches!(err, ManagementError::Permission { .. }));

        let err = crate::validation_error!("验证错误");
        assert!(matches!(err, ManagementError::Validation { .. }));

        let err = crate::validation_error!("验证错误", "字段名");
        assert!(matches!(err, ManagementError::Validation { .. }));

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
        let err = ManagementError::auth("认证失败");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let err = ManagementError::permission("权限不足");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let err = ManagementError::validation("验证失败", None);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let err = ManagementError::business("业务错误");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let err = ManagementError::not_found("用户", "123");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let err = ManagementError::conflict("用户", "admin");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);

        let err = ManagementError::rate_limit("请求过于频繁");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}