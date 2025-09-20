//! # API密钥池调度器模块
//!
//! 实现API密钥选择算法，从用户的多个API密钥中选择合适的密钥

pub mod algorithms;
pub mod api_key_health;
pub mod pool_manager;
pub mod types;

pub use algorithms::{
    ApiKeySelectionResult, ApiKeySelector, HealthBasedApiKeySelector, RoundRobinApiKeySelector,
    SelectionContext, create_api_key_selector,
};
pub use api_key_health::{
    ApiKeyCheckResult, ApiKeyCheckType, ApiKeyErrorCategory, ApiKeyHealthChecker,
    ApiKeyHealthConfig, ApiKeyHealthStatus,
};
pub use pool_manager::{ApiKeyPoolManager, PoolStats, SmartApiKeySelectionResult};
pub use types::SchedulingStrategy;
