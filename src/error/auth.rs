//! Errors related to authentication, authorization, and API key handling.

use std::time::Duration;
use thiserror::Error;

/// 种类化的限制类型，便于格式化提示
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageLimitKind {
    PerMinute,
    DailyRequests,
    DailyTokens,
    DailyCost,
}

/// 限制被触发时的完整上下文
#[derive(Debug, Clone)]
pub struct UsageLimitInfo {
    pub kind: UsageLimitKind,
    pub limit: Option<f64>,
    pub current: Option<f64>,
    pub resets_in: Option<Duration>,
    pub plan_type: String,
}

/// The primary error type for all authentication and authorization operations.
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Missing credential")]
    ApiKeyMissing,

    #[error("Invalid credential: {0}")]
    ApiKeyInvalid(String),

    #[error("Credential format is incorrect")]
    ApiKeyMalformed,

    #[error("Credential has been disabled")]
    ApiKeyInactive,

    #[error("Usage limit exceeded")]
    UsageLimitExceeded(UsageLimitInfo),

    #[error("The user is not authenticated")]
    NotAuthenticated,

    #[error("Permission denied: requires {required}, but user only has {actual}")]
    PermissionDenied { required: String, actual: String },

    #[error("Failed to parse authentication header: {0}")]
    HeaderParse(#[from] AuthParseError),

    #[error("OAuth error: {0}")]
    OAuth(#[from] OAuthError),

    #[error("PKCE error: {0}")]
    Pkce(#[from] PkceError),

    #[error("Authentication error: {0}")]
    Message(String),

    #[error("OAuth refresh task already running")]
    TaskAlreadyRunning,

    #[error("OAuth refresh task is not running")]
    TaskNotRunning,

    #[error("OAuth refresh task is not paused")]
    TaskNotPaused,
}

/// Errors that occur while parsing authentication headers.
#[derive(Debug, Error)]
pub enum AuthParseError {
    #[error(
        "Invalid authentication header format: '{0}'. Expected format: 'Header-Name: header-value'"
    )]
    InvalidFormat(String),

    #[error("Empty header name in format: '{0}'")]
    EmptyHeaderName(String),

    #[error("Empty header value template in format: '{0}'")]
    EmptyHeaderValue(String),

    #[error("Missing key placeholder '{{key}}' in header value: '{0}'")]
    MissingKeyPlaceholder(String),
}

/// Errors related to the OAuth 2.0 flow.
#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("Invalid session: {0}")]
    InvalidSession(String),

    #[error("Session expired: {0}")]
    SessionExpired(String),

    #[error("Token exchange failed: {0}")]
    TokenExchangeFailed(String),

    #[error("PKCE verification failed")]
    PkceVerificationFailed,

    #[error("Polling timeout")]
    PollingTimeout,

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Serde error: {0}")]
    SerdeError(String),

    #[error("Invalid token: {0}")]
    InvalidToken(String),
}

/// Errors related to PKCE (Proof Key for Code Exchange).
#[derive(Debug, Error)]
pub enum PkceError {
    #[error("Invalid code verifier length: {0}. Must be between {1} and {2}")]
    InvalidVerifierLength(usize, usize, usize),

    #[error("Invalid code verifier format: contains non-ASCII characters")]
    InvalidVerifierFormat,

    #[error("Code challenge verification failed")]
    VerificationFailed,

    #[error("Encoding error: {0}")]
    EncodingError(String),
}
