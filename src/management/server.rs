//! # 管理服务器
//!
//! Axum HTTP服务器，提供管理和监控API
#![allow(
    clippy::too_many_arguments,
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::unnecessary_wraps
)]

use super::middleware::{IpFilterConfig, ip_filter_middleware, timezone_middleware};
use crate::app::{context::AppContext, task_scheduler::TaskScheduler, tasks::TaskType};
use crate::auth::oauth_client::ApiKeyAuthentication;
use crate::auth::{
    ApiKeyOAuthRefreshService, ApiKeyOAuthStateService, AuthService,
    api_key_oauth_token_refresh_task::ApiKeyOAuthTokenRefreshTask,
};
use crate::config::AppConfig;
use crate::error::{ProxyError, Result};
use crate::key_pool::ApiKeySchedulerService;
use crate::logging::{LogComponent, LogStage};
use crate::{linfo, lwarn};
use axum::Router;
use axum::routing::get;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

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
            api_prefix: "/api".to_string(),
        }
    }
}

/// 管理端服务集合
#[derive(Clone)]
pub struct ManagementServices {
    auth_service: Arc<AuthService>,
    api_key_scheduler_service: Arc<ApiKeySchedulerService>,
    oauth_token_refresh_task: Arc<ApiKeyOAuthTokenRefreshTask>,
    oauth_client: Arc<ApiKeyAuthentication>,
    api_key_oauth_state_service: Arc<ApiKeyOAuthStateService>,
    api_key_oauth_refresh_service: Arc<ApiKeyOAuthRefreshService>,
}

impl ManagementServices {
    #[must_use]
    pub fn auth_service(&self) -> Arc<AuthService> {
        Arc::clone(&self.auth_service)
    }

    #[must_use]
    pub fn api_key_scheduler_service(&self) -> Arc<ApiKeySchedulerService> {
        Arc::clone(&self.api_key_scheduler_service)
    }

    #[must_use]
    pub fn oauth_token_refresh_task(&self) -> Arc<ApiKeyOAuthTokenRefreshTask> {
        Arc::clone(&self.oauth_token_refresh_task)
    }

    #[must_use]
    pub fn oauth_client(&self) -> Arc<ApiKeyAuthentication> {
        Arc::clone(&self.oauth_client)
    }

    #[must_use]
    pub fn api_key_oauth_state_service(&self) -> Arc<ApiKeyOAuthStateService> {
        Arc::clone(&self.api_key_oauth_state_service)
    }

    #[must_use]
    pub fn api_key_oauth_refresh_service(&self) -> Arc<ApiKeyOAuthRefreshService> {
        Arc::clone(&self.api_key_oauth_refresh_service)
    }
}

/// 管理服务器应用状态
#[derive(Clone)]
pub struct ManagementState {
    pub database: Arc<DatabaseConnection>,
    pub config: Arc<AppConfig>,
    context: Arc<AppContext>,
    services: Arc<ManagementServices>,
}

impl ManagementState {
    pub fn new(context: Arc<AppContext>) -> Result<Self> {
        let oauth_token_refresh_task = context
            .tasks()
            .get_task::<ApiKeyOAuthTokenRefreshTask>(TaskType::ApiKeyOAuthTokenRefresh)
            .ok_or_else(|| {
                ProxyError::internal("oauth_token_refresh task not registered in AppTasks")
            })?;

        let services = ManagementServices {
            auth_service: context.services().auth_service(),
            api_key_scheduler_service: context.services().api_key_scheduler_service(),
            oauth_token_refresh_task,
            oauth_client: context.services().oauth_client(),
            api_key_oauth_state_service: context.services().api_key_oauth_state_service(),
            api_key_oauth_refresh_service: context.services().api_key_refresh_service(),
        };
        let database = context.database();
        let config = context.config();

        Ok(Self {
            database,
            config,
            context,
            services: Arc::new(services),
        })
    }

    #[must_use]
    pub const fn context_arc(&self) -> &Arc<AppContext> {
        &self.context
    }

    #[must_use]
    pub fn database(&self) -> Arc<DatabaseConnection> {
        Arc::clone(&self.database)
    }

    #[must_use]
    pub fn config(&self) -> Arc<AppConfig> {
        Arc::clone(&self.config)
    }

    /// 获取密钥池服务的便捷方法
    #[must_use]
    pub fn key_pool(&self) -> Arc<ApiKeySchedulerService> {
        self.services.api_key_scheduler_service()
    }

    #[must_use]
    pub fn auth_service(&self) -> Arc<AuthService> {
        self.services.auth_service()
    }

    #[must_use]
    pub fn oauth_token_refresh_task(&self) -> Arc<ApiKeyOAuthTokenRefreshTask> {
        self.services.oauth_token_refresh_task()
    }

    #[must_use]
    pub fn oauth_client(&self) -> Arc<ApiKeyAuthentication> {
        self.services.oauth_client()
    }

