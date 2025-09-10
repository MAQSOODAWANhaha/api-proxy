//! # OAuth v2 客户端管理接口
//!
//! 提供基于客户端轮询的新OAuth管理API，替代传统的服务器回调方式
//! 支持公共OAuth凭据和PKCE安全机制

use crate::auth::extract_user_id_from_headers;
use crate::auth::oauth_client::{
    OAuthClient, OAuthError, AuthorizeUrlResponse, 
    OAuthSessionInfo, OAuthTokenResponse, PollingStatus
};
use crate::auth::oauth_client::session_manager::SessionStatistics;
use crate::management::{response, server::AppState};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// OAuth v2授权请求
#[derive(Debug, Deserialize)]
pub struct OAuthV2AuthorizeRequest {
    /// 提供商名称 (google/claude/openai)
    pub provider_name: String,
    /// 会话名称（用户自定义）
    pub name: String,
    /// 会话描述
    pub description: Option<String>,
    /// 用户提供的额外参数（如Gemini的project_id）
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
    AuthorizeUrl {
        data: AuthorizeUrlResponse,
    },
    #[serde(rename = "polling_status")]
    PollingStatus {
        data: PollingStatus,
    },
    #[serde(rename = "token_response")]
    TokenResponse {
        data: OAuthTokenResponse,
    },
    #[serde(rename = "session_list")]
    SessionList {
        data: Vec<OAuthSessionInfo>,
    },
    #[serde(rename = "statistics")]
    Statistics {
        data: SessionStatistics,
    },
}

/// 开始OAuth授权流程
pub async fn start_authorization(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<OAuthV2AuthorizeRequest>,
) -> impl IntoResponse {
    // 提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(_) => {
            return response::error(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "Unauthorized access");
        }
    };

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 开始授权流程
    match oauth_client.start_authorization_with_extra_params(
        user_id,
        &request.provider_name,
        &request.name,
        request.description.as_deref(),
        request.extra_params,
    ).await {
        Ok(authorize_response) => {
            response::success(authorize_response)
        }
        Err(OAuthError::ProviderNotFound(provider)) => {
            response::error(
                StatusCode::BAD_REQUEST,
                "UNSUPPORTED_PROVIDER", 
                &format!("Unsupported OAuth provider: {}", provider)
            )
        }
        Err(e) => {
            tracing::error!("Failed to start OAuth authorization: {:?}", e);
            response::error(StatusCode::INTERNAL_SERVER_ERROR, "START_AUTH_FAILED", "Failed to start authorization")
        }
    }
}

/// 轮询OAuth会话状态
pub async fn poll_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<OAuthV2PollQuery>,
) -> impl IntoResponse {
    // 提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(_) => {
            return response::error(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "Unauthorized access");
        }
    };

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 验证会话访问权限
    if !oauth_client.validate_session_access(&query.session_id, user_id).await.unwrap_or(false) {
        return response::error(StatusCode::FORBIDDEN, "ACCESS_DENIED", "Session not found or access denied");
    }

    // 轮询会话状态
    match oauth_client.poll_session(&query.session_id).await {
        Ok(polling_status) => {
            response::success(polling_status)
        }
        Err(OAuthError::InvalidSession(_)) => {
            response::error(StatusCode::NOT_FOUND, "SESSION_NOT_FOUND", "Session not found")
        }
        Err(e) => {
            tracing::error!("Failed to poll session: {:?}", e);
            response::error(StatusCode::INTERNAL_SERVER_ERROR, "POLL_FAILED", "Failed to poll session")
        }
    }
}

/// 交换授权码获取令牌
pub async fn exchange_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<OAuthV2ExchangeRequest>,
) -> impl IntoResponse {
    // 提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(_) => {
            return response::error(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "Unauthorized access");
        }
    };

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 验证会话访问权限
    if !oauth_client.validate_session_access(&request.session_id, user_id).await.unwrap_or(false) {
        return response::error(StatusCode::FORBIDDEN, "ACCESS_DENIED", "Session not found or access denied");
    }

    // 添加详细日志记录
    tracing::info!("🔄 开始OAuth令牌交换", {
        user_id = %user_id,
        session_id = %request.session_id,
        auth_code_length = request.authorization_code.len(),
        auth_code_prefix = %request.authorization_code.chars().take(10).collect::<String>()
    });

    // 交换令牌
    match oauth_client.exchange_token(&request.session_id, &request.authorization_code).await {
        Ok(token_response) => {
            response::success(token_response)
        }
        Err(OAuthError::InvalidSession(_)) => {
            response::error(StatusCode::NOT_FOUND, "SESSION_NOT_FOUND", "Session not found")
        }
        Err(OAuthError::SessionExpired(_)) => {
            response::error(StatusCode::GONE, "SESSION_EXPIRED", "Session expired")
        }
        Err(OAuthError::TokenExchangeFailed(msg)) => {
            response::error(StatusCode::BAD_REQUEST, "TOKEN_EXCHANGE_FAILED", &format!("Token exchange failed: {}", msg))
        }
        Err(e) => {
            tracing::error!("Failed to exchange token: {:?}", e);
            response::error(StatusCode::INTERNAL_SERVER_ERROR, "EXCHANGE_FAILED", "Failed to exchange token")
        }
    }
}

