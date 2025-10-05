//! # 日志管理处理器
//!
//! 基于 proxy_tracing 表的日志查询、统计和分析功能

use crate::logging::{LogComponent, LogStage};
use crate::management::middleware::auth::AuthContext;
use crate::management::response::ApiResponse;
use crate::management::server::AppState;
use crate::{lerror, linfo};
use ::entity::proxy_tracing;
use ::entity::{ProviderTypes, ProxyTracing, UserProviderKeys};
use axum::{
    extract::{Extension, Path, Query, State},
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// 日志仪表板统计响应
#[derive(Debug, Serialize)]
pub struct LogsDashboardStatsResponse {
    pub total_requests: i64,
    pub successful_requests: i64,
    pub failed_requests: i64,
    pub success_rate: f64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub avg_response_time: i64,
}

/// 代理跟踪日志条目
#[derive(Debug, Serialize)]
pub struct ProxyTraceEntry {
    pub id: i32,
    pub request_id: String,
    pub user_service_api_id: i32,
    pub user_provider_key_id: Option<i32>,
    pub user_id: Option<i32>,
    pub method: String,
    pub path: Option<String>,
    pub status_code: Option<i32>,
    pub tokens_prompt: i32,
    pub tokens_completion: i32,
    pub tokens_total: i32,
    pub token_efficiency_ratio: Option<f64>,
    pub cache_create_tokens: i32,
    pub cache_read_tokens: i32,
    pub cost: Option<f64>,
    pub cost_currency: String,
    pub model_used: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub provider_type_id: Option<i32>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub is_success: bool,
    pub created_at: DateTime<Utc>,
    pub provider_name: Option<String>,
    pub provider_key_name: Option<String>,
}

/// 日志列表响应
#[derive(Debug, Serialize)]
pub struct LogsListResponse {
    pub traces: Vec<ProxyTraceEntry>,
    pub pagination: PaginationInfo,
}

/// 分页信息
#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    pub page: u64,
    pub limit: u64,
    pub total: u64,
    pub pages: u64,
}

/// 日志分析响应
#[derive(Debug, Serialize)]
pub struct LogsAnalyticsResponse {
    pub time_series: Vec<TimeSeriesData>,
    pub model_distribution: Vec<ModelDistribution>,
    pub provider_distribution: Vec<ProviderDistribution>,
    pub status_distribution: Vec<StatusDistribution>,
}

#[derive(Debug, Serialize)]
pub struct TimeSeriesData {
    pub timestamp: DateTime<Utc>,
    pub total_requests: i64,
    pub successful_requests: i64,
    pub failed_requests: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub avg_response_time: i64,
}

#[derive(Debug, Serialize)]
pub struct ModelDistribution {
    pub model: String,
    pub request_count: i64,
    pub token_count: i64,
    pub cost: f64,
    pub percentage: f64,
}

#[derive(Debug, Serialize)]
pub struct ProviderDistribution {
    pub provider_name: String,
    pub request_count: i64,
    pub success_rate: f64,
    pub avg_response_time: i64,
}

#[derive(Debug, Serialize)]
pub struct StatusDistribution {
    pub status_code: i32,
    pub count: i64,
    pub percentage: f64,
}

/// 日志列表查询参数
#[derive(Debug, Deserialize)]
pub struct LogsListQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
    pub search: Option<String>,
    pub method: Option<String>,
    pub status_code: Option<i32>,
    pub is_success: Option<bool>,
    pub model_used: Option<String>,
    pub provider_type_id: Option<i32>,
    pub user_service_api_id: Option<i32>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
}

/// 日志分析查询参数
#[derive(Debug, Deserialize)]
pub struct LogsAnalyticsQuery {
    pub time_range: Option<String>, // 1h, 6h, 24h, 7d, 30d
    pub group_by: Option<String>,   // hour, day, model, provider, status
}

/// 获取日志仪表板统计数据
#[allow(clippy::similar_names)]
pub async fn get_dashboard_stats(
    State(state): State<AppState>,
    Extension(_auth_context): Extension<Arc<AuthContext>>,
) -> impl IntoResponse {
    match calculate_dashboard_stats(&state.database).await {
        Ok(stats) => ApiResponse::Success(stats).into_response(),
        Err(e) => {
            lerror!(
                "system",
                LogStage::Internal,
                LogComponent::Statistics,
                "dashboard_stats_fail",
                &format!("获取日志仪表板统计失败: {}", e)
            );
            crate::management::response::app_error(crate::proxy_err!(
                database,
                "获取统计数据失败: {}",
                e
            ))
        }
    }
}

