//! # OAuth v2 客户端管理处理器
//!
//! 解析请求并委托 `OAuthV2Service` 执行业务逻辑。

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Path, Query, Request, State};
use axum::response::IntoResponse;
use serde_json::json;

use crate::linfo;
use crate::logging::{LogComponent, LogStage};
use crate::management::middleware::auth::AuthContext;
use crate::management::services::{
    OAuthProviderSummary, OAuthV2AuthorizeRequest, OAuthV2ExchangeRequest, OAuthV2PollQuery,
    OAuthV2Service,
};
use crate::management::{response, server::AppState};
use crate::types::TimezoneContext;

/// 提取请求中的时区上下文
fn get_timezone_from_request(request: &Request) -> Option<TimezoneContext> {
    use crate::management::middleware::timezone::get_timezone_from_request as get_tz;
    get_tz(request).map(|tz_ctx| TimezoneContext {
        timezone: tz_ctx.timezone,
    })
}

/// 开始 OAuth 授权流程
pub async fn start_authorization(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<OAuthV2AuthorizeRequest>,
) -> impl IntoResponse {
    let service = OAuthV2Service::new(&state);
    match service
        .start_authorization(auth_context.user_id, &request)
        .await
    {
        Ok(authorize_response) => response::success(authorize_response),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 轮询 OAuth 会话状态
pub async fn poll_session(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Query(query): Query<OAuthV2PollQuery>,
) -> impl IntoResponse {
    let service = OAuthV2Service::new(&state);
    match service.poll_session(auth_context.user_id, &query).await {
        Ok(polling_status) => response::success(polling_status),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 交换授权码获取令牌
pub async fn exchange_token(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<OAuthV2ExchangeRequest>,
) -> impl IntoResponse {
    let service = OAuthV2Service::new(&state);
    match service.exchange_token(auth_context.user_id, &request).await {
        Ok(token_response) => response::success(token_response),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 获取用户 OAuth 会话列表
pub async fn list_sessions(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    request: Request,
) -> impl IntoResponse {
    let timezone_ctx = get_timezone_from_request(&request);
    let service = OAuthV2Service::new(&state);
    match service
        .list_sessions(auth_context.user_id, timezone_ctx.as_ref())
        .await
    {
        Ok(sessions) => response::success(sessions),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 删除 OAuth 会话
pub async fn delete_session(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let service = OAuthV2Service::new(&state);
    match service
        .delete_session(auth_context.user_id, &session_id)
        .await
    {
        Ok(()) => response::success("Session deleted successfully"),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 刷新 OAuth 令牌
pub async fn refresh_token(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let service = OAuthV2Service::new(&state);
    match service
        .refresh_token(auth_context.user_id, &session_id)
        .await
    {
        Ok(token_response) => response::success(token_response),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 获取 OAuth 会话统计
pub async fn get_statistics(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> impl IntoResponse {
    let service = OAuthV2Service::new(&state);
    match service.statistics(Some(auth_context.user_id)).await {
        Ok(statistics) => response::success(statistics),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 清理过期会话（管理员接口）
pub async fn cleanup_expired_sessions(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> impl IntoResponse {
    linfo!(
        "system",
        LogStage::Internal,
        LogComponent::OAuth,
        "cleanup_sessions_request",
        &format!(
            "User {} requested expired OAuth session cleanup",
            auth_context.user_id
        )
    );

    let service = OAuthV2Service::new(&state);
    match service.cleanup_expired_sessions().await {
        Ok(deleted_count) => response::success(json!({ "deleted_sessions": deleted_count })),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 获取支持的 OAuth 提供商列表
pub async fn list_providers(State(state): State<AppState>) -> impl IntoResponse {
    let service = OAuthV2Service::new(&state);
    match service.list_providers().await {
        Ok(providers) => {
            let providers: Vec<_> = providers
                .into_iter()
                .map(
                    |OAuthProviderSummary {
                         provider_name,
                         scopes,
                         pkce_required,
                     }| {
                        json!({
                            "provider_name": provider_name,
                            "scopes": scopes,
                            "pkce_required": pkce_required,
                        })
                    },
                )
                .collect();
            response::success(json!({ "providers": providers }))
        }
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}
