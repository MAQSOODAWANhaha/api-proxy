//! # Pingora 代理服务器
//!
//! 基于 Pingora 0.5.0 实现的高性能 AI 代理服务器

use super::builder::ProxyServerBuilder;
use crate::cache::CacheManager;
use crate::config::AppConfig;
use crate::error::{ProxyError, Result};
use crate::logging::{LogComponent, LogStage};
use crate::trace::TraceSystem;
use crate::{linfo, lwarn};
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
    /// 共享缓存管理器
    cache: Option<Arc<CacheManager>>,
}

impl PingoraProxyServer {
    /// 创建新的代理服务器
    #[must_use]
    pub const fn new(
        config: Arc<AppConfig>,
        db: Option<Arc<sea_orm::DatabaseConnection>>,
        cache: Option<Arc<CacheManager>>,
        trace_system: Option<Arc<TraceSystem>>,
    ) -> Self {
        Self {
            config,
            db,
            trace_system,
            cache,
        }
    }

    /// 创建Pingora服务器选项（基本配置）
    fn create_pingora_options() -> Opt {
        let opt = Opt::default();

        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "creating_pingora_options",
            "创建Pingora基础配置选项",
        );

        opt
    }

    // 超时配置现在从数据库 user_service_apis.timeout_seconds 动态获取
    // 不再需要全局的超时配置方法

    /// 启动服务器
    #[allow(clippy::cognitive_complexity)]
    pub async fn start(self) -> Result<()> {
        // 跳过env_logger初始化，因为我们已经使用tracing了
        // env_logger::init();

        // 创建服务器配置
        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "creating_server_config",
            "创建Pingora服务器配置"
        );
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

        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "timeout_config_dynamic",
            "超时配置现在从数据库动态获取"
        );

        // 使用构建器创建所有组件
        let mut builder = ProxyServerBuilder::new(self.config.clone());

        // 如果有共享数据库连接，使用它
        if let Some(shared_db) = &self.db {
            builder = builder.with_database(shared_db.clone());
        }

        if let Some(cache) = &self.cache {
            builder = builder.with_cache(cache.clone());
        }

        // 关键修复：如果有trace_system，传递给builder
        if let Some(trace_system) = &self.trace_system {
            builder = builder.with_trace_system(trace_system.clone());
            linfo!(
                "system",
                LogStage::Startup,
                LogComponent::ServerSetup,
                "using_trace_system",
                "在Pingora代理构建器中使用提供的追踪系统"
            );
        } else {
            lwarn!(
                "system",
                LogStage::Startup,
                LogComponent::ServerSetup,
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

        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
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
            .map_err(|e| ProxyError::internal_with_source("Pingora server task failed", e))?
    }

    // TODO: 实现健康检查服务
    // fn create_health_check_service(&self) -> impl pingora_core::services::Service + 'static {
    //     ...
    // }
}
