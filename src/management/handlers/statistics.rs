//! # 统一统计信息处理器
//!
//! `基于proxy_tracing表的统一统计查询API`
#![allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::result_large_err,
    clippy::significant_drop_tightening,
    clippy::needless_collect
)]
use crate::error::ProxyError;
use crate::lerror;
use crate::logging::{LogComponent, LogStage};
use crate::management::middleware::auth::AuthContext;
use crate::management::response;
use crate::management::server::AppState;
use crate::types::{ConvertToUtc, TimezoneContext, ratio_as_percentage};
use axum::extract::{Extension, Query, State};
use chrono::{DateTime, Duration, Utc};
use entity::{proxy_tracing, proxy_tracing::Entity as ProxyTracing};
use sea_orm::{
    entity::{ColumnTrait, EntityTrait},
    query::QueryFilter,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
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
    /// 自定义开始时间 (YYYY-MM-DD HH:MM:SS)
    pub start: Option<chrono::NaiveDateTime>,
    /// 自定义结束时间 (YYYY-MM-DD HH:MM:SS)
    pub end: Option<chrono::NaiveDateTime>,
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

/// 1. 今日仪表板卡片API: /api/statistics/today/cards
pub async fn get_today_dashboard_cards(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let user_id = auth_context.user_id;

    let now = Utc::now();
    let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
    let yesterday_start = (now - Duration::days(1))
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap();

    // 获取今天的数据
    let today_traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(today_start))
        .filter(proxy_tracing::Column::UserId.eq(user_id))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_today_traces_fail",
                &format!("Failed to fetch today's traces: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to fetch today's data: {}", err)
            ));
        }
    };

    // 获取昨天的数据
    let yesterday_traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(yesterday_start))
        .filter(proxy_tracing::Column::CreatedAt.lt(today_start))
        .filter(proxy_tracing::Column::UserId.eq(user_id))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_yesterday_traces_fail",
                &format!("Failed to fetch yesterday's traces: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to fetch yesterday's data: {}", err)
            ));
        }
    };

    // 计算今天的统计数据
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

    // 计算昨天的统计数据用于比较
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

    // 计算增长率
    let rate_requests = calculate_growth_rate(requests_today, requests_yesterday);
    let rate_successes = calculate_growth_rate_f64(success_rate_today, success_rate_yesterday);
    let rate_tokens = calculate_growth_rate(tokens_today, tokens_yesterday);
    let rate_response_time =
        calculate_growth_rate(avg_response_time_today, avg_response_time_yesterday);

    let cards = TodayDashboardCards {
        requests_today,
        rate_requests_today: rate_requests,
        successes_today: success_rate_today,
        rate_successes_today: rate_successes,
        tokens_today,
        rate_tokens_today: rate_tokens,
        avg_response_time_today,
        rate_avg_response_time_today: rate_response_time,
    };

    response::success(cards)
}

