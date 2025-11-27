//! Defines the core `ProxyError` enum, which is the central error type for the application.

use crate::error::{
    auth, cache, config, conversion, database, key_pool, management, network, provider,
};
use http::StatusCode;
use pingora_core::{Error as PingoraError, ErrorType as PingoraErrorType};
use pingora_error::Error as PingoraLibError;
use std::error::Error;
use std::net::AddrParseError;
use thiserror::Error;
use url::ParseError as UrlParseError;

/// The primary, unified error type for the entire application.
#[derive(Debug, Error)]
pub enum ProxyError {
    #[error(transparent)]
    Config(#[from] config::ConfigError),

    #[error(transparent)]
    Database(#[from] database::DatabaseError),

    #[error(transparent)]
    Network(#[from] network::NetworkError),

    #[error(transparent)]
    Authentication(#[from] auth::AuthError),

    #[error(transparent)]
    KeyPool(#[from] key_pool::KeyPoolError),

    #[error(transparent)]
    Cache(#[from] cache::CacheError),

    #[error(transparent)]
    Management(#[from] management::ManagementError),

    #[error(transparent)]
    Conversion(#[from] conversion::ConversionError),

    #[error(transparent)]
    Provider(#[from] provider::ProviderError),

    #[error(transparent)]
    Internal(#[from] anyhow::Error),

    /// Context wrapper to preserve error type while adding context
    #[error("{context}: {source}")]
    Context {
        context: String,
        #[source]
        source: Box<ProxyError>,
    },
}

// --- Helpers for String -> Internal conversions ---

impl From<String> for ProxyError {
    fn from(s: String) -> Self {
        Self::Internal(anyhow::anyhow!(s))
    }
}

impl From<&str> for ProxyError {
    fn from(s: &str) -> Self {
        Self::Internal(anyhow::anyhow!(s.to_string()))
    }
}

impl From<std::io::Error> for ProxyError {
    fn from(e: std::io::Error) -> Self {
        Self::Internal(anyhow::Error::new(e))
    }
}

impl From<Box<pingora_core::Error>> for ProxyError {
    fn from(e: Box<pingora_core::Error>) -> Self {
        Self::Internal(anyhow::anyhow!(e.to_string()))
    }
}

impl From<redis::RedisError> for ProxyError {
    fn from(e: redis::RedisError) -> Self {
        Self::Cache(cache::CacheError::Redis(e))
    }
}

impl From<PingoraLibError> for ProxyError {
    fn from(e: PingoraLibError) -> Self {
        Self::Internal(anyhow::Error::new(e))
    }
}

impl From<serde_json::Error> for ProxyError {
    fn from(e: serde_json::Error) -> Self {
        Self::Conversion(conversion::ConversionError::Json(e))
    }
}

impl From<AddrParseError> for ProxyError {
    fn from(e: AddrParseError) -> Self {
        Self::Config(config::ConfigError::Load(format!("地址解析失败: {e}")))
    }
}

impl From<UrlParseError> for ProxyError {
    fn from(e: UrlParseError) -> Self {
        Self::Config(config::ConfigError::Load(format!("URL 解析失败: {e}")))
    }
}

impl From<auth::OAuthError> for ProxyError {
    fn from(e: auth::OAuthError) -> Self {
        Self::Authentication(auth::AuthError::OAuth(e))
    }
}

impl From<auth::PkceError> for ProxyError {
    fn from(e: auth::PkceError) -> Self {
        Self::Authentication(auth::AuthError::Pkce(e))
    }
}

impl From<auth::AuthParseError> for ProxyError {
    fn from(e: auth::AuthParseError) -> Self {
        Self::Authentication(auth::AuthError::HeaderParse(e))
    }
}

impl From<toml::de::Error> for ProxyError {
    fn from(e: toml::de::Error) -> Self {
        Self::Config(config::ConfigError::Parse(e))
    }
}

impl From<reqwest::Error> for ProxyError {
    fn from(e: reqwest::Error) -> Self {
        Self::Network(network::NetworkError::Reqwest(e))
    }
}

impl From<sea_orm::DbErr> for ProxyError {
    fn from(e: sea_orm::DbErr) -> Self {
        Self::Database(database::DatabaseError::Query(e))
    }
}

// --- Helper methods ---

impl ProxyError {
    /// Creates an upstream-not-available network error.
    pub fn upstream_not_available(message: impl Into<String>) -> Self {
        Self::Network(network::NetworkError::UpstreamNotAvailable(message.into()))
    }

    /// Returns a stable, machine-readable error code for API responses.
    #[must_use]
    pub fn error_code(&self) -> &'static str {
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
                auth::AuthError::UsageLimitExceeded(_) => "RATE_LIMIT_EXCEEDED",
                _ => "AUTHENTICATION_FAILED",
            },
            Self::KeyPool(pool_err) => match pool_err {
                key_pool::KeyPoolError::NoAvailableKeys => "SCHEDULER_NO_AVAILABLE_KEYS",
                key_pool::KeyPoolError::KeyNotFound { .. } => "KEY_NOT_FOUND",
                key_pool::KeyPoolError::HealthCheckFailed { .. } => "HEALTH_CHECK_FAILURE",
                key_pool::KeyPoolError::LoadBalancer(_) => "LOAD_BALANCER_ERROR",
                key_pool::KeyPoolError::InvalidStrategy(_) => "SCHEDULER_FAILURE",
                key_pool::KeyPoolError::ResetTaskInactive => "SCHEDULER_TASK_INACTIVE",
                key_pool::KeyPoolError::InvalidProviderKeysFormat { .. } => {
                    "SCHEDULER_PROVIDER_KEYS_FORMAT"
                }
                key_pool::KeyPoolError::NoProviderKeysConfigured { .. } => {
                    "SCHEDULER_PROVIDER_KEYS_MISSING"
                }
                key_pool::KeyPoolError::NoActiveProviderKeys { .. } => {
                    "SCHEDULER_PROVIDER_KEYS_INACTIVE"
                }
                key_pool::KeyPoolError::HealthServiceUnavailable => {
                    "SCHEDULER_HEALTH_SERVICE_UNAVAILABLE"
                }
            },
            Self::Cache(cache_err) => match cache_err {
                cache::CacheError::Config(_) => "CACHE_CONFIG_ERROR",
                cache::CacheError::InvalidTtl(_) => "CACHE_INVALID_TTL",
                cache::CacheError::Operation(_) => "CACHE_OPERATION_ERROR",
                cache::CacheError::UnexpectedResponse(_) => "CACHE_UNEXPECTED_RESPONSE",
                cache::CacheError::Redis(_) => "CACHE_BACKEND_ERROR",
            },
            Self::Management(err) => match err {
                management::ManagementError::ProviderKeyNotFound { .. } => {
                    "MANAGEMENT_PROVIDER_KEY_NOT_FOUND"
                }
                management::ManagementError::InvalidKeyAuthType { .. } => {
                    "MANAGEMENT_INVALID_KEY_TYPE"
                }
                management::ManagementError::MissingOAuthSessionId { .. } => {
                    "MANAGEMENT_OAUTH_SESSION_ID_MISSING"
                }
                management::ManagementError::OAuthSessionNotFound { .. } => {
                    "MANAGEMENT_OAUTH_SESSION_NOT_FOUND"
                }
                management::ManagementError::OAuthSessionTokenMissing { .. } => {
                    "MANAGEMENT_OAUTH_TOKEN_MISSING"
                }
                management::ManagementError::MissingTask { .. } => "MANAGEMENT_TASK_MISSING",
                management::ManagementError::MetricsUnavailable => "MANAGEMENT_METRICS_UNAVAILABLE",
            },
            Self::Conversion(_) => "CONVERSION_ERROR",
            Self::Provider(err) => match err {
                provider::ProviderError::ApiError { .. } => "PROVIDER_API_ERROR",
                provider::ProviderError::AuthFailed(_) => "PROVIDER_AUTH_FAILED",
                provider::ProviderError::ModelNotFound { .. } => "MODEL_NOT_FOUND",
                provider::ProviderError::RateLimitExceeded(_) => "PROVIDER_RATE_LIMIT",
                provider::ProviderError::InvalidResponse { .. } => "PROVIDER_INVALID_RESPONSE",
                provider::ProviderError::ContextWindowExceeded { .. } => "CONTEXT_WINDOW_EXCEEDED",
                provider::ProviderError::UnsupportedFeature { .. } => "UNSUPPORTED_FEATURE",
                provider::ProviderError::General { .. } => "AI_PROVIDER_ERROR",
            },
            Self::Internal(_) => "INTERNAL_SERVER_ERROR",
            Self::Context { source, .. } => source.error_code(),
        }
    }

    /// Categorizes the error as either a client-side or server-side issue.
    #[must_use]
    pub fn category(&self) -> super::ErrorCategory {
        use super::ErrorCategory;
        match self {
            // Client-side errors (typically 4xx)
            Self::Authentication(_)
            | Self::Conversion(_)
            | Self::Network(network::NetworkError::RateLimitExceeded)
            | Self::Provider(
                provider::ProviderError::ModelNotFound { .. }
                | provider::ProviderError::ContextWindowExceeded { .. }
                | provider::ProviderError::UnsupportedFeature { .. },
            ) => ErrorCategory::Client, // Recursive check for context
            Self::Context { source, .. } => source.category(),

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
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Authentication(auth_err) => match auth_err {
                auth::AuthError::UsageLimitExceeded(_) => StatusCode::TOO_MANY_REQUESTS,
                _ => StatusCode::UNAUTHORIZED,
            },
            Self::Config(_) | Self::Database(_) | Self::Internal(_) => {
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
            Self::Network(_) => StatusCode::BAD_GATEWAY,

            Self::Provider(err) => match err {
                provider::ProviderError::AuthFailed(_)
                | provider::ProviderError::InvalidResponse { .. }
                | provider::ProviderError::General { .. } => StatusCode::BAD_GATEWAY,
                provider::ProviderError::ModelNotFound { .. }
                | provider::ProviderError::ContextWindowExceeded { .. }
                | provider::ProviderError::UnsupportedFeature { .. } => StatusCode::BAD_REQUEST,
                provider::ProviderError::ApiError { status, .. } => {
                    StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY)
                }
                provider::ProviderError::RateLimitExceeded(_) => StatusCode::TOO_MANY_REQUESTS,
            },

            Self::KeyPool(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Conversion(_) => StatusCode::BAD_REQUEST,
            Self::Cache(_) | Self::Management(_) => StatusCode::INTERNAL_SERVER_ERROR,

            Self::Context { source, .. } => source.status_code(),
        }
    }

    /// Splits the error into `(status, code, message)` for HTTP responses.
    #[must_use]
    pub fn as_http_parts(&self) -> (StatusCode, &'static str, String) {
        (self.status_code(), self.error_code(), self.to_string())
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
