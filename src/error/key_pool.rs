use thiserror::Error;

#[derive(Debug, Error)]
pub enum KeyPoolError {
    #[error("Key pool is empty or all keys are unavailable")]
    NoAvailableKeys,

    #[error("Key with ID {key_id} not found")]
    KeyNotFound { key_id: i32 },

    #[error("Health check failed: {reason}")]
    HealthCheckFailed { key_id: i32, reason: String },

    #[error("Invalid scheduling strategy: {0}")]
    InvalidStrategy(String),

    #[error("Load balancer error: {0}")]
    LoadBalancer(String),

    #[error("Rate limit reset task is not running")]
    ResetTaskInactive,

    #[error("user_service_api {service_api_id} 的 provider key 配置格式无效")]
    InvalidProviderKeysFormat { service_api_id: i32 },

    #[error("user_service_api {service_api_id} 未配置 provider key")]
    NoProviderKeysConfigured { service_api_id: i32 },

    #[error("user_service_api {service_api_id} 没有可用的活跃 provider key")]
    NoActiveProviderKeys { service_api_id: i32 },

    #[error("API key health service is unavailable")]
    HealthServiceUnavailable,
}
