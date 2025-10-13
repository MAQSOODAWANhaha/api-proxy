//! # 统一统计信息处理器
//!
//! `基于proxy_tracing表的统一统计查询API`
#![allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::result_large_err,
    clippy::significant_drop_tightening,
    clippy::needless_collect
)]
use crate::lerror;
use crate::logging::{LogComponent, LogStage};
use crate::management::middleware::auth::AuthContext;
use crate::management::response;
use crate::management::server::AppState;
use axum::extract::{Extension, Query, State};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use entity::{proxy_tracing, proxy_tracing::Entity as ProxyTracing};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, FromQueryResult,
    query::{QueryFilter, QuerySelect},
    sea_query::{Alias, Expr},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// 导入JWT Claims结构体

/// 统计查询参数（向后兼容）
#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    /// 时间范围（小时）
    pub hours: Option<u32>,
    /// 分组方式
    pub group_by: Option<String>,
    /// 上游类型过滤（兼容旧API）
    pub upstream_type: Option<String>,
    /// 提供商类型过滤
    pub provider_type: Option<String>,
    /// 追踪级别过滤 (0=基础, 1=详细, 2=完整)
    pub trace_level: Option<i32>,
    /// 仅显示成功请求
    pub success_only: Option<bool>,
    /// 仅显示异常请求
    pub anomaly_only: Option<bool>,
}

/// 时间范围查询参数
#[derive(Debug, Deserialize)]
pub struct TimeRangeQuery {
    /// 时间范围: today, 7days, 30days, custom
    pub range: Option<String>,
    /// 自定义开始时间 (YYYY-MM-DD)
    pub start: Option<String>,
    /// 自定义结束时间 (YYYY-MM-DD)
    pub end: Option<String>,
}

/// 今日仪表板卡片数据（包含增长率）
#[derive(Serialize)]
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
#[derive(Serialize)]
pub struct ModelUsage {
    pub model: String,
    pub usage: i64,
    pub cost: f64,
    pub successful_requests: i64,
    pub failed_requests: i64,
    pub success_rate: f64,
}

/// 模型使用占比响应
#[derive(Serialize)]
pub struct ModelsRateResponse {
    pub model_usage: Vec<ModelUsage>,
}

/// 模型详细统计数据
#[derive(Serialize)]
pub struct ModelStatistics {
    pub model: String,
    pub usage: i64,
    pub percentage: f64,
    pub cost: f64,
}

/// 模型详细统计响应
#[derive(Serialize)]
pub struct ModelsStatisticsResponse {
    pub model_usage: Vec<ModelStatistics>,
}

/// Token使用趋势数据点
#[derive(Serialize)]
pub struct TokenTrendPoint {
    pub timestamp: String,
    pub cache_create_tokens: i64,
    pub cache_read_tokens: i64,
    pub tokens_prompt: i64,
    pub tokens_completion: i64,
    pub cost: f64,
}

/// Token使用趋势响应
#[derive(Serialize)]
pub struct TokensTrendResponse {
    pub token_usage: Vec<TokenTrendPoint>,
    pub current_token_usage: i64,
    pub average_token_usage: i64,
    pub max_token_usage: i64,
}

/// 用户API Keys请求趋势数据点
#[derive(Serialize)]
pub struct UserApiKeysRequestTrendPoint {
    pub timestamp: String,
    pub request: i64,
}

/// 用户API Keys请求趋势响应
#[derive(Serialize)]
pub struct UserApiKeysRequestTrendResponse {
    pub request_usage: Vec<UserApiKeysRequestTrendPoint>,
    pub current_request_usage: i64,
    pub average_request_usage: i64,
    pub max_request_usage: i64,
}

/// 用户API Keys Token趋势数据点
#[derive(Serialize)]
pub struct UserApiKeysTokenTrendPoint {
    pub timestamp: String,
    pub total_token: i64,
}

/// 用户API Keys Token趋势响应
#[derive(Serialize)]
pub struct UserApiKeysTokenTrendResponse {
    pub token_usage: Vec<UserApiKeysTokenTrendPoint>,
    pub current_token_usage: i64,
    pub average_token_usage: i64,
    pub max_token_usage: i64,
}

#[derive(Debug, Default, FromQueryResult)]
struct DailyStats {
    requests: i64,
    successes: i64,
    tokens: Option<i64>,
    avg_response_time: Option<f64>,
}

#[derive(Debug, FromQueryResult)]
struct ModelUsageResult {
    model: String,
    usage: i64,
    cost: f64,
    successful_requests: i64,
}

