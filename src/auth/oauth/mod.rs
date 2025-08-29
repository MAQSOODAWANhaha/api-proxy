//! OAuth统一管理模块
//!
//! 提供统一的OAuth认证功能，包括会话管理和各种OAuth策略

pub mod session;
pub mod strategies;

// 导出主要类型和函数
pub use session::{
    OAuthSessionManager, CreateSessionRequest, CompleteSessionRequest, SessionInfo
};

// 导出策略
pub use strategies::{
    oauth2::OAuth2Strategy,
    google::GoogleOAuthStrategy,
};