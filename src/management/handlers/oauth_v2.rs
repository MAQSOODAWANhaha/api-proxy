//! # OAuth v2 客户端管理接口
//!
//! 提供基于客户端轮询的新OAuth管理API，替代传统的服务器回调方式
//! `支持公共OAuth凭据和PKCE安全机制`

use crate::auth::oauth_client::session_manager::SessionStatistics;
use crate::auth::oauth_client::{
    AuthorizeUrlResponse, OAuthClient, OAuthPollingResponse, OAuthSessionInfo, OAuthTokenResponse,
};
use crate::error::{ProxyError, auth::OAuthError};
use crate::logging::{LogComponent, LogStage};
use crate::management::middleware::auth::AuthContext;
use crate::management::{response, server::AppState};
use crate::types::TimezoneContext;
use crate::types::timezone_utils;
use crate::{lerror, linfo};
use axum::Json;
use axum::extract::{Extension, Path, Query, Request, State};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

fn business_error(message: impl Into<String>) -> ProxyError {
    crate::error!(Authentication, message)
}

/// 获取请求中的时区上下文
fn get_timezone_from_request(request: &Request) -> Option<TimezoneContext> {
    use crate::management::middleware::timezone::get_timezone_from_request as get_tz;
    get_tz(request).map(|tz_ctx| TimezoneContext {
        timezone: tz_ctx.timezone,
    })
}

/// `带时区信息的OAuth会话信息`
#[derive(Debug, Serialize)]
pub struct OAuthSessionInfoWithTimezone {
    /// 会话ID
    pub session_id: String,
    /// 提供商名称
    pub provider_name: String,
    /// 会话名称
    pub name: String,
    /// 会话描述
    pub description: Option<String>,
    /// 会话状态
    pub status: String,
    /// 创建时间（用户时区）
    pub created_at: String,
    /// 过期时间（用户时区）
    pub expires_at: String,
    /// 完成时间（用户时区，可选）
    pub completed_at: Option<String>,
}

/// 从 `OAuthSessionInfo` 转换为带时区的响应格式
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
        // 如果没有时区信息，使用默认的RFC3339格式
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

/// OAuth v2授权请求
#[derive(Debug, Deserialize)]
pub struct OAuthV2AuthorizeRequest {
    /// 提供商名称 (google/claude/openai)
    pub provider_name: String,
    /// 会话名称（用户自定义）
    pub name: String,
    /// 会话描述
    pub description: Option<String>,
    /// `用户提供的额外参数（如Gemini的project_id`）
    pub extra_params: Option<std::collections::HashMap<String, String>>,
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

/// OAuth v2响应格式
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum OAuthV2Response {
    #[serde(rename = "authorize_url")]
    AuthorizeUrl { data: AuthorizeUrlResponse },
    #[serde(rename = "polling_status")]
    PollingStatus { data: OAuthPollingResponse },
    #[serde(rename = "token_response")]
    TokenResponse { data: OAuthTokenResponse },
    #[serde(rename = "session_list")]
    SessionList {
        data: Vec<OAuthSessionInfoWithTimezone>,
    },
    #[serde(rename = "statistics")]
    Statistics { data: SessionStatistics },
}

/// `开始OAuth授权流程`
pub async fn start_authorization(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<OAuthV2AuthorizeRequest>,
) -> impl IntoResponse {
    // 提取用户ID
    let user_id = auth_context.user_id;

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 开始授权流程
    match oauth_client
        .start_authorization_with_extra_params(
            user_id,
            &request.provider_name,
            &request.name,
            request.description.as_deref(),
            request.extra_params,
        )
        .await
    {
        Ok(authorize_response) => response::success(authorize_response),
        Err(err) => {
            if let ProxyError::Authentication(crate::error::auth::AuthError::OAuth(
                OAuthError::ProviderNotFound(provider),
            )) = &err
            {
                return crate::management::response::app_error(business_error(format!(
                    "Unsupported OAuth provider: {provider}"
                )));
            }

            lerror!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "start_auth_fail",
                &format!("Failed to start OAuth authorization: {err:?}")
            );
            crate::management::response::app_error(ProxyError::internal(
                "Failed to start authorization",
            ))
        }
    }
}

