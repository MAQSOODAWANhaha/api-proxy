//! # 统一统计信息处理器
//!
//! 基于proxy_tracing表的统一统计查询API
use crate::management::handlers::auth_utils::extract_user_id_from_headers;
use crate::management::response;
use crate::management::server::AppState;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use chrono::{DateTime, Duration, Utc};
use entity::{proxy_tracing, proxy_tracing::Entity as ProxyTracing};
use sea_orm::{entity::*, query::*};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub cost: String,
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
    pub cost: String,
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
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

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
            tracing::error!("Failed to fetch today's traces: {}", err);
            return response::error::<TodayDashboardCards>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch today's data",
            )
            .into_response();
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
            tracing::error!("Failed to fetch yesterday's traces: {}", err);
            return response::error::<TodayDashboardCards>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch yesterday's data",
            )
            .into_response();
        }
    };

    // 计算今天的统计数据
    let requests_today = today_traces.len() as i64;
    // 使用 collect + len 避免与 SeaORM count 混淆
    let successes_today = today_traces
        .iter()
        .filter(|t| t.is_success)
        .collect::<Vec<_>>()
        .len() as i64;
    let success_rate_today = if requests_today > 0 {
        (successes_today as f64 / requests_today as f64) * 100.0
    } else {
        0.0
    };

    let tokens_today: i64 = today_traces
        .iter()
        .map(|t| t.tokens_total.unwrap_or(0) as i64)
        .sum();

    let response_times: Vec<i64> = today_traces.iter().filter_map(|t| t.duration_ms).collect();
    let avg_response_time_today = if !response_times.is_empty() {
        response_times.iter().sum::<i64>() / response_times.len() as i64
    } else {
        0
    };

    // 计算昨天的统计数据用于比较
    let requests_yesterday = yesterday_traces.len() as i64;
    let successes_yesterday = yesterday_traces
        .iter()
        .filter(|t| t.is_success)
        .collect::<Vec<_>>()
        .len() as i64;
    let success_rate_yesterday = if requests_yesterday > 0 {
        (successes_yesterday as f64 / requests_yesterday as f64) * 100.0
    } else {
        0.0
    };

    let tokens_yesterday: i64 = yesterday_traces
        .iter()
        .map(|t| t.tokens_total.unwrap_or(0) as i64)
        .sum();

    let response_times_yesterday: Vec<i64> = yesterday_traces
        .iter()
        .filter_map(|t| t.duration_ms)
        .collect();
    let avg_response_time_yesterday = if !response_times_yesterday.is_empty() {
        response_times_yesterday.iter().sum::<i64>() / response_times_yesterday.len() as i64
    } else {
        0
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

    response::success(cards).into_response()
}

/// 2. 模型使用占比API: /api/statistics/models/rate
pub async fn get_models_usage_rate(
    State(state): State<AppState>,
    Query(query): Query<TimeRangeQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    let (start_time, _end_time) = match parse_time_range(&query) {
        Ok(times) => times,
        Err(error_response) => return error_response,
    };

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .filter(proxy_tracing::Column::UserId.eq(user_id))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            tracing::error!("Failed to fetch traces for models rate: {}", err);
            return response::error::<ModelsRateResponse>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            )
            .into_response();
        }
    };

    // 按模型统计使用次数
    let mut model_stats: HashMap<String, i64> = HashMap::new();
    for trace in traces {
        let model_name = trace.model_used.unwrap_or_else(|| "Unknown".to_string());
        *model_stats.entry(model_name).or_insert(0) += 1;
    }

    // 按使用次数排序
    let mut model_vec: Vec<(String, i64)> = model_stats.into_iter().collect();
    model_vec.sort_by(|a, b| b.1.cmp(&a.1));

    // 限制最多6个模型，其余合并为"其他"
    let mut model_usage = Vec::new();
    if model_vec.len() <= 6 {
        for (model, usage) in model_vec {
            model_usage.push(ModelUsage { model, usage });
        }
    } else {
        // 前5个模型
        for (model, usage) in model_vec.iter().take(5) {
            model_usage.push(ModelUsage {
                model: model.clone(),
                usage: *usage,
            });
        }
        // 其余模型合并为"其他"
        let other_usage: i64 = model_vec.iter().skip(5).map(|(_, usage)| usage).sum();
        model_usage.push(ModelUsage {
            model: "其他".to_string(),
            usage: other_usage,
        });
    }

    let response = ModelsRateResponse { model_usage };
    response::success(response).into_response()
}

