//! # 统一统计信息处理器
//!
//! 基于proxy_tracing表的统一统计查询API

use crate::management::response;
use crate::management::server::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use chrono::NaiveDateTime;
use chrono::{DateTime, Duration, Utc};
use entity::{proxy_tracing, proxy_tracing::Entity as ProxyTracing};
use sea_orm::{entity::*, query::*, DatabaseConnection, QuerySelect};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

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

/// 新的统一统计查询参数
#[derive(Debug, Deserialize)]
pub struct UnifiedStatsQuery {
    /// 时间范围（小时）
    pub hours: Option<u32>,
    /// 分组方式
    pub group_by: Option<String>,
    /// 提供商类型过滤
    pub provider_type: Option<String>,
    /// 追踪级别过滤 (0=基础, 1=详细, 2=完整)
    pub trace_level: Option<i32>,
    /// 仅显示成功请求
    pub success_only: Option<bool>,
    /// 仅显示异常请求
    pub anomaly_only: Option<bool>,
}

#[derive(Serialize)]
struct UnifiedOverview {
    time_range: TimeRange,
    requests: RequestStats,
    performance: PerformanceStats,
    tokens: TokenStats,
    health: HealthStats,
    by_provider: HashMap<String, ProviderStatDetail>,
    error_distribution: HashMap<String, i64>,
    trace_level_distribution: HashMap<String, i64>,
}

#[derive(Serialize)]
struct TimeRange {
    hours: u32,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
}

#[derive(Serialize)]
struct RequestStats {
    total: i64,
    successful: i64,
    failed: i64,
    anomalous: i64,
    success_rate: f64,
}

#[derive(Serialize)]
struct PerformanceStats {
    avg_response_time_ms: i64,
    p50_response_time_ms: i64,
    p95_response_time_ms: i64,
    p99_response_time_ms: i64,
}

#[derive(Serialize)]
struct TokenStats {
    total_prompt_tokens: i64,
    total_completion_tokens: i64,
    total_tokens: i64,
    avg_tokens_per_request: i64,
}

#[derive(Serialize)]
struct HealthStats {
    avg_health_score: f64,
    anomaly_rate: f64,
}

#[derive(Serialize)]
struct ProviderStatDetail {
    requests: i64,
    successful_requests: i64,
    anomalous_requests: i64,
    success_rate: f64,
    anomaly_rate: f64,
    avg_response_time_ms: i64,
    avg_health_score: f64,
    total_tokens: i64,
}

#[derive(Serialize)]
struct DetailedTracesResponse {
    traces: Vec<DetailedTrace>,
    total_count: usize,
    time_range: TimeRange,
}

#[derive(Serialize)]
struct DetailedTrace {
    request_id: String,
    method: String,
    path: Option<String>,
    provider_name: Option<String>,
    start_time: Option<NaiveDateTime>,
    end_time: Option<NaiveDateTime>,
    duration_ms: Option<i64>,
    status_code: Option<i32>,
    is_success: bool,
    is_anomaly: Option<bool>,
    health_impact_score: Option<f64>,
    tokens: TraceTokenStats,
    phases: Value,
    performance_metrics: Value,
    labels: Value,
    error: Option<TraceError>,
}

#[derive(Serialize)]
struct TraceTokenStats {
    prompt: Option<i32>,
    completion: Option<i32>,
    total: Option<i32>,
    efficiency_ratio: Option<f64>,
}

#[derive(Serialize)]
struct TraceError {
    r#type: Option<String>,
    message: Option<String>,
}

#[derive(Serialize)]
struct HealthOverviewResponse {
    overall: OverallHealthStats,
    by_provider: HashMap<String, ProviderHealthSummary>,
    health_trend: Vec<HealthTrendPoint>,
}

#[derive(Serialize)]
struct OverallHealthStats {
    total_requests: usize,
    healthy_providers: usize,
    warning_providers: usize,
    critical_providers: usize,
}

#[derive(Serialize)]
struct ProviderHealthSummary {
    total_requests: i64,
    successful_requests: i64,
    anomalous_requests: i64,
    success_rate: f64,
    anomaly_rate: f64,
    avg_health_score: f64,
    health_status: String,
}

#[derive(Serialize)]
struct HealthTrendPoint {
    timestamp: DateTime<Utc>,
    total_requests: i64,
    successful_requests: i64,
    anomalous_requests: i64,
    success_rate: f64,
    avg_health_score: f64,
}

#[derive(Serialize)]
struct DashboardCards {
    total_requests_today: i64,
    success_rate_today: f64,
    total_tokens_today: i64,
    active_api_services: i64,
    healthy_keys: i64,
    total_keys: i64,
    avg_response_time: i64,
    requests_per_minute: i64,
}

/// 提供商统计信息
#[derive(Debug)]
struct UnifiedProviderStats {
    requests: i64,
    successful_requests: i64,
    anomalous_requests: i64,
    avg_response_time: f64,
    success_rate: f64,
    avg_health_score: f64,
    total_tokens: i64,
    avg_efficiency: f64,
}

/// 健康状态概览
#[derive(Debug)]
struct HealthOverview {
    total_requests: i64,
    healthy_requests: i64,
    anomalous_requests: i64,
    avg_health_score: f64,
    top_errors: HashMap<String, i64>,
}

/// 获取统计概览（兼容旧API）
pub async fn get_overview(
    State(state): State<AppState>,
    Query(query): Query<StatsQuery>,
) -> impl IntoResponse {
    // 转换为统一查询参数
    let unified_query = UnifiedStatsQuery {
        hours: query.hours,
        group_by: query.group_by,
        provider_type: query.provider_type.or(query.upstream_type), // 兼容upstream_type
        trace_level: query.trace_level,
        success_only: query.success_only,
        anomaly_only: query.anomaly_only,
    };

    get_unified_overview(State(state), Query(unified_query)).await
}

