//! # `OAuth客户端模块`
//!
//! 基于公共 OAuth 客户端的统一授权流程封装。

use crate::auth::api_key_oauth_refresh_service::ApiKeyOAuthRefreshService;
use crate::auth::api_key_oauth_state_service::ApiKeyOAuthStateService;
use crate::auth::types::{AuthStatus, OAuthProviderConfig};
use crate::cache::CacheManager;
use crate::error::Result;
use crate::provider::{ApiKeyProviderConfig, build_authorize_url};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// 授权 URL 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizeUrlResponse {
    pub authorize_url: String,
    pub session_id: String,
    pub state: String,
    pub expires_at: i64,
}

/// OAuth 令牌响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenResponse {
    pub session_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub token_type: String,
    pub expires_in: Option<i32>,
    pub scopes: Vec<String>,
}

/// OAuth 会话信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthSessionInfo {
    pub session_id: String,
    pub user_id: i32,
    pub provider_name: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub created_at: chrono::NaiveDateTime,
    pub expires_at: chrono::NaiveDateTime,
    pub completed_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug)]
pub struct ApiKeyOauthService {
    config: Arc<ApiKeyProviderConfig>,
    state: Arc<ApiKeyOAuthStateService>,
    refresh: Arc<ApiKeyOAuthRefreshService>,
}

impl ApiKeyOauthService {
    #[must_use]
    pub fn new(db: Arc<sea_orm::DatabaseConnection>, cache: Arc<CacheManager>) -> Self {
        let config = Arc::new(ApiKeyProviderConfig::new(db.clone(), cache));
        let state = Arc::new(ApiKeyOAuthStateService::new(db));
        let refresh = Arc::new(ApiKeyOAuthRefreshService::new(
            reqwest::Client::new(),
            state.clone(),
            config.clone(),
        ));

        Self {
            config,
            state,
            refresh,
        }
    }

    #[must_use]
    pub fn api_key_oauth_refresh_service(&self) -> Arc<ApiKeyOAuthRefreshService> {
        Arc::clone(&self.refresh)
    }

    #[must_use]
    pub fn api_key_oauth_state_service(&self) -> Arc<ApiKeyOAuthStateService> {
        Arc::clone(&self.state)
    }

    pub async fn start_authorization_with_extra_params(
        &self,
        user_id: i32,
        provider_name: &str,
        name: &str,
        description: Option<&str>,
        extra_params: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<AuthorizeUrlResponse> {
        let mut config = self.config.get_config(provider_name).await?;
        if let Some(user_params) = extra_params {
            for (key, value) in user_params {
                if value.is_null() {
                    continue;
                }
                config.extra_params.insert(key, value);
            }
        }

        let session = self
            .state
            .create_session(user_id, provider_name, None, name, description, &config)
            .await?;

        let authorize_url = build_authorize_url(&config, &session)?;

        Ok(AuthorizeUrlResponse {
            authorize_url,
            session_id: session.session_id,
            state: session.state,
            expires_at: session.expires_at.and_utc().timestamp(),
        })
    }

    pub async fn exchange_token(
        &self,
        session_id: &str,
        authorization_code: &str,
    ) -> Result<OAuthTokenResponse> {
        self.refresh
            .exchange_authorization_code(session_id, authorization_code)
            .await
    }

    pub async fn list_user_sessions(&self, user_id: i32) -> Result<Vec<OAuthSessionInfo>> {
        self.state.list_user_sessions(user_id).await
    }

    pub async fn delete_session(&self, session_id: &str, user_id: i32) -> Result<()> {
        self.state.delete_session(session_id, user_id).await
    }

    pub async fn refresh_token(&self, session_id: &str) -> Result<OAuthTokenResponse> {
        self.refresh.refresh_access_token(session_id).await
    }

    pub async fn cleanup_expired_sessions(&self) -> Result<u64> {
        let now = chrono::Utc::now();
        let report = self.state.prune_stale_sessions(now).await?;
        Ok((report.removed_expired + report.removed_orphaned) as u64)
    }

    pub async fn validate_session_access(&self, session_id: &str, user_id: i32) -> Result<bool> {
        self.state
            .validate_session_access(session_id, user_id)
            .await
    }

    pub async fn list_providers(&self) -> Result<Vec<OAuthProviderConfig>> {
        self.config.list_active_configs().await
    }

    pub async fn check_session_needs_refresh(
        &self,
        session_id: &str,
        threshold_seconds: Option<i64>,
    ) -> Result<bool> {
        let session = self.state.get_session(session_id).await?;
        if session.status != AuthStatus::Authorized.to_string() || session.refresh_token.is_none() {
            return Ok(false);
        }

        let threshold = threshold_seconds.unwrap_or(300);
        let now = chrono::Utc::now().naive_utc();
        let expires_at = session.expires_at;
        let threshold_duration = chrono::Duration::try_seconds(threshold).unwrap_or_default();

        Ok(session.is_expired() || expires_at <= now + threshold_duration)
    }
}
