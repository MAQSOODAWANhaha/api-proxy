//! API密钥健康检查相关处理器

use crate::manage_error;
use crate::management::{response, server::AppState};
use crate::scheduler::api_key_health::ApiKeyHealthChecker;
use crate::{
    lerror,
    logging::{LogComponent, LogStage},
    lwarn,
};
use axum::extract::{Path, State};
use entity::{provider_types, user_provider_keys};
// use pingora_http::StatusCode; // no longer needed with manage_error!
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use sea_orm::{FromQueryResult, PaginatorTrait, QuerySelect};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::sync::Arc;

/// API密钥健康检查信息
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyHealthInfo {
    /// 密钥ID
    pub key_id: i32,
    /// 提供商名称
    pub provider_name: String,
    /// 密钥名称（如果有）
    pub key_name: Option<String>,
    /// 是否健康
    pub is_healthy: bool,
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: u64,
    /// 健康分数 (0-100)
    pub health_score: f32,
    /// 最后检查时间
    pub last_check_time: Option<String>,
    /// 最后健康时间
    pub last_healthy_time: Option<String>,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 连续成功次数
    pub consecutive_successes: u32,
    /// 最后错误信息
    pub error_message: Option<String>,
}

/// 健康检查统计信息
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthCheckStats {
    /// 总密钥数量
    pub total_keys: usize,
    /// 健康密钥数量
    pub healthy_keys: usize,
    /// 不健康密钥数量
    pub unhealthy_keys: usize,
    /// 健康检查服务是否运行中
    pub health_check_running: bool,
    /// 按提供商分组的统计
    pub provider_stats: Vec<ProviderHealthStats>,
}

/// 提供商健康统计
#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderHealthStats {
    /// 提供商名称
    pub provider_name: String,
    /// 该提供商的密钥总数
    pub total_keys: usize,
    /// 该提供商的健康密钥数
    pub healthy_keys: usize,
    /// 平均健康分数
    pub avg_health_score: f64,
}

/// 简单健康检查处理器（系统存活检查）
pub async fn health_check(State(state): State<AppState>) -> axum::response::Response {
    match state.database.ping().await {
        Ok(()) => response::success(serde_json::json!({
            "status": "healthy",
            "database": "connected",
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
        Err(e) => {
            lwarn!(
                "system",
                LogStage::HealthCheck,
                LogComponent::Database,
                "db_ping_fail",
                &format!("Database ping failed: {e}")
            );
            manage_error!(crate::proxy_err!(database, "数据库连接失败: {}", e))
        }
    }
}

/// 详细健康检查处理器（系统详细状态）
#[derive(Serialize)]
struct DetailedHealthStatus {
    database: String,
    providers_active: u64,
    user_service_apis_active: u64,
    last_trace_time: Option<String>,
    system_info: SystemInfo,
    api_key_health: String,
}

#[derive(Serialize)]
struct SystemInfo {
    uptime: String,
    timestamp: String,
    version: String,
}

#[derive(FromQueryResult, Debug)]
struct LastTraceRow {
    last_end: Option<chrono::NaiveDateTime>,
    last_start: Option<chrono::NaiveDateTime>,
}

pub async fn detailed_health_check(State(state): State<AppState>) -> axum::response::Response {
    // 数据库连接状态
    let database_status = match state.database.ping().await {
        Ok(()) => "connected".to_string(),
        Err(e) => {
            lwarn!(
                "system",
                LogStage::HealthCheck,
                LogComponent::Database,
                "db_ping_fail",
                &format!("Database ping failed: {e}")
            );
            "disconnected".to_string()
        }
    };

    // 活跃 provider 与 user_service_apis 计数
    let providers_active = entity::provider_types::Entity::find()
        .filter(entity::provider_types::Column::IsActive.eq(true))
        .count(&*state.database)
        .await
        .unwrap_or(0);

    let user_service_apis_active = entity::user_service_apis::Entity::find()
        .filter(entity::user_service_apis::Column::IsActive.eq(true))
        .count(&*state.database)
        .await
        .unwrap_or(0);

    let last_row = entity::proxy_tracing::Entity::find()
        .select_only()
        .expr_as(
            sea_orm::sea_query::Expr::col(entity::proxy_tracing::Column::EndTime).max(),
            "last_end",
        )
        .expr_as(
            sea_orm::sea_query::Expr::col(entity::proxy_tracing::Column::StartTime).max(),
            "last_start",
        )
        .into_model::<LastTraceRow>()
        .one(&*state.database)
        .await
        .ok()
        .flatten();

    let last_trace_time = last_row
        .and_then(|r| r.last_end.or(r.last_start))
        .map(|dt| {
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc).to_rfc3339()
        });

    let detailed_status = DetailedHealthStatus {
        database: database_status,
        providers_active,
        user_service_apis_active,
        last_trace_time,
        system_info: SystemInfo {
            uptime: "unknown".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        api_key_health: "Available via /api/health/api-keys endpoint".to_string(),
    };

    response::success(detailed_status)
}

/// 获取所有API密钥健康状态
pub async fn get_api_keys_health(State(state): State<AppState>) -> axum::response::Response {
    // 注意：这需要从AppState中获取ApiKeyHealthChecker
    // 当前我们需要通过数据库查询来获取健康状态
    match get_api_keys_health_internal(&state).await {
        Ok(health_infos) => response::success(health_infos),
        Err(err) => {
            lerror!(
                "system",
                LogStage::HealthCheck,
                LogComponent::HealthChecker,
                "get_api_keys_health_fail",
                &format!("Failed to get API keys health status: {err}")
            );
            crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to retrieve API keys health status: {}",
                err
            ))
        }
    }
}

/// 获取健康检查统计信息
#[allow(clippy::similar_names)]
pub async fn get_health_stats(State(state): State<AppState>) -> axum::response::Response {
    match get_health_stats_internal(&state).await {
        Ok(stats) => response::success(stats),
        Err(err) => {
            lerror!(
                "system",
                LogStage::HealthCheck,
                LogComponent::HealthChecker,
                "get_health_stats_fail",
                &format!("Failed to get health statistics: {err}")
            );
            crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to retrieve health statistics: {}",
                err
            ))
        }
    }
}

