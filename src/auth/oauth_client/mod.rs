//! # OAuth客户端模块
//!
//! 实现基于客户端轮询的OAuth 2.0 + PKCE流程
//! 参考 Wei-Shaw/claude-relay-service 的实现方式
//!
//! ## 核心特性
//! - 使用公共OAuth客户端凭据（Gemini CLI、Claude、OpenAI等）
//! - 标准化重定向URI，不依赖部署域名
//! - 客户端侧轮询机制，避免服务器回调依赖
//! - PKCE安全保护，适合公共客户端场景
//! - 支持多提供商的统一OAuth接口

pub mod auto_refresh;
pub mod jwt_extractor;
pub mod pkce;
pub mod polling;
pub mod providers;
pub mod session_manager;
pub mod token_exchange;

pub use auto_refresh::{AutoRefreshManager, RefreshPolicy};
pub use jwt_extractor::{JWTParser, OpenAIAuthInfo, OpenAIJWTPayload};
pub use pkce::{PkceChallenge, PkceVerifier};
pub use polling::{OAuthPollingClient, PollingStatus};
pub use providers::OAuthProviderManager;
pub use session_manager::{SessionManager, SessionStatus};
pub use token_exchange::{TokenExchangeClient, TokenResponse};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// OAuth错误类型
#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("Invalid session: {0}")]
    InvalidSession(String),

    #[error("Session expired: {0}")]
    SessionExpired(String),

    #[error("Token exchange failed: {0}")]
    TokenExchangeFailed(String),

    #[error("PKCE verification failed")]
    PkceVerificationFailed,

    #[error("Polling timeout")]
    PollingTimeout,

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Serde error: {0}")]
    SerdeError(String),

    #[error("Invalid token: {0}")]
    InvalidToken(String),
}

impl From<reqwest::Error> for OAuthError {
    fn from(err: reqwest::Error) -> Self {
        OAuthError::NetworkError(err.to_string())
    }
}

impl From<sea_orm::DbErr> for OAuthError {
    fn from(err: sea_orm::DbErr) -> Self {
        OAuthError::DatabaseError(err.to_string())
    }
}

impl From<serde_json::Error> for OAuthError {
    fn from(err: serde_json::Error) -> Self {
        OAuthError::SerdeError(err.to_string())
    }
}

/// OAuth结果类型
pub type OAuthResult<T> = Result<T, OAuthError>;

/// OAuth授权URL响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizeUrlResponse {
    /// 授权URL
    pub authorize_url: String,
    /// 会话ID（用于轮询）
    pub session_id: String,
    /// 状态参数
    pub state: String,
    /// 轮询间隔（秒）
    pub polling_interval: u32,
    /// 过期时间（Unix时间戳）
    pub expires_at: i64,
}

/// OAuth令牌响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenResponse {
    /// 会话ID（用于后续创建provider key）
    pub session_id: String,
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌（可选）
    pub refresh_token: Option<String>,
    /// ID令牌（可选，用于OpenID Connect）
    pub id_token: Option<String>,
    /// 令牌类型（通常为"Bearer"）
    pub token_type: String,
    /// 过期时间（秒）
    pub expires_in: Option<i32>,
    /// 作用域
    pub scopes: Vec<String>,
}

/// OAuth会话信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthSessionInfo {
    /// 会话ID
    pub session_id: String,
    /// 用户ID
    pub user_id: i32,
    /// 提供商名称
    pub provider_name: String,
    /// 会话名称（用户自定义）
    pub name: String,
    /// 会话描述
    pub description: Option<String>,
    /// 会话状态
    pub status: String,
    /// 创建时间
    pub created_at: chrono::NaiveDateTime,
    /// 过期时间
    pub expires_at: chrono::NaiveDateTime,
    /// 完成时间
    pub completed_at: Option<chrono::NaiveDateTime>,
}

/// OAuth配置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProviderConfig {
    /// 提供商名称
    pub provider_name: String,
    /// 客户端ID
    pub client_id: String,
    /// 客户端密钥（可选，公共客户端通常为None）
    pub client_secret: Option<String>,
    /// 授权端点
    pub authorize_url: String,
    /// 令牌端点
    pub token_url: String,
    /// 重定向URI
    pub redirect_uri: String,
    /// 作用域
    pub scopes: Vec<String>,
    /// 是否需要PKCE
    pub pkce_required: bool,
    /// 其他参数
    pub extra_params: HashMap<String, String>,
}

/// OAuth客户端主入口
#[derive(Debug)]
pub struct OAuthClient {
    provider_manager: OAuthProviderManager,
    session_manager: SessionManager,
    polling_client: OAuthPollingClient,
    token_exchange_client: TokenExchangeClient,
    auto_refresh_manager: AutoRefreshManager,
}

