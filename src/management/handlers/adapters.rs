//! # 适配器管理处理器

use crate::management::response;
use crate::management::server::AppState;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use entity::{
    provider_types, provider_types::Entity as ProviderTypes, user_service_apis,
    user_service_apis::Entity as UserServiceApis,
};
use sea_orm::{entity::*, query::*};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Serialize)]
struct Adapter {
    id: i32,
    name: String,
    display_name: String,
    upstream_type: String,
    base_url: String,
    default_model: Option<String>,
    max_tokens: Option<i32>,
    rate_limit: Option<i32>,
    timeout_seconds: Option<i32>,
    health_check_path: Option<String>,
    auth_header_format: Option<String>,
    status: String,
    version: &'static str,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// 适配器查询参数
#[derive(Debug, Deserialize)]
pub struct AdapterQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

/// 列出所有适配器（分页）
pub async fn list_adapters(
    State(state): State<AppState>,
    Query(query): Query<AdapterQuery>,
) -> impl IntoResponse {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = (page - 1) * limit;
    let provider_types = match ProviderTypes::find()
        .filter(provider_types::Column::IsActive.eq(true))
        .all(state.database.as_ref())
        .await
    {
        Ok(types) => types,
        Err(err) => {
            tracing::error!("Failed to fetch provider types: {}", err);
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch provider types",
            );
        }
    };

    let total = provider_types.len() as u64;

    let adapters: Vec<Adapter> = provider_types
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .map(|provider| Adapter {
            id: provider.id,
            name: provider.name,
            display_name: provider.display_name,
            upstream_type: provider.api_format,
            base_url: provider.base_url,
            default_model: provider.default_model,
            max_tokens: provider.max_tokens,
            rate_limit: provider.rate_limit,
            timeout_seconds: provider.timeout_seconds,
            health_check_path: provider.health_check_path,
            auth_header_format: provider.auth_header_format,
            status: if provider.is_active {
                "active"
            } else {
                "inactive"
            }
            .to_string(),
            version: "1.0.0", // Placeholder
            created_at: provider.created_at,
            updated_at: provider.updated_at,
        })
        .collect();

    let pagination = response::Pagination {
        page: page as u64,
        limit: limit as u64,
        total,
        pages: ((total as f64) / (limit as f64)).ceil() as u64,
    };

    response::paginated(adapters, pagination)
}

#[derive(Serialize)]
struct AdapterStatsResponse {
    summary: AdapterStatsSummary,
    by_type: HashMap<String, AdapterTypeStats>,
    detailed_stats: HashMap<String, DetailedAdapterStats>,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize)]
struct AdapterStatsSummary {
    total_adapters: usize,
    adapter_types: usize,
    total_active_configs: u64,
}

#[derive(Serialize, Default)]
struct AdapterTypeStats {
    adapters: u64,
    active_configs: u64,
    names: Vec<String>,
}

#[derive(Serialize)]
struct DetailedAdapterStats {
    id: i32,
    display_name: String,
    api_format: String,
    base_url: String,
    active_configurations: u64,
    runtime_info: Option<Value>, // Keeping as Value for now
    health_status: AdapterHealthStatus,
    rate_limit: Option<i32>,
    timeout_seconds: Option<i32>,
    last_updated: chrono::NaiveDateTime,
}

#[derive(Serialize)]
struct AdapterHealthStatus {
    status: String,
    last_check: Option<DateTime<Utc>>,
    response_time_ms: u64,
    success_rate: f64,
    healthy_servers: usize,
    total_servers: usize,
    is_healthy: bool,
    details: String,
    servers: Vec<ServerHealth>,
}

#[derive(Serialize)]
struct ServerHealth {
    server: String,
    status: String,
    last_check: Option<DateTime<Utc>>,
    response_time_ms: u128,
    consecutive_failures: u32,
    is_healthy: bool,
}