/// 手动触发指定API密钥的健康检查
pub async fn trigger_key_health_check(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
) -> axum::response::Response {
    match trigger_key_health_check_internal(&state, key_id).await {
        Ok(check_result) => response::success(check_result),
        Err(err) => {
            lerror!(
                "system",
                LogStage::HealthCheck,
                LogComponent::HealthChecker,
                "trigger_health_check_fail",
                &format!("Failed to trigger health check for key {key_id}: {err}")
            );
            crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to trigger health check for key {}: {}",
                key_id,
                err
            ))
        }
    }
}

/// 标记API密钥为不健康
pub async fn mark_key_unhealthy(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
) -> axum::response::Response {
    let reason = "Manually marked unhealthy via management API".to_string();
    match mark_key_unhealthy_internal(&state, key_id, reason).await {
        Ok(()) => response::success("API key marked as unhealthy"),
        Err(err) => {
            lerror!(
                "system",
                LogStage::HealthCheck,
                LogComponent::HealthChecker,
                "mark_key_unhealthy_fail",
                &format!("Failed to mark key {key_id} as unhealthy: {err}")
            );
            crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to mark key {} as unhealthy: {}",
                key_id,
                err
            ))
        }
    }
}

// 内部实现函数

async fn get_api_keys_health_internal(state: &AppState) -> anyhow::Result<Vec<ApiKeyHealthInfo>> {
    // 从数据库获取所有活跃的API密钥
    let active_keys = user_provider_keys::Entity::find()
        .filter(user_provider_keys::Column::IsActive.eq(true))
        .all(&*state.database)
        .await?;

    if active_keys.is_empty() {
        return Ok(vec![]);
    }

    // 获取所有提供商类型信息
    let provider_types_map = {
        let provider_types = provider_types::Entity::find().all(&*state.database).await?;
        provider_types
            .into_iter()
            .map(|pt| (pt.id, pt))
            .collect::<std::collections::HashMap<_, _>>()
    };

    let mut health_infos = Vec::new();

    // 使用共享的健康检查器，如果不存在则创建临时的
    let health_checker = state.api_key_health_checker.as_ref().map_or_else(
        || Arc::new(ApiKeyHealthChecker::new(state.database.clone(), None)),
        std::clone::Clone::clone,
    );

    for key in active_keys {
        let provider_info = provider_types_map.get(&key.provider_type_id);
        let provider_name = provider_info.map_or_else(
            || format!("Provider {}", key.provider_type_id),
            |p| p.display_name.clone(),
        );

        // 获取健康状态
        let health_status = health_checker.get_key_health_status(key.id).await;

        let health_info = if let Some(status) = health_status {
            ApiKeyHealthInfo {
                key_id: key.id,
                provider_name,
                key_name: Some(key.name.clone()),
                is_healthy: status.is_healthy,
                avg_response_time_ms: status.avg_response_time_ms,
                health_score: status.health_score,
                last_check_time: status.last_check.map(|t| t.to_rfc3339()),
                last_healthy_time: status.last_healthy.map(|t| t.to_rfc3339()),
                consecutive_failures: status.consecutive_failures,
                consecutive_successes: status.consecutive_successes,
                error_message: status.last_error,
            }
        } else {
            // 没有健康状态记录，可能是新添加的密钥
            ApiKeyHealthInfo {
                key_id: key.id,
                provider_name,
                key_name: Some(key.name.clone()),
                is_healthy: true, // 默认认为是健康的
                avg_response_time_ms: 0,
                health_score: 100.0,
                last_check_time: None,
                last_healthy_time: None,
                consecutive_failures: 0,
                consecutive_successes: 0,
                error_message: None,
            }
        };

        health_infos.push(health_info);
    }

    Ok(health_infos)
}

