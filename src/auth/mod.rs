//! # 认证授权模块
//!
//! 提供完整的身份验证和权限控制功能

pub mod api_key;
pub mod cache_strategy;

pub mod gemini_code_assist_client;
pub mod header_parser;
pub mod jwt;
pub mod oauth_client;
pub mod oauth_token_refresh_service;
pub mod oauth_token_refresh_task;
pub mod permissions;
pub mod rate_limit_dist;
pub mod service;
pub mod smart_api_key_provider;
pub mod types;
pub mod utils;

pub use api_key::ApiKeyManager;
pub use header_parser::{AuthHeader, AuthHeaderParser};
pub use jwt::JwtManager;
// 注意：旧的oauth模块已被oauth_client替代
// pub use oauth::{CompleteSessionRequest, CreateSessionRequest, OAuthSessionManager, SessionInfo};
pub use oauth_token_refresh_service::{
    ApiKeyRefreshService, RefreshStats, RefreshType, TokenRefreshResult,
};
pub type TokenStateService = ApiKeyRefreshService;
pub use oauth_token_refresh_task::{OAuthTokenRefreshTask, TaskControl, TaskState};
pub use permissions::UserRole;
pub use service::AuthService;
pub use smart_api_key_provider::{
    AuthCredentialType, CredentialResult, SmartApiKeyProvider, SmartApiKeyProviderConfig,
};
pub use types::*;
pub use utils::AuthUtils;

// 统一缓存策略
pub use cache_strategy::{
    AuthCacheKey, AuthCacheStats, CacheStrategyConfig, UnifiedAuthCacheManager, hash_credentials,
    hash_token,
};

/// 统一认证结果
/// 表示用户认证成功后的完整信息，包括用户身份、角色和可选的令牌信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuthResult {
    /// 用户ID
    pub user_id: i32,
    /// 用户名
    pub username: String,
    /// 是否为管理员
    pub is_admin: bool,
    /// 用户角色
    pub role: UserRole,
    /// 认证方式
    pub auth_method: AuthMethod,
    /// 原始令牌（脱敏）
    pub token_preview: String,
    /// OAuth令牌信息（可选，仅当通过OAuth认证时包含）
    pub token_info: Option<TokenInfo>,
    /// 令牌过期时间（可选）
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 会话信息（可选，主要用于OAuth会话）
    pub session_info: Option<serde_json::Value>,
}

/// 认证方式 - 表示已完成认证的方式（认证结果状态）
///
/// 注意：与 `AuthType` 的区别：
/// - `AuthMethod` 表示请求经过哪种方式完成了认证（结果状态）
/// - `AuthType` 表示认证策略的具体类型（配置输入）
///
/// 例如：`AuthType::OAuth` 策略完成认证后，结果可能是 `AuthMethod::Jwt`
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AuthMethod {
    /// 通过API密钥认证
    ApiKey,
    /// 通过JWT令牌认证
    Jwt,
    /// 通过基础认证（用户名/密码）
    BasicAuth,
    /// 内部服务调用认证
    Internal,
    /// `通过OAuth流程完成认证`
    OAuth,
}

/// 认证上下文
#[derive(Debug, Clone, Default)]
pub struct AuthContext {
    /// 认证结果
    pub auth_result: Option<AuthResult>,
    /// 请求的资源路径
    pub resource_path: String,
    /// HTTP 方法
    pub method: String,
    /// 客户端 IP
    pub client_ip: Option<String>,
    /// 用户代理
    pub user_agent: Option<String>,
}

impl AuthContext {
    /// 创建新的认证上下文
    #[must_use]
    pub const fn new(resource_path: String, method: String) -> Self {
        Self {
            auth_result: None,
            resource_path,
            method,
            client_ip: None,
            user_agent: None,
        }
    }

    /// 设置认证结果
    pub fn set_auth_result(&mut self, result: AuthResult) {
        self.auth_result = Some(result);
    }

    /// 检查是否已认证
    #[must_use]
    pub const fn is_authenticated(&self) -> bool {
        self.auth_result.is_some()
    }

    /// 检查是否为管理员
    #[must_use]
    pub fn is_admin(&self) -> bool {
        self.auth_result.as_ref().is_some_and(|r| r.is_admin)
    }

    /// 获取用户ID
    #[must_use]
    pub fn get_user_id(&self) -> Option<i32> {
        self.auth_result.as_ref().map(|r| r.user_id)
    }

    /// 获取用户名
    #[must_use]
    pub fn get_username(&self) -> Option<&str> {
        self.auth_result.as_ref().map(|r| r.username.as_str())
    }
}

/// `OAuth令牌信息`
/// `包含在AuthResult中的令牌相关信息`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenInfo {
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌（可选）
    pub refresh_token: Option<String>,
    /// 令牌类型
    pub token_type: String,
    /// 过期时间（秒）
    pub expires_in: Option<i64>,
    /// 作用域
    pub scope: Option<String>,
}
