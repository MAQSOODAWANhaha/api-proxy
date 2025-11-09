//! # 认证授权模块
//!
//! 提供身份验证、OAuth、权限等子系统的统一入口。此模块只暴露组件化服务
//! (`ApiKeyAuthenticationService`, `ApiKeyOauthService` 等) 以及核心数据结构
//! (`AuthContext`)，其余实现需通过子模块路径访问，以保持边界清晰。

pub mod api_key_manager;
pub mod cache_strategy;

pub mod api_key_oauth_refresh_service;
pub mod api_key_oauth_service;
pub mod api_key_oauth_state_service;
pub mod api_key_oauth_token_refresh_task;
pub mod api_key_usage_limit_service;
pub mod gemini_code_assist_client;
pub mod header_parser;
pub mod jwt;
pub mod openai;
pub mod permissions;
pub mod pkce;
pub mod service;
pub mod types;
pub mod utils;

pub use types::{AuthContext, AuthMethod, TokenInfo};