/// 获取适配器统计信息
pub async fn get_adapter_stats(State(state): State<AppState>) -> impl IntoResponse {
    let provider_types = match ProviderTypes::find()
        .filter(provider_types::Column::IsActive.eq(true))
        .all(state.database.as_ref())
        .await
    {
        Ok(types) => types,
        Err(err) => {
            tracing::error!("Failed to fetch provider types: {}", err);
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch provider types",
            );
        }
    };

    let mut provider_usage_stats = HashMap::new();
    for provider in &provider_types {
        let usage_count = match UserServiceApis::find()
            .filter(user_service_apis::Column::ProviderTypeId.eq(provider.id))
            .filter(user_service_apis::Column::IsActive.eq(true))
            .count(state.database.as_ref())
            .await
        {
            Ok(count) => count,
            Err(err) => {
                tracing::warn!(
                    "Failed to get usage count for provider {}: {}",
                    provider.name,
                    err
                );
                0
            }
        };
        provider_usage_stats.insert(provider.id, usage_count);
    }

    let adapter_stats = state.adapter_manager.get_adapter_stats().await;

    let mut by_type: HashMap<String, AdapterTypeStats> = HashMap::new();
    let mut detailed_stats = HashMap::new();

    for provider in &provider_types {
        let usage_count = *provider_usage_stats.get(&provider.id).unwrap_or(&0);

        let type_entry = by_type.entry(provider.api_format.clone()).or_default();
        type_entry.adapters += 1;
        type_entry.active_configs += usage_count;
        type_entry.names.push(provider.name.clone());

        detailed_stats.insert(
            provider.name.clone(),
            DetailedAdapterStats {
                id: provider.id,
                display_name: provider.display_name.clone(),
                api_format: provider.api_format.clone(),
                base_url: provider.base_url.clone(),
                active_configurations: usage_count,
                runtime_info: adapter_stats.get(&provider.name).map(|s| json!(s)),
                health_status: get_adapter_health_status(&state, &provider.name).await,
                rate_limit: provider.rate_limit,
                timeout_seconds: provider.timeout_seconds,
                last_updated: provider.updated_at,
            },
        );
    }

    let response_data = AdapterStatsResponse {
        summary: AdapterStatsSummary {
            total_adapters: provider_types.len(),
            adapter_types: by_type.len(),
            total_active_configs: provider_usage_stats.values().sum(),
        },
        by_type,
        detailed_stats,
        timestamp: Utc::now(),
    };

    response::success(response_data)
}

/// 获取适配器健康状态
async fn get_adapter_health_status(state: &AppState, provider_name: &str) -> AdapterHealthStatus {
    let all_health_status = state.health_service.get_all_health_status().await;

    let mut matching_servers = Vec::new();
    let mut total_response_time = 0u64;
    let mut healthy_count = 0;
    let mut total_count = 0;

    for (server_address, health_status) in all_health_status {
        if server_address.contains(provider_name)
            || server_address.contains(&provider_name.to_lowercase())
        {
            total_count += 1;
            total_response_time += health_status.avg_response_time.as_millis() as u64;

            if health_status.is_healthy {
                healthy_count += 1;
            }

            matching_servers.push(ServerHealth {
                server: server_address.clone(),
                status: if health_status.is_healthy {
                    "healthy"
                } else {
                    "unhealthy"
                }
                .to_string(),
                last_check: health_status.last_check.map(|t| {
                    Utc::now() - chrono::Duration::from_std(t.elapsed()).unwrap_or_default()
                }),
                response_time_ms: health_status.avg_response_time.as_millis(),
                consecutive_failures: health_status.consecutive_failures,
                is_healthy: health_status.is_healthy,
            });
        }
    }

    if total_count == 0 {
        return AdapterHealthStatus {
            status: "no_servers".to_string(),
            last_check: None,
            response_time_ms: 0,
            success_rate: 0.0,
            healthy_servers: 0,
            total_servers: 0,
            is_healthy: false,
            details: "No health check servers found for this provider".to_string(),
            servers: vec![],
        };
    }

    let avg_response_time = if total_count > 0 {
        total_response_time / total_count as u64
    } else {
        0
    };
    let success_rate = if total_count > 0 {
        (healthy_count as f64 / total_count as f64) * 100.0
    } else {
        0.0
    };
    let overall_status = if healthy_count == total_count {
        "healthy"
    } else if healthy_count == 0 {
        "unhealthy"
    } else {
        "degraded"
    }
    .to_string();

    AdapterHealthStatus {
        status: overall_status,
        last_check: matching_servers.first().and_then(|s| s.last_check),
        response_time_ms: avg_response_time,
        success_rate,
        healthy_servers: healthy_count,
        total_servers: total_count,
        is_healthy: healthy_count > 0,
        details: format!("{}/{} servers healthy", healthy_count, total_count),
        servers: matching_servers,
    }
}