/// 获取请求统计（兼容旧API）
pub async fn get_request_stats(
    State(state): State<AppState>,
    Query(query): Query<StatsQuery>,
) -> impl IntoResponse {
    // 转换为统一查询参数并调用详细追踪
    let unified_query = UnifiedStatsQuery {
        hours: query.hours,
        group_by: query.group_by,
        provider_type: query.provider_type.or(query.upstream_type),
        trace_level: query.trace_level,
        success_only: query.success_only,
        anomaly_only: query.anomaly_only,
    };

    get_detailed_traces(State(state), Query(unified_query)).await
}

/// 获取统一统计概览
pub async fn get_unified_overview(
    State(state): State<AppState>,
    Query(query): Query<UnifiedStatsQuery>,
) -> impl IntoResponse {
    let hours = query.hours.unwrap_or(24);
    let start_time = Utc::now() - Duration::hours(hours as i64);

    let mut select =
        ProxyTracing::find().filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()));

    // provider_type filtering removed - use provider_type_id if needed
    // trace_level filtering removed - field no longer exists  
    // anomaly_only filtering removed - field no longer exists
    if query.success_only.unwrap_or(false) {
        select = select.filter(proxy_tracing::Column::IsSuccess.eq(true));
    }

    let traces = match select.all(state.database.as_ref()).await {
        Ok(traces) => traces,
        Err(err) => {
            tracing::error!("Failed to fetch proxy traces: {}", err);
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch proxy traces",
            );
        }
    };

    let total_requests = traces.len() as i64;
    let successful_requests = traces.iter().filter(|t| t.is_success).count() as i64;
    let anomalous_requests = 0i64; // 异常检测功能已移除

    let mut response_times: Vec<i64> = traces
        .iter()
        .filter_map(|t| t.duration_ms)
        .collect();
    response_times.sort_unstable();

    let total_prompt_tokens: i64 = traces
        .iter()
        .map(|t| t.tokens_prompt.unwrap_or(0) as i64)
        .sum();
    let total_completion_tokens: i64 = traces
        .iter()
        .map(|t| t.tokens_completion.unwrap_or(0) as i64)
        .sum();

    // 健康评分功能已移除
    let health_scores: Vec<f64> = Vec::new();

    let overview = UnifiedOverview {
        time_range: TimeRange {
            hours,
            start_time,
            end_time: Utc::now(),
        },
        requests: RequestStats {
            total: total_requests,
            successful: successful_requests,
            failed: total_requests - successful_requests,
            anomalous: anomalous_requests,
            success_rate: if total_requests > 0 {
                (successful_requests as f64 / total_requests as f64) * 100.0
            } else {
                0.0
            },
        },
        performance: PerformanceStats {
            avg_response_time_ms: if !response_times.is_empty() {
                (response_times.iter().sum::<i64>() as f64 / response_times.len() as f64) as i64
            } else {
                0
            },
            p50_response_time_ms: calculate_percentile_i64(&response_times, 0.5),
            p95_response_time_ms: calculate_percentile_i64(&response_times, 0.95),
            p99_response_time_ms: calculate_percentile_i64(&response_times, 0.99),
        },
        tokens: TokenStats {
            total_prompt_tokens,
            total_completion_tokens,
            total_tokens: total_prompt_tokens + total_completion_tokens,
            avg_tokens_per_request: if total_requests > 0 {
                ((total_prompt_tokens + total_completion_tokens) as f64 / total_requests as f64)
                    as i64
            } else {
                0
            },
        },
        health: HealthStats {
            avg_health_score: if !health_scores.is_empty() {
                health_scores.iter().sum::<f64>() / health_scores.len() as f64
            } else {
                0.0
            },
            anomaly_rate: if total_requests > 0 {
                (anomalous_requests as f64 / total_requests as f64) * 100.0
            } else {
                0.0
            },
        },
        by_provider: calculate_provider_stats(&traces).await,
        error_distribution: calculate_error_distribution(&traces),
        trace_level_distribution: calculate_trace_level_distribution(&traces),
    };

    response::success(overview)
}

