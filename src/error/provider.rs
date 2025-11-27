use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Provider '{provider}' API error: {status} - {message}")]
    ApiError {
        provider: String,
        status: u16,
        message: String,
    },

    #[error("Authentication failed with provider '{0}'")]
    AuthFailed(String),

    #[error("Model '{model}' not found for provider '{provider}'")]
    ModelNotFound { provider: String, model: String },

    #[error("Provider '{0}' rate limit exceeded")]
    RateLimitExceeded(String),

    #[error("Invalid response from provider '{provider}': {message}")]
    InvalidResponse { provider: String, message: String },

    #[error("Context window exceeded for provider '{provider}': {message}")]
    ContextWindowExceeded { provider: String, message: String },

    #[error("Unsupported feature '{feature}' for provider '{provider}'")]
    UnsupportedFeature { provider: String, feature: String },

    // Generic fallback for dynamic errors
    #[error("Provider '{provider}' error: {message}")]
    General { provider: String, message: String },
}