/// 2. 模型使用占比API: /api/statistics/models/rate
pub async fn get_models_usage_rate(
    State(state): State<AppState>,
    Query(query): Query<TimeRangeQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let user_id = auth_context.user_id;

    let (start_time, _end_time) = match parse_time_range(&query, &timezone_context) {
        Ok(times) => times,
        Err(error_response) => return error_response,
    };

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .filter(proxy_tracing::Column::UserId.eq(user_id))
        .filter(proxy_tracing::Column::IsSuccess.eq(true))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_models_rate_fail",
                &format!("Failed to fetch traces for models rate: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to fetch data: {}", err)
            ));
        }
    };

    // 按模型统计使用次数、成本和成功失败情况
    let mut model_stats: HashMap<String, (i64, f64, i64, i64)> = HashMap::new();
    for trace in traces {
        // 过滤空模型数据
        if let Some(model_name) = &trace.model_used {
            // 检查模型名称是否有效（非空、非空白字符）
            if !model_name.trim().is_empty() {
                let cost = trace.cost.unwrap_or(0.0);
                let successful = i64::from(trace.is_success);
                let failed = i64::from(!trace.is_success);
                let entry = model_stats
                    .entry(model_name.clone())
                    .or_insert((0, 0.0, 0, 0));
                entry.0 += 1; // usage count
                entry.1 += cost; // total cost
                entry.2 += successful; // successful requests
                entry.3 += failed; // failed requests
            }
        }
    }

    // 按使用次数排序
    let mut model_vec: Vec<(String, i64, f64, i64, i64)> = model_stats
        .into_iter()
        .map(|(model, (usage, cost, successful, failed))| (model, usage, cost, successful, failed))
        .collect();
    model_vec.sort_by(|a, b| b.1.cmp(&a.1));

    // 限制最多6个模型，其余合并为"其他"
    let mut model_usage = Vec::new();
    if model_vec.len() <= 6 {
        for (model, usage, cost, successful, failed) in model_vec {
            let success_rate = if usage > 0 {
                let successful_f64 = f64::from(u32::try_from(successful).unwrap_or(0));
                let usage_f64 = f64::from(u32::try_from(usage).unwrap_or(1));
                (successful_f64 / usage_f64) * 100.0
            } else {
                0.0
            };
            model_usage.push(ModelUsage {
                model,
                usage,
                cost,
                successful_requests: successful,
                failed_requests: failed,
                success_rate,
            });
        }
    } else {
        // 前5个模型
        for (model, usage, cost, successful, failed) in model_vec.iter().take(5) {
            let success_rate = if *usage > 0 {
                let successful_f64 = f64::from(u32::try_from(*successful).unwrap_or(0));
                let usage_f64 = f64::from(u32::try_from(*usage).unwrap_or(1));
                (successful_f64 / usage_f64) * 100.0
            } else {
                0.0
            };
            model_usage.push(ModelUsage {
                model: model.clone(),
                usage: *usage,
                cost: *cost,
                successful_requests: *successful,
                failed_requests: *failed,
                success_rate,
            });
        }
        // 其余模型合并为"其他"
        let other_usage: i64 = model_vec
            .iter()
            .skip(5)
            .map(|(_, usage, _, _, _)| usage)
            .sum();
        let other_cost: f64 = model_vec
            .iter()
            .skip(5)
            .map(|(_, _, cost, _, _)| cost)
            .sum();
        let other_successful: i64 = model_vec
            .iter()
            .skip(5)
            .map(|(_, _, _, successful, _)| successful)
            .sum();
        let other_failed: i64 = model_vec
            .iter()
            .skip(5)
            .map(|(_, _, _, _, failed)| failed)
            .sum();
        let other_success_rate = if other_usage > 0 {
            let other_successful_f64 = f64::from(u32::try_from(other_successful).unwrap_or(0));
            let other_usage_f64 = f64::from(u32::try_from(other_usage).unwrap_or(1));
            (other_successful_f64 / other_usage_f64) * 100.0
        } else {
            0.0
        };
        model_usage.push(ModelUsage {
            model: "其他".to_string(),
            usage: other_usage,
            cost: other_cost,
            successful_requests: other_successful,
            failed_requests: other_failed,
            success_rate: other_success_rate,
        });
    }

    let response = ModelsRateResponse { model_usage };
    response::success(response)
}

/// 3. 模型详细统计API: /api/statistics/models/statistics
pub async fn get_models_statistics(
    State(state): State<AppState>,
    Query(query): Query<TimeRangeQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let user_id = auth_context.user_id;

    let (start_time, _end_time) = match parse_time_range(&query, &timezone_context) {
        Ok(times) => times,
        Err(error_response) => return error_response,
    };

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .filter(proxy_tracing::Column::UserId.eq(user_id))
        .filter(proxy_tracing::Column::IsSuccess.eq(true))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_models_stats_fail",
                &format!("Failed to fetch traces for models statistics: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to fetch data: {}", err)
            ));
        }
    };

    // 计算有效请求总数（过滤空模型数据后）
    let total_requests = traces
        .iter()
        .filter(|t| {
            t.model_used
                .as_ref()
                .is_some_and(|model_name| !model_name.trim().is_empty())
        })
        .count();
    let total_requests_u64 = usize_to_u64(total_requests);

    // 按模型统计详细数据（使用次数和费用）
    let mut model_stats: HashMap<String, (u64, f64)> = HashMap::new();
    for trace in traces {
        // 过滤空模型数据
        if let Some(model_name) = &trace.model_used {
            // 检查模型名称是否有效（非空、非空白字符）
            if !model_name.trim().is_empty() {
                let cost = trace.cost.unwrap_or(0.0);
                let entry = model_stats.entry(model_name.clone()).or_insert((0, 0.0));
                entry.0 = entry.0.saturating_add(1); // usage count
                entry.1 += cost; // total cost
            }
        }
    }

    // 转换为响应格式
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

    // 按使用次数排序
    model_usage.sort_by(|a, b| b.usage.cmp(&a.usage));

    let response = ModelsStatisticsResponse { model_usage };
    response::success(response)
}

