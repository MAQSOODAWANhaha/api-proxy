//! OAuth认证策略模块
//!
//! 提供各种OAuth认证策略的具体实现

pub mod oauth2;
pub mod google;

// 重新导出策略类型
pub use oauth2::OAuth2Strategy;
pub use google::GoogleOAuthStrategy;