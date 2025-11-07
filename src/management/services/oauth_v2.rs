//! # OAuth v2 服务
//!
//! 封装 OAuth v2 客户端相关的业务逻辑，供 handler 复用。

use std::collections::HashMap;
use std::convert::TryFrom;

use serde::{Deserialize, Serialize};

use crate::auth::api_key_oauth_service::{
    ApiKeyOauthService, AuthorizeUrlResponse, OAuthSessionInfo, OAuthTokenResponse,
};
use crate::error::auth::{AuthError, OAuthError};
use crate::error::{ProxyError, Result};
use crate::logging::{LogComponent, LogStage};
use crate::management::server::ManagementState;
use crate::types::TimezoneContext;
use crate::types::timezone_utils;
use crate::{ensure, error, lerror, linfo};
use std::sync::Arc;

/// OAuth v2授权请求
#[derive(Debug, Deserialize)]
pub struct OAuthV2AuthorizeRequest {
    /// 提供商名称 (google/claude/openai)
    pub provider_name: String,
    /// 会话名称（用户自定义）
    pub name: String,
    /// 会话描述
    pub description: Option<String>,
    /// 用户提供的额外参数（如Gemini的 `project_id`）
    pub extra_params: Option<HashMap<String, String>>,
}

/// OAuth v2轮询查询参数
#[derive(Debug, Deserialize)]
pub struct OAuthV2PollQuery {
    /// 会话ID
    pub session_id: String,
}

/// OAuth v2令牌交换请求
#[derive(Debug, Deserialize)]
pub struct OAuthV2ExchangeRequest {
    /// 会话ID
    pub session_id: String,
    /// 授权码
    pub authorization_code: String,
}

/// 带时区信息的 OAuth 会话信息
#[derive(Debug, Serialize)]
pub struct OAuthSessionInfoWithTimezone {
    pub session_id: String,
    pub provider_name: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub created_at: String,
    pub expires_at: String,
    pub completed_at: Option<String>,
}

/// OAuth 提供商概要信息
#[derive(Debug, Serialize)]
pub struct OAuthProviderSummary {
    pub provider_name: String,
    pub scopes: Vec<String>,
    pub pkce_required: bool,
}

pub struct OAuthV2Service<'a> {
    state: &'a ManagementState,
}

impl<'a> OAuthV2Service<'a> {
    #[must_use]
    pub const fn new(state: &'a ManagementState) -> Self {
        Self { state }
    }

    fn client(&self) -> Arc<ApiKeyOauthService> {
        self.state.oauth_client()
    }

