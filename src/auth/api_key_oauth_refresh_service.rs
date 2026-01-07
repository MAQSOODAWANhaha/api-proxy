//! OAuth Token 刷新执行服务
//!
//! 负责对单个 OAuth 会话执行刷新动作，并维护刷新统计信息。
//! 不直接访问数据库，也不承担调度与状态管理职责。

use crate::auth::api_key_oauth_service::OAuthTokenResponse;
use crate::auth::api_key_oauth_state_service::ApiKeyOAuthStateService;
use crate::auth::types::AuthStatus;
use crate::error::{Context, ProxyError, Result, auth::OAuthError};
use crate::logging::{LogComponent, LogStage};
use crate::provider::{
    ApiKeyProviderConfig, TokenRequestPayload, build_exchange_request, build_refresh_request,
};
use crate::{ensure, ldebug, lwarn};
use chrono::{DateTime, Duration, Utc};
use entity::oauth_client_sessions;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{collections::HashMap, convert::TryFrom};
use tokio::sync::{Mutex, RwLock};

/// 令牌响应结构（来自OAuth服务器的原始响应）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub scope: Option<String>,
}

/// `OAuth错误响应结构`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthErrorResponse {
    pub error: String,
    pub error_description: Option<String>,
    pub error_uri: Option<String>,
}

/// OAuth Token 刷新执行器
#[derive(Debug)]
pub struct ApiKeyOAuthRefreshService {
    http_client: reqwest::Client,
    refresh_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
    session_manager: Arc<ApiKeyOAuthStateService>,
    provider_manager: Arc<ApiKeyProviderConfig>,
}

/// 刷新结果
#[derive(Debug, Clone)]
pub struct ApiKeyOAuthRefreshResult {
    pub session_id: String,
    pub provider_name: String,
    pub expires_at: DateTime<Utc>,
    pub token_response: OAuthTokenResponse,
}

impl ApiKeyOAuthRefreshService {
    #[must_use]
    pub fn new(
        http_client: reqwest::Client,
        session_manager: Arc<ApiKeyOAuthStateService>,
        provider_manager: Arc<ApiKeyProviderConfig>,
    ) -> Self {
        Self {
            http_client,
            refresh_locks: Arc::new(RwLock::new(HashMap::new())),
            session_manager,
            provider_manager,
        }
    }