/// 获取健康状态概览
pub async fn get_health_overview(
    State(state): State<AppState>,
    Query(query): Query<UnifiedStatsQuery>,
) -> impl IntoResponse {
    let hours = query.hours.unwrap_or(24);
    let start_time = Utc::now() - Duration::hours(hours as i64);

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            tracing::error!("Failed to fetch traces for health overview: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            );
        }
    };

    // 按提供商分组健康统计
    let mut provider_health: HashMap<String, Vec<f64>> = HashMap::new();
    let mut provider_requests: HashMap<String, (i64, i64, i64)> = HashMap::new(); // (total, success, anomaly)

    for trace in &traces {
        let provider_name = "Unknown".to_string(); // provider_name field removed

        // 收集健康评分
        if let Some(score) = None {
            provider_health
                .entry(provider_name.clone())
                .or_default()
                .push(score);
        }

        // 统计请求数
        {
            let counts = provider_requests.entry(provider_name).or_insert((0, 0, 0));
            counts.0 += 1; // total
            if trace.is_success {
                counts.1 += 1; // success
            }
            if None.unwrap_or(false) {
                counts.2 += 1; // anomaly
            }
        }
    }

    // 计算每个提供商的健康指标
    let mut provider_health_summary = HashMap::new();
    for (provider, scores) in provider_health {
        let avg_score = scores.iter().sum::<f64>() / scores.len() as f64;
        let (total, success, anomaly) = provider_requests.get(&provider).unwrap_or(&(0, 0, 0));

        provider_health_summary.insert(provider, json!({
            "total_requests": total,
            "successful_requests": success,
            "anomalous_requests": anomaly,
            "success_rate": if *total > 0 { (*success as f64 / *total as f64) * 100.0 } else { 0.0 },
            "anomaly_rate": if *total > 0 { (*anomaly as f64 / *total as f64) * 100.0 } else { 0.0 },
            "avg_health_score": avg_score,
            "health_status": classify_health_status(avg_score)
        }));
    }

    // 整体健康趋势（按时间分组）
    let health_trend = calculate_health_trend(&traces, hours).await;

    let health_overview = json!({
        "overall": {
            "total_requests": traces.len(),
            "healthy_providers": provider_health_summary.values()
                .filter(|v| v["health_status"] == "healthy")
                .count(),
            "warning_providers": provider_health_summary.values()
                .filter(|v| v["health_status"] == "warning")
                .count(),
            "critical_providers": provider_health_summary.values()
                .filter(|v| v["health_status"] == "critical")
                .count(),
        },
        "by_provider": provider_health_summary,
        "health_trend": health_trend
    });

    response::success(health_overview)
}

/// 获取详细追踪信息
pub async fn get_detailed_traces(
    State(state): State<AppState>,
    Query(query): Query<UnifiedStatsQuery>,
) -> impl IntoResponse {
    let hours = query.hours.unwrap_or(1);
    let start_time = Utc::now() - Duration::hours(hours as i64);

    let select = ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .order_by_desc(proxy_tracing::Column::CreatedAt)
        .limit(100);

    // anomaly_only filtering removed - field no longer exists

    let traces = match select.all(state.database.as_ref()).await {
        Ok(traces) => traces,
        Err(err) => {
            tracing::error!("Failed to fetch detailed traces: {}", err);
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch detailed traces",
            );
        }
    };

    let total_count = traces.len();
    let detailed_traces: Vec<DetailedTrace> = traces
        .into_iter()
        .map(|trace| {
            // serialize helpers - complex fields removed for simplification
            let phases_v = Value::Null;
            let perf_v = Value::Null;
            let labels_v = Value::Null;
            DetailedTrace {
                request_id: trace.request_id,
                method: trace.method,
                path: trace.path,
                provider_name: None,
                start_time: trace.start_time, // already naive or appropriate
                end_time: trace.end_time,
                duration_ms: trace.duration_ms,
                status_code: trace.status_code,
                is_success: trace.is_success,
                is_anomaly: None,
                health_impact_score: None,
                tokens: TraceTokenStats {
                    prompt: trace.tokens_prompt,
                    completion: trace.tokens_completion,
                    total: trace.tokens_total,
                    efficiency_ratio: trace.token_efficiency_ratio,
                },
                phases: phases_v,
                performance_metrics: perf_v,
                labels: labels_v,
                error: if trace.error_type.is_some() || trace.error_message.is_some() {
                    Some(TraceError {
                        r#type: trace.error_type,
                        message: trace.error_message,
                    })
                } else {
                    None
                },
            }
        })
        .collect();

    response::success(DetailedTracesResponse {
        traces: detailed_traces,
        total_count,
        time_range: TimeRange {
            hours,
            start_time,
            end_time: Utc::now(),
        },
    })
}