    /// 开始授权流程
    pub async fn start_authorization(
        &self,
        user_id: i32,
        request: &OAuthV2AuthorizeRequest,
    ) -> Result<AuthorizeUrlResponse> {
        let client = self.client();
        match client
            .start_authorization_with_extra_params(
                user_id,
                &request.provider_name,
                &request.name,
                request.description.as_deref(),
                request.extra_params.clone(),
            )
            .await
        {
            Ok(resp) => Ok(resp),
            Err(ProxyError::Authentication(AuthError::OAuth(OAuthError::ProviderNotFound(
                provider,
            )))) => Err(error!(
                Authentication,
                format!("Unsupported OAuth provider: {provider}")
            )),
            Err(err) => {
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::OAuth,
                    "start_auth_fail",
                    &format!(
                        "Failed to start OAuth authorization: {err:?} (provider={})",
                        request.provider_name
                    )
                );
                Err(err)
            }
        }
    }

    /// 交换授权码获取令牌
    pub async fn exchange_token(
        &self,
        user_id: i32,
        request: &OAuthV2ExchangeRequest,
    ) -> Result<OAuthTokenResponse> {
        let client = self.client();
        let has_access = client
            .validate_session_access(&request.session_id, user_id)
            .await
            .unwrap_or(false);

        ensure!(
            has_access,
            Authentication,
            "Session not found or access denied"
        );

        linfo!(
            "system",
            LogStage::Authentication,
            LogComponent::OAuth,
            "exchange_token_start",
            &format!(
                "Start OAuth token exchange: user_id={user_id}, session_id={session_id}, auth_code_len={auth_code_len}, auth_code_prefix={auth_code_prefix}",
                session_id = request.session_id,
                auth_code_len = request.authorization_code.len(),
                auth_code_prefix = request
                    .authorization_code
                    .chars()
                    .take(10)
                    .collect::<String>()
            )
        );

        match client
            .exchange_token(&request.session_id, &request.authorization_code)
            .await
        {
            Ok(resp) => Ok(resp),
            Err(ProxyError::Authentication(AuthError::OAuth(OAuthError::InvalidSession(_)))) => {
                Err(error!(
                    Authentication,
                    format!("Session not found: {}", request.session_id)
                ))
            }
            Err(ProxyError::Authentication(AuthError::OAuth(OAuthError::SessionExpired(_)))) => {
                Err(error!(Authentication, "Session expired"))
            }
            Err(ProxyError::Authentication(AuthError::OAuth(OAuthError::TokenExchangeFailed(
                msg,
            )))) => Err(error!(
                Authentication,
                format!("Token exchange failed: {msg}")
            )),
            Err(err) => {
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::OAuth,
                    "exchange_token_fail",
                    &format!(
                        "Failed to exchange token: {err:?}, session_id={}",
                        request.session_id
                    )
                );
                Err(err)
            }
        }
    }

    /// 获取会话列表
    pub async fn list_sessions(
        &self,
        user_id: i32,
        timezone_ctx: Option<&TimezoneContext>,
    ) -> Result<Vec<OAuthSessionInfoWithTimezone>> {
        let sessions = self.client().list_user_sessions(user_id).await?;
        Ok(sessions
            .into_iter()
            .map(|session| convert_oauth_session_to_timezone_response(session, timezone_ctx))
            .collect())
    }

    /// 删除会话
    pub async fn delete_session(&self, user_id: i32, session_id: &str) -> Result<()> {
        let client = self.client();
        match client.delete_session(session_id, user_id).await {
            Ok(()) => Ok(()),
            Err(ProxyError::Authentication(AuthError::OAuth(OAuthError::InvalidSession(_)))) => {
                Err(error!(
                    Authentication,
                    format!("Session not found: {session_id}")
                ))
            }
            Err(err) => {
                lerror!(
                    "system",
                    LogStage::Internal,
                    LogComponent::OAuth,
                    "delete_session_fail",
                    &format!("Failed to delete session: {err:?}, session_id={session_id}")
                );
                Err(err)
            }
        }
    }

    /// 刷新 OAuth 令牌
    pub async fn refresh_token(
        &self,
        user_id: i32,
        session_id: &str,
    ) -> Result<OAuthTokenResponse> {
        let client = self.client();
        let has_access = client
            .validate_session_access(session_id, user_id)
            .await
            .unwrap_or(false);

        ensure!(
            has_access,
            Authentication,
            "Session not found or access denied"
        );

        match client.refresh_token(session_id).await {
            Ok(resp) => Ok(resp),
            Err(ProxyError::Authentication(AuthError::OAuth(OAuthError::InvalidSession(_)))) => {
                Err(error!(
                    Authentication,
                    format!("Session not found: {session_id}")
                ))
            }
            Err(ProxyError::Authentication(AuthError::OAuth(OAuthError::TokenExchangeFailed(
                msg,
            )))) => Err(error!(
                Authentication,
                format!("Token refresh failed: {msg}")
            )),
            Err(err) => {
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::OAuth,
                    "refresh_token_fail",
                    &format!("Failed to refresh token: {err:?}, session_id={session_id}")
                );
                Err(err)
            }
        }
    }

    /// 清理过期会话
    pub async fn cleanup_expired_sessions(&self) -> Result<i64> {
        let count = self.client().cleanup_expired_sessions().await?;
        let count_i64 = i64::try_from(count).map_err(|_| {
            error!(
                Conversion,
                format!("cleanup_expired_sessions overflow: {}", count)
            )
        })?;
        Ok(count_i64)
    }

    /// 获取支持的 OAuth 提供商列表
    pub async fn list_providers(&self) -> Result<Vec<OAuthProviderSummary>> {
        let configs = self.client().list_providers().await?;
        Ok(configs
            .into_iter()
            .map(|config| OAuthProviderSummary {
                provider_name: config.provider_name,
                scopes: config.scopes,
                pkce_required: config.pkce_required,
            })
            .collect())
    }
}

fn convert_oauth_session_to_timezone_response(
    session: OAuthSessionInfo,
    timezone_ctx: Option<&TimezoneContext>,
) -> OAuthSessionInfoWithTimezone {
    if let Some(tz_ctx) = timezone_ctx {
        OAuthSessionInfoWithTimezone {
            session_id: session.session_id,
            provider_name: session.provider_name,
            name: session.name,
            description: session.description,
            status: session.status,
            created_at: timezone_utils::format_naive_utc_for_response(
                &session.created_at,
                &tz_ctx.timezone,
            ),
            expires_at: timezone_utils::format_naive_utc_for_response(
                &session.expires_at,
                &tz_ctx.timezone,
            ),
            completed_at: session
                .completed_at
                .map(|dt| timezone_utils::format_naive_utc_for_response(&dt, &tz_ctx.timezone)),
        }
    } else {
        OAuthSessionInfoWithTimezone {
            session_id: session.session_id,
            provider_name: session.provider_name,
            name: session.name,
            description: session.description,
            status: session.status,
            created_at: chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                session.created_at,
                chrono::Utc,
            )
            .to_rfc3339(),
            expires_at: chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                session.expires_at,
                chrono::Utc,
            )
            .to_rfc3339(),
            completed_at: session.completed_at.map(|dt| {
                chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc)
                    .to_rfc3339()
            }),
        }
    }
}
