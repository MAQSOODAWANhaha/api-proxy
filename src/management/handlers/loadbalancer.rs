//! # 负载均衡器管理处理器

use crate::management::response;
use crate::management::response::Pagination;
use crate::management::server::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use entity::{
    provider_types, provider_types::Entity as ProviderTypes, user_service_apis,
    user_service_apis::Entity as UserServiceApis,
};
use sea_orm::{entity::*, query::*};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tracing::info;
use tracing::warn;

#[derive(Serialize)]
struct LoadBalancerStatus {
    status: &'static str,
    algorithms: Vec<&'static str>,
    current_algorithm: &'static str,
    load_balancers: HashMap<String, ProviderStats>,
}

#[derive(Serialize)]
struct ProviderStats {
    total_servers: usize,
    healthy_servers: usize,
    current_requests: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Server {
    id: String,
    api_id: i32,
    upstream_type: String,
    display_name: String,
    host: String,
    port: u16,
    use_tls: bool,
    weight: u32,
    is_healthy: bool,
    is_active: bool,
    response_time_ms: i32,
    requests_total: i32,
    requests_successful: i32,
    requests_failed: i32,
    rate_limit: i32,
    timeout_seconds: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    last_used: Option<chrono::DateTime<chrono::Utc>>,
}

/// 获取负载均衡器状态
pub async fn get_lb_status(State(state): State<AppState>) -> impl IntoResponse {
    match get_provider_statistics(&state).await {
        Ok(provider_stats) => {
            let status = LoadBalancerStatus {
                status: "active",
                algorithms: vec!["round_robin", "weighted", "health_based"],
                current_algorithm: "health_based", // Placeholder
                load_balancers: provider_stats,
            };
            response::success(status)
        }
        Err(err) => {
            tracing::error!("Failed to get provider statistics: {}", err);
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "PROVIDER_STATS_FAILED",
                "获取提供商统计信息失败",
            )
        }
    }
}

/// 服务器查询参数
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerQuery {
    /// 上游类型过滤
    pub upstream_type: Option<String>,
    /// 健康状态过滤
    pub healthy: Option<bool>,
}

