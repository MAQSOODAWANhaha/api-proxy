//! # 公开统计查询接口
//!
//! 拆分后的统计接口，按模块提供概览、趋势、模型占比与日志数据。

use std::ops::Range;
use std::sync::Arc;

use axum::{
    Extension,
    extract::{Query, State},
    response::Response,
};
use chrono::{DateTime, Duration, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::Result,
    lerror, linfo,
    logging::{LogComponent, LogStage},
    management::{
        response,
        server::ManagementState,
        services::stats_public::{
            AggregateMode, LogsPayload, ModelShareItem, ModelSharePayload, StatsLogsParams,
            StatsModelShareParams, StatsOverviewParams, StatsService, StatsTrendParams,
            SummaryMetric, TrendPoint,
        },
    },
    types::{TimezoneContext, timezone_utils},
};

/// 默认分页页码
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

/// 模型占比接口是否包含当天数据
const fn default_include_today() -> bool {
    true
}

/// 概览查询参数
#[derive(Debug, Deserialize)]
pub struct OverviewQuery {
    pub user_service_key: String,
    #[serde(default)]
    pub from: Option<DateTime<Utc>>,
    #[serde(default)]
    pub to: Option<DateTime<Utc>>,
    #[serde(default = "default_aggregate_mode")]
    pub aggregate: AggregateMode,
}

/// 趋势查询参数
#[derive(Debug, Deserialize)]
pub struct TrendQuery {
    pub user_service_key: String,
    #[serde(default)]
    pub from: Option<DateTime<Utc>>,
    #[serde(default)]
    pub to: Option<DateTime<Utc>>,
    #[serde(default)]
    pub timeframe: Option<String>,
    #[serde(default = "default_aggregate_mode")]
    pub aggregate: AggregateMode,
}

/// 模型占比查询参数
#[derive(Debug, Deserialize)]
pub struct ModelShareQuery {
    pub user_service_key: String,
    #[serde(default)]
    pub from: Option<DateTime<Utc>>,
    #[serde(default)]
    pub to: Option<DateTime<Utc>>,
    #[serde(default = "default_include_today")]
    pub include_today: bool,
    #[serde(default = "default_aggregate_mode")]
    pub aggregate: AggregateMode,
}

/// 日志查询参数
#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    pub user_service_key: String,
    #[serde(default)]
    pub from: Option<DateTime<Utc>>,
    #[serde(default)]
    pub to: Option<DateTime<Utc>>,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default = "default_aggregate_mode")]
    pub aggregate: AggregateMode,
}

/// 概览响应
#[derive(Debug, Serialize)]
pub struct OverviewResponse {
    pub summary: Vec<SummaryMetric>,
}

/// 趋势响应
#[derive(Debug, Serialize)]
pub struct TrendResponse {
    pub trend: Vec<TrendPoint>,
}

/// 模型占比响应
#[derive(Debug, Serialize)]
pub struct ModelShareResponse {
    pub today: Vec<ModelShareItem>,
    pub total: Vec<ModelShareItem>,
}

/// 日志响应
#[derive(Debug, Serialize)]
pub struct LogsResponse {
    pub logs: LogsPayload,
}

/// 公共概览接口
pub async fn get_stats_overview(
    State(state): State<ManagementState>,
    Extension(timezone_ctx): Extension<Arc<TimezoneContext>>,
    Query(query): Query<OverviewQuery>,
) -> Response {
    let request_id = Uuid::new_v4();
    let timezone = timezone_ctx.timezone;
    let service = StatsService::new(state.database.as_ref());

    let response = async {
        let range = resolve_range(
            query.from,
            query.to,
            None,
            timezone,
            &query.user_service_key,
        )?;
        let params = StatsOverviewParams {
            user_service_key: query.user_service_key.clone(),
            range,
            aggregate: query.aggregate,
            timezone,
        };
        service.overview(&params).await
    }
    .await;

    match response {
        Ok(summary) => {
            linfo!(
                request_id,
                LogStage::ExternalApi,
                LogComponent::Statistics,
                "stats_overview_success",
                "public stats overview succeed",
                count = summary.len()
            );
            response::success(OverviewResponse { summary })
        }
        Err(err) => {
            err.log();
            lerror!(
                request_id,
                LogStage::ExternalApi,
                LogComponent::Statistics,
                "stats_overview_failed",
                "public stats overview failed",
                error = %err,
            );
            response::app_error(err)
        }
    }
}

