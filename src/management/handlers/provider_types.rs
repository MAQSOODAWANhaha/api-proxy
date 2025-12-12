use crate::management::middleware::auth::AuthContext;
use crate::management::services::provider_types;
use crate::management::{response, server::ManagementState};
use crate::types::TimezoneContext;
use axum::extract::{Extension, Query, State};
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
            err.log();
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
