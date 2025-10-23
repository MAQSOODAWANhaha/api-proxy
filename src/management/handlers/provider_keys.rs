//! # 提供商密钥管理处理器
//!
//! 仅负责请求解析、权限校验与响应包装，业务逻辑委托 `ProviderKeyService`。

use std::sync::Arc;

use axum::extract::{Extension, Path, Query, State};
use axum::response::{IntoResponse, Json};

use crate::key_pool::types::ApiKeyHealthStatus;
use crate::management::middleware::auth::AuthContext;
use crate::management::services::{
    CreateProviderKeyRequest, ProviderKeyService, ProviderKeysListQuery, ServiceResponse,
    TrendQuery, UpdateProviderKeyRequest, UserProviderKeyQuery,
};
use crate::management::{response, server::AppState};
use crate::types::TimezoneContext;

/// 获取提供商密钥列表
pub async fn get_provider_keys_list(
    State(state): State<AppState>,
    Query(query): Query<ProviderKeysListQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = ProviderKeyService::new(&state);
    match service
        .list(auth_context.user_id, &timezone_context, &query)
        .await
    {
        Ok(ServiceResponse { data, .. }) => response::success(data),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 创建提供商密钥
pub async fn create_provider_key(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
    Json(payload): Json<CreateProviderKeyRequest>,
) -> axum::response::Response {
    let service = ProviderKeyService::new(&state);
    match service
        .create(auth_context.user_id, &timezone_context, &payload)
        .await
    {
        Ok(ServiceResponse { data, message }) => {
            let msg = message.unwrap_or_else(|| "创建成功".to_string());
            response::success_with_message(data, &msg)
        }
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 获取提供商密钥详情
pub async fn get_provider_key_detail(
    State(state): State<AppState>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
    Path(key_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let service = ProviderKeyService::new(&state);
    match service
        .detail(auth_context.user_id, &timezone_context, key_id)
        .await
    {
        Ok(ServiceResponse { data, .. }) => response::success(data),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 更新提供商密钥
pub async fn update_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
    Json(payload): Json<UpdateProviderKeyRequest>,
) -> axum::response::Response {
    let service = ProviderKeyService::new(&state);
    match service
        .update(key_id, auth_context.user_id, &timezone_context, &payload)
        .await
    {
        Ok(ServiceResponse { data, message }) => {
            let msg = message.unwrap_or_else(|| "更新成功".to_string());
            response::success_with_message(data, &msg)
        }
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 删除提供商密钥
pub async fn delete_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = ProviderKeyService::new(&state);
    match service
        .delete(auth_context.user_id, &timezone_context, key_id)
        .await
    {
        Ok(ServiceResponse { data, message }) => {
            let msg = message.unwrap_or_else(|| "删除成功".to_string());
            response::success_with_message(data, &msg)
        }
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 获取密钥统计信息
pub async fn get_provider_key_stats(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = ProviderKeyService::new(&state);
    match service
        .stats(auth_context.user_id, &timezone_context, key_id)
        .await
    {
        Ok(ServiceResponse { data, .. }) => response::success(data),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 获取密钥卡片统计
pub async fn get_provider_keys_dashboard_stats(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let service = ProviderKeyService::new(&state);
    match service.dashboard(auth_context.user_id).await {
        Ok(ServiceResponse { data, .. }) => response::success(data),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 获取简化的密钥列表
pub async fn get_simple_provider_keys_list(
    State(state): State<AppState>,
    Query(query): Query<UserProviderKeyQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let service = ProviderKeyService::new(&state);
    match service.simple_list(auth_context.user_id, &query).await {
        Ok(ServiceResponse { data, .. }) => response::success(data),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 执行密钥健康检查
pub async fn health_check_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = ProviderKeyService::new(&state);
    match service
        .health_check(auth_context.user_id, &timezone_context, key_id)
        .await
    {
        Ok(ServiceResponse { data, message }) => {
            let msg = message.unwrap_or_else(|| "健康检查完成".to_string());
            response::success_with_message(data, &msg)
        }
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 获取密钥趋势数据
pub async fn get_provider_key_trends(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    Query(query): Query<TrendQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = ProviderKeyService::new(&state);
    match service
        .trends(auth_context.user_id, key_id, &query, &timezone_context)
        .await
    {
        Ok(ServiceResponse { data, .. }) => response::success(data),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 获取用户服务 API 趋势数据
pub async fn get_user_service_api_trends(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
    Query(query): Query<TrendQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = ProviderKeyService::new(&state);
    match service
        .user_service_trends(auth_context.user_id, api_id, &query, &timezone_context)
        .await
    {
        Ok(ServiceResponse { data, .. }) => response::success(data),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 获取可选的密钥健康状态列表
#[must_use]
pub fn get_provider_key_health_statuses() -> axum::response::Response {
    let mut statuses = vec![serde_json::json!({ "value": "all", "label": "全部" })];

    for status in &[
        ApiKeyHealthStatus::Healthy,
        ApiKeyHealthStatus::RateLimited,
        ApiKeyHealthStatus::Unhealthy,
    ] {
        statuses.push(serde_json::json!({
            "value": status.to_string(),
            "label": match status {
                ApiKeyHealthStatus::Healthy => "健康",
                ApiKeyHealthStatus::RateLimited => "限流中",
                ApiKeyHealthStatus::Unhealthy => "不健康",
            }
        }));
    }

    Json(statuses).into_response()
}
