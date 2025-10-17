//! Defines the core `ProxyError` enum, which is the central error type for the application.

use crate::error::{auth, config, conversion, database, key_pool, network};
use http::StatusCode;
use pingora_core::{Error as PingoraError, ErrorType as PingoraErrorType};
use std::error::Error;
use thiserror::Error;

/// The primary, unified error type for the entire application.
#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("配置错误: {0}")]
    Config(#[from] config::ConfigError),

    #[error("数据库错误: {0}")]
    Database(#[from] database::DatabaseError),

    #[error("网络错误: {0}")]
    Network(#[from] network::NetworkError),

    #[error("认证/授权错误: {0}")]
    Authentication(#[from] auth::AuthError),

    #[error("密钥池错误: {0}")]
    KeyPool(#[from] key_pool::KeyPoolError),

    #[error("数据转换错误: {0}")]
    Conversion(#[from] conversion::ConversionError),

    #[error("AI提供商错误: {message}")]
    Provider {
        message: String,
        provider: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    #[error("内部错误: {message}")]
    Internal {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),
}

// --- Helper methods for categorization and logging ---

impl ProxyError {
    /// Creates an internal error with a simple message.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            source: None,
        }
    }

    /// Creates an internal error with a source error.
    pub fn internal_with_source(
        message: impl Into<String>,
        source: impl Into<anyhow::Error>,
    ) -> Self {
        Self::Internal {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// Creates a provider error with the specified provider identifier.
    pub fn provider(message: impl Into<String>, provider: impl Into<String>) -> Self {
        Self::Provider {
            message: message.into(),
            provider: provider.into(),
            source: None,
        }
    }

    /// Creates a provider error with an underlying source error.
    pub fn provider_with_source<E>(
        message: impl Into<String>,
        provider: impl Into<String>,
        source: E,
    ) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Provider {
            message: message.into(),
            provider: provider.into(),
            source: Some(anyhow::Error::new(source)),
        }
    }

    /// Creates an upstream-not-available network error.
    pub fn upstream_not_available(message: impl Into<String>) -> Self {
        Self::Network(network::NetworkError::UpstreamNotAvailable(message.into()))
    }

    /// Returns a stable, machine-readable error code for API responses.
    #[must_use]
    pub const fn error_code(&self) -> &'static str {
        match self {
            Self::Config(_) => "CONFIG_ERROR",
            Self::Database(_) => "DATABASE_ERROR",
            Self::Network(err) => match err {
                network::NetworkError::RateLimitExceeded => "RATE_LIMIT_EXCEEDED",
                network::NetworkError::UpstreamNotFound(_) => "UPSTREAM_NOT_FOUND",
                network::NetworkError::UpstreamNotAvailable(_) => "UPSTREAM_NOT_AVAILABLE",
                network::NetworkError::ConnectionTimeout(_) => "CONNECTION_TIMEOUT",
                network::NetworkError::ReadTimeout(_) => "READ_TIMEOUT",
                network::NetworkError::WriteTimeout(_) => "WRITE_TIMEOUT",
                network::NetworkError::BadGateway(_) => "BAD_GATEWAY",
                _ => "NETWORK_ERROR",
            },
            Self::Authentication(auth_err) => match auth_err {
                auth::AuthError::ApiKeyInvalid(_) => "API_KEY_INVALID",
                auth::AuthError::PermissionDenied { .. } => "PERMISSION_DENIED",
                auth::AuthError::NotAuthenticated => "NOT_AUTHENTICATED",
                auth::AuthError::ApiKeyMissing => "API_KEY_MISSING",
                _ => "AUTHENTICATION_FAILED",
            },
            Self::KeyPool(pool_err) => match pool_err {
                key_pool::KeyPoolError::NoAvailableKeys => "SCHEDULER_NO_AVAILABLE_KEYS",
                key_pool::KeyPoolError::KeyNotFound { .. } => "KEY_NOT_FOUND",
                key_pool::KeyPoolError::HealthCheckFailed { .. } => "HEALTH_CHECK_FAILURE",
                key_pool::KeyPoolError::LoadBalancer(_) => "LOAD_BALANCER_ERROR",
                key_pool::KeyPoolError::InvalidStrategy(_) => "SCHEDULER_FAILURE",
            },
            Self::Conversion(_) => "CONVERSION_ERROR",
            Self::Provider { .. } => "AI_PROVIDER_ERROR",
            Self::Internal { .. } => "INTERNAL_SERVER_ERROR",
            Self::Io(_) => "IO_ERROR",
        }
    }

    /// Categorizes the error as either a client-side or server-side issue.
    #[must_use]
    pub const fn category(&self) -> super::ErrorCategory {
        use super::ErrorCategory;
        match self {
            // Client-side errors (typically 4xx)
            Self::Authentication(_) | Self::Network(network::NetworkError::RateLimitExceeded) => {
                ErrorCategory::Client
            }

            // Server-side errors (typically 5xx)
            _ => ErrorCategory::Server,
        }
    }

    /// Emits a structured log record for the error using the `tracing` crate.
    pub fn log(&self) {
        let error_code = self.error_code();
        let error_message = self.to_string();

        match self.category() {
            super::ErrorCategory::Client => {
                tracing::warn!(
                    error.code = error_code,
                    error.message = %error_message,
                    source = ?self.source(),
                    "Client-side error occurred"
                );
            }
            super::ErrorCategory::Server => {
                tracing::error!(
                    error.code = error_code,
                    error.message = %error_message,
                    source = ?self.source(),
                    "Server-side error occurred"
                );
            }
        }
    }

    /// Maps the error to an HTTP status code for API responses.
    #[must_use]
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::Authentication(_) => StatusCode::UNAUTHORIZED,
            Self::Config(_) | Self::Database(_) | Self::Internal { .. } | Self::Io(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::Network(network::NetworkError::RateLimitExceeded) => {
                StatusCode::TOO_MANY_REQUESTS
            }
            Self::Network(
                network::NetworkError::ConnectionTimeout(_)
                | network::NetworkError::ReadTimeout(_)
                | network::NetworkError::WriteTimeout(_),
            ) => StatusCode::GATEWAY_TIMEOUT,
            Self::Network(_) | Self::Provider { .. } => StatusCode::BAD_GATEWAY,
            Self::KeyPool(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Conversion(_) => StatusCode::BAD_REQUEST,
        }
    }

    /// Splits the error into `(status, code, message)` for HTTP responses.
    #[must_use]
    pub fn as_http_parts(&self) -> (StatusCode, &'static str, String) {
        (self.status_code(), self.error_code(), self.to_string())
    }
}

impl From<sea_orm::DbErr> for ProxyError {
    fn from(err: sea_orm::DbErr) -> Self {
        Self::Database(database::DatabaseError::from(err))
    }
}

impl From<reqwest::Error> for ProxyError {
    fn from(err: reqwest::Error) -> Self {
        Self::Network(network::NetworkError::from(err))
    }
}

impl From<toml::de::Error> for ProxyError {
    fn from(err: toml::de::Error) -> Self {
        Self::Config(config::ConfigError::from(err))
    }
}

impl From<auth::OAuthError> for ProxyError {
    fn from(err: auth::OAuthError) -> Self {
        Self::Authentication(auth::AuthError::from(err))
    }
}

impl From<auth::PkceError> for ProxyError {
    fn from(err: auth::PkceError) -> Self {
        Self::Authentication(auth::AuthError::from(err))
    }
}

impl From<ProxyError> for pingora_core::BError {
    fn from(err: ProxyError) -> Self {
        let status = err.status_code();
        let message = err.to_string();
        let context = format!("{}: {}", err.error_code(), message);
        PingoraError::explain(PingoraErrorType::HTTPStatus(status.as_u16()), context)
    }
}

impl From<auth::AuthParseError> for ProxyError {
    fn from(err: auth::AuthParseError) -> Self {
        Self::Authentication(auth::AuthError::from(err))
    }
}
