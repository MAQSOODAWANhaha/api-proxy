//! # 负载均衡器管理处理器

use crate::management::response;
use crate::management::server::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use entity::{user_service_apis, user_service_apis::Entity as UserServiceApis};
use sea_orm::{entity::*, query::*};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;
use tracing::warn;

/// 服务器查询参数
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerQuery {
    /// 上游类型过滤
    pub upstream_type: Option<String>,
    /// 健康状态过滤
    pub healthy: Option<bool>,
}

/// 添加服务器请求
#[derive(Debug, Deserialize)]
pub struct AddServerRequest {
    /// 上游类型
    pub upstream_type: String,
    /// 主机地址
    pub host: String,
    /// 端口
    pub port: u16,
    /// 是否使用TLS
    #[serde(default)]
    pub use_tls: bool,
    /// 权重
    #[serde(default = "default_weight")]
    pub weight: u32,
    /// 最大连接数
    pub max_connections: Option<u32>,
    /// 超时时间（毫秒）
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_weight() -> u32 {
    100
}

fn default_timeout() -> u64 {
    5000
}

/// 添加新服务器
pub async fn add_server(
    State(state): State<AppState>,
    Json(request): Json<AddServerRequest>,
) -> axum::response::Response {
    if request.host.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Host cannot be empty",
        );
    }

    if request.port == 0 {
        return response::error(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid port number",
        );
    }

    if let Err(e) = state
        .provider_resolver
        .resolve_provider(&request.upstream_type)
        .await
    {
        return response::error(
            StatusCode::BAD_REQUEST,
            "PROVIDER_NOT_FOUND",
            &format!(
                "Failed to resolve provider '{}': {}",
                request.upstream_type, e
            ),
        );
    }

    let server_id = format!("{}", request.host);

    match state
        .load_balancer_manager
        .add_server(
            &request.upstream_type,
            &request.host,
            request.port,
            request.weight,
            request.use_tls,
        )
        .await
    {
        Ok(_) => {
            info!(
                "Successfully added server: {} ({}:{})",
                server_id, request.host, request.port
            );
            response::success_with_message(
                json!({ "server_id": server_id }),
                "Server added successfully",
            )
        }
        Err(e) => {
            warn!(
                "Failed to add server to load balancer manager: {} ({}:{}), error: {}",
                server_id, request.host, request.port, e
            );
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "ADD_SERVER_FAILED",
                &format!("Failed to add server: {}", e),
            )
        }
    }
}

/// 更改调度策略请求
#[derive(Debug, Deserialize)]
pub struct ChangeStrategyRequest {
    /// 上游类型
    pub upstream_type: String,
    /// 新的调度策略
    pub strategy: String,
}

/// 更改负载均衡调度策略
pub async fn change_strategy(
    State(state): State<AppState>,
    Json(request): Json<ChangeStrategyRequest>,
) -> axum::response::Response {
    let strategy = match request.strategy.to_lowercase().as_str() {
        "round_robin" => crate::scheduler::types::SchedulingStrategy::RoundRobin,
        "weighted" => crate::scheduler::types::SchedulingStrategy::Weighted,
        "health_based" => crate::scheduler::types::SchedulingStrategy::HealthBased,
        _ => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                &format!(
                    "Invalid strategy: {}. Supported strategies: round_robin, weighted, health_based",
                    request.strategy
                ),
            );
        }
    };

    let provider_id = match state
        .provider_resolver
        .resolve_provider(&request.upstream_type)
        .await
    {
        Ok(provider_id) => provider_id,
        Err(e) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "PROVIDER_NOT_FOUND",
                &format!(
                    "Failed to resolve provider '{}': {}",
                    request.upstream_type, e
                ),
            );
        }
    };

    match state
        .load_balancer_manager
        .change_strategy(provider_id, strategy)
        .await
    {
        Ok(old_strategy) => {
            info!(
                "Successfully changed strategy for {} from {:?} to {:?}",
                request.upstream_type, old_strategy, strategy
            );
            response::success_with_message(
                json!({
                    "old_strategy": old_strategy.map(|s| format!("{:?}", s)),
                    "new_strategy": format!("{:?}", strategy),
                }),
                &format!(
                    "Strategy changed successfully for {}",
                    request.upstream_type
                ),
            )
        }
        Err(e) => {
            warn!(
                "Failed to change strategy for {}: {}",
                request.upstream_type, e
            );
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "CHANGE_STRATEGY_FAILED",
                &format!("Failed to change strategy: {}", e),
            )
        }
    }
}

