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
use axum::routing::{get, post, put, patch};
use axum::Router;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

/// 管理服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagementConfig {
    /// 监听地址
    pub bind_address: String,
    /// 监听端口
    pub port: u16,
    /// 是否启用CORS
    pub enable_cors: bool,
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
            api_prefix: "/api/v1".to_string(),
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
        let api_routes = Router::new()
            // 认证接口
            .route("/auth/login", post(super::handlers::auth::login))
            
            // 健康检查
            .route("/health", get(health_check))
            .route("/health/detailed", get(detailed_health_check))
            .route("/health/servers", get(super::handlers::health::get_health_servers))
            
            // 系统信息
            .route("/system/info", get(super::handlers::system::get_system_info))
            .route("/system/metrics", get(super::handlers::system::get_system_metrics))
            
            // 负载均衡管理
            .route("/loadbalancer/status", get(super::handlers::loadbalancer::get_lb_status))
            .route("/loadbalancer/servers", get(super::handlers::loadbalancer::list_servers))
            .route("/loadbalancer/servers", post(super::handlers::loadbalancer::add_server))
            .route("/loadbalancer/servers/action", post(super::handlers::loadbalancer::server_action))
            .route("/loadbalancer/strategy", patch(super::handlers::loadbalancer::change_strategy))
            .route("/loadbalancer/metrics", get(super::handlers::loadbalancer::get_lb_metrics))
            
            // 适配器管理
            .route("/adapters", get(super::handlers::adapters::list_adapters))
            .route("/adapters/stats", get(super::handlers::adapters::get_adapter_stats))
            
            // 统计查询
            .route("/statistics/overview", get(super::handlers::statistics::get_overview))
            .route("/statistics/requests", get(super::handlers::statistics::get_request_stats))
            
            // 用户管理
            .route("/users", get(super::handlers::users::list_users))
            .route("/users", post(super::handlers::users::create_user))
            .route("/users/{id}", get(super::handlers::users::get_user))
            
            // API密钥管理
            .route("/provider-types", get(super::handlers::auth::list_provider_types))
            .route("/api-keys", get(super::handlers::auth::list_api_keys))
            .route("/api-keys", post(super::handlers::auth::create_api_key))
            .route("/api-keys/{id}", get(super::handlers::auth::get_api_key))
            .route("/api-keys/{id}", put(super::handlers::auth::update_api_key))
            .route("/api-keys/{id}/revoke", post(super::handlers::auth::revoke_api_key))
            
            .with_state(state);

        let mut app = Router::new()
            .nest(&config.api_prefix, api_routes)
            .route("/", get(root_handler))
            .route("/ping", get(ping_handler));

        // 添加中间件
        let service_builder = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http());

        if config.enable_cors {
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any);
            app = app.layer(service_builder.layer(cors));
        } else {
            app = app.layer(service_builder);
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
        
        axum::serve(listener, self.router.into_make_service())
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