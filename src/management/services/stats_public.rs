//! # 公开统计服务
//!
//! 基于 `proxy_tracing` 表聚合 `user_service_key` 对应的请求、Token、费用等统计数据。

use std::ops::Range;

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use chrono_tz::Tz;
use entity::{
    proxy_tracing, proxy_tracing::Entity as ProxyTracing, user_service_apis,
    user_service_apis::Entity as UserServiceApis,
};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait,
    FromQueryResult, Order, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, sea_query::Expr,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::Result,
    types::{
        conversion::{option_u64_from_i64, ratio_as_f64, ratio_as_percentage},
        timezone_utils,
    },
};

/// 指定聚合模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum AggregateMode {
    Single,
    Aggregate,
}

impl Default for AggregateMode {
    fn default() -> Self {
        Self::Single
    }
}

impl From<bool> for AggregateMode {
    fn from(value: bool) -> Self {
        if value { Self::Aggregate } else { Self::Single }
    }
}

/// 汇总卡片数据
#[derive(Debug, Clone, Serialize, Default)]
pub struct SummaryMetric {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub unit: String,
    pub today: f64,
    pub total: f64,
    pub delta: f64,
}

/// 趋势数据点
#[derive(Debug, Clone, Serialize)]
pub struct TrendPoint {
    pub timestamp: DateTime<Utc>,
    pub requests: i64,
    pub tokens: i64,
    pub cost: f64,
    pub success_rate: f64,
}

/// 模型占比项
#[derive(Debug, Clone, Serialize)]
pub struct ModelShareItem {
    pub model: String,
    pub scope: String,
    pub requests: i64,
    pub tokens: i64,
    pub cost: f64,
    pub percentage: f64,
}

/// 模型占比响应
#[derive(Debug, Clone, Serialize)]
pub struct ModelSharePayload {
    pub today: Vec<ModelShareItem>,
    pub total: Vec<ModelShareItem>,
}

/// 日志条目
#[derive(Debug, Clone, Serialize)]
pub struct LogItem {
    pub id: i32,
    pub timestamp: NaiveDateTime,
    pub method: String,
    pub path: Option<String>,
    pub status_code: Option<i32>,
    pub is_success: bool,
    pub duration_ms: Option<i64>,
    pub model: Option<String>,
    pub tokens_prompt: i32,
    pub tokens_completion: i32,
    pub tokens_total: i32,
    pub cost: Option<f64>,
    pub cost_currency: Option<String>,
    pub request_id: String,
    pub operation: Option<String>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub provider_type_id: Option<i32>,
    pub retry_count: i32,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
}

/// 分页日志数据
#[derive(Debug, Clone, Serialize)]
pub struct LogsPayload {
    pub items: Vec<LogItem>,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
}

/// 概览查询参数
#[derive(Debug)]
pub struct StatsOverviewParams {
    pub user_service_key: String,
    pub range: Range<DateTime<Utc>>,
    pub aggregate: AggregateMode,
    pub timezone: Tz,
}

/// 趋势查询参数
#[derive(Debug)]
pub struct StatsTrendParams {
    pub user_service_key: String,
    pub range: Range<DateTime<Utc>>,
    pub aggregate: AggregateMode,
}

/// 模型占比查询参数
#[derive(Debug)]
pub struct StatsModelShareParams {
    pub user_service_key: String,
    pub range: Range<DateTime<Utc>>,
    pub aggregate: AggregateMode,
    pub timezone: Tz,
    pub include_today: bool,
}

/// 调用日志查询参数
#[derive(Debug)]
pub struct StatsLogsParams {
    pub user_service_key: String,
    pub range: Range<DateTime<Utc>>,
    pub aggregate: AggregateMode,
    pub page: u32,
    pub page_size: u32,
    pub search: Option<String>,
}

/// 内部汇总查询结果
#[derive(Debug, Default, FromQueryResult)]
struct AggregateRow {
    #[sea_orm(column_name = "requests")]
    requests: Option<i64>,
    #[sea_orm(column_name = "tokens")]
    tokens: Option<i64>,
    #[sea_orm(column_name = "cost")]
    cost: Option<f64>,
}

