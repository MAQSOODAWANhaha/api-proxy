//! # 统一统计服务
//!
//! 聚合统计相关的查询逻辑，供 handler 调用复用。

use crate::{
    error::{ProxyError, Result},
    lerror,
    logging::{LogComponent, LogStage},
    management::server::ManagementState,
    types::{TimezoneContext, ratio_as_percentage, timezone_utils},
};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use entity::{proxy_tracing, proxy_tracing::Entity as ProxyTracing};
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 统计查询参数（兼容旧接口，当前暂未使用）
#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    pub hours: Option<u32>,
    pub group_by: Option<String>,
    pub upstream_type: Option<String>,
    pub provider_type: Option<String>,
    pub trace_level: Option<i32>,
    pub success_only: Option<bool>,
    pub anomaly_only: Option<bool>,
}

/// 时间范围查询参数
#[derive(Debug, Deserialize)]
pub struct TimeRangeQuery {
    pub range: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
}

/// 今日仪表板卡片数据（包含增长率）
#[derive(Debug, Serialize)]
pub struct TodayDashboardCards {
    pub requests_today: i64,
    pub rate_requests_today: String,
    pub successes_today: f64,
    pub rate_successes_today: String,
    pub tokens_today: i64,
    pub rate_tokens_today: String,
    pub avg_response_time_today: i64,
    pub rate_avg_response_time_today: String,
}

/// 模型使用数据
#[derive(Debug, Serialize)]
pub struct ModelUsage {
    pub model: String,
    pub usage: i64,
    pub cost: f64,
    pub successful_requests: i64,
    pub failed_requests: i64,
    pub success_rate: f64,
}

/// 模型使用占比响应
#[derive(Debug, Serialize)]
pub struct ModelsRateResponse {
    pub model_usage: Vec<ModelUsage>,
}

/// 模型详细统计数据
#[derive(Debug, Serialize)]
pub struct ModelStatistics {
    pub model: String,
    pub usage: i64,
    pub percentage: f64,
    pub cost: f64,
}

/// 模型详细统计响应
#[derive(Debug, Serialize)]
pub struct ModelsStatisticsResponse {
    pub model_usage: Vec<ModelStatistics>,
}

/// Token 使用趋势数据点
#[derive(Debug, Serialize)]
pub struct TokenTrendPoint {
    pub timestamp: String,
    pub cache_create_tokens: i64,
    pub cache_read_tokens: i64,
    pub tokens_prompt: i64,
    pub tokens_completion: i64,
    pub cost: f64,
}

/// Token 使用趋势响应
#[derive(Debug, Serialize)]
pub struct TokensTrendResponse {
    pub token_usage: Vec<TokenTrendPoint>,
    pub current_token_usage: i64,
    pub average_token_usage: i64,
    pub max_token_usage: i64,
}

/// 用户 API Keys 请求趋势数据点
#[derive(Debug, Serialize)]
pub struct UserApiKeysRequestTrendPoint {
    pub timestamp: String,
    pub request: i64,
}

/// 用户 API Keys 请求趋势响应
#[derive(Debug, Serialize)]
pub struct UserApiKeysRequestTrendResponse {
    pub request_usage: Vec<UserApiKeysRequestTrendPoint>,
    pub current_request_usage: i64,
    pub average_request_usage: i64,
    pub max_request_usage: i64,
}

/// 用户 API Keys Token 趋势数据点
#[derive(Debug, Serialize)]
pub struct UserApiKeysTokenTrendPoint {
    pub timestamp: String,
    pub total_token: i64,
}

/// 用户 API Keys Token 趋势响应
#[derive(Debug, Serialize)]
pub struct UserApiKeysTokenTrendResponse {
    pub token_usage: Vec<UserApiKeysTokenTrendPoint>,
    pub current_token_usage: i64,
    pub average_token_usage: i64,
    pub max_token_usage: i64,
}