/// 计算提供商统计
async fn calculate_provider_stats(
    traces: &[proxy_tracing::Model],
) -> HashMap<String, ProviderStatDetail> {
    let mut provider_data: HashMap<String, Vec<&proxy_tracing::Model>> = HashMap::new();

    for trace in traces {
        let provider_name = "Unknown".to_string(); // provider_name field removed
        provider_data.entry(provider_name).or_default().push(trace);
    }

    let mut result = HashMap::new();
    for (provider, provider_traces) in provider_data {
        let total = provider_traces.len() as i64;
        let successful = provider_traces.iter().filter(|t| t.is_success).count() as i64;
        let anomalous = 0i64; // 异常检测功能已移除

        let response_times: Vec<i64> = provider_traces
            .iter()
            .filter_map(|t| {
                t.duration_ms
            })
            .collect();
        let avg_response_time = if !response_times.is_empty() {
            response_times.iter().sum::<i64>() as f64 / response_times.len() as f64
        } else {
            0.0
        };

        let health_scores: Vec<f64> = Vec::new(); // 健康评分功能已移除
        let avg_health_score = if !health_scores.is_empty() {
            health_scores.iter().sum::<f64>() / health_scores.len() as f64
        } else {
            0.0
        };

        let total_tokens: i64 = provider_traces
            .iter()
            .map(|t| t.tokens_total.unwrap_or(0) as i64)
            .sum();

        result.insert(
            provider,
            ProviderStatDetail {
                requests: total,
                successful_requests: successful,
                anomalous_requests: anomalous,
                success_rate: if total > 0 {
                    (successful as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
                anomaly_rate: if total > 0 {
                    (anomalous as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
                avg_response_time_ms: avg_response_time as i64,
                avg_health_score,
                total_tokens,
            },
        );
    }

    result
}

/// 计算错误分布
fn calculate_error_distribution(traces: &[proxy_tracing::Model]) -> HashMap<String, i64> {
    let mut distribution = HashMap::new();

    for trace in traces {
        if let Some(error_type) = &trace.error_type {
            *distribution.entry(error_type.clone()).or_insert(0) += 1;
        }
    }

    distribution
}

/// 计算追踪级别分布
fn calculate_trace_level_distribution(traces: &[proxy_tracing::Model]) -> HashMap<String, i64> {
    let mut distribution = HashMap::new();

    for _trace in traces {
        let level_name = match 0 {
            0 => "basic",
            1 => "detailed",
            2 => "full",
            _ => "unknown",
        };
        *distribution.entry(level_name.to_string()).or_insert(0) += 1;
    }

    distribution
}

/// 计算健康趋势
async fn calculate_health_trend(traces: &[proxy_tracing::Model], hours: u32) -> Vec<Value> {
    let mut trend = Vec::new();
    let interval_minutes = if hours <= 2 {
        10
    } else if hours <= 24 {
        60
    } else {
        240
    }; // 10分钟、1小时、4小时间隔
    let total_intervals = (hours * 60) / interval_minutes;

    let now = Utc::now();
    for i in 0..total_intervals {
        let interval_start =
            now - Duration::minutes((total_intervals - i) as i64 * interval_minutes as i64);
        let interval_end = interval_start + Duration::minutes(interval_minutes as i64);

        let interval_traces: Vec<&proxy_tracing::Model> = traces
            .iter()
            .filter(|t| {
                let trace_time: DateTime<Utc> =
                    DateTime::from_naive_utc_and_offset(t.created_at, Utc);
                trace_time >= interval_start && trace_time < interval_end
            })
            .collect();

        let total = interval_traces.len() as i64;
        let successful = interval_traces.iter().filter(|t| t.is_success).count() as i64;
        let anomalous = 0i64; // 异常检测功能已移除

        let avg_health_score = if !interval_traces.is_empty() {
            let scores: Vec<f64> = Vec::new(); // 健康评分功能已移除
            if !scores.is_empty() {
                scores.iter().sum::<f64>() / scores.len() as f64
            } else {
                0.0
            }
        } else {
            0.0
        };

        trend.push(json!({
            "timestamp": interval_start,
            "total_requests": total,
            "successful_requests": successful,
            "anomalous_requests": anomalous,
            "success_rate": if total > 0 { (successful as f64 / total as f64) * 100.0 } else { 0.0 },
            "avg_health_score": avg_health_score
        }));
    }

    trend
}

/// 分类健康状态
fn classify_health_status(score: f64) -> &'static str {
    if score >= 5.0 {
        "healthy"
    } else if score >= -5.0 {
        "warning"
    } else {
        "critical"
    }
}

/// 计算百分位数（i64版本）
fn calculate_percentile_i64(values: &[i64], percentile: f64) -> i64 {
    if values.is_empty() {
        return 0;
    }

    let index = (values.len() as f64 * percentile) as usize;
    let index = if index >= values.len() {
        values.len() - 1
    } else {
        index
    };
    values[index]
}

/// Dashboard卡片数据
pub async fn get_dashboard_cards(State(state): State<AppState>) -> impl IntoResponse {
    let hours = 24;
    let start_time = Utc::now() - Duration::hours(hours);

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            tracing::error!("Failed to fetch traces for dashboard cards: {}", err);
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch traces for dashboard cards",
            );
        }
    };

    let total_requests = traces.len() as i64;
    let successful_requests = traces.iter().filter(|t| t.is_success).count() as i64;

    let response_times: Vec<i64> = traces
        .iter()
        .filter_map(|t| t.duration_ms)
        .collect();
    let avg_response_time = if !response_times.is_empty() {
        response_times.iter().sum::<i64>() as f64 / response_times.len() as f64
    } else {
        0.0
    };

    let total_tokens: i64 = traces
        .iter()
        .map(|t| t.tokens_total.unwrap_or(0) as i64)
        .sum();

    let mut provider_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    for _trace in &traces {
        if let Some(provider) = None::<String> {
            provider_set.insert(provider);
        }
    }
    let active_providers = provider_set.len() as i64;

    let (total_keys, healthy_keys) = get_api_keys_stats(state.database.as_ref())
        .await
        .unwrap_or((0, 0));

    let cards = DashboardCards {
        total_requests_today: total_requests,
        success_rate_today: if total_requests > 0 {
            (successful_requests as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        },
        total_tokens_today: total_tokens,
        active_api_services: active_providers,
        healthy_keys,
        total_keys,
        avg_response_time: avg_response_time as i64,
        requests_per_minute: if total_requests > 0 {
            (total_requests as f64 / (24.0 * 60.0)) as i64
        } else {
            0
        },
    };

    response::success(cards)
}

/// Dashboard趋势数据
pub async fn get_dashboard_trend(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let days = params
        .get("days")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(7); // 默认7天

    let start_time = Utc::now() - Duration::days(days as i64);

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            tracing::error!("Failed to fetch traces for dashboard trend: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            );
        }
    };

    // 按天分组数据 (total, successful, total_response_time, total_tokens)
    let mut daily_data: std::collections::HashMap<String, (i64, i64, f64, i64)> =
        std::collections::HashMap::new();

    for trace in traces {
        let trace_date = DateTime::<Utc>::from_naive_utc_and_offset(trace.created_at, Utc)
            .format("%Y-%m-%d")
            .to_string();

        let entry = daily_data.entry(trace_date).or_insert((0, 0, 0.0, 0));
        entry.0 += 1; // total requests
        if trace.is_success {
            entry.1 += 1; // successful requests
        }

        // 累计响应时间
        if let Some(duration) = trace
            .duration_ms
            // response_time_ms field removed
        {
            entry.2 += duration as f64;
        }

        // 累计token数
        entry.3 += trace.tokens_total.unwrap_or(0) as i64;
    }

    // 生成趋势数据
    let mut trend_data = Vec::new();
    for i in 0..days {
        let date = (Utc::now() - Duration::days((days - 1 - i) as i64))
            .format("%Y-%m-%d")
            .to_string();

        let (total, successful, _total_response_time, tokens) =
            daily_data.get(&date).unwrap_or(&(0, 0, 0.0, 0));
        let failed = total - successful;

        trend_data.push(json!({
            "date": date,
            "requests": total,
            "successful": successful,
            "failed": failed,
            "tokens": tokens
        }));
    }

    response::success(serde_json::Value::Array(trend_data))
}

