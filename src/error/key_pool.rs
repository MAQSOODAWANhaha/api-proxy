use thiserror::Error;

#[derive(Debug, Error)]
pub enum KeyPoolError {
    #[error("密钥池为空或所有密钥都不可用")]
    NoAvailableKeys,

    #[error("找不到ID为 {key_id} 的密钥")]
    KeyNotFound { key_id: i32 },

    #[error("健康检查失败: {reason}")]
    HealthCheckFailed { key_id: i32, reason: String },

    #[error("无效的调度策略: {0}")]
    InvalidStrategy(String),

    #[error("负载均衡错误: {0}")]
    LoadBalancer(String),
}
