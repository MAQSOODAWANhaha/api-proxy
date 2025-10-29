//! OAuth Token 刷新执行服务
//!
//! 负责对单个 OAuth 会话执行刷新动作，并维护刷新统计信息。
//! 不直接访问数据库，也不承担调度与状态管理职责。

use crate::auth::api_key_oauth_state_service::ApiKeyOAuthStateService;
use crate::auth::oauth_client::{ApiKeyConfig, OAuthTokenResponse};
use crate::auth::types::AuthStatus;
use crate::error::AuthResult;
use crate::error::auth::OAuthError;
use crate::logging::{LogComponent, LogStage};
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

/// 令牌交换请求参数
#[derive(Debug, Clone)]
pub struct TokenExchangeRequest {
    pub session_id: String,
    pub authorization_code: String,
    pub code_verifier: String,
    pub redirect_uri: String,
}

/// Token交换统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenExchangeStats {
    /// 成功交换次数
    pub successful_exchanges: u64,
    /// 失败交换次数
    pub failed_exchanges: u64,
    /// 刷新令牌次数
    pub token_refreshes: u64,
    /// 令牌撤销次数
    pub token_revocations: u64,
    /// 平均交换时间（毫秒）
    pub average_exchange_time_ms: u64,
    /// 各提供商成功率
    pub provider_success_rates: HashMap<String, f64>,
    /// 最后更新时间
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl Default for TokenExchangeStats {
    fn default() -> Self {
        Self {
            successful_exchanges: 0,
            failed_exchanges: 0,
            token_refreshes: 0,
            token_revocations: 0,
            average_exchange_time_ms: 0,
            provider_success_rates: HashMap::new(),
            last_updated: chrono::Utc::now(),
        }
    }
}

