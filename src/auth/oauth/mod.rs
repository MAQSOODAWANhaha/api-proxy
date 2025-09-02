//! OAuth统一管理模块
//!
//! 提供统一的OAuth认证功能，包括会话管理和各种OAuth策略

pub mod config;
pub mod simple_oauth_manager;
pub mod unified_oauth_client;
pub mod error;
pub mod session;

// 导出核心类型
pub use config::OAuth2Config;
pub use simple_oauth_manager::SimpleOAuthManager;
pub use unified_oauth_client::{UnifiedOAuthClient, UnifiedOAuthClientFactory};
pub use error::{OAuth2Error, OAuth2Result};

// 导出主要类型和函数
pub use session::{
    OAuthSessionManager, CreateSessionRequest, CompleteSessionRequest, SessionInfo
};