/// 统计服务
pub struct StatisticsService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> StatisticsService<'a> {
    #[must_use]
    pub fn new(state: &'a ManagementState) -> Self {
        Self {
            db: state.database.as_ref(),
        }
    }

    const fn db(&self) -> &'a DatabaseConnection {
        self.db
    }

    /// 今日仪表板卡片数据
    pub async fn today_dashboard_cards(
        &self,
        user_id: i32,
        timezone: &TimezoneContext,
    ) -> Result<TodayDashboardCards> {
        let now = Utc::now();
        let (today_start_utc, today_end_utc) =
            timezone_utils::local_day_bounds(&now, &timezone.timezone)
                .ok_or_else(|| conversion_error("Failed to calculate today's time range"))?;
        let (yesterday_start_utc, yesterday_end_utc) =
            timezone_utils::local_previous_day_bounds(&now, &timezone.timezone)
                .ok_or_else(|| conversion_error("Failed to calculate yesterday's time range"))?;

        let today_traces = self
            .fetch_traces(user_id, today_start_utc, today_end_utc)
            .await?;
        let yesterday_traces = self
            .fetch_traces(user_id, yesterday_start_utc, yesterday_end_utc)
            .await?;

        let requests_today_count = today_traces.len();
        let successes_today_count = today_traces.iter().filter(|t| t.is_success).count();
        let requests_today = usize_to_i64(requests_today_count);
        let success_rate_today = ratio_as_percentage(
            usize_to_u64(successes_today_count),
            usize_to_u64(requests_today_count),
        );

        let tokens_today: i64 = today_traces
            .iter()
            .map(|t| i64::from(t.tokens_total.unwrap_or(0)))
            .sum();

        let response_times: Vec<i64> = today_traces.iter().filter_map(|t| t.duration_ms).collect();
        let avg_response_time_today = match i64::try_from(response_times.len()) {
            Ok(count) if count > 0 => response_times.iter().sum::<i64>() / count,
            _ => 0,
        };

        let requests_yesterday_count = yesterday_traces.len();
        let successes_yesterday_count = yesterday_traces.iter().filter(|t| t.is_success).count();
        let requests_yesterday = usize_to_i64(requests_yesterday_count);
        let success_rate_yesterday = ratio_as_percentage(
            usize_to_u64(successes_yesterday_count),
            usize_to_u64(requests_yesterday_count),
        );

        let tokens_yesterday: i64 = yesterday_traces
            .iter()
            .map(|t| i64::from(t.tokens_total.unwrap_or(0)))
            .sum();

        let response_times_yesterday: Vec<i64> = yesterday_traces
            .iter()
            .filter_map(|t| t.duration_ms)
            .collect();
        let avg_response_time_yesterday = match i64::try_from(response_times_yesterday.len()) {
            Ok(count) if count > 0 => response_times_yesterday.iter().sum::<i64>() / count,
            _ => 0,
        };

        Ok(TodayDashboardCards {
            requests_today,
            rate_requests_today: calculate_growth_rate(requests_today, requests_yesterday),
            successes_today: success_rate_today,
            rate_successes_today: calculate_growth_rate_f64(
                success_rate_today,
                success_rate_yesterday,
            ),
            tokens_today,
            rate_tokens_today: calculate_growth_rate(tokens_today, tokens_yesterday),
            avg_response_time_today,
            rate_avg_response_time_today: calculate_growth_rate(
                avg_response_time_today,
                avg_response_time_yesterday,
            ),
        })
    }

    /// 模型使用占比
    pub async fn models_usage_rate(
        &self,
        user_id: i32,
        query: &TimeRangeQuery,
        timezone: &TimezoneContext,
    ) -> Result<ModelsRateResponse> {
        let (start_time, end_time) = parse_time_range(query, timezone)?;

        let traces = self
            .fetch_success_traces(user_id, start_time, end_time)
            .await?
            .into_iter()
            .filter_map(|trace| {
                trace
                    .model_used
                    .as_ref()
                    .map(|name| name.trim().to_string())
                    .filter(|name| !name.is_empty())
                    .map(|name| (name, trace))
            })
            .collect::<Vec<_>>();

        let aggregated = aggregate_model_usage(traces);
        let model_usage = build_model_usage_list(aggregated);
        Ok(ModelsRateResponse { model_usage })
    }

    /// 模型详细统计
    pub async fn models_statistics(
        &self,
        user_id: i32,
        query: &TimeRangeQuery,
        timezone: &TimezoneContext,
    ) -> Result<ModelsStatisticsResponse> {
        let (start_time, end_time) = parse_time_range(query, timezone)?;

        let traces = ProxyTracing::find()
            .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
            .filter(proxy_tracing::Column::CreatedAt.lt(end_time.naive_utc()))
            .filter(proxy_tracing::Column::UserId.eq(user_id))
            .filter(proxy_tracing::Column::IsSuccess.eq(true))
            .all(self.db())
            .await
            .map_err(|err| db_error("Failed to fetch traces for models statistics", &err))?;

        let total_requests = traces
            .iter()
            .filter(|t| {
                t.model_used
                    .as_ref()
                    .is_some_and(|model_name| !model_name.trim().is_empty())
            })
            .count();
        let total_requests_u64 = usize_to_u64(total_requests);

        let mut model_stats: HashMap<String, (u64, f64)> = HashMap::new();
        for trace in traces {
            if let Some(model_name) = &trace.model_used
                && !model_name.trim().is_empty()
            {
                let cost = trace.cost.unwrap_or(0.0);
                let entry = model_stats.entry(model_name.clone()).or_insert((0, 0.0));
                entry.0 = entry.0.saturating_add(1);
                entry.1 += cost;
            }
        }

        let mut model_usage: Vec<ModelStatistics> = model_stats
            .into_iter()
            .map(|(model, (usage, cost))| {
                let percentage = ratio_as_percentage(usage, total_requests_u64);
                ModelStatistics {
                    model,
                    usage: i64::try_from(usage).unwrap_or(i64::MAX),
                    percentage,
                    cost,
                }
            })
            .collect();

        model_usage.sort_by(|a, b| b.usage.cmp(&a.usage));

        Ok(ModelsStatisticsResponse { model_usage })
    }

    /// Token 使用趋势
    pub async fn tokens_trend(
        &self,
        user_id: i32,
        timezone: &TimezoneContext,
    ) -> Result<TokensTrendResponse> {
        let now = Utc::now();
        let (today_start_utc, _) = timezone_utils::local_day_bounds(&now, &timezone.timezone)
            .unwrap_or((now - Duration::days(1), now));
        let window_start = today_start_utc - Duration::days(29);

        let traces = ProxyTracing::find()
            .filter(proxy_tracing::Column::CreatedAt.gte(window_start.naive_utc()))
            .filter(
                proxy_tracing::Column::CreatedAt
                    .lt((today_start_utc + Duration::days(1)).naive_utc()),
            )
            .filter(proxy_tracing::Column::UserId.eq(user_id))
            .all(self.db())
            .await
            .map_err(|err| db_error("Failed to fetch traces for tokens trend", &err))?;

        let mut daily_stats: HashMap<String, (i64, i64, i64, i64, f64)> = HashMap::new();
        for trace in &traces {
            let date = timezone_utils::local_date_label(&trace.created_at, &timezone.timezone);
            let entry = daily_stats.entry(date).or_insert((0, 0, 0, 0, 0.0));
            entry.0 += i64::from(trace.cache_create_tokens.unwrap_or(0));
            entry.1 += i64::from(trace.cache_read_tokens.unwrap_or(0));
            entry.2 += i64::from(trace.tokens_prompt.unwrap_or(0));
            entry.3 += i64::from(trace.tokens_completion.unwrap_or(0));
            entry.4 += trace.cost.unwrap_or(0.0);
        }

        let mut token_usage = Vec::new();
        let mut daily_totals = Vec::new();

        let local_today = now.with_timezone(&timezone.timezone).date_naive();
        for offset in 0..30 {
            let offset_from_start = i64::from(offset);
            let offset_to_today = i64::from(29 - offset);
            let local_date = local_today - Duration::days(offset_to_today);
            let label = local_date.format("%Y-%m-%d").to_string();
            let default_start = window_start + Duration::days(offset_from_start);
            let (day_start_utc, _) =
                timezone_utils::local_date_window(local_date, 1, &timezone.timezone)
                    .unwrap_or((default_start, default_start + Duration::days(1)));
            let timestamp = day_start_utc.with_timezone(&timezone.timezone).to_rfc3339();

            let (cache_create, cache_read, prompt, completion, cost) = daily_stats
                .get(&label)
                .copied()
                .unwrap_or((0, 0, 0, 0, 0.0));
            let total_tokens = prompt + completion;
            daily_totals.push(total_tokens);

            token_usage.push(TokenTrendPoint {
                timestamp,
                cache_create_tokens: cache_create,
                cache_read_tokens: cache_read,
                tokens_prompt: prompt,
                tokens_completion: completion,
                cost,
            });
        }

        let current_token_usage: i64 = traces
            .iter()
            .filter(|t| timezone_utils::is_same_local_day(&t.created_at, &now, &timezone.timezone))
            .map(|t| i64::from(t.tokens_total.unwrap_or(0)))
            .sum();

        let average_token_usage = match i64::try_from(daily_totals.len()) {
            Ok(count) if count > 0 => daily_totals.iter().sum::<i64>() / count,
            _ => 0,
        };

        let max_token_usage = daily_totals.iter().max().copied().unwrap_or(0);

        Ok(TokensTrendResponse {
            token_usage,
            current_token_usage,
            average_token_usage,
            max_token_usage,
        })
    }

    /// 用户 API Keys 请求趋势
    pub async fn user_api_keys_request_trend(
        &self,
        user_id: i32,
        timezone: &TimezoneContext,
    ) -> Result<UserApiKeysRequestTrendResponse> {
        let now = Utc::now();
        let (today_start_utc, _) = timezone_utils::local_day_bounds(&now, &timezone.timezone)
            .unwrap_or((now - Duration::days(1), now));
        let window_start = today_start_utc - Duration::days(29);

        let traces = ProxyTracing::find()
            .filter(proxy_tracing::Column::CreatedAt.gte(window_start.naive_utc()))
            .filter(
                proxy_tracing::Column::CreatedAt
                    .lt((today_start_utc + Duration::days(1)).naive_utc()),
            )
            .filter(proxy_tracing::Column::UserId.eq(user_id))
            .all(self.db())
            .await
            .map_err(|err| {
                db_error(
                    "Failed to fetch traces for user API keys request trend",
                    &err,
                )
            })?;

        let mut daily_requests: HashMap<String, i64> = HashMap::new();
        for trace in &traces {
            let date = timezone_utils::local_date_label(&trace.created_at, &timezone.timezone);
            *daily_requests.entry(date).or_insert(0) += 1;
        }

        let mut request_usage = Vec::new();
        let mut daily_totals = Vec::new();

        let local_today = now.with_timezone(&timezone.timezone).date_naive();
        for offset in 0..30 {
            let offset_from_start = i64::from(offset);
            let offset_to_today = i64::from(29 - offset);
            let local_date = local_today - Duration::days(offset_to_today);
            let label = local_date.format("%Y-%m-%d").to_string();
            let default_start = window_start + Duration::days(offset_from_start);
            let (day_start_utc, _) =
                timezone_utils::local_date_window(local_date, 1, &timezone.timezone)
                    .unwrap_or((default_start, default_start + Duration::days(1)));
            let timestamp = day_start_utc.with_timezone(&timezone.timezone).to_rfc3339();

            let request_count = daily_requests.get(&label).copied().unwrap_or(0);
            daily_totals.push(request_count);

            request_usage.push(UserApiKeysRequestTrendPoint {
                timestamp,
                request: request_count,
            });
        }

        let current_request_usage = usize_to_i64(
            traces
                .iter()
                .filter(|t| {
                    timezone_utils::is_same_local_day(&t.created_at, &now, &timezone.timezone)
                })
                .count(),
        );

        let average_request_usage = match i64::try_from(daily_totals.len()) {
            Ok(count) if count > 0 => daily_totals.iter().sum::<i64>() / count,
            _ => 0,
        };

        let max_request_usage = daily_totals.iter().max().copied().unwrap_or(0);

        Ok(UserApiKeysRequestTrendResponse {
            request_usage,
            current_request_usage,
            average_request_usage,
            max_request_usage,
        })
    }

    /// 用户 API Keys Token 趋势
    pub async fn user_api_keys_token_trend(
        &self,
        user_id: i32,
        timezone: &TimezoneContext,
    ) -> Result<UserApiKeysTokenTrendResponse> {
        let now = Utc::now();
        let (today_start_utc, _) = timezone_utils::local_day_bounds(&now, &timezone.timezone)
            .unwrap_or((now - Duration::days(1), now));
        let window_start = today_start_utc - Duration::days(29);

        let traces = ProxyTracing::find()
            .filter(proxy_tracing::Column::CreatedAt.gte(window_start.naive_utc()))
            .filter(
                proxy_tracing::Column::CreatedAt
                    .lt((today_start_utc + Duration::days(1)).naive_utc()),
            )
            .filter(proxy_tracing::Column::UserId.eq(user_id))
            .all(self.db())
            .await
            .map_err(|err| {
                db_error("Failed to fetch traces for user API keys token trend", &err)
            })?;

        let mut daily_tokens: HashMap<String, i64> = HashMap::new();
        for trace in &traces {
            let date = timezone_utils::local_date_label(&trace.created_at, &timezone.timezone);
            let tokens = i64::from(trace.tokens_total.unwrap_or(0));
            *daily_tokens.entry(date).or_insert(0) += tokens;
        }

        let mut token_usage = Vec::new();
        let mut daily_totals = Vec::new();

        let local_today = now.with_timezone(&timezone.timezone).date_naive();
        for offset in 0..30 {
            let offset_from_start = i64::from(offset);
            let offset_to_today = i64::from(29 - offset);
            let local_date = local_today - Duration::days(offset_to_today);
            let label = local_date.format("%Y-%m-%d").to_string();
            let default_start = window_start + Duration::days(offset_from_start);
            let (day_start_utc, _) =
                timezone_utils::local_date_window(local_date, 1, &timezone.timezone)
                    .unwrap_or((default_start, default_start + Duration::days(1)));
            let timestamp = day_start_utc.with_timezone(&timezone.timezone).to_rfc3339();

            let total_token = daily_tokens.get(&label).copied().unwrap_or(0);
            daily_totals.push(total_token);

            token_usage.push(UserApiKeysTokenTrendPoint {
                timestamp,
                total_token,
            });
        }

        let current_token_usage: i64 = traces
            .iter()
            .filter(|t| timezone_utils::is_same_local_day(&t.created_at, &now, &timezone.timezone))
            .map(|t| i64::from(t.tokens_total.unwrap_or(0)))
            .sum();

        let average_token_usage = match i64::try_from(daily_totals.len()) {
            Ok(count) if count > 0 => daily_totals.iter().sum::<i64>() / count,
            _ => 0,
        };

        let max_token_usage = daily_totals.iter().max().copied().unwrap_or(0);

        Ok(UserApiKeysTokenTrendResponse {
            token_usage,
            current_token_usage,
            average_token_usage,
            max_token_usage,
        })
    }

    async fn fetch_success_traces(
        &self,
        user_id: i32,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<proxy_tracing::Model>> {
        ProxyTracing::find()
            .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
            .filter(proxy_tracing::Column::CreatedAt.lt(end_time.naive_utc()))
            .filter(proxy_tracing::Column::UserId.eq(user_id))
            .filter(proxy_tracing::Column::IsSuccess.eq(true))
            .all(self.db())
            .await
            .map_err(|err| db_error("Failed to fetch traces for models rate", &err))
    }

    async fn fetch_traces(
        &self,
        user_id: i32,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<proxy_tracing::Model>> {
        ProxyTracing::find()
            .filter(proxy_tracing::Column::CreatedAt.gte(start.naive_utc()))
            .filter(proxy_tracing::Column::CreatedAt.lt(end.naive_utc()))
            .filter(proxy_tracing::Column::UserId.eq(user_id))
            .all(self.db())
            .await
            .map_err(|err| db_error("Failed to fetch traces for range", &err))
    }
}

fn usize_to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[derive(Default)]
struct ModelUsageAggregate {
    usage: i64,
    cost: f64,
    successful: i64,
    failed: i64,
}

fn aggregate_model_usage(
    traces: Vec<(String, proxy_tracing::Model)>,
) -> HashMap<String, ModelUsageAggregate> {
    let mut stats: HashMap<String, ModelUsageAggregate> = HashMap::new();
    for (name, trace) in traces {
        let entry = stats.entry(name).or_default();
        entry.usage += 1;
        entry.cost += trace.cost.unwrap_or(0.0);
        if trace.is_success {
            entry.successful += 1;
        } else {
            entry.failed += 1;
        }
    }
    stats
}

fn build_model_usage_list(mut stats: HashMap<String, ModelUsageAggregate>) -> Vec<ModelUsage> {
    let mut sorted: Vec<(String, ModelUsageAggregate)> = stats.drain().collect();
    sorted.sort_by(|a, b| b.1.usage.cmp(&a.1.usage));

    if sorted.len() <= 6 {
        return sorted
            .into_iter()
            .map(|(model, agg)| to_model_usage(model, &agg))
            .collect();
    }

    let mut result: Vec<ModelUsage> = sorted
        .iter()
        .take(5)
        .map(|(model, agg)| to_model_usage(model.clone(), agg))
        .collect();

    let others = sorted
        .iter()
        .skip(5)
        .fold(ModelUsageAggregate::default(), |mut acc, (_, agg)| {
            acc.usage += agg.usage;
            acc.cost += agg.cost;
            acc.successful += agg.successful;
            acc.failed += agg.failed;
            acc
        });

    if others.usage > 0 {
        result.push(to_model_usage("其他".to_string(), &others));
    }

    result
}

fn to_model_usage(model: String, agg: &ModelUsageAggregate) -> ModelUsage {
    let success_rate = if agg.usage > 0 {
        ((ratio_component(agg.successful) / ratio_component(agg.usage)) * 100.0).round() / 100.0
    } else {
        0.0
    };

    ModelUsage {
        model,
        usage: agg.usage,
        cost: agg.cost,
        successful_requests: agg.successful,
        failed_requests: agg.failed,
        success_rate,
    }
}

fn ratio_component(value: i64) -> f64 {
    let clamped = value.clamp(0, i64::from(i32::MAX));
    let limited = i32::try_from(clamped).unwrap_or(i32::MAX);
    f64::from(limited)
}

fn calculate_growth_rate(current: i64, previous: i64) -> String {
    if previous == 0 {
        if current > 0 {
            "+100%".to_string()
        } else {
            "0%".to_string()
        }
    } else {
        let current_f64 = f64::from(u32::try_from(current).unwrap_or(0));
        let previous_f64 = f64::from(u32::try_from(previous).unwrap_or(1));
        let rate = ((current_f64 - previous_f64) / previous_f64) * 100.0;
        if rate > 0.0 {
            format!("+{rate:.1}%")
        } else {
            format!("{rate:.1}%")
        }
    }
}

fn calculate_growth_rate_f64(current: f64, previous: f64) -> String {
    if previous == 0.0 {
        if current > 0.0 {
            "+100%".to_string()
        } else {
            "0%".to_string()
        }
    } else {
        let rate = ((current - previous) / previous) * 100.0;
        if rate > 0.0 {
            format!("+{rate:.1}%")
        } else {
            format!("{rate:.1}%")
        }
    }
}

/// 解析自定义时间范围的起始参数，支持完整日期时间与仅日期格式
fn parse_custom_range_start(value: &str) -> Option<NaiveDateTime> {
    parse_naive_datetime_with_time(value).or_else(|| parse_whole_day_start(value))
}

/// 解析自定义时间范围的结束参数，日期格式会自动扩展到次日零点以确保包含整日
fn parse_custom_range_end(value: &str) -> Option<NaiveDateTime> {
    parse_naive_datetime_with_time(value).or_else(|| {
        parse_whole_day_start(value).and_then(|dt| dt.checked_add_signed(Duration::days(1)))
    })
}

/// 支持常见格式的日期时间解析
fn parse_naive_datetime_with_time(value: &str) -> Option<NaiveDateTime> {
    const FORMATS: &[&str] = &[
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ];

    let trimmed = value.trim();
    for format in FORMATS {
        if let Ok(dt) = NaiveDateTime::parse_from_str(trimmed, format) {
            return Some(dt);
        }
    }
    None
}

fn parse_whole_day_start(value: &str) -> Option<NaiveDateTime> {
    let trimmed = value.trim();
    let date = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").ok()?;
    date.and_hms_opt(0, 0, 0)
}

fn parse_time_range(
    query: &TimeRangeQuery,
    timezone: &TimezoneContext,
) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let now = Utc::now();
    match query.range.as_deref() {
        Some("today") => timezone_utils::local_day_bounds(&now, &timezone.timezone)
            .ok_or_else(|| conversion_error("Failed to calculate local day bounds")),
        Some("30days") => Ok((now - Duration::days(30), now)),
        Some("custom") => {
            if let (Some(start_raw), Some(end_raw)) = (&query.start, &query.end) {
                let start_naive = parse_custom_range_start(start_raw)
                    .ok_or_else(|| conversion_error("Invalid start datetime format"))?;
                let end_naive = parse_custom_range_end(end_raw)
                    .ok_or_else(|| conversion_error("Invalid end datetime format"))?;

                if start_naive >= end_naive {
                    Err(conversion_error(
                        "Start datetime must be before end datetime",
                    ))
                } else {
                    timezone_utils::convert_range_to_utc(
                        &start_naive,
                        &end_naive,
                        &timezone.timezone,
                    )
                    .ok_or_else(|| conversion_error("Invalid datetime values"))
                }
            } else {
                Err(conversion_error(
                    "Custom range requires both start and end datetime",
                ))
            }
        }
        _ => Ok((now - Duration::days(7), now)),
    }
}

