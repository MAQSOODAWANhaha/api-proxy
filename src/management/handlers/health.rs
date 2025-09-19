//! API密钥健康检查相关处理器

use crate::management::{response, server::AppState};
use crate::scheduler::api_key_health::ApiKeyHealthChecker;
use axum::extract::{Path, State};
use entity::{user_provider_keys, provider_types};
// use pingora_http::StatusCode; // no longer needed with manage_error!
use sea_orm::{EntityTrait, ColumnTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::error;

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
    pub avg_health_score: f32,
}

/// 获取所有API密钥健康状态
pub async fn get_api_keys_health(State(state): State<AppState>) -> axum::response::Response {
    // 注意：这需要从AppState中获取ApiKeyHealthChecker
    // 当前我们需要通过数据库查询来获取健康状态
    match get_api_keys_health_internal(&state).await {
        Ok(health_infos) => response::success(health_infos),
        Err(err) => {
            error!(error = %err, "Failed to get API keys health status");
            crate::manage_error!(crate::proxy_err!(database, "Failed to retrieve API keys health status: {}", err))
        }
    }
}

/// 获取健康检查统计信息
#[allow(clippy::similar_names)]
pub async fn get_health_stats(State(state): State<AppState>) -> axum::response::Response {
    match get_health_stats_internal(&state).await {
        Ok(stats) => response::success(stats),
        Err(err) => {
            error!(error = %err, "Failed to get health statistics");
            crate::manage_error!(crate::proxy_err!(database, "Failed to retrieve health statistics: {}", err))
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
            error!(key_id = key_id, error = %err, "Failed to trigger health check");
            crate::manage_error!(crate::proxy_err!(database, "Failed to trigger health check for key {}: {}", key_id, err))
        }
    }
}

/// 标记API密钥为不健康
pub async fn mark_key_unhealthy(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
) -> axum::response::Response {
    let reason = format!("Manually marked unhealthy via management API");
    match mark_key_unhealthy_internal(&state, key_id, reason).await {
        Ok(_) => response::success("API key marked as unhealthy"),
        Err(err) => {
            error!(key_id = key_id, error = %err, "Failed to mark key unhealthy");
            crate::manage_error!(crate::proxy_err!(database, "Failed to mark key {} as unhealthy: {}", key_id, err))
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
    let health_checker = match &state.api_key_health_checker {
        Some(checker) => checker.clone(),
        None => Arc::new(ApiKeyHealthChecker::new(state.database.clone(), None)),
    };

    for key in active_keys {
        let provider_info = provider_types_map.get(&key.provider_type_id);
        let provider_name = provider_info
            .map(|p| p.display_name.clone())
            .unwrap_or_else(|| format!("Provider {}", key.provider_type_id));

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
                scores.iter().sum::<f32>() / scores.len() as f32
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
        .ok_or_else(|| anyhow::anyhow!("API key not found: {}", key_id))?;

    // 使用共享的健康检查器，如果不存在则创建临时的
    let health_checker = match &state.api_key_health_checker {
        Some(checker) => checker.clone(),
        None => Arc::new(ApiKeyHealthChecker::new(state.database.clone(), None)),
    };
    health_checker.check_api_key(&key).await
}

async fn mark_key_unhealthy_internal(
    state: &AppState,
    key_id: i32,
    reason: String,
) -> anyhow::Result<()> {
    // 使用共享的健康检查器，如果不存在则创建临时的
    let health_checker = match &state.api_key_health_checker {
        Some(checker) => checker.clone(),
        None => Arc::new(ApiKeyHealthChecker::new(state.database.clone(), None)),
    };
    health_checker.mark_key_unhealthy(key_id, reason).await
}