/// Dashboard服务商分布数据
pub async fn get_provider_distribution(State(state): State<AppState>) -> impl IntoResponse {
    let hours = 24; // 固定24小时数据
    let start_time = Utc::now() - Duration::hours(hours);

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            tracing::error!("Failed to fetch traces for provider distribution: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            );
        }
    };

    // 按服务商统计 (requests, successful, total_response_time, tokens)
    let mut provider_stats: std::collections::HashMap<String, (i64, i64, f64, i64)> =
        std::collections::HashMap::new();
    let total_requests = traces.len() as i64;

    for trace in traces {
        let provider_name = "Unknown".to_string(); // provider_name field removed
        let entry = provider_stats
            .entry(provider_name)
            .or_insert((0, 0, 0.0, 0));

        entry.0 += 1; // requests count
        if trace.is_success {
            entry.1 += 1; // successful requests
        }

        // 累计响应时间
        if let Some(duration) = trace
            .duration_ms
            // response_time_ms field removed
        {
            entry.2 += duration as f64;
        }

        // 累计tokens
        entry.3 += trace.tokens_total.unwrap_or(0) as i64;
    }

    // 生成分布数据
    let mut distribution = Vec::new();
    for (provider, (requests, _successful, _total_response_time, tokens)) in provider_stats {
        let percentage = if total_requests > 0 {
            (requests as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };

        distribution.push(json!({
            "provider": provider,
            "requests": requests,
            "percentage": percentage,
            "tokens": tokens
        }));
    }

    // 按请求数排序
    distribution.sort_by(|a, b| {
        let a_requests = a["requests"].as_i64().unwrap_or(0);
        let b_requests = b["requests"].as_i64().unwrap_or(0);
        b_requests.cmp(&a_requests)
    });

    response::success(serde_json::Value::Array(distribution))
}

/// 获取请求日志列表
pub async fn get_request_logs(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let page = params
        .get("page")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1);
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(20);
    let offset = (page - 1) * limit;

    // 状态过滤（success, failed, all）
    let status_filter = params.get("status").map(|s| s.as_str()).unwrap_or("all");

    // 时间范围过滤
    let hours = params
        .get("hours")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(24);
    let start_time = Utc::now() - Duration::hours(hours as i64);

    // 构建查询
    let mut select =
        ProxyTracing::find().filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()));

    // 应用状态过滤
    match status_filter {
        "success" => select = select.filter(proxy_tracing::Column::IsSuccess.eq(true)),
        "failed" => select = select.filter(proxy_tracing::Column::IsSuccess.eq(false)),
        _ => {} // "all" - 不过滤
    }

    // 服务商过滤
    if let Some(provider) = params.get("provider") {
        select = select.filter(proxy_tracing::Column::ProviderTypeId.eq(provider));
    }

    let traces = match select
        .offset(offset as u64)
        .limit(limit as u64)
        .order_by_desc(proxy_tracing::Column::CreatedAt)
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            tracing::error!("Failed to fetch request logs: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            );
        }
    };

    // 获取总数
    let mut count_select =
        ProxyTracing::find().filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()));

    match status_filter {
        "success" => count_select = count_select.filter(proxy_tracing::Column::IsSuccess.eq(true)),
        "failed" => count_select = count_select.filter(proxy_tracing::Column::IsSuccess.eq(false)),
        _ => {}
    }

    if let Some(provider) = params.get("provider") {
        count_select = count_select.filter(proxy_tracing::Column::ProviderTypeId.eq(provider));
    }

    let total = match count_select.count(state.database.as_ref()).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!("Failed to count request logs: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            );
        }
    };

    // 转换为响应格式
    let logs: Vec<Value> = traces.iter().map(|trace| {
        json!({
            "id": trace.id,
            "request_id": trace.request_id,
            "method": trace.method,
            "path": trace.path,
            "provider_name": None::<String>,
            "status_code": trace.status_code,
            "is_success": trace.is_success,
            "duration_ms": trace.duration_ms,
            "tokens_total": trace.tokens_total,
            "created_at": trace.created_at,
            "error_type": trace.error_type,
            "error_message": if trace.error_message.as_ref().map_or(false, |s| !s.is_empty()) {
                trace.error_message.as_ref().map(|s| s.chars().take(100).collect::<String>())
            } else {
                None
            }
        })
    }).collect();

    // 统一分页响应格式：data = 日志数组，顶层具备 pagination 字段
    let pagination = response::Pagination {
        page: page as u64,
        limit: limit as u64,
        total: total as u64,
        pages: ((total as f64) / (limit as f64)).ceil() as u64,
    };
    response::paginated(logs, pagination)
}