/// 3. 模型详细统计API: /api/statistics/models/statistics
pub async fn get_models_statistics(
    State(state): State<AppState>,
    Query(query): Query<TimeRangeQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    let (start_time, _end_time) = match parse_time_range(&query) {
        Ok(times) => times,
        Err(error_response) => return error_response,
    };

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .filter(proxy_tracing::Column::UserId.eq(user_id))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            tracing::error!("Failed to fetch traces for models statistics: {}", err);
            return response::error::<ModelsStatisticsResponse>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            )
            .into_response();
        }
    };

    let total_requests = traces.len() as i64;

    // 按模型统计详细数据（使用次数和费用）
    let mut model_stats: HashMap<String, (i64, f64)> = HashMap::new();
    for trace in traces {
        let model_name = trace.model_used.unwrap_or_else(|| "Unknown".to_string());
        let cost = trace.cost.unwrap_or(0.0);
        let entry = model_stats.entry(model_name).or_insert((0, 0.0));
        entry.0 += 1; // usage count
        entry.1 += cost; // total cost
    }

    // 转换为响应格式
    let mut model_usage: Vec<ModelStatistics> = model_stats
        .into_iter()
        .map(|(model, (usage, cost))| {
            let percentage = if total_requests > 0 {
                (usage as f64 / total_requests as f64) * 100.0
            } else {
                0.0
            };
            ModelStatistics {
                model,
                usage,
                percentage,
                cost: format!("${:.2}", cost),
            }
        })
        .collect();

    // 按使用次数排序
    model_usage.sort_by(|a, b| b.usage.cmp(&a.usage));

    let response = ModelsStatisticsResponse { model_usage };
    response::success(response).into_response()
}

