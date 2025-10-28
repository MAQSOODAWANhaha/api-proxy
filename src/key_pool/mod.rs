//! # API密钥池调度器模块
//!
//! 实现API密钥选择算法，从用户的多个API密钥中选择合适的密钥

pub mod algorithms;
pub mod api_key_health;
pub mod api_key_rate_limit_reset_task;
pub mod api_key_scheduler_service;
pub mod types;

pub use algorithms::{
    ApiKeySelectionResult, ApiKeySelector, RoundRobinApiKeySelector, SelectionContext,
    create_api_key_selector,
};
pub use api_key_health::ApiKeyHealthService;
pub use api_key_rate_limit_reset_task::ApiKeyRateLimitResetTask;
pub use api_key_scheduler_service::ApiKeySchedulerService;
pub use types::SchedulingStrategy;