/// 获取实时统计数据
pub async fn get_realtime_stats(State(state): State<AppState>) -> impl IntoResponse {
    // 获取最近5分钟的数据
    let start_time = Utc::now() - Duration::minutes(5);

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            tracing::error!("Failed to fetch realtime stats: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            );
        }
    };

    let total_requests = traces.len() as i64;
    let successful_requests = traces.iter().filter(|t| t.is_success).count() as i64;
    let failed_requests = total_requests - successful_requests;

    // 计算每分钟请求量
    let mut minute_stats: std::collections::HashMap<String, (i64, i64)> =
        std::collections::HashMap::new();

    for trace in &traces {
        let minute_key = DateTime::<Utc>::from_naive_utc_and_offset(trace.created_at, Utc)
            .format("%H:%M")
            .to_string();

        let entry = minute_stats.entry(minute_key).or_insert((0, 0));
        entry.0 += 1; // total
        if trace.is_success {
            entry.1 += 1; // successful
        }
    }

    // 生成最近5分钟的数据点
    let mut timeline = Vec::new();
    for i in 0..5 {
        let time_point = Utc::now() - Duration::minutes(4 - i);
        let minute_key = time_point.format("%H:%M").to_string();
        let (total, successful) = minute_stats.get(&minute_key).unwrap_or(&(0, 0));

        timeline.push(json!({
            "time": minute_key,
            "timestamp": time_point,
            "total_requests": total,
            "successful_requests": successful,
            "failed_requests": total - successful,
            "success_rate": if *total > 0 { (*successful as f64 / *total as f64) * 100.0 } else { 0.0 }
        }));
    }

    // 按服务商统计
    let mut provider_stats: std::collections::HashMap<String, (i64, i64)> =
        std::collections::HashMap::new();
    for trace in &traces {
        let provider = "Unknown".to_string(); // provider_name field removed
        let entry = provider_stats.entry(provider).or_insert((0, 0));
        entry.0 += 1;
        if trace.is_success {
            entry.1 += 1;
        }
    }

    let provider_distribution: Vec<Value> = provider_stats.into_iter().map(|(provider, (total, successful))| {
        json!({
            "provider": provider,
            "requests": total,
            "success_rate": if total > 0 { (successful as f64 / total as f64) * 100.0 } else { 0.0 }
        })
    }).collect();

    // 平均响应时间
    let response_times: Vec<i64> = traces
        .iter()
        .filter_map(|t| t.duration_ms)
        .collect();
    let avg_response_time = if !response_times.is_empty() {
        response_times.iter().sum::<i64>() as f64 / response_times.len() as f64
    } else {
        0.0
    };

    response::success(json!({
        "summary": {
            "total_requests": total_requests,
            "successful_requests": successful_requests,
            "failed_requests": failed_requests,
            "success_rate": if total_requests > 0 { (successful_requests as f64 / total_requests as f64) * 100.0 } else { 0.0 },
            "avg_response_time": avg_response_time as i64,
            "period": "5min"
        },
        "timeline": timeline,
        "provider_distribution": provider_distribution,
        "last_updated": Utc::now()
    }))
}

/// 获取Token统计数据
pub async fn get_token_stats(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let hours = params
        .get("hours")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(24);
    let start_time = Utc::now() - Duration::hours(hours as i64);

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .filter(proxy_tracing::Column::IsSuccess.eq(true)) // 只统计成功的请求
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            tracing::error!("Failed to fetch token stats: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            );
        }
    };

    // Token统计
    let total_prompt_tokens: i64 = traces
        .iter()
        .map(|t| t.tokens_prompt.unwrap_or(0) as i64)
        .sum();
    let total_completion_tokens: i64 = traces
        .iter()
        .map(|t| t.tokens_completion.unwrap_or(0) as i64)
        .sum();
    let total_tokens = total_prompt_tokens + total_completion_tokens;

    let requests_with_tokens = traces
        .iter()
        .filter(|t| t.tokens_total.map_or(false, |total| total > 0))
        .count() as i64;

    let avg_tokens_per_request = if requests_with_tokens > 0 {
        total_tokens as f64 / requests_with_tokens as f64
    } else {
        0.0
    };

    // 按服务商统计Token使用量
    let mut provider_token_stats: std::collections::HashMap<String, (i64, i64, i64, i64)> =
        std::collections::HashMap::new();

    for trace in &traces {
        let provider = "Unknown".to_string(); // provider_name field removed
        let entry = provider_token_stats.entry(provider).or_insert((0, 0, 0, 0));
        entry.0 += trace.tokens_prompt.unwrap_or(0) as i64;
        entry.1 += trace.tokens_completion.unwrap_or(0) as i64;
        entry.2 += trace.tokens_total.unwrap_or(0) as i64;
        entry.3 += 1; // request count
    }

    let provider_breakdown: Vec<Value> = provider_token_stats.into_iter().map(|(provider, (prompt, completion, total, requests))| {
        json!({
            "provider": provider,
            "prompt_tokens": prompt,
            "completion_tokens": completion,
            "total_tokens": total,
            "requests": requests,
            "avg_tokens_per_request": if requests > 0 { total as f64 / requests as f64 } else { 0.0 }
        })
    }).collect();

    // 按小时统计Token使用趋势（过去24小时）
    let mut hourly_usage: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

    for trace in &traces {
        let hour_key = DateTime::<Utc>::from_naive_utc_and_offset(trace.created_at, Utc)
            .format("%Y-%m-%d %H:00")
            .to_string();

        let tokens = trace.tokens_total.unwrap_or(0) as i64;
        *hourly_usage.entry(hour_key).or_insert(0) += tokens;
    }

    // 生成过去24小时的趋势数据
    let mut token_trend = Vec::new();
    for i in 0..24 {
        let hour_time = Utc::now() - Duration::hours(23 - i);
        let hour_key = hour_time.format("%Y-%m-%d %H:00").to_string();
        let tokens = hourly_usage.get(&hour_key).unwrap_or(&0);

        token_trend.push(json!({
            "hour": hour_time.format("%H:00").to_string(),
            "timestamp": hour_time,
            "tokens": tokens
        }));
    }

    response::success(json!({
        "summary": {
            "total_tokens": total_tokens,
            "prompt_tokens": total_prompt_tokens,
            "completion_tokens": total_completion_tokens,
            "requests_with_tokens": requests_with_tokens,
            "avg_tokens_per_request": avg_tokens_per_request as i64,
            "period": format!("{}h", hours)
        },
        "provider_breakdown": provider_breakdown,
        "token_trend": token_trend,
        "token_efficiency": {
            "prompt_ratio": if total_tokens > 0 { (total_prompt_tokens as f64 / total_tokens as f64) * 100.0 } else { 0.0 },
            "completion_ratio": if total_tokens > 0 { (total_completion_tokens as f64 / total_tokens as f64) * 100.0 } else { 0.0 }
        }
    }))
}

