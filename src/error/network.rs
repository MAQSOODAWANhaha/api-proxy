use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Network request failed: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Upstream unreachable: {0}")]
    UpstreamUnreachable(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Bad gateway: {0}")]
    BadGateway(String),

    #[error("Upstream not found: {0}")]
    UpstreamNotFound(String),

    #[error("Upstream not available: {0}")]
    UpstreamNotAvailable(String),

    #[error("Connection timeout: {0}")]
    ConnectionTimeout(String),

    #[error("Read timeout: {0}")]
    ReadTimeout(String),

    #[error("Write timeout: {0}")]
    WriteTimeout(String),
}
