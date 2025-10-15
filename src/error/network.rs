use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("网络请求失败: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("上游服务不可达: {0}")]
    UpstreamUnreachable(String),

    #[error("速率超限")]
    RateLimitExceeded,

    #[error("网关错误: {0}")]
    BadGateway(String),

    #[error("上游服务未找到: {0}")]
    UpstreamNotFound(String),

    #[error("上游服务不可用: {0}")]
    UpstreamNotAvailable(String),

    #[error("连接超时: {0}")]
    ConnectionTimeout(String),

    #[error("读取超时: {0}")]
    ReadTimeout(String),

    #[error("写入超时: {0}")]
    WriteTimeout(String),
}