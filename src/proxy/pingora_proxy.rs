//! # Pingora 代理服务器
//!
//! 基于 Pingora 0.5.0 实现的高性能 AI 代理服务器

use super::builder::ProxyServerBuilder;
use crate::config::AppConfig;
use crate::error::{ProxyError, Result};
// 使用 tracing 替代 log
use crate::logging::{LogComponent, LogStage};
use crate::trace::TraceSystem;
use crate::{proxy_info, proxy_warn};
use pingora_core::server::{Server, configuration::Opt};
use pingora_proxy::http_proxy_service;
use std::sync::Arc;

/// Pingora 代理服务器
pub struct PingoraProxyServer {
    config: Arc<AppConfig>,
    /// 共享数据库连接
    db: Option<Arc<sea_orm::DatabaseConnection>>,
    /// 追踪系统（TraceSystem）
    trace_system: Option<Arc<TraceSystem>>,
}

impl PingoraProxyServer {
    /// 创建新的代理服务器
    pub fn new(config: AppConfig) -> Self {
        let config_arc = Arc::new(config);

        Self {
            config: config_arc,
            db: None,
            trace_system: None,
        }
    }

    /// 创建新的代理服务器（带数据库连接）
    pub fn new_with_db(config: AppConfig, db: Arc<sea_orm::DatabaseConnection>) -> Self {
        let mut server = Self::new(config);
        server.db = Some(db);
        server
    }

    /// 创建新的代理服务器（带数据库连接和追踪系统）
    pub fn new_with_db_and_trace(
        config: AppConfig,
        db: Arc<sea_orm::DatabaseConnection>,
        trace_system: Arc<TraceSystem>,
    ) -> Self {
        let mut server = Self::new(config);
        server.db = Some(db);
        server.trace_system = Some(trace_system);
        server
    }

    /// 创建Pingora服务器选项（基本配置）
    fn create_pingora_options(&self) -> Result<Opt> {
        let opt = Opt::default();

        proxy_info!(
            "server_init",
            LogStage::RequestStart,
            LogComponent::UpstreamService,
            "creating_pingora_options",
            "创建Pingora基础配置选项",
        );

        Ok(opt)
    }

    // 超时配置现在从数据库 user_service_apis.timeout_seconds 动态获取
    // 不再需要全局的超时配置方法

    /// 启动服务器
    pub async fn start(self) -> Result<()> {
        // 跳过env_logger初始化，因为我们已经使用tracing了
        // env_logger::init();

        // 创建服务器配置
        proxy_info!(
            "server_init",
            LogStage::RequestStart,
            LogComponent::UpstreamService,
            "creating_server_config",
            "创建Pingora服务器配置"
        );
        let opt = self.create_pingora_options()?;
        let mut server = Server::new(Some(opt)).map_err(|e| {
            ProxyError::server_init(format!("Failed to create Pingora server: {}", e))
        })?;

        proxy_info!(
            "server_init",
            LogStage::RequestStart,
            LogComponent::UpstreamService,
            "bootstrapping_server",
            "启动Pingora服务器引导"
        );
        server.bootstrap();

        proxy_info!(
            "server_init",
            LogStage::RequestStart,
            LogComponent::UpstreamService,
            "timeout_config_dynamic",
            "超时配置现在从数据库动态获取"
        );

        // 使用构建器创建所有组件
        let mut builder = ProxyServerBuilder::new(self.config.clone());

        // 如果有共享数据库连接，使用它
        if let Some(shared_db) = &self.db {
            builder = builder.with_database(shared_db.clone());
        }

        // 关键修复：如果有trace_system，传递给builder
        if let Some(trace_system) = &self.trace_system {
            builder = builder.with_trace_system(trace_system.clone());
            proxy_info!(
                "server_init",
                LogStage::RequestStart,
                LogComponent::UpstreamService,
                "using_trace_system",
                "在Pingora代理构建器中使用提供的追踪系统"
            );
        } else {
            proxy_warn!(
                "server_init",
                LogStage::RequestStart,
                LogComponent::UpstreamService,
                "no_trace_system",
                "未提供追踪系统给Pingora代理 - 追踪将被禁用"
            );
        }

        let components = builder.build_components().await?;

        // 创建 HTTP 代理服务
        let mut proxy_service = http_proxy_service(&server.configuration, components.proxy_service);

        // 添加监听地址
        proxy_service.add_tcp(&builder.get_server_address());

        // 注册服务并启动
        server.add_service(proxy_service);

        proxy_info!(
            "server_init",
            LogStage::RequestStart,
            LogComponent::UpstreamService,
            "starting_server",
            "启动Pingora代理服务器",
            address = builder.get_server_address()
        );

        // 在 tokio 任务中运行服务器以避免运行时冲突
        let handle = tokio::task::spawn_blocking(move || {
            server.run_forever();
        });

        // 等待服务器任务完成（实际上不会完成，因为 run_forever 不会返回）
        handle
            .await
            .map_err(|e| ProxyError::server_start(format!("Pingora server task failed: {}", e)))?
    }

    // TODO: 实现健康检查服务
    // fn create_health_check_service(&self) -> impl pingora_core::services::Service + 'static {
    //     ...
    // }
}