/// OAuth Token 刷新执行器
#[derive(Debug)]
pub struct ApiKeyOAuthRefreshService {
    http_client: reqwest::Client,
    refresh_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
    session_manager: Arc<ApiKeyOAuthStateService>,
    provider_manager: Arc<ApiKeyConfig>,
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
        provider_manager: Arc<ApiKeyConfig>,
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
    ) -> AuthResult<ApiKeyOAuthRefreshResult> {
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
    ) -> AuthResult<ApiKeyOAuthRefreshResult> {
        let lock = self.get_refresh_lock(session_id).await;
        let _guard = lock.lock().await;

        let session = self.session_manager.get_session(session_id).await?;
        ensure!(
            session.status == AuthStatus::Authorized.to_string(),
            Authentication,
            format!("OAuth session {session_id} is not authorized")
        );

        let refresh_token = session.refresh_token.as_ref().ok_or_else(|| {
            crate::error!(
                Authentication,
                OAuth(OAuthError::TokenExchangeFailed(
                    "Refresh token missing for session".to_string()
                ))
            )
        })?;

        self.refresh_with_session(&session, refresh_token).await
    }

    async fn refresh_with_session(
        &self,
        session: &oauth_client_sessions::Model,
        refresh_token: &str,
    ) -> AuthResult<ApiKeyOAuthRefreshResult> {
        let config = self
            .provider_manager
            .get_config(&session.provider_name)
            .await?;

        let mut form_params = HashMap::new();
        form_params.insert("grant_type".to_string(), "refresh_token".to_string());
        form_params.insert("client_id".to_string(), config.client_id.clone());
        form_params.insert("refresh_token".to_string(), refresh_token.to_string());

        if let Some(client_secret) = &config.client_secret {
            form_params.insert("client_secret".to_string(), client_secret.clone());
        }

        let token_response = self
            .send_token_request(&config.token_url, form_params)
            .await?;

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
    pub async fn refresh_access_token(&self, session_id: &str) -> AuthResult<OAuthTokenResponse> {
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
    ) -> AuthResult<OAuthTokenResponse> {
        let session = self.session_manager.get_session(session_id).await?;

        ensure!(
            session.status == AuthStatus::Pending.to_string(),
            Authentication,
            format!("Session {session_id} is not in pending state")
        );

        if session.is_expired() {
            crate::bail!(
                Authentication,
                OAuth(OAuthError::SessionExpired(session_id.to_string()))
            );
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

        let mut form_params = HashMap::new();
        form_params.insert("grant_type".to_string(), "authorization_code".to_string());
        form_params.insert("client_id".to_string(), config.client_id.clone());
        form_params.insert("code".to_string(), actual_code);
        form_params.insert("redirect_uri".to_string(), config.redirect_uri.clone());

        if let Some(client_secret) = &config.client_secret {
            form_params.insert("client_secret".to_string(), client_secret.clone());
        }

        if config.pkce_required {
            form_params.insert("code_verifier".to_string(), session.code_verifier.clone());
        }

        Self::add_provider_specific_params(&mut form_params, &session.provider_name, &session);
        Self::add_extra_params(&mut form_params, &config.extra_params);

        let token_response = self
            .send_token_request(&config.token_url, form_params)
            .await?;

        let oauth_response = Self::process_token_response(token_response, session_id);
        self.session_manager
            .update_session_tokens(session_id, &oauth_response)
            .await?;

        Ok(oauth_response)
    }

    fn add_extra_params(
        form_params: &mut HashMap<String, String>,
        extra_params: &HashMap<String, String>,
    ) {
        for (key, value) in extra_params {
            form_params
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
    }

    fn add_provider_specific_params(
        form_params: &mut HashMap<String, String>,
        provider_name: &str,
        session: &oauth_client_sessions::Model,
    ) {
        let base = provider_name.split(':').next().unwrap_or(provider_name);

        match base {
            "google" | "gemini" => {
                form_params
                    .entry("access_type".to_string())
                    .or_insert_with(|| "offline".to_string());
                form_params
                    .entry("include_granted_scopes".to_string())
                    .or_insert_with(|| "true".to_string());
                form_params
                    .entry("prompt".to_string())
                    .or_insert_with(|| "consent".to_string());
            }
            "claude" => {
                form_params
                    .entry("client_secret".to_string())
                    .or_insert_with(|| session.code_verifier.clone());
            }
            _ => {}
        }
    }

    async fn send_token_request(
        &self,
        token_url: &str,
        form_params: HashMap<String, String>,
    ) -> AuthResult<TokenResponse> {
        let response = self
            .http_client
            .post(token_url)
            .form(&form_params)
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            return response.json::<TokenResponse>().await.map_err(|e| {
                crate::error!(
                    Authentication,
                    OAuth(OAuthError::TokenExchangeFailed(format!(
                        "Failed to parse token response: {e}"
                    )))
                )
            });
        }

        let error_body = response.text().await.unwrap_or_default();
        Err(crate::error!(
            Authentication,
            OAuth(OAuthError::TokenExchangeFailed(format!(
                "Token request failed: {status} - {error_body}"
            )))
        ))
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

    /// 撤销令牌
    pub async fn revoke_token(
        &self,
        session_id: &str,
        token: &str,
        token_type_hint: Option<&str>,
    ) -> AuthResult<()> {
        // 获取会话信息
        let session = self.session_manager.get_session(session_id).await?;
        let config = self
            .provider_manager
            .get_config(&session.provider_name)
            .await?;

        // 解析基础提供商名称
        let base_provider = if session.provider_name.contains(':') {
            session
                .provider_name
                .split(':')
                .next()
                .unwrap_or(&session.provider_name)
        } else {
            &session.provider_name
        };

        // 构建撤销请求URL（不是所有提供商都支持）
        let revoke_url = match base_provider {
            "google" | "gemini" => "https://oauth2.googleapis.com/revoke",
            "openai" => "https://auth.openai.com/oauth/revoke",
            _ => {
                // 对于不支持撤销的提供商，只是在本地标记为失效
                ldebug!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::OAuth,
                    "revocation_unsupported",
                    &format!("Provider {base_provider} does not support token revocation")
                );
                return Ok(());
            }
        };

        let mut form_params = HashMap::new();
        form_params.insert("token".to_string(), token.to_string());
        form_params.insert("client_id".to_string(), config.client_id.clone());

        if let Some(hint) = token_type_hint {
            form_params.insert("token_type_hint".to_string(), hint.to_string());
        }

        // 发送撤销请求
        let response = self
            .http_client
            .post(revoke_url)
            .form(&form_params)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(crate::error!(
                Authentication,
                OAuth(OAuthError::TokenExchangeFailed(format!(
                    "Token revocation failed: {}",
                    response.status()
                )))
            ));
        }

        Ok(())
    }
}
