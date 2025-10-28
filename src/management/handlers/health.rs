//! API密钥健康检查相关处理器

use crate::error::ProxyError;
use crate::management::{response, server::ManagementState};
use crate::{
    lerror,
    logging::{LogComponent, LogStage},
};
use axum::extract::{Path, State};

/// 标记API密钥为不健康
pub async fn mark_key_unhealthy(
    State(state): State<ManagementState>,
    Path(key_id): Path<i32>,
) -> axum::response::Response {
    let reason = "Manually marked unhealthy via management API".to_string();
    match mark_key_unhealthy_internal(&state, key_id, reason).await {
        Ok(()) => response::success("API key marked as unhealthy"),
        Err(err) => {
            lerror!(
                "system",
                LogStage::HealthCheck,
                LogComponent::HealthChecker,
                "mark_key_unhealthy_fail",
                &format!("Failed to mark key {key_id} as unhealthy: {err}")
            );
            response::app_error(ProxyError::internal_with_source(
                format!("Failed to mark key {key_id} as unhealthy"),
                err,
            ))
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