/// `轮询OAuth会话状态`
pub async fn poll_session(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Query(query): Query<OAuthV2PollQuery>,
) -> impl IntoResponse {
    // 提取用户ID
    let user_id = auth_context.user_id;

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 验证会话访问权限
    if !oauth_client
        .validate_session_access(&query.session_id, user_id)
        .await
        .unwrap_or(false)
    {
        return crate::management::response::app_error(crate::error!(
            Authentication,
            "Session not found or access denied"
        ));
    }

    // 轮询会话状态
    match oauth_client.poll_session(&query.session_id).await {
        Ok(polling_status) => response::success(polling_status),
        Err(err) => {
            if let ProxyError::Authentication(crate::error::auth::AuthError::OAuth(
                OAuthError::InvalidSession(_),
            )) = &err
            {
                return crate::management::response::app_error(business_error(format!(
                    "Session not found: {session_id}",
                    session_id = query.session_id
                )));
            }

            lerror!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "poll_session_fail",
                &format!("Failed to poll session: {err:?}")
            );
            crate::management::response::app_error(ProxyError::internal("Failed to poll session"))
        }
    }
}

/// 交换授权码获取令牌
pub async fn exchange_token(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<OAuthV2ExchangeRequest>,
) -> impl IntoResponse {
    // 提取用户ID
    let user_id = auth_context.user_id;

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 验证会话访问权限
    if !oauth_client
        .validate_session_access(&request.session_id, user_id)
        .await
        .unwrap_or(false)
    {
        return crate::management::response::app_error(crate::error!(
            Authentication,
            "Session not found or access denied"
        ));
    }

    // 添加详细日志记录
    linfo!(
        "system",
        LogStage::Authentication,
        LogComponent::OAuth,
        "exchange_token_start",
        &format!(
            "🔄 开始OAuth令牌交换: user_id={user_id}, session_id={session_id}, auth_code_length={auth_code_len}, auth_code_prefix={auth_code_prefix}",
            user_id = user_id,
            session_id = request.session_id,
            auth_code_len = request.authorization_code.len(),
            auth_code_prefix = request
                .authorization_code
                .chars()
                .take(10)
                .collect::<String>()
        )
    );

    // 交换令牌
    match oauth_client
        .exchange_token(&request.session_id, &request.authorization_code)
        .await
    {
        Ok(token_response) => response::success(token_response),
        Err(err) => match &err {
            ProxyError::Authentication(crate::error::auth::AuthError::OAuth(
                OAuthError::InvalidSession(_),
            )) => crate::management::response::app_error(business_error(format!(
                "Session not found: {session_id}",
                session_id = request.session_id
            ))),
            ProxyError::Authentication(crate::error::auth::AuthError::OAuth(
                OAuthError::SessionExpired(_),
            )) => crate::management::response::app_error(business_error("Session expired")),
            ProxyError::Authentication(crate::error::auth::AuthError::OAuth(
                OAuthError::TokenExchangeFailed(msg),
            )) => crate::management::response::app_error(business_error(format!(
                "Token exchange failed: {msg}"
            ))),
            _ => {
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::OAuth,
                    "exchange_token_fail",
                    &format!("Failed to exchange token: {err:?}")
                );
                crate::management::response::app_error(ProxyError::internal(
                    "Failed to exchange token",
                ))
            }
        },
    }
}

/// `获取用户的OAuth会话列表`
pub async fn list_sessions(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    request: Request,
) -> impl IntoResponse {
    // 提取用户ID
    let user_id = auth_context.user_id;

    // 获取时区上下文
    let timezone_ctx = get_timezone_from_request(&request);

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 获取用户会话列表
    match oauth_client.list_user_sessions(user_id).await {
        Ok(sessions) => {
            // 转换时间字段以支持时区
            let timezone_sessions: Vec<OAuthSessionInfoWithTimezone> = sessions
                .into_iter()
                .map(|session| {
                    convert_oauth_session_to_timezone_response(session, timezone_ctx.as_ref())
                })
                .collect();

            response::success(timezone_sessions)
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Internal,
                LogComponent::OAuth,
                "list_sessions_fail",
                &format!("Failed to list sessions: {err:?}")
            );
            crate::management::response::app_error(err)
        }
    }
}

