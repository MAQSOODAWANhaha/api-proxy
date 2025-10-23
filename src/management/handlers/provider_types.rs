use crate::management::middleware::auth::AuthContext;
use crate::management::services::provider_types;
use crate::management::{response, server::AppState};
use crate::types::TimezoneContext;
use axum::extract::{Extension, State};
use serde_json::json;
use std::sync::Arc;

/// 获取服务提供商类型列表
pub async fn list_provider_types(
    State(state): State<AppState>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    match provider_types::list_active_types(&state, &timezone_context.timezone).await {
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