/// 计算仪表板统计数据
async fn calculate_dashboard_stats(
    db: &DatabaseConnection,
) -> Result<LogsDashboardStatsResponse, DbErr> {
    // 获取总请求数
    let total_requests = ProxyTracing::find().count(db).await? as i64;

    // 获取成功请求数
    let successful_requests = ProxyTracing::find()
        .filter(proxy_tracing::Column::IsSuccess.eq(true))
        .count(db)
        .await? as i64;

    // 计算失败请求数
    let failed_requests = total_requests - successful_requests;

    // 计算成功率
    let success_rate = if total_requests > 0 {
        (successful_requests as f64 / total_requests as f64) * 100.0
    } else {
        0.0
    };

    // 获取总token数（使用聚合查询）
    let token_result = ProxyTracing::find()
        .select_only()
        .column_as(proxy_tracing::Column::TokensTotal.sum(), "total_tokens")
        .into_tuple::<Option<i64>>()
        .one(db)
        .await?;
    let total_tokens = token_result.flatten().unwrap_or(0);

    // 获取总费用
    let cost_result = ProxyTracing::find()
        .select_only()
        .column_as(proxy_tracing::Column::Cost.sum(), "total_cost")
        .into_tuple::<Option<f64>>()
        .one(db)
        .await?;
    let total_cost = cost_result.flatten().unwrap_or(0.0);

    // 获取平均响应时间
    let duration_result = ProxyTracing::find()
        .filter(proxy_tracing::Column::DurationMs.is_not_null())
        .select_only()
        .column_as(proxy_tracing::Column::DurationMs.sum(), "total_duration")
        .column_as(proxy_tracing::Column::Id.count(), "request_count")
        .into_tuple::<(Option<i64>, i64)>()
        .one(db)
        .await?;

    let avg_response_time = match duration_result {
        Some((Some(total_duration), count)) if count > 0 => total_duration / count,
        _ => 0,
    };

    Ok(LogsDashboardStatsResponse {
        total_requests,
        successful_requests,
        failed_requests,
        success_rate: (success_rate * 100.0).round() / 100.0, // 保留两位小数
        total_tokens,
        total_cost: (total_cost * 100.0).round() / 100.0, // 保留两位小数
        avg_response_time,
    })
}

/// 获取日志列表
pub async fn get_traces_list(
    State(state): State<AppState>,
    Query(query): Query<LogsListQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> impl IntoResponse {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20).min(100); // 限制最大每页100条

    match fetch_traces_list(
        &state.database,
        &query,
        page,
        limit,
        auth_context.user_id,
        auth_context.is_admin,
    )
    .await
    {
        Ok(response) => ApiResponse::Success(response).into_response(),
        Err(e) => {
            lerror!(
                "system",
                LogStage::Internal,
                LogComponent::Tracing,
                "get_traces_fail",
                &format!("获取日志列表失败: {}", e)
            );
            crate::management::response::app_error(crate::proxy_err!(
                database,
                "获取日志列表失败: {}",
                e
            ))
        }
    }
}