#[derive(Debug, FromQueryResult)]
struct ModelStatsResult {
    model: String,
    usage: i64,
    cost: f64,
}

#[derive(Debug, FromQueryResult)]
struct DailyTokenStats {
    date: NaiveDate,
    cache_create_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    tokens_prompt: Option<i64>,
    tokens_completion: Option<i64>,
    cost: Option<f64>,
}

#[derive(Debug, FromQueryResult)]
struct DailyRequestStats {
    date: NaiveDate,
    requests: i64,
}

#[derive(Debug, FromQueryResult)]
struct DailyTokenTotalStats {
    date: NaiveDate,
    total_tokens: Option<i64>,
}

async fn get_daily_stats(
    db: &DatabaseConnection,
    user_id: i32,
    date: NaiveDate,
) -> Result<DailyStats, sea_orm::DbErr> {
    let start_of_day = date.and_hms_opt(0, 0, 0).unwrap();
    let end_of_day = date.and_hms_opt(23, 59, 59).unwrap();

    ProxyTracing::find()
        .select_only()
        .column_as(proxy_tracing::Column::Id.count(), "requests")
        .column_as(
            Expr::cust("SUM(CASE WHEN IsSuccess = true THEN 1 ELSE 0 END)"),
            "successes",
        )
        .column_as(proxy_tracing::Column::TokensTotal.sum(), "tokens")
        .column_as(Expr::cust("AVG(DurationMs)"), "avg_response_time")
        .filter(
            Condition::all()
                .add(proxy_tracing::Column::UserId.eq(user_id))
                .add(proxy_tracing::Column::CreatedAt.between(start_of_day, end_of_day)),
        )
        .into_model::<DailyStats>()
        .one(db)
        .await
        .map(Option::unwrap_or_default)
}

/// 1. 今日仪表板卡片API: /api/statistics/today/cards
#[allow(clippy::cast_precision_loss)]
pub async fn get_today_dashboard_cards(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let user_id = auth_context.user_id;
    let db = state.database.as_ref();
    let today = Utc::now().date_naive();
    let yesterday = today - Duration::days(1);

    let today_stats = match get_daily_stats(db, user_id, today).await {
        Ok(daily_stats) => daily_stats,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_today_stats_fail",
                &format!("Failed to fetch today's stats: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch today's data: {}",
                err
            ));
        }
    };

    let yesterday_stats = match get_daily_stats(db, user_id, yesterday).await {
        Ok(daily_stats) => daily_stats,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_yesterday_stats_fail",
                &format!("Failed to fetch yesterday's stats: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch yesterday's data: {}",
                err
            ));
        }
    };

    let success_rate_today = if today_stats.requests > 0 {
        (today_stats.successes as f64 / today_stats.requests as f64) * 100.0
    } else {
        0.0
    };

    let success_rate_yesterday = if yesterday_stats.requests > 0 {
        (yesterday_stats.successes as f64 / yesterday_stats.requests as f64) * 100.0
    } else {
        0.0
    };

    let rate_requests = calculate_growth_rate(today_stats.requests, yesterday_stats.requests);
    let rate_successes = calculate_growth_rate_f64(success_rate_today, success_rate_yesterday);
    let rate_tokens = calculate_growth_rate(
        today_stats.tokens.unwrap_or(0),
        yesterday_stats.tokens.unwrap_or(0),
    );
    let rate_response_time = calculate_growth_rate(
        today_stats.avg_response_time.unwrap_or(0.0) as i64,
        yesterday_stats.avg_response_time.unwrap_or(0.0) as i64,
    );

    let cards = TodayDashboardCards {
        requests_today: today_stats.requests,
        rate_requests_today: rate_requests,
        successes_today: success_rate_today,
        rate_successes_today: rate_successes,
        tokens_today: today_stats.tokens.unwrap_or(0),
        rate_tokens_today: rate_tokens,
        avg_response_time_today: today_stats.avg_response_time.unwrap_or(0.0) as i64,
        rate_avg_response_time_today: rate_response_time,
    };

    response::success(cards)
}

