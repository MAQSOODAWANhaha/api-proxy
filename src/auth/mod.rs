//! # 认证授权模块
//!
//! 提供完整的身份验证和权限控制功能

pub mod api_key;
pub mod background_task_manager; // 后台任务管理器
pub mod cache_strategy; // 统一缓存策略
pub mod dual_auth_boundary; // 双认证机制边界控制
pub mod gemini_code_assist_client; // Gemini Code Assist API客户端
pub mod header_parser;
pub mod jwt;
pub mod management;
// pub mod oauth; // 已删除，使用oauth_client替代
pub mod oauth_cleanup_task; // OAuth 会话清理任务
pub mod oauth_client; // 新的OAuth客户端模块
pub mod oauth_token_refresh_service; // OAuth token智能刷新服务
pub mod oauth_token_refresh_task; // OAuth token刷新后台任务
pub mod permissions;
pub mod rate_limit_dist;
pub mod service;
pub mod smart_api_key_provider; // 智能API密钥提供者
pub mod strategies;
pub mod strategy_manager;
pub mod types; // 分布式限流器
// pub mod unified; // 已删除，使用services架构替代
pub mod auth_manager; // 统一认证管理器实现（原RefactoredUnified重命名）
pub mod utils;

pub use api_key::ApiKeyManager;
pub use background_task_manager::{
    BackgroundTaskInfo, BackgroundTaskManager, BackgroundTaskStatus, BackgroundTaskType,
};
pub use header_parser::{AuthHeader, AuthHeaderParser, AuthParseError};
pub use jwt::JwtManager;
pub use management::{Claims, check_is_admin_from_headers, extract_user_id_from_headers};
// 注意：旧的oauth模块已被oauth_client替代
// pub use oauth::{CompleteSessionRequest, CreateSessionRequest, OAuthSessionManager, SessionInfo};
pub use oauth_cleanup_task::{OAuthCleanupStats, OAuthCleanupTask};
pub use oauth_token_refresh_service::{
    OAuthTokenRefreshService, OAuthTokenRefreshServiceBuilder, RefreshServiceConfig, RefreshStats,
    RefreshType, TokenRefreshResult,
};
pub use oauth_token_refresh_task::{
    OAuthTokenRefreshTask, OAuthTokenRefreshTaskBuilder, RefreshTaskConfig, TaskControl, TaskState,
    TaskStats,
};
pub use permissions::{Permission, Role};
pub use service::AuthService;
pub use smart_api_key_provider::{
    AuthCredentialType, CredentialResult, SmartApiKeyProvider, SmartApiKeyProviderConfig,
};
pub use strategies::{AuthStrategy, OAuthTokenResult};
pub use types::*;
// 统一导出：统一认证管理器（新命名）
pub use auth_manager::{AuthManager, AuthRequest};
pub use utils::AuthUtils;

// 统一缓存策略
pub use cache_strategy::{
    AuthCacheKey, AuthCacheStats, CacheStrategyConfig, UnifiedAuthCacheManager, hash_credentials,
    hash_token,
};

// 双认证边界控制
pub use dual_auth_boundary::{
    AuthBoundaryRule, AuthRequestContext, BoundaryViolationStats, DualAuthBoundaryController,
    PortType, get_violation_stats, validate_auth_boundary,
};

/// 统一认证结果
/// 表示用户认证成功后的完整信息，包括用户身份、权限和可选的令牌信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuthResult {
    /// 用户ID
    pub user_id: i32,
    /// 用户名
    pub username: String,
    /// 是否为管理员
    pub is_admin: bool,
    /// 权限列表
    pub permissions: Vec<Permission>,
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
    /// 通过OAuth流程完成认证
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
    pub fn new(resource_path: String, method: String) -> Self {
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
    pub fn is_authenticated(&self) -> bool {
        self.auth_result.is_some()
    }

    /// 检查是否为管理员
    pub fn is_admin(&self) -> bool {
        self.auth_result
            .as_ref()
            .map(|r| r.is_admin)
            .unwrap_or(false)
    }

    /// 检查是否有特定权限
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.auth_result
            .as_ref()
            .map(|r| r.permissions.contains(permission))
            .unwrap_or(false)
    }

    /// 获取用户ID
    pub fn get_user_id(&self) -> Option<i32> {
        self.auth_result.as_ref().map(|r| r.user_id)
    }

    /// 获取用户名
    pub fn get_username(&self) -> Option<&str> {
        self.auth_result.as_ref().map(|r| r.username.as_str())
    }
}

/// OAuth令牌信息
/// 包含在AuthResult中的令牌相关信息
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

impl From<OAuthTokenResult> for TokenInfo {
    fn from(oauth_result: OAuthTokenResult) -> Self {
        Self {
            access_token: oauth_result.access_token,
            refresh_token: oauth_result.refresh_token,
            token_type: oauth_result.token_type,
            expires_in: oauth_result.expires_in,
            scope: oauth_result.scope,
        }
    }
}