/// 获取日志列表数据
async fn fetch_traces_list(
    db: &DatabaseConnection,
    query: &LogsListQuery,
    page: u64,
    limit: u64,
    current_user_id: i32,
    is_admin: bool,
) -> Result<LogsListResponse, DbErr> {
    // 构建基础查询
    let mut select = ProxyTracing::find();

    // 权限控制：非管理员只能查看自己的日志记录
    if !is_admin {
        select = select.filter(proxy_tracing::Column::UserId.eq(current_user_id));
        linfo!(
            "system",
            LogStage::Internal,
            LogComponent::Tracing,
            "non_admin_access",
            &format!(
                "Non-admin user {} accessing traces - filtering by user_id",
                current_user_id
            )
        );
    } else {
        linfo!(
            "system",
            LogStage::Internal,
            LogComponent::Tracing,
            "admin_access",
            &format!("Admin user {} accessing all traces", current_user_id)
        );
    }

    // 应用搜索过滤
    if let Some(search) = &query.search {
        if !search.trim().is_empty() {
            let search_pattern = format!("%{}%", search.trim());
            select = select.filter(
                Condition::any()
                    .add(proxy_tracing::Column::RequestId.like(&search_pattern))
                    .add(proxy_tracing::Column::Path.like(&search_pattern))
                    .add(proxy_tracing::Column::ModelUsed.like(&search_pattern)),
            );
        }
    }

    // 应用方法过滤
    if let Some(method) = &query.method {
        select = select.filter(proxy_tracing::Column::Method.eq(method));
    }

    // 应用状态码过滤
    if let Some(status_code) = query.status_code {
        select = select.filter(proxy_tracing::Column::StatusCode.eq(status_code));
    }

    // 应用成功状态过滤
    if let Some(is_success) = query.is_success {
        select = select.filter(proxy_tracing::Column::IsSuccess.eq(is_success));
    }

    // 应用模型过滤
    if let Some(model_used) = &query.model_used {
        select = select.filter(proxy_tracing::Column::ModelUsed.eq(model_used));
    }

    // 应用服务商类型过滤
    if let Some(provider_type_id) = query.provider_type_id {
        select = select.filter(proxy_tracing::Column::ProviderTypeId.eq(provider_type_id));
    }

    // 应用用户服务API过滤
    if let Some(user_service_api_id) = query.user_service_api_id {
        select = select.filter(proxy_tracing::Column::UserServiceApiId.eq(user_service_api_id));
    }

    // 应用时间范围过滤
    if let Some(start_time) = query.start_time {
        select = select.filter(proxy_tracing::Column::CreatedAt.gte(start_time));
    }
    if let Some(end_time) = query.end_time {
        select = select.filter(proxy_tracing::Column::CreatedAt.lte(end_time));
    }

    // 获取总数
    let total = select.clone().count(db).await? as u64;

    // 应用分页和排序
    let offset = (page - 1) * limit;
    let traces_models = select
        .order_by_desc(proxy_tracing::Column::CreatedAt)
        .offset(offset)
        .limit(limit)
        .find_with_related(ProviderTypes)
        .all(db)
        .await?;

    // 转换为响应格式
    let mut traces = Vec::new();
    for (trace_model, provider_types) in traces_models {
        let provider_name = provider_types.first().map(|pt| pt.display_name.clone());

        traces.push(ProxyTraceEntry {
            id: trace_model.id,
            request_id: trace_model.request_id,
            user_service_api_id: trace_model.user_service_api_id,
            user_provider_key_id: trace_model.user_provider_key_id,
            user_id: trace_model.user_id,
            method: trace_model.method,
            path: trace_model.path,
            status_code: trace_model.status_code,
            tokens_prompt: trace_model.tokens_prompt.unwrap_or(0),
            tokens_completion: trace_model.tokens_completion.unwrap_or(0),
            tokens_total: trace_model.tokens_total.unwrap_or(0),
            token_efficiency_ratio: trace_model.token_efficiency_ratio,
            cache_create_tokens: trace_model.cache_create_tokens.unwrap_or(0),
            cache_read_tokens: trace_model.cache_read_tokens.unwrap_or(0),
            cost: trace_model.cost,
            cost_currency: trace_model
                .cost_currency
                .unwrap_or_else(|| "USD".to_string()),
            model_used: trace_model.model_used,
            client_ip: trace_model.client_ip,
            user_agent: trace_model.user_agent,
            error_type: trace_model.error_type,
            error_message: trace_model.error_message,
            retry_count: trace_model.retry_count.unwrap_or(0),
            provider_type_id: trace_model.provider_type_id,
            start_time: trace_model.start_time.map(|dt| dt.and_utc()),
            end_time: trace_model.end_time.map(|dt| dt.and_utc()),
            duration_ms: trace_model.duration_ms,
            is_success: trace_model.is_success,
            created_at: trace_model.created_at.and_utc(),
            provider_name,
            provider_key_name: None, // TODO: 如果需要，可以添加关联查询
        });
    }

    // 计算总页数
    let pages = (total + limit - 1) / limit;

    Ok(LogsListResponse {
        traces,
        pagination: PaginationInfo {
            page,
            limit,
            total,
            pages,
        },
    })
}

/// 获取日志详情
pub async fn get_trace_detail(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Extension(_auth_context): Extension<Arc<AuthContext>>,
) -> impl IntoResponse {
    match fetch_trace_detail(&state.database, id).await {
        Ok(Some(trace)) => ApiResponse::Success(trace).into_response(),
        Ok(None) => crate::management::response::app_error(crate::proxy_err!(
            business,
            "ProxyTrace not found: {}",
            id
        )),
        Err(e) => {
            lerror!(
                "system",
                LogStage::Internal,
                LogComponent::Tracing,
                "get_trace_detail_fail",
                &format!("获取日志详情失败: {}", e)
            );
            crate::management::response::app_error(crate::proxy_err!(
                database,
                "获取日志详情失败: {}",
                e
            ))
        }
    }
}

