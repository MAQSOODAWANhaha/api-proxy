use thiserror::Error;

/// 描述缓存及 Redis 相关的错误。
#[derive(Debug, Error)]
pub enum CacheError {
    #[error("缓存配置错误: {0}")]
    Config(String),

    #[error("缓存 TTL 无效: {0}")]
    InvalidTtl(String),

    #[error("缓存操作失败: {0}")]
    Operation(String),

    #[error("缓存响应异常: {0}")]
    UnexpectedResponse(String),

    #[error("Redis 客户端错误: {0}")]
    Redis(#[from] redis::RedisError),
}

impl CacheError {
    /// 便捷构造函数，统一字符串转换。
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    pub fn invalid_ttl(message: impl Into<String>) -> Self {
        Self::InvalidTtl(message.into())
    }

    pub fn operation(message: impl Into<String>) -> Self {
        Self::Operation(message.into())
    }

    pub fn unexpected_response(message: impl Into<String>) -> Self {
        Self::UnexpectedResponse(message.into())
    }
}
