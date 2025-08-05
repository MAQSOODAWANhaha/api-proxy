//! # 管理服务器
//!
//! Axum HTTP服务器，提供管理和监控API

use crate::auth::service::AuthService;
use crate::config::AppConfig;
use crate::health::service::HealthCheckService as HealthService;
use crate::providers::manager::AdapterManager;
use crate::scheduler::manager::LoadBalancerManager;
use crate::statistics::service::StatisticsService;
use sea_orm::DatabaseConnection;
use anyhow::Result;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};
use super::middleware::{ip_filter_middleware, IpFilterConfig};

/// 管理服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagementConfig {
    /// 监听地址
    pub bind_address: String,
    /// 监听端口
    pub port: u16,
    /// 是否启用CORS
    pub enable_cors: bool,
    /// 允许的CORS源地址
    pub cors_origins: Vec<String>,
    /// 允许访问的IP地址列表
    pub allowed_ips: Vec<String>,
    /// 拒绝访问的IP地址列表
    pub denied_ips: Vec<String>,
    /// API前缀
    pub api_prefix: String,
    /// 最大请求大小
    pub max_request_size: usize,
    /// 请求超时时间（秒）
    pub request_timeout: u64,
}

impl Default for ManagementConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
            enable_cors: true,
            cors_origins: vec!["*".to_string()],
            allowed_ips: vec!["0.0.0.0/0".to_string()],
            denied_ips: vec![],
            api_prefix: "/api".to_string(), // 修改为 /api 与前端一致
            max_request_size: 1024 * 1024, // 1MB
            request_timeout: 30,
        }
    }
}

/// 管理服务器应用状态
#[derive(Clone)]
pub struct AppState {
    /// 应用配置
    pub config: Arc<AppConfig>,
    /// 数据库连接
    pub database: Arc<DatabaseConnection>,
    /// 认证服务
    pub auth_service: Arc<AuthService>,
    /// 健康检查服务
    pub health_service: Arc<HealthService>,
    /// 适配器管理器
    pub adapter_manager: Arc<AdapterManager>,
    /// 负载均衡管理器
    pub load_balancer_manager: Arc<LoadBalancerManager>,
    /// 统计服务
    pub statistics_service: Arc<StatisticsService>,
}

/// 管理服务器
pub struct ManagementServer {
    /// 配置
    config: ManagementConfig,
    /// 应用状态
    state: AppState,
    /// 路由器
    router: Router,
}

impl ManagementServer {
    /// 创建新的管理服务器
    pub fn new(
        config: ManagementConfig,
        app_config: Arc<AppConfig>,
        database: Arc<DatabaseConnection>,
        auth_service: Arc<AuthService>,
        health_service: Arc<HealthService>,
        adapter_manager: Arc<AdapterManager>,
        load_balancer_manager: Arc<LoadBalancerManager>,
        statistics_service: Arc<StatisticsService>,
    ) -> Result<Self> {
        let state = AppState {
            config: app_config,
            database,
            auth_service,
            health_service,
            adapter_manager,
            load_balancer_manager,
            statistics_service,
        };

        let router = Self::create_router(state.clone(), &config)?;

        Ok(Self {
            config,
            state,
            router,
        })
    }

    /// 创建路由器
    fn create_router(state: AppState, config: &ManagementConfig) -> Result<Router> {
        // 使用统一的路由配置
        let api_routes = super::routes::create_routes(state.clone());

        let mut app = Router::new()
            .nest(&config.api_prefix, api_routes)
            .route("/", get(root_handler))
            .route("/ping", get(ping_handler));

        // 创建IP过滤配置
        let ip_filter_config = IpFilterConfig::from_strings(&config.allowed_ips, &config.denied_ips)
            .unwrap_or_else(|e| {
                warn!("Failed to create IP filter config: {}, using default", e);
                IpFilterConfig {
                    allowed_ips: vec![],
                    denied_ips: vec![],
                }
            });

        // 添加中间件
        let service_builder = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http());

        // 配置CORS
        if config.enable_cors {
            let mut cors_layer = CorsLayer::new()
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PUT,
                    axum::http::Method::DELETE,
                    axum::http::Method::PATCH,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::ACCEPT,
                    axum::http::header::ORIGIN,
                ]);

            // 配置允许的源
            if config.cors_origins.contains(&"*".to_string()) {
                cors_layer = cors_layer.allow_origin(Any);
            } else {
                let origins: Result<Vec<_>, _> = config.cors_origins
                    .iter()
                    .map(|origin| origin.parse::<axum::http::HeaderValue>())
                    .collect();
                
                match origins {
                    Ok(origins) => {
                        cors_layer = cors_layer.allow_origin(origins);
                    }
                    Err(e) => {
                        warn!("Invalid CORS origin configuration: {}, falling back to allow any", e);
                        cors_layer = cors_layer.allow_origin(Any);
                    }
                }
            }

            app = app.layer(service_builder.layer(cors_layer));
        } else {
            app = app.layer(service_builder);
        }

        // 添加IP过滤中间件（如果配置了限制）
        if !config.allowed_ips.is_empty() || !config.denied_ips.is_empty() {
            app = app.layer(axum::middleware::from_fn(ip_filter_middleware));
            // 将IP过滤配置添加到请求扩展中
            app = app.layer(axum::Extension(ip_filter_config));
        }

        Ok(app)
    }

    /// 启动服务器
    pub async fn serve(self) -> Result<()> {
        let addr = SocketAddr::new(
            self.config.bind_address.parse()?,
            self.config.port,
        );

        info!("Starting management server on {}", addr);

        let listener = TcpListener::bind(&addr).await?;
        
        axum::serve(
            listener, 
            self.router.into_make_service_with_connect_info::<SocketAddr>()
        )
            .await
            .map_err(|e| anyhow::anyhow!("Management server error: {}", e))?;

        Ok(())
    }

    /// 获取绑定地址
    pub fn bind_address(&self) -> SocketAddr {
        SocketAddr::new(
            self.config.bind_address.parse().unwrap(),
            self.config.port,
        )
    }
}

/// 根路径处理器
async fn root_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "service": "AI Proxy Management API",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "running"
    }))
}

/// Ping处理器
async fn ping_handler() -> &'static str {
    "pong"
}

/// 健康检查处理器
pub async fn health_check(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let health_status = match state.health_service.get_overall_health().await {
        Ok(status) => status,
        Err(e) => {
            warn!("Health check failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let response = serde_json::json!({
        "status": if health_status.is_running { "healthy" } else { "unhealthy" },
        "timestamp": chrono::Utc::now(),
        "details": {
            "healthy_servers": health_status.healthy_servers,
            "total_servers": health_status.total_servers,
            "avg_response_time_ms": health_status.avg_response_time.as_millis()
        }
    });

    Ok(Json(response))
}

/// 详细健康检查处理器
pub async fn detailed_health_check(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let health_status = match state.health_service.get_overall_health().await {
        Ok(status) => status,
        Err(e) => {
            warn!("Detailed health check failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let response = serde_json::json!({
        "status": if health_status.is_running { "healthy" } else { "unhealthy" },
        "timestamp": chrono::Utc::now(),
        "system": {
            "total_servers": health_status.total_servers,
            "healthy_servers": health_status.healthy_servers,
            "unhealthy_servers": health_status.unhealthy_servers,
            "active_tasks": health_status.active_tasks,
            "avg_response_time": health_status.avg_response_time,
            "is_running": health_status.is_running
        },
        "adapters": state.adapter_manager.get_adapter_stats(),
        "load_balancers": "TODO: 添加负载均衡器状态"
    });

    Ok(Json(response))
}