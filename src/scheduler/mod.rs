//! # API密钥池调度器模块
//!
//! 实现API密钥选择算法，从用户的多个API密钥中选择合适的密钥

pub mod algorithms;
pub mod pool_manager;
pub mod types;

pub use algorithms::{
    ApiKeySelector, ApiKeySelectionResult, SelectionContext,
    RoundRobinApiKeySelector, HealthBasedApiKeySelector,
    create_api_key_selector,
};
pub use pool_manager::{ApiKeyPoolManager, PoolStats};
pub use types::SchedulingStrategy;