/// 获取响应时间分析
pub async fn get_response_time_analysis(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let hours = params
        .get("hours")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(24);
    let start_time = Utc::now() - Duration::hours(hours as i64);

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .filter(proxy_tracing::Column::IsSuccess.eq(true))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            tracing::error!("Failed to fetch traces for response time analysis: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            );
        }
    };

    // 按时间分组统计响应时间
    let interval_hours = if hours <= 24 {
        1
    } else if hours <= 168 {
        4
    } else {
        24
    }; // 1小时、4小时、24小时间隔
    let total_intervals = hours / interval_hours;

    let mut time_series = Vec::new();
    let now = Utc::now();

    for i in 0..total_intervals {
        let interval_start =
            now - Duration::hours((total_intervals - i) as i64 * interval_hours as i64);
        let interval_end = interval_start + Duration::hours(interval_hours as i64);

        let interval_traces: Vec<&proxy_tracing::Model> = traces
            .iter()
            .filter(|t| {
                let trace_time = DateTime::<Utc>::from_naive_utc_and_offset(t.created_at, Utc);
                trace_time >= interval_start && trace_time < interval_end
            })
            .collect();

        if !interval_traces.is_empty() {
            let mut response_times: Vec<i64> = interval_traces
                .iter()
                .filter_map(|t| {
                    t.duration_ms // response_time_ms field removed
                })
                .collect();
            response_times.sort_unstable();

            let avg_response_time =
                response_times.iter().sum::<i64>() as f64 / response_times.len() as f64;
            let p50_response_time = calculate_percentile_i64(&response_times, 0.5);
            let p95_response_time = calculate_percentile_i64(&response_times, 0.95);
            let p99_response_time = calculate_percentile_i64(&response_times, 0.99);

            time_series.push(json!({
                "timestamp": interval_start.format("%Y-%m-%d %H:%M").to_string(),
                "avg_response_time": avg_response_time as i64,
                "p50_response_time": p50_response_time,
                "p95_response_time": p95_response_time,
                "p99_response_time": p99_response_time,
                "request_count": interval_traces.len()
            }));
        }
    }

    // 整体统计
    let mut all_response_times: Vec<i64> = traces
        .iter()
        .filter_map(|t| t.duration_ms)
        .collect();
    all_response_times.sort_unstable();

    let overall_avg = if !all_response_times.is_empty() {
        all_response_times.iter().sum::<i64>() as f64 / all_response_times.len() as f64
    } else {
        0.0
    };

    response::success(json!({
        "data": time_series,
        "summary": {
            "avg_response_time": overall_avg as i64,
            "p50_response_time": calculate_percentile_i64(&all_response_times, 0.5),
            "p95_response_time": calculate_percentile_i64(&all_response_times, 0.95),
            "p99_response_time": calculate_percentile_i64(&all_response_times, 0.99),
            "total_requests": all_response_times.len(),
            "period": format!("{}h", hours)
        }
    }))
}