    #[must_use]
    pub fn api_key_oauth_state_service(&self) -> Arc<ApiKeyOAuthStateService> {
        self.services.api_key_oauth_state_service()
    }

    #[must_use]
    pub fn api_key_oauth_refresh_service(&self) -> Arc<ApiKeyOAuthRefreshService> {
        self.services.api_key_oauth_refresh_service()
    }

    #[must_use]
    pub fn services(&self) -> &ManagementServices {
        self.services.as_ref()
    }

    #[must_use]
    pub fn scheduler(&self) -> Arc<TaskScheduler> {
        self.context.tasks().scheduler()
    }
}

impl Deref for ManagementState {
    type Target = AppContext;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

/// 管理服务器
pub struct ManagementServer {
    /// 配置
    config: ManagementConfig,
    /// 应用状态
    state: Arc<ManagementState>,
    /// 路由器
    router: Router,
}

impl ManagementServer {
    /// 创建新的管理服务器
    pub fn new(config: ManagementConfig, state: Arc<ManagementState>) -> Result<Self> {
        let router = Self::create_router(&state, &config)?;

        Ok(Self {
            config,
            state,
            router,
        })
    }

    /// 创建路由器
    fn create_router(state: &Arc<ManagementState>, config: &ManagementConfig) -> Result<Router> {
        // 使用统一的路由配置，现在认证中间件已在 routes.rs 中应用
        let api_routes = super::routes::create_routes(state.as_ref().clone());

        // 静态文件服务配置
        let static_dir = std::path::Path::new("/app/static");
        let static_service = if static_dir.exists() {
            linfo!(
                "system",
                LogStage::Startup,
                LogComponent::ServerSetup,
                "static_service_enabled",
                "Enabling static file service from /app/static"
            );
            // 创建静态文件服务，支持SPA应用的fallback
            Some(
                ServeDir::new(static_dir)
                    .not_found_service(ServeFile::new("/app/static/index.html")),
            )
        } else {
            lwarn!(
                "system",
                LogStage::Startup,
                LogComponent::ServerSetup,
                "static_dir_not_found",
                "Static directory /app/static not found, static files will not be served"
            );
            None
        };

        let mut app = Router::new()
            .nest(&config.api_prefix, api_routes) // 将所有API路由嵌套在/api下
            .route(
                "/ping",
                get(crate::management::handlers::system::ping_handler),
            );

        // 添加静态文件服务（如果可用）
        if let Some(service) = static_service {
            // 使用静态文件服务处理所有未匹配的路由（包括根路径）
            app = app.fallback_service(service);
        } else {
            // 如果没有静态文件，则提供API信息页面
            app = app.route("/", get(crate::management::handlers::system::root_handler));
        }

        // 创建IP过滤配置
        let ip_filter_config =
            IpFilterConfig::from_strings(&config.allowed_ips, &config.denied_ips).unwrap_or_else(
                |e| {
                    lwarn!(
                        "system",
                        LogStage::Startup,
                        LogComponent::ServerSetup,
                        "ip_filter_config_fail",
                        &format!("Failed to create IP filter config: {e}, using default")
                    );
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
                let origins = config
                    .cors_origins
                    .iter()
                    .map(|origin| origin.parse::<axum::http::HeaderValue>())
                    .collect::<std::result::Result<Vec<_>, axum::http::header::InvalidHeaderValue>>(
                    );

                match origins {
                    Ok(origins) => {
                        cors_layer = cors_layer.allow_origin(origins);
                    }
                    Err(e) => {
                        lwarn!(
                            "system",
                            LogStage::Startup,
                            LogComponent::ServerSetup,
                            "cors_config_fail",
                            &format!(
                                "Invalid CORS origin configuration: {e}, falling back to allow any"
                            )
                        );
                        cors_layer = cors_layer.allow_origin(Any);
                    }
                }
            }

            app = app.layer(service_builder.layer(cors_layer));
        } else {
            app = app.layer(service_builder);
        }

        // 添加时区中间件
        app = app.layer(axum::middleware::from_fn(timezone_middleware));

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
        let bind_address = self.config.bind_address.clone();
        let ip = bind_address.parse::<std::net::IpAddr>().map_err(|e| {
            crate::error!(
                Config,
                format!("Invalid management bind address '{bind_address}': {e}")
            )
        })?;
        let addr = SocketAddr::new(ip, self.config.port);

        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "server_start",
            &format!("Starting management server on {addr}")
        );

        let listener = TcpListener::bind(&addr).await?;

        axum::serve(
            listener,
            self.router
                .into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|e| crate::error!(Network, format!("Management server error: {e}")))?;

        Ok(())
    }

    /// 获取绑定地址
    #[must_use]
    pub fn bind_address(&self) -> SocketAddr {
        SocketAddr::new(self.config.bind_address.parse().unwrap(), self.config.port)
    }
}

// 根路径处理器与 Ping 已迁移至 handlers::system