/// 获取日志详情数据
async fn fetch_trace_detail(
    db: &DatabaseConnection,
    id: i32,
) -> Result<Option<ProxyTraceEntry>, DbErr> {
    // 查询日志记录及关联的服务商类型和用户提供商密钥信息
    let trace_with_relations = ProxyTracing::find_by_id(id)
        .find_with_related(ProviderTypes)
        .all(db)
        .await?;

    if let Some((trace_model, provider_types)) = trace_with_relations.into_iter().next() {
        let provider_name = provider_types.first().map(|pt| pt.display_name.clone());

        // 如果需要，可以额外查询用户提供商密钥信息
        let provider_key_name = if let Some(provider_key_id) = trace_model.user_provider_key_id {
            UserProviderKeys::find_by_id(provider_key_id)
                .one(db)
                .await?
                .map(|pk| pk.name)
        } else {
            None
        };

        let trace_entry = ProxyTraceEntry {
            id: trace_model.id,
            request_id: trace_model.request_id,
            user_service_api_id: trace_model.user_service_api_id,
            user_provider_key_id: trace_model.user_provider_key_id,
            user_id: trace_model.user_id,
            method: trace_model.method,
            path: trace_model.path,
            status_code: trace_model.status_code,
            tokens_prompt: trace_model.tokens_prompt.unwrap_or(0),
            tokens_completion: trace_model.tokens_completion.unwrap_or(0),
            tokens_total: trace_model.tokens_total.unwrap_or(0),
            token_efficiency_ratio: trace_model.token_efficiency_ratio,
            cache_create_tokens: trace_model.cache_create_tokens.unwrap_or(0),
            cache_read_tokens: trace_model.cache_read_tokens.unwrap_or(0),
            cost: trace_model.cost,
            cost_currency: trace_model
                .cost_currency
                .unwrap_or_else(|| "USD".to_string()),
            model_used: trace_model.model_used,
            client_ip: trace_model.client_ip,
            user_agent: trace_model.user_agent,
            error_type: trace_model.error_type,
            error_message: trace_model.error_message,
            retry_count: trace_model.retry_count.unwrap_or(0),
            provider_type_id: trace_model.provider_type_id,
            start_time: trace_model.start_time.map(|dt| dt.and_utc()),
            end_time: trace_model.end_time.map(|dt| dt.and_utc()),
            duration_ms: trace_model.duration_ms,
            is_success: trace_model.is_success,
            created_at: trace_model.created_at.and_utc(),
            provider_name,
            provider_key_name,
        };

        Ok(Some(trace_entry))
    } else {
        Ok(None)
    }
}

/// 获取日志统计分析
pub async fn get_logs_analytics(
    State(state): State<AppState>,
    Query(query): Query<LogsAnalyticsQuery>,
    Extension(_auth_context): Extension<Arc<AuthContext>>,
) -> impl IntoResponse {
    let time_range = query.time_range.as_deref().unwrap_or("24h");
    let group_by = query.group_by.as_deref().unwrap_or("hour");

    match fetch_logs_analytics(&state.database, time_range, group_by).await {
        Ok(response) => ApiResponse::Success(response).into_response(),
        Err(e) => {
            lerror!(
                "system",
                LogStage::Internal,
                LogComponent::Statistics,
                "analytics_fail",
                &format!("获取日志统计分析失败: {}", e)
            );
            crate::management::response::app_error(crate::proxy_err!(
                database,
                "获取统计分析失败: {}",
                e
            ))
        }
    }
}