    async fn get_refresh_lock(&self, session_id: &str) -> Arc<Mutex<()>> {
        let mut locks = self.refresh_locks.write().await;
        locks
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// 执行刷新任务并返回刷新结果，供后台调度任务使用
    pub async fn execute_token_refresh(
        &self,
        request_id: String,
        session_id: &str,
    ) -> Result<ApiKeyOAuthRefreshResult> {
        ldebug!(
            &request_id,
            LogStage::Authentication,
            LogComponent::OAuth,
            "execute_token_refresh",
            &format!("开始刷新OAuth会话: session_id={session_id}")
        );

        match self.refresh_session_by_id(session_id).await {
            Ok(result) => {
                ldebug!(
                    &request_id,
                    LogStage::Authentication,
                    LogComponent::OAuth,
                    "execute_token_refresh_ok",
                    &format!(
                        "完成刷新: session_id={session_id}, expires_at={}",
                        result.expires_at
                    )
                );
                Ok(result)
            }
            Err(err) => {
                lwarn!(
                    &request_id,
                    LogStage::Authentication,
                    LogComponent::OAuth,
                    "execute_token_refresh_fail",
                    &format!("刷新失败: session_id={session_id}, error={err}")
                );
                Err(err)
            }
        }
    }

    /// 按会话ID执行刷新逻辑，不直接更新数据库
    pub async fn refresh_session_by_id(
        &self,
        session_id: &str,
    ) -> Result<ApiKeyOAuthRefreshResult> {
        let lock = self.get_refresh_lock(session_id).await;
        let _guard = lock.lock().await;

        let session = self.session_manager.get_session(session_id).await?;
        ensure!(
            session.status == AuthStatus::Authorized.to_string(),
            crate::error::auth::AuthError::Message(format!(
                "OAuth session {session_id} is not authorized"
            ))
        );

        let refresh_token = session.refresh_token.as_ref().ok_or_else(|| {
            ProxyError::from(OAuthError::TokenExchangeFailed(
                "Refresh token missing for session".to_string(),
            ))
        })?;

        self.refresh_with_session(&session, refresh_token).await
    }

    async fn refresh_with_session(
        &self,
        session: &oauth_client_sessions::Model,
        refresh_token: &str,
    ) -> Result<ApiKeyOAuthRefreshResult> {
        let config = self
            .provider_manager
            .get_config(&session.provider_name)
            .await?;
        let payload = build_refresh_request(&config, session, refresh_token)?;
        let token_response = self.send_token_request(payload).await?;

        let oauth_response = Self::process_token_response(token_response, &session.session_id);
        let expires_at = Self::compute_expires_at(&oauth_response);

        Ok(ApiKeyOAuthRefreshResult {
            session_id: session.session_id.clone(),
            provider_name: session.provider_name.clone(),
            expires_at,
            token_response: oauth_response,
        })
    }

    fn compute_expires_at(response: &OAuthTokenResponse) -> DateTime<Utc> {
        response.expires_in.map_or_else(
            || Utc::now() + Duration::hours(1),
            |seconds| Utc::now() + Duration::seconds(i64::from(seconds)),
        )
    }

    /// 刷新访问令牌，并将最新的令牌结果写回数据库
    pub async fn refresh_access_token(&self, session_id: &str) -> Result<OAuthTokenResponse> {
        let result = self.refresh_session_by_id(session_id).await?;
        let token_response = result.token_response.clone();
        self.session_manager
            .update_session_tokens(session_id, &token_response)
            .await?;
        Ok(token_response)
    }

    /// 交换授权码以获取访问令牌
    pub async fn exchange_authorization_code(
        &self,
        session_id: &str,
        authorization_code: &str,
    ) -> Result<OAuthTokenResponse> {
        let session = self.session_manager.get_session(session_id).await?;

        ensure!(
            session.status == AuthStatus::Pending.to_string(),
            crate::error::auth::AuthError::Message(format!(
                "Session {session_id} is not in pending state"
            ))
        );

        if session.is_expired() {
            return Err(ProxyError::from(OAuthError::SessionExpired(
                session_id.to_string(),
            )));
        }

        let config = self
            .provider_manager
            .get_config(&session.provider_name)
            .await?;

        let actual_code = authorization_code
            .split('#')
            .next()
            .unwrap_or(authorization_code)
            .to_string();

        let payload = build_exchange_request(&config, &session, &actual_code)?;
        let token_response = self.send_token_request(payload).await?;

        let oauth_response = Self::process_token_response(token_response, session_id);
        self.session_manager
            .update_session_tokens(session_id, &oauth_response)
            .await?;

        Ok(oauth_response)
    }

    // 注意：此处不做任何缓存重试；OAuth 配置会始终从数据库读取。

    async fn send_token_request(&self, payload: TokenRequestPayload) -> Result<TokenResponse> {
        let method = reqwest::Method::from_bytes(payload.method.as_bytes()).map_err(|_| {
            ProxyError::from(OAuthError::TokenExchangeFailed(format!(
                "Invalid HTTP method: {}",
                payload.method
            )))
        })?;

        let mut req = self
            .http_client
            .request(method, &payload.url)
            .form(&payload.form);
        // headers 由配置驱动；如果存在同名 header（如 Content-Type），以配置为准覆盖 reqwest 默认值
        for (k, v) in payload.headers {
            req = req.header(k, v);
        }

        let response = req.send().await?;

        let status = response.status();
        if status.is_success() {
            let token_response = response
                .json::<TokenResponse>()
                .await
                .context("Failed to parse token response")?;
            return Ok(token_response);
        }

        let error_body = response.text().await.unwrap_or_default();
        Err(ProxyError::from(OAuthError::TokenExchangeFailed(format!(
            "Token request failed: {status} - {error_body}"
        ))))
    }

    fn process_token_response(response: TokenResponse, session_id: &str) -> OAuthTokenResponse {
        let scopes = response
            .scope
            .as_ref()
            .map(|s| s.split_whitespace().map(ToString::to_string).collect())
            .unwrap_or_default();

        let expires_in = response
            .expires_in
            .and_then(|value| i32::try_from(value).ok());

        OAuthTokenResponse {
            session_id: session_id.to_string(),
            access_token: response.access_token,
            refresh_token: response.refresh_token,
            id_token: response.id_token,
            token_type: response.token_type,
            expires_in,
            scopes,
        }
    }
}
