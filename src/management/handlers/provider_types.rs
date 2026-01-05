use crate::logging::{LogComponent, LogStage, log_management_error};
use crate::management::middleware::{RequestId, auth::AuthContext};
use crate::management::services::provider_types;
use crate::management::services::{
    CreateProviderTypeRequest, ProviderTypesCrudService, UpdateProviderTypeRequest,
};
use crate::management::{response, server::ManagementState};
use crate::types::TimezoneContext;
use axum::extract::{Extension, Path, Query, State};
use axum::response::Json;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

/// Provider 类型列表查询参数
#[derive(Debug, Deserialize)]
pub struct ProviderTypesQuery {
    /// 是否仅返回启用/禁用的服务商类型
    ///
    /// - 未传时默认 true（保持旧行为）
    pub is_active: Option<bool>,
    /// 是否包含禁用服务商类型
    ///
    /// - 为 true 时忽略 `is_active` 过滤，返回全部
    #[serde(default)]
    pub include_inactive: bool,
}

/// 获取服务提供商类型列表
pub async fn list_provider_types(
    State(state): State<ManagementState>,
    Query(query): Query<ProviderTypesQuery>,
    Extension(request_id): Extension<RequestId>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    // 兼容旧逻辑：默认只返回启用的服务商；Providers 页面可通过 include_inactive=true 获取全量
    let is_active_filter = if query.include_inactive {
        None
    } else {
        Some(query.is_active.unwrap_or(true))
    };

    match provider_types::list_types(&state, &timezone_context.timezone, is_active_filter).await {
        Ok(list) => {
            let data = json!({ "provider_types": list });
            response::success(data)
        }
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::Config,
                "list_provider_types_failed",
                "获取服务商类型列表失败",
                &err,
            );
            crate::management::response::app_error(err)
        }
    }
}

pub async fn get_scheduling_strategies(
    Extension(_auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let strategies = provider_types::list_scheduling_strategies();
    let data = json!({ "scheduling_strategies": strategies });
    response::success(data)
}

/// 获取单个服务商类型（按 id）
pub async fn get_provider_type(
    State(state): State<ManagementState>,
    Path(id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = ProviderTypesCrudService::new(state.database());
    match service.get(auth_context.as_ref(), id).await {
        Ok(model) => response::success(provider_types::to_item(&model, &timezone_context.timezone)),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Config,
                "get_provider_type_failed",
                "获取服务商类型失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 创建服务商类型（按 `auth_type` 分行）
pub async fn create_provider_type(
    State(state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
    Json(request): Json<CreateProviderTypeRequest>,
) -> axum::response::Response {
    let service = ProviderTypesCrudService::new(state.database());
    match service.create(auth_context.as_ref(), &request).await {
        Ok(model) => response::success(json!({
            "provider_type": provider_types::to_item(&model, &timezone_context.timezone)
        })),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Config,
                "create_provider_type_failed",
                "创建服务商类型失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 更新服务商类型
pub async fn update_provider_type(
    State(state): State<ManagementState>,
    Path(id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
    Json(request): Json<UpdateProviderTypeRequest>,
) -> axum::response::Response {
    let service = ProviderTypesCrudService::new(state.database());
    match service.update(auth_context.as_ref(), id, &request).await {
        Ok(model) => response::success(json!({
            "provider_type": provider_types::to_item(&model, &timezone_context.timezone)
        })),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Config,
                "update_provider_type_failed",
                "更新服务商类型失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 删除服务商类型
pub async fn delete_provider_type(
    State(state): State<ManagementState>,
    Path(id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let service = ProviderTypesCrudService::new(state.database());
    match service.delete(auth_context.as_ref(), id).await {
        Ok(()) => response::success(json!({ "deleted": true })),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Config,
                "delete_provider_type_failed",
                "删除服务商类型失败",
                &err,
            );
            response::app_error(err)
        }
    }
}