fn db_error(message: &str, err: &DbErr) -> ProxyError {
    lerror!(
        "system",
        LogStage::Db,
        LogComponent::Database,
        "statistics_service_db_error",
        &format!("{message}: {err}")
    );
    crate::error!(Database, format!("{message}: {err}"))
}

fn conversion_error(message: &str) -> ProxyError {
    crate::error!(Conversion, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use chrono_tz::Asia::Shanghai;

    #[test]
    fn parse_time_range_supports_date_only_inputs() {
        let timezone = TimezoneContext { timezone: Shanghai };
        let query = TimeRangeQuery {
            range: Some("custom".to_string()),
            start: Some("2025-11-18".to_string()),
            end: Some("2025-11-25".to_string()),
        };

        let (start, end) =
            parse_time_range(&query, &timezone).expect("custom date range should parse");

        let expected_start_local = NaiveDate::from_ymd_opt(2025, 11, 18)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let expected_end_local = NaiveDate::from_ymd_opt(2025, 11, 26)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let (expected_start, expected_end) = timezone_utils::convert_range_to_utc(
            &expected_start_local,
            &expected_end_local,
            &timezone.timezone,
        )
        .unwrap();

        assert_eq!(start, expected_start);
        assert_eq!(end, expected_end);
    }

    #[test]
    fn parse_time_range_supports_full_datetime_inputs() {
        let timezone = TimezoneContext { timezone: Shanghai };
        let query = TimeRangeQuery {
            range: Some("custom".to_string()),
            start: Some("2025-11-18T08:30:00".to_string()),
            end: Some("2025-11-25T20:45:00".to_string()),
        };

        let (start, end) =
            parse_time_range(&query, &timezone).expect("custom datetime range should parse");

        let expected_start_local = NaiveDate::from_ymd_opt(2025, 11, 18)
            .unwrap()
            .and_hms_opt(8, 30, 0)
            .unwrap();
        let expected_end_local = NaiveDate::from_ymd_opt(2025, 11, 25)
            .unwrap()
            .and_hms_opt(20, 45, 0)
            .unwrap();
        let (expected_start, expected_end) = timezone_utils::convert_range_to_utc(
            &expected_start_local,
            &expected_end_local,
            &timezone.timezone,
        )
        .unwrap();

        assert_eq!(start, expected_start);
        assert_eq!(end, expected_end);
    }
}