/// 2. 模型使用占比API: /api/statistics/models/rate
#[allow(clippy::cast_precision_loss)]
pub async fn get_models_usage_rate(
    State(state): State<AppState>,
    Query(query): Query<TimeRangeQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let user_id = auth_context.user_id;

    let (start_time, _end_time) = match parse_time_range(&query) {
        Ok(times) => times,
        Err(error_response) => return error_response,
    };

    let model_stats_result = match ProxyTracing::find()
        .select_only()
        .column_as(proxy_tracing::Column::ModelUsed, "model")
        .column_as(proxy_tracing::Column::Id.count(), "usage")
        .column_as(proxy_tracing::Column::Cost.sum(), "cost")
        .column_as(
            Expr::cust("SUM(CASE WHEN IsSuccess = true THEN 1 ELSE 0 END)"),
            "successful_requests",
        )
        .filter(
            Condition::all()
                .add(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
                .add(proxy_tracing::Column::UserId.eq(user_id))
                .add(proxy_tracing::Column::ModelUsed.is_not_null())
                .add(proxy_tracing::Column::ModelUsed.ne("")),
        )
        .group_by(proxy_tracing::Column::ModelUsed)
        .into_model::<ModelUsageResult>()
        .all(state.database.as_ref())
        .await
    {
        Ok(results) => results,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_models_rate_fail",
                &format!("Failed to fetch traces for models rate: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch data: {}",
                err
            ));
        }
    };

    let mut model_vec: Vec<ModelUsage> = model_stats_result
        .into_iter()
        .map(|res| {
            let success_rate = if res.usage > 0 {
                (res.successful_requests as f64 / res.usage as f64) * 100.0
            } else {
                0.0
            };
            ModelUsage {
                model: res.model,
                usage: res.usage,
                cost: res.cost,
                successful_requests: res.successful_requests,
                failed_requests: res.usage - res.successful_requests,
                success_rate,
            }
        })
        .collect();

    model_vec.sort_by(|a, b| b.usage.cmp(&a.usage));

    let response = ModelsRateResponse {
        model_usage: if model_vec.len() > 6 {
            let other_usage: i64 = model_vec.iter().skip(5).map(|m| m.usage).sum();
            let other_cost: f64 = model_vec.iter().skip(5).map(|m| m.cost).sum();
            let other_successful: i64 = model_vec
                .iter()
                .skip(5)
                .map(|m| m.successful_requests)
                .sum();
            let other_failed: i64 = model_vec.iter().skip(5).map(|m| m.failed_requests).sum();
            let other_success_rate = if other_usage > 0 {
                (other_successful as f64 / other_usage as f64) * 100.0
            } else {
                0.0
            };

            let mut model_usage: Vec<ModelUsage> = model_vec.into_iter().take(5).collect();
            model_usage.push(ModelUsage {
                model: "其他".to_string(),
                usage: other_usage,
                cost: other_cost,
                successful_requests: other_successful,
                failed_requests: other_failed,
                success_rate: other_success_rate,
            });
            model_usage
        } else {
            model_vec
        },
    };
    response::success(response)
}

/// 3. 模型详细统计API: /api/statistics/models/statistics
#[allow(clippy::cast_precision_loss)]
pub async fn get_models_statistics(
    State(state): State<AppState>,
    Query(query): Query<TimeRangeQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let user_id = auth_context.user_id;

    let (start_time, _end_time) = match parse_time_range(&query) {
        Ok(times) => times,
        Err(error_response) => return error_response,
    };

    let model_stats_results = match ProxyTracing::find()
        .select_only()
        .column_as(proxy_tracing::Column::ModelUsed, "model")
        .column_as(proxy_tracing::Column::Id.count(), "usage")
        .column_as(proxy_tracing::Column::Cost.sum(), "cost")
        .filter(
            Condition::all()
                .add(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
                .add(proxy_tracing::Column::UserId.eq(user_id))
                .add(proxy_tracing::Column::IsSuccess.eq(true))
                .add(proxy_tracing::Column::ModelUsed.is_not_null())
                .add(proxy_tracing::Column::ModelUsed.ne("")),
        )
        .group_by(proxy_tracing::Column::ModelUsed)
        .into_model::<ModelStatsResult>()
        .all(state.database.as_ref())
        .await
    {
        Ok(results) => results,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_models_stats_fail",
                &format!("Failed to fetch traces for models statistics: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch data: {}",
                err
            ));
        }
    };

    let total_requests: i64 = model_stats_results.iter().map(|res| res.usage).sum();

    let mut model_usage: Vec<ModelStatistics> = model_stats_results
        .into_iter()
        .map(|res| {
            let percentage = if total_requests > 0 {
                (res.usage as f64 / total_requests as f64) * 100.0
            } else {
                0.0
            };
            ModelStatistics {
                model: res.model,
                usage: res.usage,
                percentage,
                cost: res.cost,
            }
        })
        .collect();

    // 按使用次数排序
    model_usage.sort_by(|a, b| b.usage.cmp(&a.usage));

    let response = ModelsStatisticsResponse { model_usage };
    response::success(response)
}