/// 列出所有服务器
pub async fn list_servers(
    State(state): State<AppState>,
    Query(query): Query<ServerQuery>,
) -> impl IntoResponse {
    let mut select = UserServiceApis::find().find_also_related(ProviderTypes);

    if let Some(upstream_type) = &query.upstream_type {
        let provider_ids: Vec<i32> = match ProviderTypes::find()
            .filter(provider_types::Column::Name.eq(upstream_type))
            .all(state.database.as_ref())
            .await
        {
            Ok(providers) => providers.into_iter().map(|p| p.id).collect(),
            Err(err) => {
                tracing::error!("Failed to fetch provider types: {}", err);
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "DB_ERROR",
                    "Failed to fetch provider types",
                );
            }
        };

        if !provider_ids.is_empty() {
            select = select.filter(user_service_apis::Column::ProviderTypeId.is_in(provider_ids));
        } else {
            select = select.filter(user_service_apis::Column::Id.eq(-1));
        }
    }

    let servers_data = match select.all(state.database.as_ref()).await {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to fetch servers: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch servers",
            );
        }
    };

    let mut servers: Vec<Server> = Vec::new();

    for (api, provider_type) in servers_data {
        let provider = provider_type.unwrap_or_default();

        let is_healthy = api.is_active && provider.is_active;

        if let Some(healthy_filter) = query.healthy {
            if is_healthy != healthy_filter {
                continue;
            }
        }

        let (host, port, use_tls) = parse_base_url(&provider.base_url);

        let server = Server {
            id: format!("{}-{}", provider.name, api.id),
            api_id: api.id,
            upstream_type: provider.name,
            display_name: provider.display_name,
            host,
            port,
            use_tls,
            weight: 100, // Placeholder
            is_healthy,
            is_active: api.is_active,
            response_time_ms: calculate_avg_response_time_sync(&api),
            requests_total: api.total_requests.unwrap_or(0),
            requests_successful: api.successful_requests.unwrap_or(0),
            requests_failed: api.total_requests.unwrap_or(0) - api.successful_requests.unwrap_or(0),
            rate_limit: api.rate_limit.unwrap_or(0),
            timeout_seconds: api.timeout_seconds.unwrap_or(30),
            created_at: api.created_at.and_utc(),
            last_used: api.last_used.map(|dt| dt.and_utc()),
        };

        servers.push(server);
    }

    let total = servers.len() as u64;
    let pagination = Pagination {
        page: 1,
        limit: total, // Not implemented yet
        total,
        pages: 1,
    };

    response::paginated(servers, pagination)
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
) -> impl IntoResponse {
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

/// 获取提供商统计信息
async fn get_provider_statistics(
    state: &AppState,
) -> Result<HashMap<String, ProviderStats>, sea_orm::DbErr> {
    let providers = ProviderTypes::find().all(state.database.as_ref()).await?;
    let mut provider_stats = HashMap::new();

    for provider in providers {
        let apis = UserServiceApis::find()
            .filter(user_service_apis::Column::ProviderTypeId.eq(provider.id))
            .all(state.database.as_ref())
            .await?;

        let total_servers = apis.len();
        let healthy_servers = apis.iter().filter(|api| api.is_active).count();
        let current_requests = apis
            .iter()
            .map(|api| api.total_requests.unwrap_or(0))
            .sum::<i32>();

        provider_stats.insert(
            provider.display_name,
            ProviderStats {
                total_servers,
                healthy_servers,
                current_requests,
            },
        );
    }

    Ok(provider_stats)
}

/// 解析base_url获取主机、端口和TLS信息
fn parse_base_url(base_url: &str) -> (String, u16, bool) {
    if base_url.is_empty() {
        return ("unknown".to_string(), 80, false);
    }

    // 简单的URL解析
    let use_tls = base_url.starts_with("https://");
    let default_port = if use_tls { 443 } else { 80 };

    // 移除协议前缀
    let url_without_protocol = base_url
        .strip_prefix("https://")
        .or_else(|| base_url.strip_prefix("http://"))
        .unwrap_or(base_url);

    // 移除路径部分，只保留主机:端口
    let host_port = url_without_protocol
        .split('/')
        .next()
        .unwrap_or(url_without_protocol);

    // 解析主机和端口
    if let Some((host_part, port_part)) = host_port.split_once(':') {
        let port = port_part.parse().unwrap_or(default_port);
        (host_part.to_string(), port, use_tls)
    } else {
        (host_port.to_string(), default_port, use_tls)
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
) -> impl IntoResponse {
    let strategy = match request.strategy.to_lowercase().as_str() {
        "round_robin" => crate::scheduler::types::SchedulingStrategy::RoundRobin,
        "weighted" => crate::scheduler::types::SchedulingStrategy::Weighted,
        "health_based" => crate::scheduler::types::SchedulingStrategy::HealthBased,
        _ => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                &format!("Invalid strategy: {}. Supported strategies: round_robin, weighted, health_based", request.strategy),
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
) -> impl IntoResponse {
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
pub async fn get_lb_metrics(State(state): State<AppState>) -> impl IntoResponse {
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

/// 计算平均响应时间
async fn calculate_avg_response_time(api: &user_service_apis::Model, state: &AppState) -> i32 {
    // 基于实际统计数据或启发式规则计算响应时间

    // 目前模型中没有avg_response_time_ms字段，所以直接使用启发式计算

    // 如果没有实际数据，基于提供商类型进行启发式估算
    let base_time = if let Ok(provider_types) =
        entity::provider_types::Entity::find_by_id(api.provider_type_id)
            .one(state.database.as_ref())
            .await
    {
        match provider_types {
            Some(provider) => {
                // 优先从配置中获取预期响应时间，否则根据超时时间动态计算
                let config_response_time = provider
                    .config_json
                    .as_ref()
                    .and_then(|config_str| {
                        serde_json::from_str::<serde_json::Value>(config_str).ok()
                    })
                    .and_then(|config| {
                        config
                            .get("expected_response_time_ms")
                            .and_then(|rt| rt.as_i64())
                            .map(|rt| rt as i32)
                    });

                config_response_time.unwrap_or_else(|| {
                    // 如果没有配置，使用超时时间的25%作为预期响应时间
                    let timeout_ms = provider.timeout_seconds.unwrap_or(30) * 1000;
                    (timeout_ms as f32 * 0.25) as i32
                })
            }
            None => 180, // 找不到提供商信息时的默认值
        }
    } else {
        tracing::warn!(
            "Failed to fetch provider type for API {}, using default response time",
            api.id
        );
        180 // 数据库查询失败时的默认值
    };

    // 根据成功率调整响应时间
    let total = api.total_requests.unwrap_or(0);
    let successful = api.successful_requests.unwrap_or(0);

    if total > 0 {
        let success_rate = successful as f32 / total as f32;
        let penalty = if success_rate < 0.9 {
            // 成功率低时增加响应时间惩罚
            ((1.0 - success_rate) * 100.0) as i32
        } else {
            0
        };
        base_time + penalty
    } else {
        base_time
    }
}

/// 同步版本的响应时间计算（用于不方便使用async的地方）
fn calculate_avg_response_time_sync(api: &user_service_apis::Model) -> i32 {
    // 目前模型中没有avg_response_time_ms字段，所以直接使用启发式计算

    // 基于provider_type_id的简单映射（临时解决方案）
    let base_time = match api.provider_type_id {
        1 => 120, // 假设ID=1是OpenAI
        2 => 150, // 假设ID=2是Google
        3 => 200, // 假设ID=3是Claude
        _ => 180, // 其他
    };

    // 根据成功率调整响应时间
    let total = api.total_requests.unwrap_or(0);
    let successful = api.successful_requests.unwrap_or(0);

    if total > 0 {
        let success_rate = successful as f32 / total as f32;
        let penalty = if success_rate < 0.9 {
            ((1.0 - success_rate) * 100.0) as i32
        } else {
            0
        };
        base_time + penalty
    } else {
        base_time
    }
}