async fn get_health_stats_internal(state: &AppState) -> anyhow::Result<HealthCheckStats> {
    let health_infos = get_api_keys_health_internal(state).await?;

    let total_keys = health_infos.len();
    let healthy_keys = health_infos.iter().filter(|h| h.is_healthy).count();
    let unhealthy_keys = total_keys - healthy_keys;

    // 按提供商分组统计
    let mut provider_stats_map = std::collections::HashMap::new();

    for info in &health_infos {
        let entry = provider_stats_map
            .entry(info.provider_name.clone())
            .or_insert((0, 0, Vec::new())); // (total, healthy, scores)

        entry.0 += 1; // total count
        if info.is_healthy {
            entry.1 += 1; // healthy count
        }
        entry.2.push(info.health_score); // collect scores
    }

    let provider_stats = provider_stats_map
        .into_iter()
        .map(|(provider_name, (total, healthy, scores))| {
            let avg_health_score = if scores.is_empty() {
                0.0
            } else {
                let sum: f64 = scores.iter().map(|&v| f64::from(v)).sum();
                let count_u32 = u32::try_from(scores.len()).unwrap_or(u32::MAX);
                if count_u32 == 0 {
                    0.0
                } else {
                    sum / f64::from(count_u32)
                }
            };

            ProviderHealthStats {
                provider_name,
                total_keys: total,
                healthy_keys: healthy,
                avg_health_score,
            }
        })
        .collect();

    // 检查健康检查服务是否运行中
    let health_check_running = state.api_key_health_checker.is_some();

    Ok(HealthCheckStats {
        total_keys,
        healthy_keys,
        unhealthy_keys,
        health_check_running,
        provider_stats,
    })
}

async fn trigger_key_health_check_internal(
    state: &AppState,
    key_id: i32,
) -> anyhow::Result<crate::scheduler::api_key_health::ApiKeyCheckResult> {
    // 获取指定的API密钥
    let key = user_provider_keys::Entity::find_by_id(key_id)
        .one(&*state.database)
        .await?
        .ok_or_else(|| anyhow::anyhow!("API key not found: {key_id}"))?;

    // 使用共享的健康检查器，如果不存在则创建临时的
    let health_checker = state.api_key_health_checker.as_ref().map_or_else(
        || Arc::new(ApiKeyHealthChecker::new(state.database.clone(), None)),
        std::clone::Clone::clone,
    );
    health_checker.check_api_key(&key).await
}

async fn mark_key_unhealthy_internal(
    state: &AppState,
    key_id: i32,
    reason: String,
) -> anyhow::Result<()> {
    // 使用共享的健康检查器，如果不存在则创建临时的
    let health_checker = state.api_key_health_checker.as_ref().map_or_else(
        || Arc::new(ApiKeyHealthChecker::new(state.database.clone(), None)),
        std::clone::Clone::clone,
    );
    health_checker.mark_key_unhealthy(key_id, reason).await
}
