//! # 用户服务 API 管理处理器
//!
//! 负责解析 HTTP 请求并委托业务逻辑到 `services::service_apis`。

#![allow(clippy::too_many_lines)]

use std::sync::Arc;

use axum::{
    Json,
    extract::{Extension, Path, Query, State},
};
use serde_json::Value;

use crate::{
    logging::{LogComponent, LogStage, log_management_error},
    management::{
        middleware::{RequestId, auth::AuthContext},
        response,
        server::ManagementState,
        services::service_apis::{
            CreateUserServiceKeyRequest, ServiceApiService, UpdateStatusRequest,
            UpdateUserServiceKeyRequest, UsageStatsQuery, UserServiceKeyQuery,
        },
    },
    types::TimezoneContext,
};

/// 1. 用户 API Keys 卡片展示
pub async fn get_user_service_cards(
    State(state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let service = ServiceApiService::new(&state);
    match service.cards(auth_context.user_id).await {
        Ok(cards) => response::success(cards),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::ApiKey,
                "get_user_service_cards_failed",
                "获取用户 API Key 卡片失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 2. 用户 API Keys 列表
pub async fn list_user_service_keys(
    State(state): State<ManagementState>,
    Query(query): Query<UserServiceKeyQuery>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = ServiceApiService::new(&state);
    match service
        .list(auth_context.user_id, &query, &timezone_context.timezone)
        .await
    {
        Ok(payload) => response::success(payload),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::ApiKey,
                "list_user_service_keys_failed",
                "获取用户 API Key 列表失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 3. 新增 API Key
pub async fn create_user_service_key(
    State(state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
    Json(request): Json<CreateUserServiceKeyRequest>,
) -> axum::response::Response {
    let service = ServiceApiService::new(&state);
    match service
        .create(auth_context.user_id, &request, &timezone_context.timezone)
        .await
    {
        Ok(result) => response::success_with_message(result, "API Key创建成功"),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::ApiKey,
                "create_user_service_key_failed",
                "创建用户 API Key 失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 4. 获取 API Key 详情
pub async fn get_user_service_key(
    State(state): State<ManagementState>,
    Path(api_id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = ServiceApiService::new(&state);
    match service
        .detail(api_id, auth_context.user_id, &timezone_context.timezone)
        .await
    {
        Ok(detail) => response::success(detail),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::ApiKey,
                "get_user_service_key_failed",
                "获取用户 API Key 详情失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 5. 编辑 API Key
pub async fn update_user_service_key(
    State(state): State<ManagementState>,
    Path(api_id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<UpdateUserServiceKeyRequest>,
) -> axum::response::Response {
    let service = ServiceApiService::new(&state);
    match service.update(api_id, auth_context.user_id, &request).await {
        Ok(result) => response::success_with_message(result, "API Key更新成功"),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::ApiKey,
                "update_user_service_key_failed",
                "更新用户 API Key 失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 6. 删除 API Key
pub async fn delete_user_service_key(
    State(state): State<ManagementState>,
    Path(api_id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let service = ServiceApiService::new(&state);
    match service.delete(api_id, auth_context.user_id).await {
        Ok(()) => response::success_with_message(Value::Null, "API Key删除成功"),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::ApiKey,
                "delete_user_service_key_failed",
                "删除用户 API Key 失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 7. API Key 使用统计
pub async fn get_user_service_key_usage(
    State(state): State<ManagementState>,
    Path(api_id): Path<i32>,
    Query(query): Query<UsageStatsQuery>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = ServiceApiService::new(&state);
    match service
        .usage_stats(
            api_id,
            auth_context.user_id,
            &query,
            &timezone_context.timezone,
        )
        .await
    {
        Ok(summary) => response::success(summary),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::ApiKey,
                "get_user_service_key_usage_failed",
                "获取用户 API Key 使用统计失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 8. 重新生成 API Key
pub async fn regenerate_user_service_key(
    State(state): State<ManagementState>,
    Path(api_id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let service = ServiceApiService::new(&state);
    match service.regenerate(api_id, auth_context.user_id).await {
        Ok(result) => response::success_with_message(result, "API Key重新生成成功"),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::ApiKey,
                "regenerate_user_service_key_failed",
                "重新生成用户 API Key 失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 9. 启用/禁用 API Key
pub async fn update_user_service_key_status(
    State(state): State<ManagementState>,
    Path(api_id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<UpdateStatusRequest>,
) -> axum::response::Response {
    let service = ServiceApiService::new(&state);
    match service
        .update_status(api_id, auth_context.user_id, &request)
        .await
    {
        Ok(result) => response::success_with_message(result, "API Key状态更新成功"),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::ApiKey,
                "update_user_service_key_status_failed",
                "更新用户 API Key 状态失败",
                &err,
            );
            response::app_error(err)
        }
    }
}