/// 服务器操作请求
#[derive(Debug, Deserialize)]
pub struct ServerActionRequest {
    /// 服务器ID
    pub server_id: String,
    /// 操作类型: "enable", "disable", "remove"
    pub action: String,
}

/// 执行服务器操作（启用/禁用/移除）
pub async fn server_action(
    State(state): State<AppState>,
    Json(request): Json<ServerActionRequest>,
) -> axum::response::Response {
    let action = request.action.to_lowercase();

    let parts: Vec<&str> = request.server_id.split('-').collect();
    if parts.len() < 2 {
        return response::error(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid server ID format",
        );
    }

    let upstream_type_str = parts[0];
    let api_id: i32 = match parts.last().unwrap().parse() {
        Ok(id) => id,
        Err(_) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "Invalid API ID in server ID",
            );
        }
    };

    let result = match action.as_str() {
        "enable" => enable_server(&state, api_id).await,
        "disable" => disable_server(&state, api_id).await,
        "remove" => remove_server(&state, api_id, upstream_type_str).await,
        _ => Err(format!("Unknown action: {}", action)),
    };

    match result {
        Ok(message) => {
            info!(
                "Server action {} on {} successful: {}",
                action, request.server_id, message
            );
            response::success_without_data(&message)
        }
        Err(error) => {
            warn!(
                "Server action {} on {} failed: {}",
                action, request.server_id, error
            );
            response::error(StatusCode::INTERNAL_SERVER_ERROR, "ACTION_FAILED", &error)
        }
    }
}

/// 启用服务器
async fn enable_server(state: &AppState, api_id: i32) -> Result<String, String> {
    match UserServiceApis::update_many()
        .col_expr(user_service_apis::Column::IsActive, true.into())
        .filter(user_service_apis::Column::Id.eq(api_id))
        .exec(state.database.as_ref())
        .await
    {
        Ok(_) => Ok("Server enabled successfully".to_string()),
        Err(e) => Err(format!("Database error: {}", e)),
    }
}

/// 禁用服务器
async fn disable_server(state: &AppState, api_id: i32) -> Result<String, String> {
    match UserServiceApis::update_many()
        .col_expr(user_service_apis::Column::IsActive, false.into())
        .filter(user_service_apis::Column::Id.eq(api_id))
        .exec(state.database.as_ref())
        .await
    {
        Ok(_) => Ok("Server disabled successfully".to_string()),
        Err(e) => Err(format!("Database error: {}", e)),
    }
}

/// 移除服务器
async fn remove_server(
    state: &AppState,
    api_id: i32,
    upstream_type_str: &str,
) -> Result<String, String> {
    // 从数据库删除
    match UserServiceApis::delete_many()
        .filter(user_service_apis::Column::Id.eq(api_id))
        .exec(state.database.as_ref())
        .await
    {
        Ok(_) => {
            // 同时从负载均衡管理器中移除
            if let Err(e) = state
                .load_balancer_manager
                .remove_server(upstream_type_str, api_id)
                .await
            {
                warn!("Failed to remove server from load balancer manager: {}", e);
            }
            Ok("Server removed successfully".to_string())
        }
        Err(e) => Err(format!("Database error: {}", e)),
    }
}

/// 获取负载均衡器指标
pub async fn get_lb_metrics(State(state): State<AppState>) -> axum::response::Response {
    match state.load_balancer_manager.get_detailed_metrics().await {
        Ok(metrics) => response::success(metrics),
        Err(e) => {
            tracing::error!("Failed to get load balancer metrics: {}", e);
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "METRICS_FAILED",
                "获取负载均衡器指标失败",
            )
        }
    }
}
