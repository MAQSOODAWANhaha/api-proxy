//! # Pingora 代理服务器
//!
//! 基于 Pingora 的高性能代理服务器实现

use crate::config::AppConfig;
use crate::error::{ProxyError, Result};
use crate::proxy::service::ProxyService;
use crate::auth::{AuthService, jwt::JwtManager, api_key::ApiKeyManager, types::AuthConfig};
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

        // 设置日志级别
        let _log_level = "info";

        // 初始化 Pingora 服务器
        let mut server = Server::new(Some(opt)).map_err(|e| {
            ProxyError::server_init(format!("Failed to create Pingora server: {}", e))
        })?;

        // 设置错误处理器
        // 配置日志和 PID 文件
        // server.configuration.error_log = Some(format!("logs/proxy-error.log"));
        // server.configuration.pid_file = Some(format!("logs/proxy.pid"));

        // 创建认证服务
        let auth_service = Self::create_auth_service().await
            .map_err(|e| ProxyError::server_init(format!("Failed to create auth service: {}", e)))?;

        // 创建健康检查服务
        let health_service = Arc::new(crate::health::HealthCheckService::new(None));

        // 创建代理服务
        let proxy_service = ProxyService::new(Arc::clone(&self.config), auth_service.clone(), health_service.clone())
            .map_err(|e| ProxyError::server_init(format!("Failed to create proxy service: {}", e)))?;

        // 配置 HTTP 代理服务
        let mut http_proxy = http_proxy_service(&server.configuration, proxy_service);

        http_proxy.add_tcp(&format!(
            "{}:{}",
            self.config.server.host, self.config.server.port
        ));

        server.add_service(http_proxy);

        // 如果启用了 HTTPS，添加 HTTPS 监听器
        if self.config.server.https_port > 0 {
            let health_service_https = Arc::new(crate::health::HealthCheckService::new(None));
            let proxy_service_https = ProxyService::new(Arc::clone(&self.config), auth_service.clone(), health_service_https.clone())
                .map_err(|e| ProxyError::server_init(format!("Failed to create HTTPS proxy service: {}", e)))?;
            
            let _https_proxy = http_proxy_service(&server.configuration, proxy_service_https);

            // 添加 TLS 配置
            // 注意：这里需要根据实际的证书配置来设置
            // https_proxy.add_tls(&format!("{}:{}",
            //     self.config.server.host,
            //     self.config.server.https_port
            // ), None);

            // server.add_service(https_proxy);

            tracing::info!("HTTPS listener will be configured when TLS certificates are available");
        }

        self.server = Some(server);

        tracing::info!(
            "Pingora proxy server initialized on {}:{}",
            self.config.server.host,
            self.config.server.port
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

    /// 创建认证服务
    async fn create_auth_service() -> Result<Arc<AuthService>> {
        // 创建数据库连接
        let db = Arc::new(
            sea_orm::Database::connect("sqlite::memory:")
                .await
                .map_err(|e| ProxyError::database(format!("Failed to connect to database: {}", e)))?
        );

        // 创建认证配置
        let auth_config = Arc::new(AuthConfig::default());

        // 创建 JWT 管理器
        let jwt_manager = Arc::new(
            JwtManager::new(auth_config.clone())
                .map_err(|e| ProxyError::server_init(format!("Failed to create JWT manager: {}", e)))?
        );

        // 创建 API 密钥管理器
        let api_key_manager = Arc::new(ApiKeyManager::new(db.clone(), auth_config.clone()));

        // 创建认证服务
        let auth_service = AuthService::new(jwt_manager, api_key_manager, db, auth_config);

        Ok(Arc::new(auth_service))
    }
}

impl std::fmt::Debug for ProxyServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyServer")
            .field("host", &self.config.server.host)
            .field("port", &self.config.server.port)
            .field("is_running", &self.is_running())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::helpers::init_test_env;

    #[test]
    fn test_proxy_server_creation() {
        init_test_env();

        #[cfg(any(test, feature = "testing"))]
        {
            use crate::testing::fixtures::TestConfig;
            
            let config = TestConfig::app_config();
            let server = ProxyServer::new(config);

            assert!(!server.is_running());
            assert_eq!(server.config().server.host, "127.0.0.1");
        }
    }

    #[tokio::test]
    async fn test_proxy_server_init() {
        init_test_env();

        #[cfg(any(test, feature = "testing"))]
        {
            use crate::testing::fixtures::TestConfig;
            
            let config = TestConfig::app_config();
            let mut server = ProxyServer::new(config);

            // 注意：这个测试可能需要实际的网络权限
            // 在 CI 环境中可能需要跳过
            if std::env::var("CI").is_err() {
                assert!(server.init().await.is_ok());
                assert!(server.is_running());
            }
        }
    }
}
