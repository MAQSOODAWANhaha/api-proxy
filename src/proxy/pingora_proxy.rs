//! # Pingora 代理服务器
//!
//! 基于 Pingora 0.5.0 实现的高性能 AI 代理服务器

use std::sync::Arc;
use log::info;
use pingora_core::{
    server::{configuration::Opt, Server},
};
use pingora_proxy::http_proxy_service;
use crate::config::AppConfig;
use crate::error::{ProxyError, Result};
use crate::auth::{AuthService, jwt::JwtManager, api_key::ApiKeyManager, types::AuthConfig};
use super::service::ProxyService;

/// Pingora 代理服务器
pub struct PingoraProxyServer {
    config: Arc<AppConfig>,
}

impl PingoraProxyServer {
    /// 创建新的代理服务器
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// 启动服务器
    pub async fn start(&self) -> Result<()> {
        // 初始化日志
        env_logger::init();

        // 创建服务器配置
        let opt = Opt::default();
        let mut server = Server::new(Some(opt))
            .map_err(|e| ProxyError::server_init(format!("Failed to create Pingora server: {}", e)))?;
        
        server.bootstrap();

        // 创建认证服务
        let auth_service = Self::create_auth_service().await
            .map_err(|e| ProxyError::server_init(format!("Failed to create auth service: {}", e)))?;

        // 创建健康检查服务
        let health_service = Arc::new(crate::health::HealthCheckService::new(None));

        // 创建 AI 代理服务
        let ai_proxy = ProxyService::new(Arc::clone(&self.config), auth_service, health_service)
            .map_err(|e| ProxyError::server_init(format!("Failed to create proxy service: {}", e)))?;

        // 创建 HTTP 代理服务
        let mut proxy_service = http_proxy_service(&server.configuration, ai_proxy);
        
        // 添加监听地址
        proxy_service.add_tcp(&format!("{}:{}", 
            self.config.server.host, 
            self.config.server.port
        ));

        // 如果配置了 HTTPS，添加 TLS 监听器
        if self.config.server.https_port > 0 {
            let https_port = self.config.server.https_port;
            info!("HTTPS listener configured on port {}", https_port);
            // TODO: 实现 TLS 配置
            // proxy_service.add_tls(...);
        }

        server.add_service(proxy_service);

        // TODO: 添加健康检查服务
        // let health_check_service = self.create_health_check_service();
        // server.add_service(health_check_service);

        info!("Starting Pingora proxy server on {}:{}", 
            self.config.server.host, 
            self.config.server.port
        );

        // 启动服务器 - run_forever 返回 ! 类型，永不返回
        server.run_forever();
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

    // TODO: 实现健康检查服务
    // fn create_health_check_service(&self) -> impl pingora_core::services::Service + 'static {
    //     ...
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::helpers::init_test_env;

    #[test]
    fn test_pingora_proxy_server_creation() {
        init_test_env();
        
        #[cfg(any(test, feature = "testing"))]
        {
            use crate::testing::fixtures::TestConfig;
            
            let config = TestConfig::app_config();
            let server = PingoraProxyServer::new(config);
            
            assert_eq!(server.config.server.host, "127.0.0.1");
        }
    }

    #[test]
    fn test_server_configuration() {
        init_test_env();
        
        #[cfg(any(test, feature = "testing"))]
        {
            use crate::testing::fixtures::TestConfig;
            
            let config = TestConfig::app_config();
            let server = PingoraProxyServer::new(config);
            
            // 验证配置正确性
            assert_eq!(server.config.server.port, 0); // 测试配置使用随机端口
            assert_eq!(server.config.server.https_port, 0);
        }
    }
}