/// 4. `Token使用趋势API`: /api/statistics/tokens/trend
pub async fn get_tokens_trend(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let user_id = auth_context.user_id;

    // 固定获取最近30天的数据
    let start_time = Utc::now() - Duration::days(30);

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .filter(proxy_tracing::Column::UserId.eq(user_id))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_tokens_trend_fail",
                &format!("Failed to fetch traces for tokens trend: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to fetch data: {}", err)
            ));
        }
    };

    // 按天分组统计Token使用情况
    let mut daily_stats: HashMap<String, (i64, i64, i64, i64, f64)> = HashMap::new();
    for trace in &traces {
        let date = DateTime::<Utc>::from_naive_utc_and_offset(trace.created_at, Utc)
            .format("%Y-%m-%d")
            .to_string();

        let entry = daily_stats.entry(date).or_insert((0, 0, 0, 0, 0.0));
        entry.0 += i64::from(trace.cache_create_tokens.unwrap_or(0));
        entry.1 += i64::from(trace.cache_read_tokens.unwrap_or(0));
        entry.2 += i64::from(trace.tokens_prompt.unwrap_or(0));
        entry.3 += i64::from(trace.tokens_completion.unwrap_or(0));
        entry.4 += trace.cost.unwrap_or(0.0);
    }

    // 生成30天的时间序列数据
    let mut token_usage = Vec::new();
    let mut daily_totals = Vec::new();

    for i in 0..30 {
        let date = (Utc::now() - Duration::days(29 - i))
            .format("%Y-%m-%d")
            .to_string();
        let utc_time = Utc::now() - Duration::days(29 - i);
        let timestamp = crate::types::timezone_utils::format_utc_for_response(
            &utc_time,
            &timezone_context.timezone,
        );

        if let Some((cache_create, cache_read, prompt, completion, cost)) = daily_stats.get(&date) {
            let total_tokens = prompt + completion;
            daily_totals.push(total_tokens);

            token_usage.push(TokenTrendPoint {
                timestamp,
                cache_create_tokens: *cache_create,
                cache_read_tokens: *cache_read,
                tokens_prompt: *prompt,
                tokens_completion: *completion,
                cost: *cost,
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

    // 计算今天、平均值和最大值
    let today_traces: Vec<&proxy_tracing::Model> = traces
        .iter()
        .filter(|t| {
            let trace_date =
                DateTime::<Utc>::from_naive_utc_and_offset(t.created_at, Utc).date_naive();
            trace_date == Utc::now().date_naive()
        })
        .collect();

    let current_token_usage: i64 = today_traces
        .iter()
        .map(|t| i64::from(t.tokens_total.unwrap_or(0)))
        .sum();

    let average_token_usage = match i64::try_from(daily_totals.len()) {
        Ok(count) if count > 0 => daily_totals.iter().sum::<i64>() / count,
        _ => 0,
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
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let user_id = auth_context.user_id;

    // 固定获取最近30天的数据
    let start_time = Utc::now() - Duration::days(30);

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .filter(proxy_tracing::Column::UserId.eq(user_id))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_user_keys_request_trend_fail",
                &format!("Failed to fetch traces for user API keys request trend: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to fetch data: {}", err)
            ));
        }
    };

    // 按天分组统计请求次数
    let mut daily_requests: HashMap<String, i64> = HashMap::new();
    for trace in &traces {
        let date = DateTime::<Utc>::from_naive_utc_and_offset(trace.created_at, Utc)
            .format("%Y-%m-%d")
            .to_string();
        *daily_requests.entry(date).or_insert(0) += 1;
    }

    // 生成30天的时间序列数据
    let mut request_usage = Vec::new();
    let mut daily_totals = Vec::new();

    for i in 0..30 {
        let date = (Utc::now() - Duration::days(29 - i))
            .format("%Y-%m-%d")
            .to_string();
        let timestamp = (Utc::now() - Duration::days(29 - i)).to_rfc3339();

        let request_count = daily_requests.get(&date).copied().unwrap_or(0);
        daily_totals.push(request_count);

        request_usage.push(UserApiKeysRequestTrendPoint {
            timestamp,
            request: request_count,
        });
    }

    // 计算今天、平均值和最大值
    let current_request_usage = usize_to_i64(
        traces
            .iter()
            .filter(|t| {
                let trace_date =
                    DateTime::<Utc>::from_naive_utc_and_offset(t.created_at, Utc).date_naive();
                trace_date == Utc::now().date_naive()
            })
            .count(),
    );

    let average_request_usage = match i64::try_from(daily_totals.len()) {
        Ok(count) if count > 0 => daily_totals.iter().sum::<i64>() / count,
        _ => 0,
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

    // 固定获取最近30天的数据
    let start_time = Utc::now() - Duration::days(30);

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .filter(proxy_tracing::Column::UserId.eq(user_id))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_user_keys_token_trend_fail",
                &format!("Failed to fetch traces for user API keys token trend: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to fetch data: {}", err)
            ));
        }
    };

    // 按天分组统计Token使用量
    let mut daily_tokens: HashMap<String, i64> = HashMap::new();
    for trace in &traces {
        let date = DateTime::<Utc>::from_naive_utc_and_offset(trace.created_at, Utc)
            .format("%Y-%m-%d")
            .to_string();
        let tokens = i64::from(trace.tokens_total.unwrap_or(0));
        *daily_tokens.entry(date).or_insert(0) += tokens;
    }

    // 生成30天的时间序列数据
    let mut token_usage = Vec::new();
    let mut daily_totals = Vec::new();

    for i in 0..30 {
        let date = (Utc::now() - Duration::days(29 - i))
            .format("%Y-%m-%d")
            .to_string();
        let timestamp = (Utc::now() - Duration::days(29 - i)).to_rfc3339();

        let total_token = daily_tokens.get(&date).copied().unwrap_or(0);
        daily_totals.push(total_token);

        token_usage.push(UserApiKeysTokenTrendPoint {
            timestamp,
            total_token,
        });
    }

    // 计算今天、平均值和最大值
    let today_traces: Vec<&proxy_tracing::Model> = traces
        .iter()
        .filter(|t| {
            let trace_date =
                DateTime::<Utc>::from_naive_utc_and_offset(t.created_at, Utc).date_naive();
            trace_date == Utc::now().date_naive()
        })
        .collect();

    let current_token_usage: i64 = today_traces
        .iter()
        .map(|t| i64::from(t.tokens_total.unwrap_or(0)))
        .sum();

    let average_token_usage = match i64::try_from(daily_totals.len()) {
        Ok(count) if count > 0 => daily_totals.iter().sum::<i64>() / count,
        _ => 0,
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

fn usize_to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// 解析时间范围参数（支持时区转换）
fn parse_time_range(
    query: &TimeRangeQuery,
    timezone_context: &TimezoneContext,
) -> Result<(DateTime<Utc>, DateTime<Utc>), axum::response::Response> {
    let end_time = Utc::now();

    let start_time = match query.range.as_deref() {
        Some("today") => {
            // 获取用户时区的今天开始时间
            let user_today = end_time
                .with_timezone(&timezone_context.timezone)
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap();

            // 转换为UTC
            user_today
                .to_utc(&timezone_context.timezone)
                .unwrap_or(end_time - Duration::days(1))
        }
        Some("30days") => {
            // 获取30天前的UTC时间
            end_time - Duration::days(30)
        }
        Some("custom") => {
            if let (Some(start_naive), Some(end_naive)) = (&query.start, &query.end) {
                // 使用ConvertToUtc trait转换时区
                match (
                    start_naive.to_utc(&timezone_context.timezone),
                    end_naive.to_utc(&timezone_context.timezone),
                ) {
                    (Some(start_utc), Some(_end_utc)) => start_utc,
                    _ => {
                        return Err(crate::management::response::app_error(
                            ProxyError::internal("Invalid datetime values"),
                        ));
                    }
                }
            } else {
                return Err(crate::management::response::app_error(
                    ProxyError::internal("Custom range requires both start and end datetime"),
                ));
            }
        }
        _ => {
            // 默认7天前
            end_time - Duration::days(7)
        }
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
