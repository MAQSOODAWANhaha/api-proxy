//! # 统一统计信息处理器
//!
//! 调度 `StatisticsService` 处理管理端统计查询。

use crate::{
    logging::{LogComponent, LogStage, log_management_error},
    management::{
        middleware::{RequestId, auth::AuthContext},
        response,
        server::ManagementState,
        services::statistics::{StatisticsService, TimeRangeQuery},
    },
    types::TimezoneContext,
};
use axum::extract::{Extension, Query, State};
use std::sync::Arc;

/// 今日仪表板卡片 API: /api/statistics/today/cards
pub async fn get_today_dashboard_cards(
    State(state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = StatisticsService::new(&state);
    match service
        .today_dashboard_cards(auth_context.user_id, &timezone_context)
        .await
    {
        Ok(cards) => response::success(cards),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Internal,
                LogComponent::Statistics,
                "today_cards_fail",
                "获取今日仪表板卡片失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 模型使用占比 API: /api/statistics/models/rate
pub async fn get_models_usage_rate(
    State(state): State<ManagementState>,
    Query(query): Query<TimeRangeQuery>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = StatisticsService::new(&state);
    match service
        .models_usage_rate(auth_context.user_id, &query, &timezone_context)
        .await
    {
        Ok(data) => response::success(data),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "fetch_models_rate_fail",
                "获取模型使用占比失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 模型详细统计 API: /api/statistics/models/statistics
pub async fn get_models_statistics(
    State(state): State<ManagementState>,
    Query(query): Query<TimeRangeQuery>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = StatisticsService::new(&state);
    match service
        .models_statistics(auth_context.user_id, &query, &timezone_context)
        .await
    {
        Ok(data) => response::success(data),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "fetch_models_stats_fail",
                "获取模型详细统计失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// Token 使用趋势 API: /api/statistics/tokens/trend
pub async fn get_tokens_trend(
    State(state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = StatisticsService::new(&state);
    match service
        .tokens_trend(auth_context.user_id, &timezone_context)
        .await
    {
        Ok(data) => response::success(data),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "fetch_tokens_trend_fail",
                "获取 Token 趋势失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 用户 API Keys 请求趋势 API: /api/statistics/user-service-api-keys/request
pub async fn get_user_api_keys_request_trend(
    State(state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = StatisticsService::new(&state);
    match service
        .user_api_keys_request_trend(auth_context.user_id, &timezone_context)
        .await
    {
        Ok(data) => response::success(data),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "fetch_user_keys_request_trend_fail",
                "获取用户 API Keys 请求趋势失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 用户 API Keys Token 趋势 API: /api/statistics/user-service-api-keys/token
pub async fn get_user_api_keys_token_trend(
    State(state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = StatisticsService::new(&state);
    match service
        .user_api_keys_token_trend(auth_context.user_id, &timezone_context)
        .await
    {
        Ok(data) => response::success(data),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "fetch_user_keys_token_trend_fail",
                "获取用户 API Keys Token 趋势失败",
                &err,
            );
            response::app_error(err)
        }
    }
}
