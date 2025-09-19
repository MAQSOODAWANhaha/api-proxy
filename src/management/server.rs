//! # 管理服务器
//!
//! Axum HTTP服务器，提供管理和监控API

use super::middleware::{IpFilterConfig, ip_filter_middleware};
use crate::auth::service::AuthService;
use crate::config::{AppConfig, ProviderConfigManager};
// Note: 旧的HealthCheckService已移除，健康检查功能现在通过API密钥健康检查实现
use crate::management::response::{self};
use crate::providers::DynamicAdapterManager;
use crate::providers::dynamic_manager::AdapterStats;
use anyhow::Result;
use axum::Json;
use axum::Router;
use axum::extract::State;
// use axum::http::StatusCode; // not needed with manage_error!
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
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
            max_request_size: 1024 * 1024,  // 1MB
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
    /// 适配器管理器
    pub adapter_manager: Arc<DynamicAdapterManager>,
    /// 提供商配置管理器
    pub provider_config_manager: Arc<ProviderConfigManager>,
    /// API密钥健康检查器
    pub api_key_health_checker: Option<Arc<crate::scheduler::api_key_health::ApiKeyHealthChecker>>,
    /// OAuth客户端
    pub oauth_client: Option<Arc<crate::auth::oauth_client::OAuthClient>>,
    /// 智能API密钥提供者
    pub smart_api_key_provider: Option<Arc<crate::auth::smart_api_key_provider::SmartApiKeyProvider>>,
}

/// 管理服务器
pub struct ManagementServer {
    /// 配置
    config: ManagementConfig,
    /// 应用状态
    #[allow(dead_code)]
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
        adapter_manager: Arc<DynamicAdapterManager>,
        provider_config_manager: Arc<ProviderConfigManager>,
        api_key_health_checker: Option<Arc<crate::scheduler::api_key_health::ApiKeyHealthChecker>>,
        oauth_client: Option<Arc<crate::auth::oauth_client::OAuthClient>>,
        smart_api_key_provider: Option<Arc<crate::auth::smart_api_key_provider::SmartApiKeyProvider>>,
    ) -> Result<Self> {
        let state = AppState {
            config: app_config,
            database,
            auth_service,
            adapter_manager,
            provider_config_manager,
            api_key_health_checker,
            oauth_client,
            smart_api_key_provider,
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

        // 静态文件服务配置
        let static_dir = std::path::Path::new("/app/static");
        let static_service = if static_dir.exists() {
            info!("Enabling static file service from /app/static");
            // 创建静态文件服务，支持SPA应用的fallback
            Some(
                ServeDir::new(static_dir)
                    .not_found_service(ServeFile::new("/app/static/index.html")),
            )
        } else {
            warn!("Static directory /app/static not found, static files will not be served");
            None
        };

        let mut app = Router::new()
            .nest(&config.api_prefix, api_routes)
            .route("/ping", get(ping_handler));

        // 添加静态文件服务（如果可用）
        if let Some(service) = static_service {
            // 使用静态文件服务处理所有未匹配的路由（包括根路径）
            app = app.fallback_service(service);
        } else {
            // 如果没有静态文件，则提供API信息页面
            app = app.route("/", get(root_handler));
        }

        // 创建IP过滤配置
        let ip_filter_config =
            IpFilterConfig::from_strings(&config.allowed_ips, &config.denied_ips).unwrap_or_else(
                |e| {
                    warn!("Failed to create IP filter config: {}, using default", e);
                    IpFilterConfig {
                        allowed_ips: vec![],
                        denied_ips: vec![],
                    }
                },
            );

        // 添加中间件
        let service_builder = ServiceBuilder::new().layer(TraceLayer::new_for_http());

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
                let origins: Result<Vec<_>, _> = config
                    .cors_origins
                    .iter()
                    .map(|origin| origin.parse::<axum::http::HeaderValue>())
                    .collect();

                match origins {
                    Ok(origins) => {
                        cors_layer = cors_layer.allow_origin(origins);
                    }
                    Err(e) => {
                        warn!(
                            "Invalid CORS origin configuration: {}, falling back to allow any",
                            e
                        );
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
        let addr = SocketAddr::new(self.config.bind_address.parse()?, self.config.port);

        info!("Starting management server on {}", addr);

        let listener = TcpListener::bind(&addr).await?;

        axum::serve(
            listener,
            self.router
                .into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("Management server error: {}", e))?;

        Ok(())
    }

    /// 获取绑定地址
    pub fn bind_address(&self) -> SocketAddr {
        SocketAddr::new(self.config.bind_address.parse().unwrap(), self.config.port)
    }
}

/// 根路径处理器
async fn root_handler() -> Response {
    Json(serde_json::json!({
        "success": true,
        "message": "AI Proxy Management API",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
    .into_response()
}

/// Ping处理器
async fn ping_handler() -> &'static str {
    "pong"
}

/// 简单健康检查处理器（系统存活检查）
pub async fn health_check(State(state): State<AppState>) -> axum::response::Response {
    // 简单的数据库连接检查
    match state.database.ping().await {
        Ok(_) => response::success(serde_json::json!({
            "status": "healthy",
            "database": "connected",
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
        Err(e) => {
            warn!("Database ping failed: {}", e);
            crate::manage_error!(crate::proxy_err!(database, "数据库连接失败: {}", e))
        }
    }
}

/// 详细健康检查处理器（系统详细状态）
#[derive(Serialize)]
struct DetailedHealthStatus {
    database: String,
    adapters: HashMap<String, AdapterStats>,
    system_info: SystemInfo,
    api_key_health: String,
}

#[derive(Serialize)]
struct SystemInfo {
    uptime: String,
    timestamp: String,
    version: String,
}

pub async fn detailed_health_check(State(state): State<AppState>) -> axum::response::Response {
    // 检查数据库连接
    let database_status = match state.database.ping().await {
        Ok(_) => "connected".to_string(),
        Err(e) => {
            warn!("Database ping failed: {}", e);
            "disconnected".to_string()
        }
    };

    // 获取适配器统计信息
    let adapter_stats = state.adapter_manager.get_adapter_stats().await;

    let detailed_status = DetailedHealthStatus {
        database: database_status,
        adapters: adapter_stats,
        system_info: SystemInfo {
            uptime: "unknown".to_string(), // TODO: 实现真实的运行时间
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        api_key_health: "Available via /api/health/api-keys endpoint".to_string(),
    };

    response::success(detailed_status)
}
