//! # Pingora 代理服务器
//!
//! 基于 Pingora 的高性能代理服务器实现

use crate::config::AppConfig;
use crate::error::{ProxyError, Result};
use crate::proxy::builder::ProxyServerBuilder;
use pingora_core::prelude::*;
use pingora_core::server::configuration::Opt;
use pingora_proxy::http_proxy_service;
use std::sync::Arc;

/// Pingora 代理服务器
pub struct ProxyServer {
    config: Arc<AppConfig>,
    server: Option<Server>,
}

impl ProxyServer {
    /// 创建新的代理服务器实例
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: Arc::new(config),
            server: None,
        }
    }

    /// 初始化 Pingora 服务器
    pub async fn init(&mut self) -> Result<()> {
        // 创建 Pingora 服务器配置
        let mut opt = Opt::default();
        opt.daemon = false;
        opt.nocapture = false;
        opt.test = false;
        opt.upgrade = false;

        // 初始化 Pingora 服务器
        let mut server = Server::new(Some(opt)).map_err(|e| {
            ProxyError::server_init(format!("Failed to create Pingora server: {}", e))
        })?;

        // 使用构建器创建所有组件
        let mut builder = ProxyServerBuilder::new(self.config.clone());
        let components = builder.build_components().await?;

        // 配置 HTTP 代理服务
        let mut http_proxy = http_proxy_service(&server.configuration, components.proxy_service);
        http_proxy.add_tcp(&builder.get_server_address());
        server.add_service(http_proxy);


        self.server = Some(server);

        tracing::info!(
            "Pingora proxy server initialized on {}",
            builder.get_server_address()
        );

        Ok(())
    }

    /// 启动代理服务器
    pub async fn start(&mut self) -> Result<()> {
        if self.server.is_none() {
            self.init().await?;
        }

        let server = self
            .server
            .take()
            .ok_or_else(|| ProxyError::server_init("Server not initialized"))?;

        tracing::info!("Starting Pingora proxy server...");

        // run_forever 返回 ! 类型，永不返回
        server.run_forever();
    }

    /// 优雅关闭服务器
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Gracefully shutting down proxy server...");

        // Pingora 服务器的优雅关闭由信号处理器处理
        // 这里可以添加额外的清理逻辑

        Ok(())
    }

    /// 获取服务器状态
    pub fn is_running(&self) -> bool {
        self.server.is_some()
    }

    /// 获取配置引用
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// 获取服务器地址
    pub fn get_server_address(&self) -> String {
        format!(
            "{}:{}",
            self.config.server.as_ref().map_or("0.0.0.0", |s| &s.host),
            self.config.server.as_ref().map_or(8080, |s| s.port)
        )
    }
}

impl std::fmt::Debug for ProxyServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyServer")
            .field(
                "host",
                &self.config.server.as_ref().map_or("0.0.0.0", |s| &s.host),
            )
            .field(
                "port",
                &self.config.server.as_ref().map_or(8080, |s| s.port),
            )
            .field("is_running", &self.is_running())
            .finish()
    }
}