/// 4. `Token使用趋势API`: /api/statistics/tokens/trend
pub async fn get_tokens_trend(
    State(state): State<AppState>,
    Extension(_auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let db = state.database.as_ref();

    let daily_stats_result = match ProxyTracing::find()
        .select_only()
        .column_as(
            Expr::col(proxy_tracing::Column::CreatedAt).cast_as(Alias::new("date")),
            "date",
        )
        .column_as(
            proxy_tracing::Column::CacheCreateTokens.sum(),
            "cache_create_tokens",
        )
        .column_as(
            proxy_tracing::Column::CacheReadTokens.sum(),
            "cache_read_tokens",
        )
        .column_as(proxy_tracing::Column::TokensPrompt.sum(), "tokens_prompt")
        .column_as(
            proxy_tracing::Column::TokensCompletion.sum(),
            "tokens_completion",
        )
        .group_by(Expr::col(proxy_tracing::Column::CreatedAt).cast_as(Alias::new("date")))
        .into_model::<DailyTokenStats>()
        .all(db)
        .await
    {
        Ok(results) => results,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_tokens_trend_fail",
                &format!("Failed to fetch traces for tokens trend: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch data: {}",
                err
            ));
        }
    };

    let daily_stats_map: HashMap<NaiveDate, DailyTokenStats> = daily_stats_result
        .into_iter()
        .map(|stats| (stats.date, stats))
        .collect();

    let mut token_usage = Vec::new();
    let mut daily_totals = Vec::new();
    let today = Utc::now().date_naive();
    let mut current_token_usage = 0;

    for i in 0..30 {
        let date = today - Duration::days(29 - i);
        let timestamp = date.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339();

        if let Some(stats) = daily_stats_map.get(&date) {
            let total_tokens =
                stats.tokens_prompt.unwrap_or(0) + stats.tokens_completion.unwrap_or(0);
            daily_totals.push(total_tokens);
            if date == today {
                current_token_usage = total_tokens;
            }

            token_usage.push(TokenTrendPoint {
                timestamp,
                cache_create_tokens: stats.cache_create_tokens.unwrap_or(0),
                cache_read_tokens: stats.cache_read_tokens.unwrap_or(0),
                tokens_prompt: stats.tokens_prompt.unwrap_or(0),
                tokens_completion: stats.tokens_completion.unwrap_or(0),
                cost: stats.cost.unwrap_or(0.0),
            });
        } else {
            daily_totals.push(0);
            token_usage.push(TokenTrendPoint {
                timestamp,
                cache_create_tokens: 0,
                cache_read_tokens: 0,
                tokens_prompt: 0,
                tokens_completion: 0,
                cost: 0.0,
            });
        }
    }

    let total_token_usage: i64 = daily_totals.iter().sum();
    let average_token_usage = if daily_totals.is_empty() {
        0
    } else {
        total_token_usage / daily_totals.len() as i64
    };
    let max_token_usage = daily_totals.iter().max().copied().unwrap_or(0);

    let response = TokensTrendResponse {
        token_usage,
        current_token_usage,
        average_token_usage,
        max_token_usage,
    };

    response::success(response)
}

/// 5. 用户API `Keys请求趋势API`: /api/statistics/user-service-api-keys/request
pub async fn get_user_api_keys_request_trend(
    State(state): State<AppState>,
    Extension(_auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let db = state.database.as_ref();

    let daily_stats_result = match ProxyTracing::find()
        .select_only()
        .column_as(
            Expr::col(proxy_tracing::Column::CreatedAt).cast_as(Alias::new("date")),
            "date",
        )
        .group_by(Expr::col(proxy_tracing::Column::CreatedAt).cast_as(Alias::new("date")))
        .into_model::<DailyRequestStats>()
        .all(db)
        .await
    {
        Ok(results) => results,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_user_keys_request_trend_fail",
                &format!("Failed to fetch traces for user API keys request trend: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch data: {}",
                err
            ));
        }
    };

    let daily_requests_map: HashMap<NaiveDate, i64> = daily_stats_result
        .into_iter()
        .map(|stats| (stats.date, stats.requests))
        .collect();

    let mut request_usage = Vec::new();
    let mut daily_totals = Vec::new();
    let today = Utc::now().date_naive();

    for i in 0..30 {
        let date = today - Duration::days(29 - i);
        let timestamp = date.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339();
        let request_count = daily_requests_map.get(&date).copied().unwrap_or(0);
        daily_totals.push(request_count);
        request_usage.push(UserApiKeysRequestTrendPoint {
            timestamp,
            request: request_count,
        });
    }

    let current_request_usage = daily_requests_map.get(&today).copied().unwrap_or(0);
    let total_requests: i64 = daily_totals.iter().sum();
    let average_request_usage = if daily_totals.is_empty() {
        0
    } else {
        total_requests / daily_totals.len() as i64
    };
    let max_request_usage = daily_totals.iter().max().copied().unwrap_or(0);

    let response = UserApiKeysRequestTrendResponse {
        request_usage,
        current_request_usage,
        average_request_usage,
        max_request_usage,
    };

    response::success(response)
}

