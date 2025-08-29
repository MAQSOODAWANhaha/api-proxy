//! # 认证策略模块
//!
//! 实现各种非OAuth认证策略的具体实现
//! OAuth相关策略已移至 `oauth::strategies` 模块

pub mod traits;
pub mod api_key;
pub mod bearer_token;
pub mod service_account;

// 导出核心trait和类型
pub use traits::{AuthStrategy, OAuthTokenResult};

// 导出具体策略实现
pub use api_key::ApiKeyStrategy;
pub use bearer_token::BearerTokenStrategy;
pub use service_account::ServiceAccountStrategy;