impl OAuthClient {
    /// 创建新的OAuth客户端
    pub fn new(db: Arc<sea_orm::DatabaseConnection>) -> Self {
        let provider_manager = OAuthProviderManager::new((*db).clone());
        let session_manager = SessionManager::new((*db).clone());
        let polling_client = OAuthPollingClient::new();
        let token_exchange_client = TokenExchangeClient::new();

        // 创建自动刷新管理器
        let auto_refresh_manager = AutoRefreshManager::new(
            session_manager.clone(),
            provider_manager.clone(),
            token_exchange_client.clone(),
            (*db).clone(),
        );

        Self {
            provider_manager,
            session_manager,
            polling_client,
            token_exchange_client,
            auto_refresh_manager,
        }
    }

    /// 开始OAuth授权流程
    pub async fn start_authorization(
        &self,
        user_id: i32,
        provider_name: &str,
        name: &str,
        description: Option<&str>,
    ) -> OAuthResult<AuthorizeUrlResponse> {
        tracing::info!("🚀 [OAuth] 开始授权流程: user_id={}, provider_name={}, name={}",
            user_id, provider_name, name);

        // 获取提供商配置
        let config = self.provider_manager.get_config(provider_name).await?;

        tracing::debug!("✅ [OAuth] 提供商配置获取成功: provider_name={}, client_id={}",
            provider_name, config.client_id);

        // 解析provider_type_id（如果provider_name包含了类型信息，如"gemini:oauth"）
        let provider_type_id = if provider_name.contains(':') {
            // 这里可以通过数据库查询获取真正的provider_type_id
            // 现在暂时设为None，后续可以完善
            None
        } else {
            None
        };

        tracing::debug!("📝 [OAuth] 创建会话: user_id={}, provider_name={}, provider_type_id={:?}",
            user_id, provider_name, provider_type_id);

        // 创建会话
        let session = self
            .session_manager
            .create_session(
                user_id,
                provider_name,
                provider_type_id,
                name,
                description,
                &config,
            )
            .await?;

        tracing::info!("✅ [OAuth] 会话创建成功: session_id={}, state={}",
            session.session_id, session.state);

        // 生成授权URL
        let authorize_url = self
            .provider_manager
            .build_authorize_url(&config, &session)?;

        tracing::info!("🎯 [OAuth] 授权流程启动完成: session_id={}, polling_interval=2s",
            session.session_id);

        Ok(AuthorizeUrlResponse {
            authorize_url,
            session_id: session.session_id,
            state: session.state,
            polling_interval: 2, // 2秒轮询间隔
            expires_at: session.expires_at.and_utc().timestamp(),
        })
    }

    /// 开始OAuth授权流程（带provider_type_id）
    pub async fn start_authorization_with_provider_id(
        &self,
        user_id: i32,
        provider_name: &str,
        provider_type_id: Option<i32>,
        name: &str,
        description: Option<&str>,
    ) -> OAuthResult<AuthorizeUrlResponse> {
        // 获取提供商配置
        let config = self.provider_manager.get_config(provider_name).await?;

        // 创建会话
        let session = self
            .session_manager
            .create_session(
                user_id,
                provider_name,
                provider_type_id,
                name,
                description,
                &config,
            )
            .await?;

        // 生成授权URL
        let authorize_url = self
            .provider_manager
            .build_authorize_url(&config, &session)?;

        Ok(AuthorizeUrlResponse {
            authorize_url,
            session_id: session.session_id,
            state: session.state,
            polling_interval: 2, // 2秒轮询间隔
            expires_at: session.expires_at.and_utc().timestamp(),
        })
    }

    /// 开始OAuth授权流程（支持用户提供的额外参数）
    pub async fn start_authorization_with_extra_params(
        &self,
        user_id: i32,
        provider_name: &str,
        name: &str,
        description: Option<&str>,
        extra_params: Option<std::collections::HashMap<String, String>>,
    ) -> OAuthResult<AuthorizeUrlResponse> {
        // 获取提供商配置
        let mut config = self.provider_manager.get_config(provider_name).await?;

        // 合并用户提供的额外参数
        if let Some(user_params) = extra_params {
            // 只添加非空的用户参数，覆盖配置中的默认值
            for (key, value) in user_params {
                if !value.trim().is_empty() {
                    config.extra_params.insert(key, value);
                }
            }
        }

        // 创建会话
        let session = self
            .session_manager
            .create_session(user_id, provider_name, None, name, description, &config)
            .await?;

        // 生成授权URL
        let authorize_url = self
            .provider_manager
            .build_authorize_url(&config, &session)?;

        Ok(AuthorizeUrlResponse {
            authorize_url,
            session_id: session.session_id,
            state: session.state,
            polling_interval: 2, // 2秒轮询间隔
            expires_at: session.expires_at.and_utc().timestamp(),
        })
    }

