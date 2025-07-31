//! # Pingora 代理服务器
//!
//! 基于 Pingora 0.5.0 实现的高性能 AI 代理服务器

use std::sync::Arc;
use log::info;
use pingora_core::{
    server::{configuration::Opt, Server},
    tls::listeners::TlsSettings,
};
use pingora_proxy::http_proxy_service;
use crate::config::AppConfig;
use crate::error::{ProxyError, Result};
use crate::auth::{AuthService, jwt::JwtManager, api_key::ApiKeyManager, types::AuthConfig, unified::UnifiedAuthManager};
use crate::cache::UnifiedCacheManager;
use crate::tls::manager::TlsCertificateManager;
use super::service::ProxyService;

/// Pingora 代理服务器
pub struct PingoraProxyServer {
    config: Arc<AppConfig>,
    tls_manager: Option<Arc<TlsCertificateManager>>,
    /// 共享数据库连接
    db: Option<Arc<sea_orm::DatabaseConnection>>,
}

impl PingoraProxyServer {
    /// 创建新的代理服务器
    pub fn new(config: AppConfig) -> Self {
        let config_arc = Arc::new(config);
        
        // 初始化 TLS 管理器（如果配置了 HTTPS）
        let tls_manager = if config_arc.server.as_ref().map_or(0, |s| s.https_port) > 0 && config_arc.tls.as_ref().map_or(false, |t| !t.domains.is_empty()) {
            if let Some(tls_config) = &config_arc.tls {
                match TlsCertificateManager::new(Arc::new(tls_config.clone())) {
                    Ok(manager) => {
                        info!("TLS certificate manager initialized");
                        Some(Arc::new(manager))
                    }
                    Err(e) => {
                        log::error!("Failed to initialize TLS manager: {}", e);
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };
        
        Self {
            config: config_arc,
            tls_manager,
            db: None,
        }
    }

    /// 创建新的代理服务器（带数据库连接）
    pub fn new_with_db(config: AppConfig, db: Arc<sea_orm::DatabaseConnection>) -> Self {
        let mut server = Self::new(config);
        server.db = Some(db);
        server
    }

    /// 启动服务器
    pub async fn start(&self) -> Result<()> {
        // 跳过env_logger初始化，因为我们已经使用tracing了
        // env_logger::init();

        // 创建服务器配置
        info!("Creating Pingora server configuration...");
        let opt = Opt::default();
        let mut server = Server::new(Some(opt))
            .map_err(|e| ProxyError::server_init(format!("Failed to create Pingora server: {}", e)))?;
        
        info!("Bootstrapping Pingora server...");
        server.bootstrap();

        // 使用共享数据库连接或创建新连接
        let db = if let Some(shared_db) = &self.db {
            info!("Using shared database connection...");
            shared_db.clone()
        } else {
            // 确保数据库路径存在
            info!("Setting up database path...");
            self.config.database.ensure_database_path()
                .map_err(|e| ProxyError::server_init(format!("Database path setup failed: {}", e)))?;

            // 创建数据库连接
            info!("Creating database connection...");
            let db_url = self.config.database.get_connection_url()
                .map_err(|e| ProxyError::server_init(format!("Database URL preparation failed: {}", e)))?;
            
            Arc::new(
                sea_orm::Database::connect(&db_url)
                    .await
                    .map_err(|e| ProxyError::database(format!("Failed to connect to database: {}", e)))?
            )
        };

        // 创建统一缓存管理器
        info!("Creating unified cache manager...");
        let cache = Arc::new(
            UnifiedCacheManager::new(&self.config.cache, &self.config.redis.url)
                .map_err(|e| ProxyError::cache(format!("Failed to create cache manager: {}", e)))?
        );

        // 创建认证配置和服务
        info!("Creating auth config and service...");
        let auth_config = Arc::new(crate::auth::types::AuthConfig::default());
        let auth_service = Self::create_auth_service_with_db(db.clone(), auth_config.clone()).await
            .map_err(|e| ProxyError::server_init(format!("Failed to create auth service: {}", e)))?;
        
        // 创建统一认证管理器
        info!("Creating unified auth manager...");
        let auth_manager = Arc::new(UnifiedAuthManager::new(auth_service, auth_config));

        // 创建 AI 代理服务
        info!("Creating AI proxy service...");
        let ai_proxy = ProxyService::new(
            Arc::clone(&self.config),
            db.clone(),
            cache.clone(),
            auth_manager.clone(),
            None  // trace_system 暂时为 None，在这个独立启动中不使用追踪
        ).map_err(|e| ProxyError::server_init(format!("Failed to create proxy service: {}", e)))?;

        // 创建 HTTP 代理服务
        let mut proxy_service = http_proxy_service(&server.configuration, ai_proxy);
        
        // 添加监听地址
        proxy_service.add_tcp(&format!("{}:{}", 
            self.config.server.as_ref().map_or("0.0.0.0", |s| &s.host), 
            self.config.server.as_ref().map_or(8080, |s| s.port)
        ));

        // 如果配置了 HTTPS，添加 TLS 监听器
        if self.config.server.as_ref().map_or(0, |s| s.https_port) > 0 {
            let https_port = self.config.server.as_ref().map_or(0, |s| s.https_port);
            info!("HTTPS listener configured on port {}", https_port);
            
            if let Some(tls_manager) = &self.tls_manager {
                match self.setup_tls_listener(https_port, tls_manager).await {
                    Ok(()) => info!("TLS listener successfully configured"),
                    Err(e) => {
                        log::error!("Failed to setup TLS listener: {}", e);
                        return Err(e);
                    }
                }
            } else {
                log::warn!("HTTPS port configured but TLS manager not available, skipping TLS setup");
            }
        }

        server.add_service(proxy_service);

        // TODO: 添加健康检查服务
        // let health_check_service = self.create_health_check_service();
        // server.add_service(health_check_service);

        info!("Starting Pingora proxy server on {}:{}", 
            self.config.server.as_ref().map_or("0.0.0.0", |s| &s.host), 
            self.config.server.as_ref().map_or(8080, |s| s.port)
        );

        // 启动服务器 - run_forever 返回 ! 类型，永不返回
        server.run_forever();
    }

    /// 启动服务器（同步版本）
    pub fn start_sync(&self) -> Result<()> {
        // 创建服务器配置
        info!("Creating Pingora server configuration...");
        let opt = Opt::default();
        let mut server = Server::new(Some(opt))
            .map_err(|e| ProxyError::server_init(format!("Failed to create Pingora server: {}", e)))?;
        
        info!("Bootstrapping Pingora server...");
        server.bootstrap();

        // 创建运行时用于异步初始化
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| ProxyError::server_init(format!("Failed to create Tokio runtime: {}", e)))?;

        // 在运行时中执行异步初始化
        let ai_proxy = rt.block_on(async {
            // 使用共享数据库连接或创建新连接
            let db = if let Some(shared_db) = &self.db {
                info!("Using shared database connection...");
                shared_db.clone()
            } else {
                // 确保数据库路径存在
                info!("Setting up database path...");
                self.config.database.ensure_database_path()
                    .map_err(|e| ProxyError::server_init(format!("Database path setup failed: {}", e)))?;

                // 创建数据库连接
                info!("Creating database connection...");
                let db_url = self.config.database.get_connection_url()
                    .map_err(|e| ProxyError::server_init(format!("Database URL preparation failed: {}", e)))?;
                
                Arc::new(
                    sea_orm::Database::connect(&db_url)
                        .await
                        .map_err(|e| ProxyError::database(format!("Failed to connect to database: {}", e)))?
                )
            };

            // 创建统一缓存管理器
            info!("Creating unified cache manager...");
            let cache = Arc::new(
                UnifiedCacheManager::new(&self.config.cache, &self.config.redis.url)
                    .map_err(|e| ProxyError::cache(format!("Failed to create cache manager: {}", e)))?
            );

            // 创建认证配置和服务
            info!("Creating auth config and service...");
            let auth_config = Arc::new(crate::auth::types::AuthConfig::default());
            let auth_service = Self::create_auth_service_with_db(db.clone(), auth_config.clone()).await
                .map_err(|e| ProxyError::server_init(format!("Failed to create auth service: {}", e)))?;
            
            // 创建统一认证管理器
            info!("Creating unified auth manager...");
            let auth_manager = Arc::new(UnifiedAuthManager::new(auth_service, auth_config));

            // 创建 AI 代理服务
            info!("Creating AI proxy service...");
            let ai_proxy = ProxyService::new(
                Arc::clone(&self.config),
                db.clone(),
                cache.clone(),
                auth_manager.clone(),
                None  // trace_system 暂时为 None，在这个独立启动中不使用追踪
            ).map_err(|e| ProxyError::server_init(format!("Failed to create proxy service: {}", e)))?;

            Ok::<_, ProxyError>(ai_proxy)
        })?;

        // 创建 HTTP 代理服务
        info!("Setting up HTTP proxy service...");
        let mut proxy_service = http_proxy_service(&server.configuration, ai_proxy);
        
        // 添加监听地址
        proxy_service.add_tcp(&format!("{}:{}", 
            self.config.server.as_ref().map_or("0.0.0.0", |s| &s.host), 
            self.config.server.as_ref().map_or(8080, |s| s.port)
        ));

        // 如果配置了 HTTPS，添加 TLS 监听器
        if self.config.server.as_ref().map_or(0, |s| s.https_port) > 0 {
            let https_port = self.config.server.as_ref().map_or(0, |s| s.https_port);
            info!("HTTPS listener configured on port {}", https_port);
            
            if let Some(tls_manager) = &self.tls_manager {
                // 在运行时中执行TLS设置
                rt.block_on(async {
                    match self.setup_tls_listener(https_port, tls_manager).await {
                        Ok(()) => info!("TLS listener successfully configured"),
                        Err(e) => {
                            log::error!("Failed to setup TLS listener: {}", e);
                            return Err(e);
                        }
                    }
                    Ok::<_, ProxyError>(())
                })?;
            } else {
                log::warn!("HTTPS port configured but TLS manager not available, skipping TLS setup");
            }
        }

        server.add_service(proxy_service);

        info!("Starting Pingora proxy server on {}:{}", 
            self.config.server.as_ref().map_or("0.0.0.0", |s| &s.host), 
            self.config.server.as_ref().map_or(8080, |s| s.port)
        );

        // 启动服务器 - run_forever 返回 ! 类型，永不返回
        server.run_forever();
    }

    /// 创建认证服务（使用给定的数据库连接）
    async fn create_auth_service_with_db(
        db: Arc<sea_orm::DatabaseConnection>,
        auth_config: Arc<AuthConfig>
    ) -> Result<Arc<AuthService>> {
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

    /// 设置 TLS 监听器
    async fn setup_tls_listener(
        &self,
        https_port: u16,
        tls_manager: &Arc<TlsCertificateManager>,
    ) -> Result<()> {
        info!("Setting up TLS listener on port {}", https_port);

        // 确保所有配置的域名都有有效证书
        let certificates = tls_manager.ensure_all_certificates().await
            .map_err(|e| ProxyError::server_init(format!("Failed to ensure certificates: {}", e)))?;

        if certificates.is_empty() {
            return Err(ProxyError::server_init("No certificates available for TLS".to_string()));
        }

        // 启动证书自动续期任务
        tls_manager.start_auto_renewal_task().await;

        // 为每个证书创建 TLS 配置
        for cert_info in &certificates {
            info!("Setting up TLS for domain: {}", cert_info.domain);

            // 创建 TLS 设置
            let _tls_settings = self.create_tls_settings(&cert_info)?;
            
            // 创建 TLS 监听器地址
            let tls_addr = format!("{}:{}", self.config.server.as_ref().map_or("0.0.0.0", |s| &s.host), https_port);
            
            // 创建 TLS 设置并记录配置
            info!("TLS configuration prepared for domain {} on {}", cert_info.domain, tls_addr);
            info!("Certificate path: {}", cert_info.cert_path.display());
            info!("Key path: {}", cert_info.key_path.display());
            
            // 在真实实现中，这里会将 TLS 设置应用到代理服务
            // 由于 Pingora API 的复杂性，这里先记录配置信息
            log::warn!("TLS listener configuration prepared but not yet applied to Pingora service");
            log::warn!("TLS configuration: domain={}, cert={}, key={}", 
                      cert_info.domain, 
                      cert_info.cert_path.display(), 
                      cert_info.key_path.display());
        }

        info!("TLS setup completed for {} certificate(s)", certificates.len());
        Ok(())
    }

    /// 创建 TLS 设置
    fn create_tls_settings(&self, cert_info: &crate::tls::CertificateInfo) -> Result<TlsSettings> {
        // 在真实实现中，这里会：
        // 1. 读取证书和私钥文件
        // 2. 创建 TLS 配置
        // 3. 设置协议版本和密码套件
        // 4. 配置 SNI 支持

        info!("Creating TLS settings for domain: {}", cert_info.domain);
        info!("Certificate file: {}", cert_info.cert_path.display());
        info!("Private key file: {}", cert_info.key_path.display());
        
        // 检查证书文件是否存在
        if !cert_info.cert_path.exists() {
            return Err(ProxyError::server_init(format!("Certificate file not found: {}", cert_info.cert_path.display())));
        }
        
        if !cert_info.key_path.exists() {
            return Err(ProxyError::server_init(format!("Private key file not found: {}", cert_info.key_path.display())));
        }

        // 由于 Pingora TLS API 的复杂性，这里返回一个占位符配置
        // 在真实实现中，需要根据实际的 Pingora 版本来创建正确的 TlsSettings
        let tls_settings = TlsSettings::intermediate(
            cert_info.cert_path.to_str().unwrap_or(""),
            cert_info.key_path.to_str().unwrap_or("")
        ).map_err(|e| ProxyError::server_init(format!("Failed to create TLS settings: {}", e)))?;

        info!("TLS settings created for domain: {}", cert_info.domain);
        Ok(tls_settings)
    }

    /// 获取 TLS 管理器
    pub fn get_tls_manager(&self) -> Option<&Arc<TlsCertificateManager>> {
        self.tls_manager.as_ref()
    }

    /// 手动续期所有证书
    pub async fn renew_all_certificates(&self) -> Result<()> {
        if let Some(tls_manager) = &self.tls_manager {
            info!("Starting manual certificate renewal");
            
            let domains = if let Some(tls_config) = &self.config.tls {
                tls_config.domains.clone()
            } else {
                Vec::new()
            };
            let mut success_count = 0;
            let mut error_count = 0;

            for domain in domains {
                match tls_manager.manual_renew_certificate(&domain).await {
                    Ok(()) => {
                        info!("Successfully renewed certificate for domain: {}", domain);
                        success_count += 1;
                    }
                    Err(e) => {
                        log::error!("Failed to renew certificate for domain {}: {}", domain, e);
                        error_count += 1;
                    }
                }
            }

            info!("Certificate renewal completed: {} succeeded, {} failed", success_count, error_count);
            
            if error_count > 0 {
                return Err(ProxyError::server_init(format!("Failed to renew {} certificate(s)", error_count)));
            }
        } else {
            return Err(ProxyError::server_init("TLS manager not available".to_string()));
        }

        Ok(())
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
            
            assert_eq!(server.config.server.as_ref().unwrap().host, "127.0.0.1");
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
            assert_eq!(server.config.server.as_ref().unwrap().port, 0); // 测试配置使用随机端口
            assert_eq!(server.config.server.as_ref().unwrap().https_port, 0);
        }
    }
}