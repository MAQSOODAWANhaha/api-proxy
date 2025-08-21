//! # Pingora 代理服务器
//!
//! 基于 Pingora 0.5.0 实现的高性能 AI 代理服务器

use super::builder::ProxyServerBuilder;
use crate::config::AppConfig;
use crate::error::{ProxyError, Result};
// 使用 tracing 替代 log
use crate::trace::UnifiedTraceSystem;
use pingora_core::server::{Server, configuration::Opt};
use pingora_proxy::http_proxy_service;
use std::sync::Arc;

/// Pingora 代理服务器
pub struct PingoraProxyServer {
    config: Arc<AppConfig>,
    /// 共享数据库连接
    db: Option<Arc<sea_orm::DatabaseConnection>>,
    /// 统一追踪系统
    trace_system: Option<Arc<UnifiedTraceSystem>>,
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
        trace_system: Arc<UnifiedTraceSystem>,
    ) -> Self {
        let mut server = Self::new(config);
        server.db = Some(db);
        server.trace_system = Some(trace_system);
        server
    }

    /// 启动服务器
    pub async fn start(self) -> Result<()> {
        // 跳过env_logger初始化，因为我们已经使用tracing了
        // env_logger::init();

        // 创建服务器配置
        tracing::info!("Creating Pingora server configuration...");
        let opt = Opt::default();
        let mut server = Server::new(Some(opt)).map_err(|e| {
            ProxyError::server_init(format!("Failed to create Pingora server: {}", e))
        })?;

        tracing::info!("Bootstrapping Pingora server...");
        server.bootstrap();

        // 使用构建器创建所有组件
        let mut builder = ProxyServerBuilder::new(self.config.clone());

        // 如果有共享数据库连接，使用它
        if let Some(shared_db) = &self.db {
            builder = builder.with_database(shared_db.clone());
        }

        let components = builder.build_components().await?;

        // 创建 HTTP 代理服务
        let mut proxy_service = http_proxy_service(&server.configuration, components.proxy_service);

        // 添加监听地址
        proxy_service.add_tcp(&builder.get_server_address());

        // 已移除 TLS/HTTPS 支持，若配置了 https_port，仅记录告警
        if self.config.server.as_ref().map_or(0, |s| s.https_port) > 0 {
            let https_port = self.config.server.as_ref().map_or(0, |s| s.https_port);
            tracing::warn!(
                "HTTPS/TLS support is disabled. Configured https_port={} will be ignored",
                https_port
            );
        }

        // 注册服务并启动
        server.add_service(proxy_service);

        tracing::info!(
            "Starting Pingora proxy server on {}",
            builder.get_server_address()
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