    /// 轮询会话状态
    pub async fn poll_session(&self, session_id: &str) -> OAuthResult<PollingStatus> {
        self.polling_client
            .poll_session(&self.session_manager, session_id)
            .await
    }

    /// 完成Token交换
    pub async fn exchange_token(
        &self,
        session_id: &str,
        authorization_code: &str,
    ) -> OAuthResult<OAuthTokenResponse> {
        self.token_exchange_client
            .exchange_token(
                &self.provider_manager,
                &self.session_manager,
                session_id,
                authorization_code,
            )
            .await
    }

    /// 获取用户的OAuth会话列表
    pub async fn list_user_sessions(&self, user_id: i32) -> OAuthResult<Vec<OAuthSessionInfo>> {
        self.session_manager.list_user_sessions(user_id).await
    }

    /// 删除会话
    pub async fn delete_session(&self, session_id: &str, user_id: i32) -> OAuthResult<()> {
        self.session_manager
            .delete_session(session_id, user_id)
            .await
    }

    /// 刷新访问令牌
    pub async fn refresh_token(&self, session_id: &str) -> OAuthResult<OAuthTokenResponse> {
        self.token_exchange_client
            .refresh_token(&self.provider_manager, &self.session_manager, session_id)
            .await
    }

    /// 获取会话统计信息
    pub async fn get_session_statistics(
        &self,
        user_id: Option<i32>,
    ) -> OAuthResult<session_manager::SessionStatistics> {
        self.session_manager.get_session_statistics(user_id).await
    }

    /// 清理过期会话
    pub async fn cleanup_expired_sessions(&self) -> OAuthResult<u64> {
        self.session_manager.cleanup_expired_sessions().await
    }

    /// 验证会话访问权限
    pub async fn validate_session_access(
        &self,
        session_id: &str,
        user_id: i32,
    ) -> OAuthResult<bool> {
        self.session_manager
            .validate_session_access(session_id, user_id)
            .await
    }

    /// 列出支持的OAuth提供商
    pub async fn list_providers(&self) -> OAuthResult<Vec<OAuthProviderConfig>> {
        self.provider_manager.list_active_configs().await
    }

    // === 自动Token刷新相关方法 ===

    /// 智能获取有效的访问令牌
    ///
    /// 如果token即将过期，会自动刷新后返回新token
    /// 推荐使用此方法替代直接访问session.access_token
    pub async fn get_valid_access_token(&self, session_id: &str) -> OAuthResult<Option<String>> {
        self.auto_refresh_manager
            .get_valid_access_token(session_id, None)
            .await
    }

    /// 带自定义刷新策略的智能token获取
    pub async fn get_valid_access_token_with_policy(
        &self,
        session_id: &str,
        policy: RefreshPolicy,
    ) -> OAuthResult<Option<String>> {
        self.auto_refresh_manager
            .get_valid_access_token(session_id, Some(policy))
            .await
    }

    /// 批量刷新用户的即将过期token
    ///
    /// 用于主动维护用户的所有OAuth会话
    pub async fn refresh_user_expiring_tokens(
        &self,
        user_id: i32,
        policy: Option<RefreshPolicy>,
    ) -> OAuthResult<Vec<(String, OAuthResult<OAuthTokenResponse>)>> {
        self.auto_refresh_manager
            .refresh_expiring_sessions_for_user(user_id, policy)
            .await
    }

    /// 批量获取多个会话的有效token
    ///
    /// 会自动刷新需要刷新的token
    pub async fn batch_get_valid_tokens(
        &self,
        session_ids: Vec<String>,
        policy: Option<RefreshPolicy>,
    ) -> Vec<(String, OAuthResult<Option<String>>)> {
        self.auto_refresh_manager
            .batch_refresh_tokens(session_ids, policy)
            .await
    }

    /// 检查会话是否需要刷新token
    ///
    /// 用于UI展示或批量处理前的预检查
    pub async fn check_session_needs_refresh(
        &self,
        session_id: &str,
        threshold_seconds: Option<i64>,
    ) -> OAuthResult<bool> {
        let session = self.session_manager.get_session(session_id).await?;

        if session.status != "completed" || session.refresh_token.is_none() {
            return Ok(false);
        }

        let threshold = threshold_seconds.unwrap_or(300); // 默认5分钟
        let now = chrono::Utc::now().naive_utc();
        let expires_at = session.expires_at;
        let threshold_duration = chrono::Duration::try_seconds(threshold).unwrap_or_default();

        Ok(session.is_expired() || expires_at <= now + threshold_duration)
    }
}
