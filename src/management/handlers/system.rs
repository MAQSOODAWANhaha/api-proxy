//! # 系统信息处理器

use crate::logging::{LogComponent, LogStage, log_management_error};
use crate::management::middleware::RequestId;
use crate::management::response;
use crate::management::server::ManagementState;
use crate::management::services::system;
use crate::types::TimezoneContext;
use axum::extract::{Extension, State};
use std::sync::Arc;

/// 初始化启动时间
pub fn init_start_time() {
    system::init_start_time();
}

/// 获取系统信息
pub async fn get_system_info(State(state): State<ManagementState>) -> axum::response::Response {
    let info = system::build_system_info(&state);
    response::success(info)
}

/// 获取系统指标
pub async fn get_system_metrics(
    State(_state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
) -> axum::response::Response {
    match system::collect_system_metrics().await {
        Ok(metrics) => response::success(metrics),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::Main,
                "get_system_metrics_failed",
                "获取系统指标失败",
                &err,
            );
            crate::management::response::app_error(err)
        }
    }
}

/// 根路径处理器（管理API信息）
pub async fn root_handler(
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    response::success(system::build_root_metadata(&timezone_context.timezone))
}

/// Ping 处理器
pub async fn ping_handler() -> &'static str {
    "pong"
}