/// 获取错误统计分析
pub async fn get_error_statistics(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let hours = params
        .get("hours")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(24);
    let start_time = Utc::now() - Duration::hours(hours as i64);

    let traces = match ProxyTracing::find()
        .filter(proxy_tracing::Column::CreatedAt.gte(start_time.naive_utc()))
        .all(state.database.as_ref())
        .await
    {
        Ok(traces) => traces,
        Err(err) => {
            tracing::error!("Failed to fetch traces for error statistics: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            );
        }
    };

    let total_requests = traces.len() as i64;
    let failed_traces: Vec<&proxy_tracing::Model> =
        traces.iter().filter(|t| !t.is_success).collect();
    let total_errors = failed_traces.len() as i64;

    // 错误类型分布统计
    let mut error_type_stats: HashMap<String, i64> = HashMap::new();
    for trace in &failed_traces {
        if let Some(error_type) = &trace.error_type {
            *error_type_stats.entry(error_type.clone()).or_insert(0) += 1;
        } else {
            *error_type_stats.entry("Unknown".to_string()).or_insert(0) += 1;
        }
    }

    // 按服务商统计错误
    let mut provider_error_stats: HashMap<String, (i64, i64)> = HashMap::new(); // (total, errors)
    for trace in &traces {
        let provider = "Unknown".to_string(); // provider_name field removed
        let entry = provider_error_stats.entry(provider).or_insert((0, 0));
        entry.0 += 1; // total requests
        if !trace.is_success {
            entry.1 += 1; // error count
        }
    }

    // 时间序列错误趋势
    let interval_hours = if hours <= 24 { 1 } else { 4 };
    let total_intervals = hours / interval_hours;
    let mut error_trend = Vec::new();
    let now = Utc::now();

    for i in 0..total_intervals {
        let interval_start =
            now - Duration::hours((total_intervals - i) as i64 * interval_hours as i64);
        let interval_end = interval_start + Duration::hours(interval_hours as i64);

        let interval_traces: Vec<&proxy_tracing::Model> = traces
            .iter()
            .filter(|t| {
                let trace_time = DateTime::<Utc>::from_naive_utc_and_offset(t.created_at, Utc);
                trace_time >= interval_start && trace_time < interval_end
            })
            .collect();

        let interval_total = interval_traces.len() as i64;
        let interval_errors = interval_traces.iter().filter(|t| !t.is_success).count() as i64;
        let error_rate = if interval_total > 0 {
            (interval_errors as f64 / interval_total as f64) * 100.0
        } else {
            0.0
        };

        error_trend.push(json!({
            "timestamp": interval_start.format("%Y-%m-%d %H:%M").to_string(),
            "error_count": interval_errors,
            "total_requests": interval_total,
            "error_rate": error_rate
        }));
    }

    // 热门错误列表（按发生次数排序）
    let mut top_errors: Vec<_> = error_type_stats.into_iter().collect();
    top_errors.sort_by(|a, b| b.1.cmp(&a.1));
    top_errors.truncate(10); // 只保留前10个

    let top_errors_list: Vec<Value> = top_errors
        .into_iter()
        .map(|(error_type, count)| {
            let percentage = if total_errors > 0 {
                (count as f64 / total_errors as f64) * 100.0
            } else {
                0.0
            };
            json!({
                "error_type": error_type,
                "count": count,
                "percentage": percentage
            })
        })
        .collect();

    // 服务商错误率
    let provider_error_rates: Vec<Value> = provider_error_stats
        .into_iter()
        .map(|(provider, (total, errors))| {
            let error_rate = if total > 0 {
                (errors as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            json!({
                "provider": provider,
                "total_requests": total,
                "error_count": errors,
                "error_rate": error_rate
            })
        })
        .collect();

    response::success(json!({
        "summary": {
            "total_requests": total_requests,
            "total_errors": total_errors,
            "error_rate": if total_requests > 0 { (total_errors as f64 / total_requests as f64) * 100.0 } else { 0.0 },
            "period": format!("{}h", hours)
        },
        "data": error_trend,
        "top_errors": top_errors_list,
        "by_provider": provider_error_rates
    }))
}

/// 获取单个请求日志详情
pub async fn get_request_log_detail(
    State(state): State<AppState>,
    axum::extract::Path(log_id): axum::extract::Path<i32>,
) -> impl IntoResponse {
    let trace = match ProxyTracing::find_by_id(log_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(trace)) => trace,
        Ok(None) => {
            return response::error(StatusCode::NOT_FOUND, "NOT_FOUND", "Request log not found")
        }
        Err(err) => {
            tracing::error!("Failed to fetch request log detail: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch data",
            );
        }
    };

    // 复杂字段功能已移除
    let phases: Vec<serde_json::Value> = Vec::new();
    let performance_metrics = serde_json::Value::Null;
    let labels: std::collections::HashMap<String, String> = std::collections::HashMap::new(); // Labels功能已移除

    // 构建详细响应（只包含数据库中实际存在的字段）
    let detail = json!({
        "id": trace.id,
        "request_id": trace.request_id,
        "method": trace.method,
        "path": trace.path,
        "provider_name": None::<String>,
        "start_time": trace.start_time,
        "end_time": trace.end_time,
        "created_at": trace.created_at,
        "duration_ms": trace.duration_ms,
        "response_time_ms": None::<i64>,
        "status_code": trace.status_code,
        "is_success": trace.is_success,
        "is_anomaly": None::<bool>,
        "health_impact_score": None::<f64>,
        "tokens": {
            "prompt": trace.tokens_prompt,
            "completion": trace.tokens_completion,
            "total": trace.tokens_total,
            "efficiency_ratio": trace.token_efficiency_ratio
        },
        "error": if trace.error_type.is_some() || trace.error_message.is_some() {
            json!({
                "type": trace.error_type,
                "message": trace.error_message
            })
        } else {
            Value::Null
        },
        "trace_level": 0,
        "phases": phases,
        "performance_metrics": performance_metrics,
        "labels": labels
    });

    response::success(detail)
}

/// 获取API密钥统计信息（总数和健康密钥数）
async fn get_api_keys_stats(db: &DatabaseConnection) -> Result<(i64, i64), sea_orm::DbErr> {
    use entity::{user_provider_keys, user_service_apis};

    // 统计Provider API密钥
    let total_provider_keys = user_provider_keys::Entity::find().count(db).await? as i64;

    let healthy_provider_keys = user_provider_keys::Entity::find()
        .filter(user_provider_keys::Column::IsActive.eq(true))
        .count(db)
        .await? as i64;

    // 统计Service API密钥
    let total_service_keys = user_service_apis::Entity::find().count(db).await? as i64;

    let healthy_service_keys = user_service_apis::Entity::find()
        .filter(user_service_apis::Column::IsActive.eq(true))
        .count(db)
        .await? as i64;

    let total_keys = total_provider_keys + total_service_keys;
    let healthy_keys = healthy_provider_keys + healthy_service_keys;

    Ok((total_keys, healthy_keys))
}