/// `删除OAuth会话`
pub async fn delete_session(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    // 提取用户ID
    let user_id = auth_context.user_id;

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 删除会话
    match oauth_client.delete_session(&session_id, user_id).await {
        Ok(()) => response::success("Session deleted successfully"),
        Err(err) => {
            if let ProxyError::Authentication(crate::error::auth::AuthError::OAuth(
                OAuthError::InvalidSession(_),
            )) = &err
            {
                return crate::management::response::app_error(business_error(format!(
                    "Session not found: {session_id}"
                )));
            }

            lerror!(
                "system",
                LogStage::Internal,
                LogComponent::OAuth,
                "delete_session_fail",
                &format!("Failed to delete session: {err:?}")
            );
            crate::management::response::app_error(err)
        }
    }
}

/// `刷新OAuth令牌`
pub async fn refresh_token(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    // 提取用户ID
    let user_id = auth_context.user_id;

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 验证会话访问权限
    if !oauth_client
        .validate_session_access(&session_id, user_id)
        .await
        .unwrap_or(false)
    {
        return crate::management::response::app_error(crate::error!(
            Authentication,
            "Session not found or access denied"
        ));
    }

    // 刷新令牌
    match oauth_client.refresh_token(&session_id).await {
        Ok(token_response) => response::success(token_response),
        Err(err) => match &err {
            ProxyError::Authentication(crate::error::auth::AuthError::OAuth(
                OAuthError::InvalidSession(_),
            )) => crate::management::response::app_error(business_error(format!(
                "Session not found: {session_id}"
            ))),
            ProxyError::Authentication(crate::error::auth::AuthError::OAuth(
                OAuthError::TokenExchangeFailed(msg),
            )) => crate::management::response::app_error(business_error(format!(
                "Token refresh failed: {msg}"
            ))),
            _ => {
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::OAuth,
                    "refresh_token_fail",
                    &format!("Failed to refresh token: {err:?}")
                );
                crate::management::response::app_error(ProxyError::internal(
                    "Failed to refresh token",
                ))
            }
        },
    }
}

/// `获取OAuth统计信息`
pub async fn get_statistics(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> impl IntoResponse {
    // 提取用户ID（管理员权限检查可选）
    let user_id = Some(auth_context.user_id);

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 获取统计信息
    match oauth_client.get_session_statistics(user_id).await {
        Ok(statistics) => response::success(statistics),
        Err(err) => {
            lerror!(
                "system",
                LogStage::Internal,
                LogComponent::OAuth,
                "get_stats_fail",
                &format!("Failed to get statistics: {err:?}")
            );
            crate::management::response::app_error(err)
        }
    }
}

/// 清理过期会话（管理员接口）
pub async fn cleanup_expired_sessions(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> impl IntoResponse {
    // 提取用户ID并检查管理员权限
    let user_id = auth_context.user_id;
    linfo!(
        "system",
        LogStage::Internal,
        LogComponent::OAuth,
        "cleanup_sessions_request",
        &format!("User {user_id} requested expired OAuth session cleanup")
    );

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 清理过期会话
    match oauth_client.cleanup_expired_sessions().await {
        Ok(deleted_count) => response::success(json!({
            "deleted_sessions": deleted_count
        })),
        Err(err) => {
            lerror!(
                "system",
                LogStage::Internal,
                LogComponent::OAuth,
                "cleanup_sessions_fail",
                &format!("Failed to cleanup sessions: {err:?}")
            );
            crate::management::response::app_error(err)
        }
    }
}

/// `获取支持的OAuth提供商列表`
pub async fn list_providers(State(state): State<AppState>) -> impl IntoResponse {
    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 获取活跃的提供商配置
    match oauth_client.list_providers().await {
        Ok(configs) => {
            let providers: Vec<_> = configs
                .into_iter()
                .map(|config| {
                    json!({
                        "provider_name": config.provider_name,
                        "scopes": config.scopes,
                        "pkce_required": config.pkce_required,
                    })
                })
                .collect();

            response::success(json!({
                "providers": providers
            }))
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Internal,
                LogComponent::OAuth,
                "list_providers_fail",
                &format!("Failed to list providers: {err:?}")
            );
            crate::management::response::app_error(err)
        }
    }
}