/// 获取日志统计分析数据
async fn fetch_logs_analytics(
    db: &DatabaseConnection,
    time_range: &str,
    _group_by: &str,
) -> Result<LogsAnalyticsResponse, DbErr> {
    // 计算时间范围
    let now = Utc::now();
    let start_time = match time_range {
        "1h" => now - chrono::Duration::hours(1),
        "6h" => now - chrono::Duration::hours(6),
        "24h" => now - chrono::Duration::hours(24),
        "7d" => now - chrono::Duration::days(7),
        "30d" => now - chrono::Duration::days(30),
        _ => now - chrono::Duration::hours(24),
    };

    // 基础查询条件
    let base_query = ProxyTracing::find().filter(proxy_tracing::Column::CreatedAt.gte(start_time));

    // 1. 获取模型分布统计
    let model_stats = base_query
        .clone()
        .filter(proxy_tracing::Column::ModelUsed.is_not_null())
        .select_only()
        .column(proxy_tracing::Column::ModelUsed)
        .column_as(proxy_tracing::Column::Id.count(), "request_count")
        .column_as(proxy_tracing::Column::TokensTotal.sum(), "token_count")
        .column_as(proxy_tracing::Column::Cost.sum(), "cost")
        .group_by(proxy_tracing::Column::ModelUsed)
        .into_tuple::<(Option<String>, i64, Option<i64>, Option<f64>)>()
        .all(db)
        .await?;

    let total_requests = base_query.clone().count(db).await? as i64;

    let model_distribution: Vec<ModelDistribution> = model_stats
        .into_iter()
        .map(|(model, request_count, token_count, cost)| {
            let percentage = if total_requests > 0 {
                (request_count as f64 / total_requests as f64) * 100.0
            } else {
                0.0
            };

            ModelDistribution {
                model: model.unwrap_or_else(|| "Unknown".to_string()),
                request_count,
                token_count: token_count.unwrap_or(0),
                cost: cost.unwrap_or(0.0),
                percentage: (percentage * 100.0).round() / 100.0,
            }
        })
        .collect();

    // 2. 获取服务商分布统计
    let provider_stats = base_query
        .clone()
        .find_with_related(ProviderTypes)
        .all(db)
        .await?;

    let mut provider_map: HashMap<String, (i64, i64, i64)> = HashMap::new(); // (total, success, total_duration)

    for (trace, provider_types) in provider_stats {
        let provider_name = provider_types
            .first()
            .map(|pt| pt.display_name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let entry = provider_map.entry(provider_name).or_insert((0, 0, 0));
        entry.0 += 1; // total requests
        if trace.is_success {
            entry.1 += 1; // successful requests
        }
        if let Some(duration) = trace.duration_ms {
            entry.2 += duration; // total duration
        }
    }

    let provider_distribution: Vec<ProviderDistribution> = provider_map
        .into_iter()
        .map(|(provider_name, (total, success, total_duration))| {
            let success_rate = if total > 0 {
                (success as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            let avg_response_time = if total > 0 { total_duration / total } else { 0 };

            ProviderDistribution {
                provider_name,
                request_count: total,
                success_rate: (success_rate * 100.0).round() / 100.0,
                avg_response_time,
            }
        })
        .collect();

    // 3. 获取状态码分布统计
    let status_stats = base_query
        .clone()
        .filter(proxy_tracing::Column::StatusCode.is_not_null())
        .select_only()
        .column(proxy_tracing::Column::StatusCode)
        .column_as(proxy_tracing::Column::Id.count(), "count")
        .group_by(proxy_tracing::Column::StatusCode)
        .into_tuple::<(Option<i32>, i64)>()
        .all(db)
        .await?;

    let status_distribution: Vec<StatusDistribution> = status_stats
        .into_iter()
        .map(|(status_code, count)| {
            let percentage = if total_requests > 0 {
                (count as f64 / total_requests as f64) * 100.0
            } else {
                0.0
            };

            StatusDistribution {
                status_code: status_code.unwrap_or(0),
                count,
                percentage: (percentage * 100.0).round() / 100.0,
            }
        })
        .collect();

    // 4. 时间序列数据（简化版本）
    // TODO: 根据 group_by 参数实现更精细的时间分组
    let time_series = vec![TimeSeriesData {
        timestamp: now,
        total_requests,
        successful_requests: base_query
            .clone()
            .filter(proxy_tracing::Column::IsSuccess.eq(true))
            .count(db)
            .await? as i64,
        failed_requests: base_query
            .clone()
            .filter(proxy_tracing::Column::IsSuccess.eq(false))
            .count(db)
            .await? as i64,
        total_tokens: base_query
            .clone()
            .select_only()
            .column_as(proxy_tracing::Column::TokensTotal.sum(), "total_tokens")
            .into_tuple::<Option<i64>>()
            .one(db)
            .await?
            .flatten()
            .unwrap_or(0),
        total_cost: base_query
            .clone()
            .select_only()
            .column_as(proxy_tracing::Column::Cost.sum(), "total_cost")
            .into_tuple::<Option<f64>>()
            .one(db)
            .await?
            .flatten()
            .unwrap_or(0.0),
        avg_response_time: {
            let duration_result = base_query
                .clone()
                .filter(proxy_tracing::Column::DurationMs.is_not_null())
                .select_only()
                .column_as(proxy_tracing::Column::DurationMs.sum(), "total_duration")
                .column_as(proxy_tracing::Column::Id.count(), "request_count")
                .into_tuple::<(Option<i64>, i64)>()
                .one(db)
                .await?;

            match duration_result {
                Some((Some(total_duration), count)) if count > 0 => total_duration / count,
                _ => 0,
            }
        },
    }];

    Ok(LogsAnalyticsResponse {
        time_series,
        model_distribution,
        provider_distribution,
        status_distribution,
    })
}