/// 4. Token使用趋势API: /api/statistics/tokens/trend
pub async fn get_tokens_trend(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

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
            tracing::error!("Failed to fetch traces for tokens trend: {}", err);
            return response::error::<TokensTrendResponse>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            )
            .into_response();
        }
    };

    // 按天分组统计Token使用情况
    let mut daily_stats: HashMap<String, (i64, i64, i64, i64, f64)> = HashMap::new();
    for trace in &traces {
        let date = DateTime::<Utc>::from_naive_utc_and_offset(trace.created_at, Utc)
            .format("%Y-%m-%d")
            .to_string();

        let entry = daily_stats.entry(date).or_insert((0, 0, 0, 0, 0.0));
        entry.0 += trace.cache_create_tokens.unwrap_or(0) as i64;
        entry.1 += trace.cache_read_tokens.unwrap_or(0) as i64;
        entry.2 += trace.tokens_prompt.unwrap_or(0) as i64;
        entry.3 += trace.tokens_completion.unwrap_or(0) as i64;
        entry.4 += trace.cost.unwrap_or(0.0);
    }

    // 生成30天的时间序列数据
    let mut token_usage = Vec::new();
    let mut daily_totals = Vec::new();

    for i in 0..30 {
        let date = (Utc::now() - Duration::days(29 - i))
            .format("%Y-%m-%d")
            .to_string();
        let timestamp = (Utc::now() - Duration::days(29 - i)).to_rfc3339();

        if let Some((cache_create, cache_read, prompt, completion, cost)) = daily_stats.get(&date) {
            let total_tokens = prompt + completion;
            daily_totals.push(total_tokens);

            token_usage.push(TokenTrendPoint {
                timestamp,
                cache_create_tokens: *cache_create,
                cache_read_tokens: *cache_read,
                tokens_prompt: *prompt,
                tokens_completion: *completion,
                cost: format!("${:.2}", cost),
            });
        } else {
            daily_totals.push(0);
            token_usage.push(TokenTrendPoint {
                timestamp,
                cache_create_tokens: 0,
                cache_read_tokens: 0,
                tokens_prompt: 0,
                tokens_completion: 0,
                cost: "$0.00".to_string(),
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
        .map(|t| t.tokens_total.unwrap_or(0) as i64)
        .sum();

    let average_token_usage = if !daily_totals.is_empty() {
        daily_totals.iter().sum::<i64>() / daily_totals.len() as i64
    } else {
        0
    };

    let max_token_usage = daily_totals.iter().max().copied().unwrap_or(0);

    let response = TokensTrendResponse {
        token_usage,
        current_token_usage,
        average_token_usage,
        max_token_usage,
    };

    response::success(response).into_response()
}

/// 5. 用户API Keys请求趋势API: /api/statistics/user-service-api-keys/request
pub async fn get_user_api_keys_request_trend(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

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
            tracing::error!(
                "Failed to fetch traces for user API keys request trend: {}",
                err
            );
            return response::error::<UserApiKeysRequestTrendResponse>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            )
            .into_response();
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
    let today_traces: Vec<&proxy_tracing::Model> = traces
        .iter()
        .filter(|t| {
            let trace_date =
                DateTime::<Utc>::from_naive_utc_and_offset(t.created_at, Utc).date_naive();
            trace_date == Utc::now().date_naive()
        })
        .collect();

    let current_request_usage = today_traces.len() as i64;

    let average_request_usage = if !daily_totals.is_empty() {
        daily_totals.iter().sum::<i64>() / daily_totals.len() as i64
    } else {
        0
    };

    let max_request_usage = daily_totals.iter().max().copied().unwrap_or(0);

    let response = UserApiKeysRequestTrendResponse {
        request_usage,
        current_request_usage,
        average_request_usage,
        max_request_usage,
    };

    response::success(response).into_response()
}

/// 6. 用户API Keys Token趋势API: /api/statistics/user-service-api-keys/token
pub async fn get_user_api_keys_token_trend(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

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
            tracing::error!(
                "Failed to fetch traces for user API keys token trend: {}",
                err
            );
            return response::error::<UserApiKeysTokenTrendResponse>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            )
            .into_response();
        }
    };

    // 按天分组统计Token使用量
    let mut daily_tokens: HashMap<String, i64> = HashMap::new();
    for trace in &traces {
        let date = DateTime::<Utc>::from_naive_utc_and_offset(trace.created_at, Utc)
            .format("%Y-%m-%d")
            .to_string();
        let tokens = trace.tokens_total.unwrap_or(0) as i64;
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
        .map(|t| t.tokens_total.unwrap_or(0) as i64)
        .sum();

    let average_token_usage = if !daily_totals.is_empty() {
        daily_totals.iter().sum::<i64>() / daily_totals.len() as i64
    } else {
        0
    };

    let max_token_usage = daily_totals.iter().max().copied().unwrap_or(0);

    let response = UserApiKeysTokenTrendResponse {
        token_usage,
        current_token_usage,
        average_token_usage,
        max_token_usage,
    };

    response::success(response).into_response()
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
        Some("7days") => end_time - Duration::days(7),
        Some("30days") => end_time - Duration::days(30),
        Some("custom") => {
            if let (Some(start_str), Some(_end_str)) = (&query.start, &query.end) {
                match chrono::NaiveDate::parse_from_str(start_str, "%Y-%m-%d") {
                    Ok(start_date) => {
                        let start_datetime = start_date.and_hms_opt(0, 0, 0).unwrap();
                        DateTime::from_naive_utc_and_offset(start_datetime, Utc)
                    }
                    Err(_) => {
                        return Err(response::error::<()>(
                            StatusCode::BAD_REQUEST,
                            "INVALID_DATE",
                            "Invalid start date format. Use YYYY-MM-DD",
                        )
                        .into_response());
                    }
                }
            } else {
                return Err(response::error::<()>(
                    StatusCode::BAD_REQUEST,
                    "MISSING_DATES",
                    "Custom range requires both start and end dates",
                )
                .into_response());
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
        let rate = ((current - previous) as f64 / previous as f64) * 100.0;
        if rate > 0.0 {
            format!("+{:.1}%", rate)
        } else {
            format!("{:.1}%", rate)
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
            format!("+{:.1}%", rate)
        } else {
            format!("{:.1}%", rate)
        }
    }
}
