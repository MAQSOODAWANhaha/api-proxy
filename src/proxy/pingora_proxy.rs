//! # Pingora 代理服务器
//!
//! 基于 Pingora 实现的高性能 AI 代理服务器

use crate::error::{ProxyError, Result};
use crate::linfo;
use crate::logging::{LogComponent, LogStage};
use crate::proxy::state::ProxyState;
use pingora_core::server::{Server, configuration::Opt};
use pingora_proxy::http_proxy_service;
use std::sync::Arc;

/// Pingora 代理服务器
pub struct PingoraProxyServer {
    state: Arc<ProxyState>,
}

impl PingoraProxyServer {
    /// 创建新的代理服务器
    #[must_use]
    pub const fn new(state: Arc<ProxyState>) -> Self {
        Self { state }
    }

    /// 创建Pingora服务器选项（基本配置）
    fn create_pingora_options() -> Opt {
        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "creating_pingora_options",
            "创建Pingora基础配置选项",
        );
        Opt {
            daemon: false,   // 在前台运行
            upgrade: false,  // 不支持在线升级
            nocapture: true, // 不捕获标准输出/错误
            ..Opt::default()
        }
    }

    /// 获取代理服务器监听地址
    #[must_use]
    pub fn get_server_address(&self) -> String {
        let config = &self.state.context.config;
        let proxy_port = config.get_proxy_port();
        let host = config
            .dual_port
            .as_ref()
            .map_or("0.0.0.0", |d| &d.proxy.http.host);
        format!("{host}:{proxy_port}")
    }

    /// 启动服务器
    #[allow(clippy::cognitive_complexity)]
    pub async fn start(self) -> Result<()> {
        let opt = Self::create_pingora_options();
        let mut server = Server::new(Some(opt))
            .map_err(|e| ProxyError::internal_with_source("Failed to create Pingora server", e))?;

        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "bootstrapping_server",
            "启动Pingora服务器引导"
        );
        server.bootstrap();

        let proxy_service = crate::proxy::service::ProxyService::new(self.state.clone())
            .map_err(|e| ProxyError::internal_with_source("Failed to create proxy service", e))?;

        let mut http_service = http_proxy_service(&server.configuration, proxy_service);

        let server_address = self.get_server_address();
        http_service.add_tcp(&server_address);

        server.add_service(http_service);

        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "starting_server",
            "启动Pingora代理服务器",
            address = &server_address
        );

        let handle = tokio::task::spawn_blocking(move || {
            server.run_forever();
        });

        handle
            .await
            .map_err(|e| ProxyError::internal_with_source("Pingora server task failed", e))?
    }
}