/// 公共趋势接口
pub async fn get_stats_trend(
    State(state): State<ManagementState>,
    Extension(timezone_ctx): Extension<Arc<TimezoneContext>>,
    Query(query): Query<TrendQuery>,
) -> Response {
    let request_id = Uuid::new_v4();
    let timezone = timezone_ctx.timezone;
    let service = StatsService::new(state.database.as_ref());

    let response = async {
        let timeframe = parse_timeframe(query.timeframe.as_deref())?;
        let range = resolve_range(
            query.from,
            query.to,
            timeframe,
            timezone,
            &query.user_service_key,
        )?;
        let params = StatsTrendParams {
            user_service_key: query.user_service_key.clone(),
            range,
            aggregate: query.aggregate,
            timezone,
        };
        service.trend(&params).await
    }
    .await;

    match response {
        Ok(trend) => {
            linfo!(
                request_id,
                LogStage::ExternalApi,
                LogComponent::Statistics,
                "stats_trend_success",
                "public stats trend succeed",
                count = trend.len()
            );
            response::success(TrendResponse { trend })
        }
        Err(err) => {
            err.log();
            lerror!(
                request_id,
                LogStage::ExternalApi,
                LogComponent::Statistics,
                "stats_trend_failed",
                "public stats trend failed",
                error = %err,
            );
            response::app_error(err)
        }
    }
}

/// 公共模型占比接口
pub async fn get_stats_model_share(
    State(state): State<ManagementState>,
    Extension(timezone_ctx): Extension<Arc<TimezoneContext>>,
    Query(query): Query<ModelShareQuery>,
) -> Response {
    let request_id = Uuid::new_v4();
    let timezone = timezone_ctx.timezone;
    let service = StatsService::new(state.database.as_ref());

    let response = async {
        let range = resolve_range(
            query.from,
            query.to,
            None,
            timezone,
            &query.user_service_key,
        )?;
        let params = StatsModelShareParams {
            user_service_key: query.user_service_key.clone(),
            range,
            aggregate: query.aggregate,
            timezone,
            include_today: query.include_today,
        };
        service.model_share(&params).await
    }
    .await;

    match response {
        Ok(ModelSharePayload { today, total }) => {
            linfo!(
                request_id,
                LogStage::ExternalApi,
                LogComponent::Statistics,
                "stats_model_share_success",
                "public stats model share succeed",
                today = today.len(),
                total = total.len()
            );
            response::success(ModelShareResponse { today, total })
        }
        Err(err) => {
            err.log();
            lerror!(
                request_id,
                LogStage::ExternalApi,
                LogComponent::Statistics,
                "stats_model_share_failed",
                "public stats model share failed",
                error = %err,
            );
            response::app_error(err)
        }
    }
}

/// 公共日志接口
pub async fn get_stats_logs(
    State(state): State<ManagementState>,
    Extension(timezone_ctx): Extension<Arc<TimezoneContext>>,
    Query(query): Query<LogsQuery>,
) -> Response {
    let request_id = Uuid::new_v4();
    let timezone = timezone_ctx.timezone;
    let service = StatsService::new(state.database.as_ref());

    let response = async {
        let range = resolve_range(
            query.from,
            query.to,
            None,
            timezone,
            &query.user_service_key,
        )?;
        let params = StatsLogsParams {
            user_service_key: query.user_service_key.clone(),
            range,
            aggregate: query.aggregate,
            page: query.page,
            page_size: query.page_size,
            search: query.search.clone(),
            timezone,
        };
        service.logs(&params).await
    }
    .await;

    match response {
        Ok(logs) => {
            linfo!(
                request_id,
                LogStage::ExternalApi,
                LogComponent::Statistics,
                "stats_logs_success",
                "public stats logs succeed",
                page = logs.page,
                page_size = logs.page_size,
                total = logs.total
            );
            response::success(LogsResponse { logs })
        }
        Err(err) => {
            err.log();
            lerror!(
                request_id,
                LogStage::ExternalApi,
                LogComponent::Statistics,
                "stats_logs_failed",
                "public stats logs failed",
                error = %err,
            );
            response::app_error(err)
        }
    }
}

fn resolve_range(
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    timeframe: Option<Duration>,
    timezone: Tz,
    user_service_key: &str,
) -> Result<Range<DateTime<Utc>>> {
    crate::ensure!(
        !user_service_key.trim().is_empty(),
        Authentication,
        "user_service_key_required"
    );

    let end = to.unwrap_or_else(Utc::now);
    let start = from.unwrap_or_else(|| {
        timeframe.map_or_else(
            || {
                timezone_utils::local_day_bounds(&end, &timezone).map_or_else(
                    || {
                        let start_of_day = end
                            .date_naive()
                            .and_hms_opt(0, 0, 0)
                            .unwrap_or_else(|| end.naive_utc());
                        DateTime::<Utc>::from_naive_utc_and_offset(start_of_day, Utc)
                    },
                    |(start, _)| start,
                )
            },
            |duration| end - duration,
        )
    });

    crate::ensure!(start < end, Authentication, "invalid_time_range");

    Ok(Range { start, end })
}

fn parse_timeframe(value: Option<&str>) -> Result<Option<Duration>> {
    let Some(text) = value else {
        return Ok(None);
    };

    let duration = match text {
        "7d" => Duration::days(7),
        "30d" => Duration::days(30),
        "90d" => Duration::days(90),
        "1d" => Duration::days(1),
        _ => {
            return Err(crate::error!(
                Authentication,
                format!("invalid_timeframe_value: {text}")
            ));
        }
    };

    Ok(Some(duration))
}