#[derive(Debug, FromQueryResult)]
struct TrendRow {
    #[sea_orm(column_name = "bucket")]
    bucket: NaiveDateTime,
    #[sea_orm(column_name = "requests")]
    requests: Option<i64>,
    #[sea_orm(column_name = "tokens")]
    tokens: Option<i64>,
    #[sea_orm(column_name = "cost")]
    cost: Option<f64>,
    #[sea_orm(column_name = "success_rate")]
    success_rate: Option<f64>,
}

#[derive(Debug, FromQueryResult)]
struct ModelShareRow {
    #[sea_orm(column_name = "model")]
    model: Option<String>,
    #[sea_orm(column_name = "requests")]
    requests: Option<i64>,
    #[sea_orm(column_name = "tokens")]
    tokens: Option<i64>,
    #[sea_orm(column_name = "cost")]
    cost: Option<f64>,
}

/// 公开统计服务
pub struct StatsService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> StatsService<'a> {
    #[must_use]
    pub const fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// 执行统计聚合
    pub async fn overview(&self, params: &StatsOverviewParams) -> Result<Vec<SummaryMetric>> {
        crate::ensure!(
            !params.user_service_key.trim().is_empty(),
            Authentication,
            "user_service_key_required"
        );

        crate::ensure!(
            params.range.start < params.range.end,
            Authentication,
            "invalid_time_range"
        );

        let service_ids = self
            .resolve_service_ids(&params.user_service_key, params.aggregate)
            .await?;

        let today_range = timezone_utils::local_day_bounds(&params.range.end, &params.timezone)
            .map(|(start, end)| Range { start, end });

        let (today_summary, total_summary, previous_summary) = self
            .fetch_summary(&service_ids, today_range.as_ref(), &params.range)
            .await?;

        Ok(build_summary_metrics(
            &today_summary,
            &total_summary,
            &previous_summary,
        ))
    }

    pub async fn trend(&self, params: &StatsTrendParams) -> Result<Vec<TrendPoint>> {
        crate::ensure!(
            !params.user_service_key.trim().is_empty(),
            Authentication,
            "user_service_key_required"
        );

        crate::ensure!(
            params.range.start < params.range.end,
            Authentication,
            "invalid_time_range"
        );

        let service_ids = self
            .resolve_service_ids(&params.user_service_key, params.aggregate)
            .await?;

        self.fetch_trend(&service_ids, &params.range).await
    }

    pub async fn model_share(&self, params: &StatsModelShareParams) -> Result<ModelSharePayload> {
        crate::ensure!(
            !params.user_service_key.trim().is_empty(),
            Authentication,
            "user_service_key_required"
        );

        crate::ensure!(
            params.range.start < params.range.end,
            Authentication,
            "invalid_time_range"
        );

        let service_ids = self
            .resolve_service_ids(&params.user_service_key, params.aggregate)
            .await?;

        let today_range = if params.include_today {
            timezone_utils::local_day_bounds(&params.range.end, &params.timezone)
                .map(|(start, end)| Range { start, end })
        } else {
            None
        };

        self.fetch_model_share(&service_ids, today_range.as_ref(), &params.range)
            .await
    }

    pub async fn logs(&self, params: &StatsLogsParams) -> Result<LogsPayload> {
        crate::ensure!(
            !params.user_service_key.trim().is_empty(),
            Authentication,
            "user_service_key_required"
        );

        crate::ensure!(
            params.range.start < params.range.end,
            Authentication,
            "invalid_time_range"
        );

        let service_ids = self
            .resolve_service_ids(&params.user_service_key, params.aggregate)
            .await?;

        self.fetch_logs(
            &service_ids,
            &params.range,
            params.page,
            params.page_size,
            params.search.clone(),
        )
        .await
    }

    /// 解析 `user_service_key` 对应的 service id 列表
    pub async fn resolve_service_ids(
        &self,
        user_service_key: &str,
        aggregate: AggregateMode,
    ) -> Result<Vec<i32>> {
        let key = user_service_key.trim();
        let service = UserServiceApis::find()
            .filter(user_service_apis::Column::ApiKey.eq(key))
            .one(self.db)
            .await
            .map_err(|err| {
                crate::error!(Database, format!("fetch_user_service_api_failed: {err}"))
            })?
            .ok_or_else(|| crate::error!(Authentication, "user_service_key_invalid"))?;

        if matches!(aggregate, AggregateMode::Aggregate) {
            let ids: Vec<i32> = UserServiceApis::find()
                .select_only()
                .column(user_service_apis::Column::Id)
                .filter(user_service_apis::Column::UserId.eq(service.user_id))
                .into_tuple()
                .all(self.db)
                .await
                .map_err(|err| {
                    crate::error!(
                        Database,
                        format!("fetch_aggregate_user_service_ids_failed: {err}")
                    )
                })?;

            crate::ensure!(!ids.is_empty(), Authentication, "no_service_keys_for_user");

            return Ok(ids);
        }

        Ok(vec![service.id])
    }

    async fn fetch_summary(
        &self,
        service_ids: &[i32],
        today: Option<&Range<DateTime<Utc>>>,
        range: &Range<DateTime<Utc>>,
    ) -> Result<(AggregateRow, AggregateRow, AggregateRow)> {
        let today_summary = if let Some(today_range) = today {
            self.aggregate(service_ids, today_range).await?
        } else {
            AggregateRow::default()
        };

        let total_summary = self.aggregate(service_ids, range).await?;

        let previous_range = previous_period(range);
        let previous_summary = self.aggregate(service_ids, &previous_range).await?;

        Ok((today_summary, total_summary, previous_summary))
    }

    async fn aggregate(
        &self,
        service_ids: &[i32],
        range: &Range<DateTime<Utc>>,
    ) -> Result<AggregateRow> {
        #[derive(FromQueryResult)]
        struct Row {
            #[sea_orm(column_name = "requests")]
            requests: Option<i64>,
            #[sea_orm(column_name = "tokens")]
            tokens: Option<i64>,
            #[sea_orm(column_name = "cost")]
            cost: Option<f64>,
        }

        let select = ProxyTracing::find()
            .select_only()
            .column_as(proxy_tracing::Column::Id.count(), "requests")
            .column_as(proxy_tracing::Column::TokensTotal.sum(), "tokens")
            .column_as(proxy_tracing::Column::Cost.sum(), "cost")
            .filter(proxy_tracing::Column::UserServiceApiId.is_in(service_ids.to_vec()))
            .filter(proxy_tracing::Column::CreatedAt.gte(range.start.naive_utc()))
            .filter(proxy_tracing::Column::CreatedAt.lt(range.end.naive_utc()));

        let row = select
            .into_model::<Row>()
            .one(self.db)
            .await
            .map_err(|err| crate::error!(Database, format!("aggregate_stats_failed: {err}")))?;

        let (requests, tokens, cost) =
            row.map_or((None, None, None), |r| (r.requests, r.tokens, r.cost));

        Ok(AggregateRow {
            requests,
            tokens,
            cost,
        })
    }

    async fn fetch_trend(
        &self,
        service_ids: &[i32],
        range: &Range<DateTime<Utc>>,
    ) -> Result<Vec<TrendPoint>> {
        let interval = pick_trend_interval(range);
        let bucket_expr = trend_bucket_expr(self.db.get_database_backend(), interval);

        let select = ProxyTracing::find()
            .select_only()
            .column_as(Expr::cust(bucket_expr), "bucket")
            .column_as(proxy_tracing::Column::Id.count(), "requests")
            .column_as(proxy_tracing::Column::TokensTotal.sum(), "tokens")
            .column_as(proxy_tracing::Column::Cost.sum(), "cost")
            .column_as(
                Expr::cust("AVG(CASE WHEN is_success = true THEN 1 ELSE 0 END)"),
                "success_rate",
            )
            .filter(proxy_tracing::Column::UserServiceApiId.is_in(service_ids.to_vec()))
            .filter(proxy_tracing::Column::CreatedAt.gte(range.start.naive_utc()))
            .filter(proxy_tracing::Column::CreatedAt.lt(range.end.naive_utc()))
            .group_by(Expr::cust(bucket_expr))
            .order_by(Expr::cust(bucket_expr), Order::Asc);

        let rows = select
            .into_model::<TrendRow>()
            .all(self.db)
            .await
            .map_err(|err| crate::error!(Database, format!("trend_query_failed: {err}")))?;

        Ok(rows
            .into_iter()
            .map(|row| TrendPoint {
                timestamp: DateTime::<Utc>::from_naive_utc_and_offset(row.bucket, Utc),
                requests: row.requests.unwrap_or(0),
                tokens: row.tokens.unwrap_or(0),
                cost: row.cost.unwrap_or(0.0),
                success_rate: row.success_rate.unwrap_or(0.0),
            })
            .collect())
    }

    async fn fetch_model_share(
        &self,
        service_ids: &[i32],
        today: Option<&Range<DateTime<Utc>>>,
        range: &Range<DateTime<Utc>>,
    ) -> Result<ModelSharePayload> {
        let today_items = if let Some(today_range) = today {
            let today_rows = self.model_share_by_scope(service_ids, today_range).await?;
            compute_model_percentage(today_rows, "today")
        } else {
            Vec::new()
        };

        let total_rows = self.model_share_by_scope(service_ids, range).await?;
        let total_items = compute_model_percentage(total_rows, "total");

        Ok(ModelSharePayload {
            today: today_items,
            total: total_items,
        })
    }

    async fn model_share_by_scope(
        &self,
        service_ids: &[i32],
        range: &Range<DateTime<Utc>>,
    ) -> Result<Vec<ModelShareRow>> {
        let select = ProxyTracing::find()
            .select_only()
            .column_as(Expr::cust("COALESCE(model_used, 'unknown')"), "model")
            .column_as(proxy_tracing::Column::Id.count(), "requests")
            .column_as(proxy_tracing::Column::TokensTotal.sum(), "tokens")
            .column_as(proxy_tracing::Column::Cost.sum(), "cost")
            .filter(proxy_tracing::Column::UserServiceApiId.is_in(service_ids.to_vec()))
            .filter(proxy_tracing::Column::CreatedAt.gte(range.start.naive_utc()))
            .filter(proxy_tracing::Column::CreatedAt.lt(range.end.naive_utc()))
            .group_by(Expr::cust("COALESCE(model_used, 'unknown')"))
            .order_by(Expr::cust("requests"), Order::Desc);

        select
            .into_model::<ModelShareRow>()
            .all(self.db)
            .await
            .map_err(|err| crate::error!(Database, format!("model_share_query_failed: {err}")))
    }

    async fn fetch_logs(
        &self,
        service_ids: &[i32],
        range: &Range<DateTime<Utc>>,
        page: u32,
        page_size: u32,
        search: Option<String>,
    ) -> Result<LogsPayload> {
        let page = page.max(1);
        let page_size = page_size.clamp(1, 200);

        let mut base = ProxyTracing::find()
            .filter(proxy_tracing::Column::UserServiceApiId.is_in(service_ids.to_vec()))
            .filter(proxy_tracing::Column::CreatedAt.gte(range.start.naive_utc()))
            .filter(proxy_tracing::Column::CreatedAt.lt(range.end.naive_utc()));

        if let Some(query) = search.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            let pattern = format!("%{query}%");
            base = base.filter(
                Condition::any()
                    .add(proxy_tracing::Column::RequestId.like(&pattern))
                    .add(proxy_tracing::Column::ModelUsed.like(&pattern)),
            );
        }

        let paginator = base
            .clone()
            .order_by_desc(proxy_tracing::Column::CreatedAt)
            .paginate(self.db, u64::from(page_size));

        let total = paginator
            .num_items()
            .await
            .map_err(|err| crate::error!(Database, format!("logs_count_failed: {err}")))?;

        let records = paginator
            .fetch_page(u64::from(page.saturating_sub(1)))
            .await
            .map_err(|err| crate::error!(Database, format!("logs_fetch_failed: {err}")))?;

        let items = records
            .into_iter()
            .map(|model| LogItem {
                id: model.id,
                timestamp: model.created_at,
                method: model.method,
                path: model.path,
                status_code: model.status_code,
                is_success: model.is_success,
                duration_ms: model.duration_ms,
                model: model.model_used,
                tokens_prompt: model.tokens_prompt.unwrap_or(0),
                tokens_completion: model.tokens_completion.unwrap_or(0),
                tokens_total: model.tokens_total.unwrap_or(0),
                cost: model.cost,
                cost_currency: model.cost_currency,
                request_id: model.request_id,
                operation: None,
                error_type: model.error_type,
                error_message: model.error_message,
                provider_type_id: model.provider_type_id,
                retry_count: model.retry_count.unwrap_or(0),
                client_ip: model.client_ip,
                user_agent: model.user_agent,
            })
            .collect();

        Ok(LogsPayload {
            items,
            page,
            page_size,
            total,
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum TrendInterval {
    Hour,
    Day,
}

fn pick_trend_interval(range: &Range<DateTime<Utc>>) -> TrendInterval {
    let duration = range.end - range.start;
    if duration <= Duration::hours(48) {
        TrendInterval::Hour
    } else {
        TrendInterval::Day
    }
}

fn previous_period(range: &Range<DateTime<Utc>>) -> Range<DateTime<Utc>> {
    let duration = range.end - range.start;
    Range {
        start: range.start - duration,
        end: range.start,
    }
}

const fn trend_bucket_expr(backend: DatabaseBackend, interval: TrendInterval) -> &'static str {
    match backend {
        DatabaseBackend::Sqlite => match interval {
            TrendInterval::Hour => {
                "DATETIME((CAST(strftime('%s', created_at) AS INTEGER) / 3600) * 3600, 'unixepoch')"
            }
            TrendInterval::Day => {
                "DATETIME((CAST(strftime('%s', created_at) AS INTEGER) / 86400) * 86400, 'unixepoch')"
            }
        },
        _ => match interval {
            TrendInterval::Hour => "DATE_TRUNC('hour', created_at)",
            TrendInterval::Day => "DATE_TRUNC('day', created_at)",
        },
    }
}

fn build_summary_metrics(
    today: &AggregateRow,
    total: &AggregateRow,
    previous: &AggregateRow,
) -> Vec<SummaryMetric> {
    let requests_delta = percentage_delta(
        opt_i64_to_f64(total.requests),
        opt_i64_to_f64(previous.requests),
    );
    let tokens_delta = percentage_delta(
        opt_i64_to_f64(total.tokens),
        opt_i64_to_f64(previous.tokens),
    );
    let cost_delta = percentage_delta(total.cost.unwrap_or(0.0), previous.cost.unwrap_or(0.0));

    vec![
        SummaryMetric {
            id: "requests".to_string(),
            label: "请求次数".to_string(),
            icon: "BarChart4".to_string(),
            unit: "count".to_string(),
            today: opt_i64_to_f64(today.requests),
            total: opt_i64_to_f64(total.requests),
            delta: requests_delta,
        },
        SummaryMetric {
            id: "tokens".to_string(),
            label: "Token 消耗".to_string(),
            icon: "Coins".to_string(),
            unit: "token".to_string(),
            today: opt_i64_to_f64(today.tokens),
            total: opt_i64_to_f64(total.tokens),
            delta: tokens_delta,
        },
        SummaryMetric {
            id: "cost".to_string(),
            label: "费用".to_string(),
            icon: "DollarSign".to_string(),
            unit: "usd".to_string(),
            today: today.cost.unwrap_or(0.0),
            total: total.cost.unwrap_or(0.0),
            delta: cost_delta,
        },
    ]
}

fn opt_i64_to_f64(value: Option<i64>) -> f64 {
    option_u64_from_i64(value)
        .and_then(|num| ratio_as_f64(num, 1))
        .unwrap_or(0.0)
}

fn percentage_delta(current: f64, previous: f64) -> f64 {
    if previous.abs() < f64::EPSILON {
        return if current.abs() < f64::EPSILON {
            0.0
        } else {
            100.0
        };
    }

    ((current - previous) / previous) * 100.0
}

fn compute_model_percentage(rows: Vec<ModelShareRow>, scope: &str) -> Vec<ModelShareItem> {
    let total_requests: u64 = rows
        .iter()
        .filter_map(|row| option_u64_from_i64(row.requests))
        .sum();

    rows.into_iter()
        .map(|row| {
            let requests = row.requests.unwrap_or(0);
            let percentage = option_u64_from_i64(Some(requests))
                .map_or(0.0, |count| ratio_as_percentage(count, total_requests));

            ModelShareItem {
                model: row.model.unwrap_or_else(|| "unknown".to_string()),
                scope: scope.to_string(),
                requests,
                tokens: row.tokens.unwrap_or(0),
                cost: row.cost.unwrap_or(0.0),
                percentage,
            }
        })
        .collect()
}