/// 6. 用户API Keys `Token趋势API`: /api/statistics/user-service-api-keys/token
pub async fn get_user_api_keys_token_trend(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let user_id = auth_context.user_id;
    let db = state.database.as_ref();
    let start_time = Utc::now() - Duration::days(30);

    let daily_stats_result = match ProxyTracing::find()
        .select_only()
        .column_as(
            Expr::col(proxy_tracing::Column::CreatedAt).cast_as(Alias::new("date")),
            "date",
        )
        .column_as(proxy_tracing::Column::TokensTotal.sum(), "total_tokens")
        .filter(
            Condition::all()
                .add(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
                .add(proxy_tracing::Column::UserId.eq(user_id)),
        )
        .group_by(Expr::col(proxy_tracing::Column::CreatedAt).cast_as(Alias::new("date")))
        .into_model::<DailyTokenTotalStats>()
        .all(db)
        .await
    {
        Ok(results) => results,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_user_keys_token_trend_fail",
                &format!("Failed to fetch traces for user API keys token trend: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch data: {}",
                err
            ));
        }
    };

    let daily_tokens_map: HashMap<NaiveDate, i64> = daily_stats_result
        .into_iter()
        .map(|stats| (stats.date, stats.total_tokens.unwrap_or(0)))
        .collect();

    let mut token_usage = Vec::new();
    let mut daily_totals = Vec::new();
    let today = Utc::now().date_naive();

    for i in 0..30 {
        let date = today - Duration::days(29 - i);
        let timestamp = date.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339();
        let total_token = daily_tokens_map.get(&date).copied().unwrap_or(0);
        daily_totals.push(total_token);
        token_usage.push(UserApiKeysTokenTrendPoint {
            timestamp,
            total_token,
        });
    }

    let current_token_usage = daily_tokens_map.get(&today).copied().unwrap_or(0);
    let total_tokens: i64 = daily_totals.iter().sum();
    let average_token_usage = if daily_totals.is_empty() {
        0
    } else {
        total_tokens / daily_totals.len() as i64
    };
    let max_token_usage = daily_totals.iter().max().copied().unwrap_or(0);

    let response = UserApiKeysTokenTrendResponse {
        token_usage,
        current_token_usage,
        average_token_usage,
        max_token_usage,
    };

    response::success(response)
}

// ===== 辅助函数 =====

/// 解析时间范围参数
fn parse_time_range(
    query: &TimeRangeQuery,
) -> Result<(DateTime<Utc>, DateTime<Utc>), axum::response::Response> {
    let end_time = Utc::now();

    let start_time = match query.range.as_deref() {
        Some("today") => {
            let today = end_time.date_naive().and_hms_opt(0, 0, 0).unwrap();
            DateTime::from_naive_utc_and_offset(today, Utc)
        }
        Some("30days") => end_time - Duration::days(30),
        Some("custom") => {
            if let (Some(start_str), Some(_end_str)) = (&query.start, &query.end) {
                match chrono::NaiveDate::parse_from_str(start_str, "%Y-%m-%d") {
                    Ok(start_date) => {
                        let start_datetime = start_date.and_hms_opt(0, 0, 0).unwrap();
                        DateTime::from_naive_utc_and_offset(start_datetime, Utc)
                    }
                    Err(_) => {
                        return Err(crate::manage_error!(crate::proxy_err!(
                            business,
                            "Invalid start date format. Use YYYY-MM-DD"
                        )));
                    }
                }
            } else {
                return Err(crate::manage_error!(crate::proxy_err!(
                    business,
                    "Custom range requires both start and end dates"
                )));
            }
        }
        _ => end_time - Duration::days(7), // 默认7天
    };

    Ok((start_time, end_time))
}

/// 计算增长率（整数）
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

/// 计算增长率（浮点数）
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
