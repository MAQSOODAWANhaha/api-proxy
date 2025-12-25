//! API密钥健康检查相关处理器

use crate::logging::{LogComponent, LogStage, log_management_error};
use crate::management::middleware::RequestId;
use crate::management::{response, server::ManagementState};
use axum::extract::{Extension, Path, State};

/// 标记API密钥为不健康
pub async fn mark_key_unhealthy(
    State(state): State<ManagementState>,
    Path(key_id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
) -> axum::response::Response {
    let reason = "Manually marked unhealthy via management API".to_string();
    match mark_key_unhealthy_internal(&state, key_id, reason).await {
        Ok(()) => response::success("API key marked as unhealthy"),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::HealthCheck,
                LogComponent::HealthChecker,
                "mark_key_unhealthy_fail",
                "标记 API Key 不健康失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

// 内部实现函数

async fn mark_key_unhealthy_internal(
    state: &ManagementState,
    key_id: i32,
    reason: String,
) -> crate::error::Result<()> {
    // 使用共享的健康检查器
    let api_key_health_service = state.key_pool().api_key_health_service().clone();
    api_key_health_service
        .mark_key_unhealthy(key_id, reason)
        .await
}
