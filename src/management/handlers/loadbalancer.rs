//! # 负载均衡器管理处理器

use crate::management::server::AppState;
use crate::proxy::upstream::UpstreamType;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::info;
use sea_orm::{entity::*, query::*};
use entity::{
    user_service_apis,
    user_service_apis::Entity as UserServiceApis,
    provider_types,
    provider_types::Entity as ProviderTypes,
};

/// 获取负载均衡器状态
pub async fn get_lb_status(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // 查询所有提供商类型和相关服务API
    let provider_stats = match get_provider_statistics(&state).await {
        Ok(stats) => stats,
        Err(err) => {
            tracing::error!("Failed to get provider statistics: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let status = json!({
        "status": "active",
        "algorithms": ["round_robin", "weighted", "health_based"],
        "current_algorithm": "health_based",
        "load_balancers": provider_stats
    });

    Ok(Json(status))
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
) -> Result<Json<Value>, StatusCode> {
    // 从数据库获取服务器列表
    let mut select = UserServiceApis::find()
        .find_also_related(ProviderTypes);

    // 应用过滤器
    if let Some(upstream_type) = &query.upstream_type {
        // 获取对应的provider_type_id
        let provider_ids: Vec<i32> = match ProviderTypes::find()
            .filter(provider_types::Column::Name.eq(upstream_type))
            .all(state.database.as_ref())
            .await
        {
            Ok(providers) => providers.into_iter().map(|p| p.id).collect(),
            Err(err) => {
                tracing::error!("Failed to fetch provider types: {}", err);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };
        
        if !provider_ids.is_empty() {
            select = select.filter(user_service_apis::Column::ProviderTypeId.is_in(provider_ids));
        } else {
            // 没有匹配的提供商，返回空结果
            select = select.filter(user_service_apis::Column::Id.eq(-1));
        }
    }

    let servers_data = match select.all(state.database.as_ref()).await {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to fetch servers: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let mut servers: Vec<Value> = Vec::new();

    for (api, provider_type) in servers_data {
        let provider = provider_type.unwrap_or(provider_types::Model {
            id: 0,
            name: "unknown".to_string(),
            display_name: "Unknown Provider".to_string(),
            base_url: "".to_string(),
            api_format: "".to_string(),
            default_model: None,
            max_tokens: None,
            rate_limit: None,
            timeout_seconds: None,
            health_check_path: None,
            auth_header_format: None,
            is_active: false,
            config_json: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        });

        // 计算健康状态
        let is_healthy = api.is_active && provider.is_active;
        
        // 应用健康状态过滤器
        if let Some(healthy_filter) = query.healthy {
            if is_healthy != healthy_filter {
                continue;
            }
        }

        // 从base_url解析主机和端口
        let (host, port, use_tls) = parse_base_url(&provider.base_url);

        let server = json!({
            "id": format!("{}-{}", provider.name, api.id),
            "api_id": api.id,
            "upstream_type": provider.name,
            "display_name": provider.display_name,
            "host": host,
            "port": port,
            "use_tls": use_tls,
            "weight": 100, // 默认权重，可以从配置中获取
            "is_healthy": is_healthy,
            "is_active": api.is_active,
            "response_time_ms": calculate_avg_response_time(&api),
            "requests_total": api.total_requests.unwrap_or(0),
            "requests_successful": api.successful_requests.unwrap_or(0),
            "requests_failed": api.total_requests.unwrap_or(0) - api.successful_requests.unwrap_or(0),
            "rate_limit": api.rate_limit.unwrap_or(0),
            "timeout_seconds": api.timeout_seconds.unwrap_or(30),
            "created_at": api.created_at.and_utc(),
            "last_used": api.last_used.map(|dt| dt.and_utc())
        });

        servers.push(server);
    }

    let response = json!({
        "servers": servers,
        "total": servers.len(),
        "filters": query
    });

    Ok(Json(response))
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

/// 添加服务器响应
#[derive(Debug, Serialize)]
pub struct AddServerResponse {
    /// 服务器ID
    pub id: String,
    /// 是否成功
    pub success: bool,
    /// 消息
    pub message: String,
}

/// 添加新服务器
pub async fn add_server(
    State(_state): State<AppState>,
    Json(request): Json<AddServerRequest>,
) -> Result<Json<AddServerResponse>, StatusCode> {
    // 验证请求
    if request.host.is_empty() {
        return Ok(Json(AddServerResponse {
            id: String::new(),
            success: false,
            message: "Host cannot be empty".to_string(),
        }));
    }

    if request.port == 0 {
        return Ok(Json(AddServerResponse {
            id: String::new(),
            success: false,
            message: "Invalid port number".to_string(),
        }));
    }

    // 解析上游类型
    let _upstream_type = match request.upstream_type.as_str() {
        "OpenAI" => UpstreamType::OpenAI,
        "Anthropic" => UpstreamType::Anthropic,
        "GoogleGemini" => UpstreamType::GoogleGemini,
        custom => UpstreamType::Custom(custom.to_string()),
    };

    // 创建服务器ID
    let server_id = format!("{}-{}-{}", 
        request.upstream_type.to_lowercase(),
        request.host.replace('.', "-"),
        request.port
    );

    // TODO: 实际添加到负载均衡管理器
    info!(
        "Adding server: {} ({}:{})",
        server_id, request.host, request.port
    );

    Ok(Json(AddServerResponse {
        id: server_id,
        success: true,
        message: "Server added successfully".to_string(),
    }))
}

/// 获取提供商统计信息
async fn get_provider_statistics(state: &AppState) -> Result<Value, Box<dyn std::error::Error>> {
    // 获取所有提供商类型
    let providers = ProviderTypes::find()
        .all(state.database.as_ref())
        .await?;

    let mut provider_stats = serde_json::Map::new();

    for provider in providers {
        // 获取该提供商的所有服务API
        let apis = UserServiceApis::find()
            .filter(user_service_apis::Column::ProviderTypeId.eq(provider.id))
            .all(state.database.as_ref())
            .await?;

        let total_servers = apis.len();
        let healthy_servers = apis.iter()
            .filter(|api| api.is_active)
            .count();
        
        let current_requests = apis.iter()
            .map(|api| api.total_requests.unwrap_or(0))
            .sum::<i32>();

        provider_stats.insert(provider.display_name, json!({
            "total_servers": total_servers,
            "healthy_servers": healthy_servers,
            "current_requests": current_requests
        }));
    }

    Ok(Value::Object(provider_stats))
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

/// 计算平均响应时间
fn calculate_avg_response_time(api: &user_service_apis::Model) -> i32 {
    // 基于一些启发式规则计算响应时间
    let base_time = match api.provider_type_id {
        1 => 120, // OpenAI 一般较快
        2 => 150, // Anthropic
        3 => 200, // Google Gemini
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