/// 获取用户的OAuth会话列表
pub async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // 提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(_) => {
            return response::error(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "Unauthorized access");
        }
    };

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 获取用户会话列表
    match oauth_client.list_user_sessions(user_id).await {
        Ok(sessions) => {
            response::success(sessions)
        }
        Err(e) => {
            tracing::error!("Failed to list sessions: {:?}", e);
            response::error(StatusCode::INTERNAL_SERVER_ERROR, "LIST_SESSIONS_FAILED", "Failed to list sessions")
        }
    }
}

/// 删除OAuth会话
pub async fn delete_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    // 提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(_) => {
            return response::error(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "Unauthorized access");
        }
    };

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 删除会话
    match oauth_client.delete_session(&session_id, user_id).await {
        Ok(()) => {
            response::success("Session deleted successfully")
        }
        Err(OAuthError::InvalidSession(_)) => {
            response::error(StatusCode::NOT_FOUND, "SESSION_NOT_FOUND", "Session not found")
        }
        Err(e) => {
            tracing::error!("Failed to delete session: {:?}", e);
            response::error(StatusCode::INTERNAL_SERVER_ERROR, "DELETE_SESSION_FAILED", "Failed to delete session")
        }
    }
}

/// 刷新OAuth令牌
pub async fn refresh_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    // 提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(_) => {
            return response::error(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "Unauthorized access");
        }
    };

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 验证会话访问权限
    if !oauth_client.validate_session_access(&session_id, user_id).await.unwrap_or(false) {
        return response::error(StatusCode::FORBIDDEN, "ACCESS_DENIED", "Session not found or access denied");
    }

    // 刷新令牌
    match oauth_client.refresh_token(&session_id).await {
        Ok(token_response) => {
            response::success(token_response)
        }
        Err(OAuthError::InvalidSession(_)) => {
            response::error(StatusCode::NOT_FOUND, "SESSION_NOT_FOUND", "Session not found")
        }
        Err(OAuthError::TokenExchangeFailed(msg)) => {
            response::error(StatusCode::BAD_REQUEST, "TOKEN_REFRESH_FAILED", &format!("Token refresh failed: {}", msg))
        }
        Err(e) => {
            tracing::error!("Failed to refresh token: {:?}", e);
            response::error(StatusCode::INTERNAL_SERVER_ERROR, "REFRESH_FAILED", "Failed to refresh token")
        }
    }
}

/// 获取OAuth统计信息
pub async fn get_statistics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // 提取用户ID（管理员权限检查可选）
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => Some(id),
        Err(_) => {
            return response::error(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "Unauthorized access");
        }
    };

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 获取统计信息
    match oauth_client.get_session_statistics(user_id).await {
        Ok(statistics) => {
            response::success(statistics)
        }
        Err(e) => {
            tracing::error!("Failed to get statistics: {:?}", e);
            response::error(StatusCode::INTERNAL_SERVER_ERROR, "STATS_FAILED", "Failed to get statistics")
        }
    }
}

/// 清理过期会话（管理员接口）
pub async fn cleanup_expired_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // 提取用户ID并检查管理员权限
    let _user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(_) => {
            return response::error(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "Unauthorized access");
        }
    };

    // TODO: 添加管理员权限检查
    // if !is_admin(user_id) { return forbidden; }

    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 清理过期会话
    match oauth_client.cleanup_expired_sessions().await {
        Ok(deleted_count) => {
            response::success(json!({
                "deleted_sessions": deleted_count
            }))
        }
        Err(e) => {
            tracing::error!("Failed to cleanup sessions: {:?}", e);
            response::error(StatusCode::INTERNAL_SERVER_ERROR, "CLEANUP_FAILED", "Failed to cleanup sessions")
        }
    }
}

/// 获取支持的OAuth提供商列表
pub async fn list_providers(
    State(state): State<AppState>,
) -> impl IntoResponse {
    // 创建OAuth客户端
    let oauth_client = OAuthClient::new(state.database.clone());

    // 获取活跃的提供商配置
    match oauth_client.list_providers().await {
        Ok(configs) => {
            let providers: Vec<_> = configs.into_iter()
                .map(|config| json!({
                    "provider_name": config.provider_name,
                    "scopes": config.scopes,
                    "pkce_required": config.pkce_required,
                }))
                .collect();

            response::success(json!({
                "providers": providers
            }))
        }
        Err(e) => {
            tracing::error!("Failed to list providers: {:?}", e);
            response::error(StatusCode::INTERNAL_SERVER_ERROR, "LIST_PROVIDERS_FAILED", "Failed to list providers")
        }
    }
}