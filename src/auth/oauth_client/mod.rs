//! # `OAuth客户端模块`
//!
//! `实现基于客户端轮询的OAuth` 2.0 + PKCE流程
//! 参考 Wei-Shaw/claude-relay-service 的实现方式
//!
//! ## 核心特性
//! - 使用公共OAuth客户端凭据（Gemini CLI、Claude、OpenAI等）
//! - 标准化重定向URI，不依赖部署域名
//! - 客户端侧轮询机制，避免服务器回调依赖
//! - PKCE安全保护，适合公共客户端场景
//! - `支持多提供商的统一OAuth接口`

pub mod jwt_extractor;
pub mod pkce;
pub mod providers;

pub use jwt_extractor::{JWTParser, OpenAIAuthInfo, OpenAIJWTPayload};
pub use pkce::{PkceChallenge, PkceVerifier};
pub use providers::ApiKeyProviderConfig;

use crate::auth::types::AuthStatus;
use crate::error::AuthResult;
use crate::error::auth::OAuthError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::auth::api_key_oauth_refresh_service::ApiKeyOAuthRefreshService;
use crate::auth::api_key_oauth_state_service::ApiKeyOAuthStateService;

// The local OAuthError enum has been moved to src/error/auth.rs
// The From implementations are also moved or will be handled by the top-level ProxyError.

/// `OAuth授权URL响应`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizeUrlResponse {
    /// 授权URL
    pub authorize_url: String,
    /// 会话ID（用于轮询）
    pub session_id: String,
    /// 状态参数
    pub state: String,
    /// 过期时间（Unix时间戳）
    pub expires_at: i64,
}

/// `OAuth`令牌响应
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

/// `OAuth`会话信息
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

/// `OAuth`配置信息
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

/// `ApiKeyAuthentication`客户端主入口
#[derive(Debug)]
pub struct ApiKeyAuthentication {
    /// 提供商配置管理器
    config: Arc<ApiKeyProviderConfig>,
    /// 会话状态服务
    state: Arc<ApiKeyOAuthStateService>,
    /// 令牌刷新服务
    refresh: Arc<ApiKeyOAuthRefreshService>,
}

impl ApiKeyAuthentication {
    /// 创建新的`OAuth`客户端
    #[must_use]
    pub fn new(db: Arc<sea_orm::DatabaseConnection>) -> Self {
        let config = Arc::new(ApiKeyProviderConfig::new(db.clone()));
        let state = Arc::new(ApiKeyOAuthStateService::new(db));
        let refresh = Arc::new(ApiKeyOAuthRefreshService::new(
            reqwest::Client::new(), // Create a new reqwest client
            state.clone(),
            config.clone(),
        ));

        Self {
            config,
            state,
            refresh,
        }
    }

    /// 获取内部的 Token 刷新服务实例
    #[must_use]
    pub fn api_key_oauth_refresh_service(&self) -> Arc<ApiKeyOAuthRefreshService> {
        Arc::clone(&self.refresh)
    }

    /// 获取内部的 OAuth 会话状态服务实例
    #[must_use]
    pub fn api_key_oauth_state_service(&self) -> Arc<ApiKeyOAuthStateService> {
        Arc::clone(&self.state)
    }

    /// 开始`OAuth`授权流程（支持用户提供的额外参数）
    pub async fn start_authorization_with_extra_params(
        &self,
        user_id: i32,
        provider_name: &str,
        name: &str,
        description: Option<&str>,
        extra_params: Option<std::collections::HashMap<String, String>>,
    ) -> AuthResult<AuthorizeUrlResponse> {
        // 获取提供商配置
        let mut config = self.config.get_config(provider_name).await?;

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
            .state
            .create_session(user_id, provider_name, None, name, description, &config)
            .await?;

        // 生成授权URL
        let authorize_url = self.config.build_authorize_url(&config, &session)?;

        Ok(AuthorizeUrlResponse {
            authorize_url,
            session_id: session.session_id,
            state: session.state,
            expires_at: session.expires_at.and_utc().timestamp(),
        })
    }

    /// 完成`Token`交换
    pub async fn exchange_token(
        &self,
        session_id: &str,
        authorization_code: &str,
    ) -> AuthResult<OAuthTokenResponse> {
        self.refresh
            .exchange_authorization_code(session_id, authorization_code)
            .await
    }

    /// 获取用户的`OAuth`会话列表
    pub async fn list_user_sessions(&self, user_id: i32) -> AuthResult<Vec<OAuthSessionInfo>> {
        self.state.list_user_sessions(user_id).await
    }

    /// 删除会话
    pub async fn delete_session(&self, session_id: &str, user_id: i32) -> AuthResult<()> {
        self.state.delete_session(session_id, user_id).await
    }

    /// 刷新访问令牌
    pub async fn refresh_token(&self, session_id: &str) -> AuthResult<OAuthTokenResponse> {
        self.refresh.refresh_access_token(session_id).await
    }

    /// 清理过期会话
    pub async fn cleanup_expired_sessions(&self) -> AuthResult<u64> {
        let now = chrono::Utc::now();
        let report = self.state.prune_stale_sessions(now).await?;
        Ok((report.removed_expired + report.removed_orphaned) as u64)
    }

    /// 验证会话访问权限
    pub async fn validate_session_access(
        &self,
        session_id: &str,
        user_id: i32,
    ) -> AuthResult<bool> {
        self.state
            .validate_session_access(session_id, user_id)
            .await
    }

    /// 列出支持的`OAuth`提供商
    pub async fn list_providers(&self) -> AuthResult<Vec<OAuthProviderConfig>> {
        self.config.list_active_configs().await
    }

    // === 自动Token刷新相关方法 ===

    /// 检查会话是否需要刷新token
    ///
    /// 用于UI展示或批量处理前的预检查
    pub async fn check_session_needs_refresh(
        &self,
        session_id: &str,
        threshold_seconds: Option<i64>,
    ) -> AuthResult<bool> {
        let session = self.state.get_session(session_id).await?;

        if session.status != AuthStatus::Authorized.to_string() || session.refresh_token.is_none() {
            return Ok(false);
        }

        let threshold = threshold_seconds.unwrap_or(300); // 默认5分钟
        let now = chrono::Utc::now().naive_utc();
        let expires_at = session.expires_at;
        let threshold_duration = chrono::Duration::try_seconds(threshold).unwrap_or_default();

        Ok(session.is_expired() || expires_at <= now + threshold_duration)
    }
}
