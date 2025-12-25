//! # 日志管理处理器
//!
//! 处理 HTTP 请求，委托具体业务给 `LogsService`。

use crate::{
    logging::{LogComponent, LogStage, log_management_error},
    management::{
        middleware::{RequestId, auth::AuthContext},
        response::{self, ApiResponse},
        server::ManagementState,
        services::logs::{LogsAnalyticsQuery, LogsListQuery, LogsService},
    },
    types::TimezoneContext,
};
use axum::{
    extract::{Extension, Path, Query, State},
    response::IntoResponse,
};
use std::sync::Arc;

/// 获取日志仪表板统计数据
pub async fn get_dashboard_stats(
    State(state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
    Extension(_auth_context): Extension<Arc<AuthContext>>,
) -> impl IntoResponse {
    let service = LogsService::new(&state);
    match service.dashboard_stats().await {
        Ok(summary) => response::success(summary),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::Tracing,
                "get_dashboard_stats_fail",
                "获取日志仪表板统计失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 获取日志列表
pub async fn get_traces_list(
    State(state): State<ManagementState>,
    Query(query): Query<LogsListQuery>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> impl IntoResponse {
    let service = LogsService::new(&state);
    match service
        .traces_list(auth_context.as_ref(), &timezone_context, &query)
        .await
    {
        Ok(result) => response::paginated(result.traces, result.pagination.into()),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::Tracing,
                "get_traces_fail",
                "获取日志列表失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 获取日志详情
pub async fn get_trace_detail(
    State(state): State<ManagementState>,
    Path(id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
    Extension(_auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> impl IntoResponse {
    let service = LogsService::new(&state);
    match service.trace_detail(id, &timezone_context).await {
        Ok(Some(trace)) => response::success(trace),
        Ok(None) => response::app_error(
            crate::error::database::DatabaseError::NotFound(format!("Trace not found: {id}"))
                .into(),
        ),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::Tracing,
                "get_trace_detail_fail",
                "获取日志详情失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 获取日志统计分析
pub async fn get_logs_analytics(
    State(state): State<ManagementState>,
    Query(query): Query<LogsAnalyticsQuery>,
    Extension(request_id): Extension<RequestId>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
    Extension(_auth_context): Extension<Arc<AuthContext>>,
) -> impl IntoResponse {
    let service = LogsService::new(&state);
    match service.analytics(&query, &timezone_context).await {
        Ok(data) => ApiResponse::Success(data).into_response(),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::Statistics,
                "analytics_fail",
                "获取日志统计分析失败",
                &err,
            );
            response::app_error(err)
        }
    }
}
