//! # 公开统计查询接口
//!
//! 提供无需认证的 `user_service_key` 数据查询能力。

use std::ops::Range;

use axum::{
    Extension,
    extract::{Query, State},
    response::Response,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::Result,
    lerror, linfo,
    logging::{LogComponent, LogStage},
    management::{
        response,
        server::AppState,
        services::stats::{AggregateMode, StatsParams, StatsPayload, StatsService},
    },
    types::{TimezoneContext, timezone_utils},
};

/// 默认分页
const fn default_page() -> u32 {
    1
}

/// 默认分页大小
const fn default_page_size() -> u32 {
    20
}

/// 聚合默认模式
const fn default_aggregate_mode() -> AggregateMode {
    AggregateMode::Single
}

/// 查询参数
#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    pub user_service_key: String,
    #[serde(default)]
    pub from: Option<DateTime<Utc>>,
    #[serde(default)]
    pub to: Option<DateTime<Utc>>,
    #[serde(default = "default_aggregate_mode")]
    pub aggregate: AggregateMode,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    #[serde(default)]
    pub search: Option<String>,
}

/// 公开统计查询
pub async fn get_stats(
    State(state): State<AppState>,
    Extension(timezone_ctx): Extension<Arc<TimezoneContext>>,
    Query(query): Query<StatsQuery>,
) -> Response {
    let request_id = Uuid::new_v4();
    let timezone = timezone_ctx.timezone;

    match process_stats_request(state, query, timezone, request_id).await {
        Ok(payload) => {
            linfo!(
                request_id,
                LogStage::ExternalApi,
                LogComponent::Statistics,
                "stats_query_success",
                "public stats query succeed",
                items = payload.logs.items.len(),
                total = payload.logs.total
            );
            response::success(payload)
        }
        Err(err) => {
            err.log();
            lerror!(
                request_id,
                LogStage::ExternalApi,
                LogComponent::Statistics,
                "stats_query_failed",
                "public stats query failed",
                error = %err,
            );
            response::app_error(err)
        }
    }
}

async fn process_stats_request(
    state: AppState,
    query: StatsQuery,
    timezone: chrono_tz::Tz,
    request_id: Uuid,
) -> Result<StatsPayload> {
    let range = resolve_range(&query, timezone)?;

    linfo!(
        request_id,
        LogStage::ExternalApi,
        LogComponent::Statistics,
        "stats_query_start",
        "public stats query started",
        user_service_key = &query.user_service_key,
        aggregate = ?query.aggregate,
        page = query.page,
        page_size = query.page_size
    );

    let params = StatsParams {
        user_service_key: query.user_service_key.clone(),
        range,
        aggregate: query.aggregate,
        timezone,
        page: query.page,
        page_size: query.page_size,
        search: query.search.clone(),
    };

    let service = StatsService::new(state.database.as_ref());
    service.collect(&params).await
}

fn resolve_range(query: &StatsQuery, timezone: chrono_tz::Tz) -> Result<Range<DateTime<Utc>>> {
    crate::ensure!(
        !query.user_service_key.trim().is_empty(),
        Authentication,
        "user_service_key_required"
    );

    let to = query.to.unwrap_or_else(Utc::now);
    let default_from = || {
        timezone_utils::local_day_bounds(&to, &timezone).map_or_else(
            || {
                let start_of_day = to
                    .date_naive()
                    .and_hms_opt(0, 0, 0)
                    .unwrap_or_else(|| to.naive_utc());
                DateTime::<Utc>::from_naive_utc_and_offset(start_of_day, Utc)
            },
            |(start, _)| start,
        )
    };
    let from = query.from.unwrap_or_else(default_from);

    crate::ensure!(from < to, Authentication, "invalid_time_range");

    Ok(Range {
        start: from,
        end: to,
    })
}
