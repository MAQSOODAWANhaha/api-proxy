//! # Pingora 代理服务器构建器
//!
//! 提供统一的服务器初始化逻辑，避免代码重复

use crate::auth::{AuthService, RefactoredUnifiedAuthManager};
use crate::cache::UnifiedCacheManager;
use crate::config::{AppConfig, ProviderConfigManager};
use crate::error::{ProxyError, Result};
use crate::proxy::service::ProxyService;
use crate::trace::UnifiedTraceSystem;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// 代理服务器组件构建器
///
/// 统一管理数据库连接、缓存管理器、服务商配置等组件的创建逻辑
pub struct ProxyServerBuilder {
    config: Arc<AppConfig>,
    db: Option<Arc<DatabaseConnection>>,
    cache: Option<Arc<UnifiedCacheManager>>,
    provider_config_manager: Option<Arc<ProviderConfigManager>>,
    trace_system: Option<Arc<UnifiedTraceSystem>>,
}

impl ProxyServerBuilder {
    /// 创建新的构建器实例
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self {
            config,
            db: None,
            cache: None,
            provider_config_manager: None,
            trace_system: None,
        }
    }

    /// 设置共享数据库连接
    pub fn with_database(mut self, db: Arc<DatabaseConnection>) -> Self {
        self.db = Some(db);
        self
    }

    /// 设置追踪系统
    pub fn with_trace_system(mut self, trace_system: Arc<UnifiedTraceSystem>) -> Self {
        self.trace_system = Some(trace_system);
        self
    }

    /// 创建或获取数据库连接
    pub async fn ensure_database(&mut self) -> Result<Arc<DatabaseConnection>> {
        if let Some(db) = &self.db {
            tracing::info!("使用共享数据库连接");
            return Ok(db.clone());
        }

        tracing::info!("设置数据库路径");
        self.config
            .database
            .ensure_database_path()
            .map_err(|e| ProxyError::server_init(format!("数据库路径设置失败: {}", e)))?;

        tracing::info!("创建数据库连接");
        let db_url = self
            .config
            .database
            .get_connection_url()
            .map_err(|e| ProxyError::server_init(format!("数据库URL准备失败: {}", e)))?;

        let db = Arc::new(
            sea_orm::Database::connect(&db_url)
                .await
                .map_err(|e| ProxyError::database(format!("数据库连接失败: {}", e)))?,
        );

        self.db = Some(db.clone());
        Ok(db)
    }

    /// 创建或获取统一缓存管理器
    pub fn ensure_cache(&mut self) -> Result<Arc<UnifiedCacheManager>> {
        if let Some(cache) = &self.cache {
            return Ok(cache.clone());
        }

        tracing::info!("创建统一缓存管理器");
        let cache = Arc::new(
            UnifiedCacheManager::new(&self.config.cache, &self.config.redis.url)
                .map_err(|e| ProxyError::cache(format!("缓存管理器创建失败: {}", e)))?,
        );

        self.cache = Some(cache.clone());
        Ok(cache)
    }

    /// 创建或获取服务商配置管理器
    pub fn ensure_provider_config_manager(
        &mut self,
        db: Arc<DatabaseConnection>,
        cache: Arc<UnifiedCacheManager>,
    ) -> Arc<ProviderConfigManager> {
        if let Some(manager) = &self.provider_config_manager {
            return manager.clone();
        }

        tracing::info!("创建服务商配置管理器");
        let manager = Arc::new(ProviderConfigManager::new(db, cache));
        self.provider_config_manager = Some(manager.clone());
        manager
    }

    /// 创建统一认证管理器
    async fn create_auth_manager(
        &self,
        db: Arc<DatabaseConnection>,
        cache: Arc<UnifiedCacheManager>,
    ) -> Result<Arc<RefactoredUnifiedAuthManager>> {
        // 创建认证配置 - 使用默认配置
        let auth_config = Arc::new(crate::auth::types::AuthConfig::default());
        
        // 创建JWT和API密钥管理器
        let jwt_manager = Arc::new(
            crate::auth::JwtManager::new(auth_config.clone())
                .map_err(|e| ProxyError::server_init(format!("JWT管理器创建失败: {}", e)))?
        );
        let api_key_manager = Arc::new(crate::auth::ApiKeyManager::new(db.clone(), auth_config.clone()));
        
        // 创建认证服务
        let auth_service = Arc::new(AuthService::new(
            jwt_manager,
            api_key_manager,
            db.clone(),
            auth_config.clone(),
        ));
        
        // 创建统一认证管理器
        let auth_manager = RefactoredUnifiedAuthManager::new(
            auth_service,
            auth_config,
            db,
            cache,
        ).await?;

        tracing::info!("统一认证管理器创建完成");
        Ok(Arc::new(auth_manager))
    }

    /// 创建代理服务实例
    pub async fn create_proxy_service(
        &self,
        db: Arc<DatabaseConnection>,
        cache: Arc<UnifiedCacheManager>,
        provider_config_manager: Arc<ProviderConfigManager>,
    ) -> pingora_core::Result<ProxyService> {
        tracing::info!("创建AI代理服务");

        // 创建统一认证管理器
        let auth_manager = self.create_auth_manager(db.clone(), cache.clone())
            .await
            .map_err(|_| pingora_core::Error::new_str("认证管理器创建失败"))?;

        ProxyService::new(
            self.config.clone(),
            db,
            cache,
            provider_config_manager,
            self.trace_system.clone(),
            auth_manager,
        )
    }

    /// 构建完整的组件集合
    ///
    /// 按照正确的依赖顺序创建所有必需的组件
    pub async fn build_components(&mut self) -> Result<ProxyServerComponents> {
        // 1. 确保数据库连接
        let db = self.ensure_database().await?;

        // 2. 确保缓存管理器
        let cache = self.ensure_cache()?;

        // 3. 确保服务商配置管理器
        let provider_config_manager =
            self.ensure_provider_config_manager(db.clone(), cache.clone());

        // 4. 创建代理服务
        let proxy_service = self
            .create_proxy_service(db.clone(), cache.clone(), provider_config_manager.clone())
            .await
            .map_err(|e| ProxyError::server_init(format!("代理服务创建失败: {}", e)))?;

        Ok(ProxyServerComponents {
            config: self.config.clone(),
            db,
            cache,
            provider_config_manager,
            proxy_service,
            trace_system: self.trace_system.clone(),
        })
    }

    /// 获取服务器监听地址
    pub fn get_server_address(&self) -> String {
        format!(
            "{}:{}",
            self.config.server.as_ref().map_or("0.0.0.0", |s| &s.host),
            self.config.server.as_ref().map_or(8080, |s| s.port)
        )
    }

    /// 检查是否配置了HTTPS
    pub fn has_https_config(&self) -> bool {
        self.config.server.as_ref().map_or(0, |s| s.https_port) > 0
    }

    /// 获取HTTPS端口
    pub fn get_https_port(&self) -> u16 {
        self.config.server.as_ref().map_or(0, |s| s.https_port)
    }
}

/// 代理服务器组件集合
///
/// 包含创建代理服务器所需的所有组件
pub struct ProxyServerComponents {
    pub config: Arc<AppConfig>,
    pub db: Arc<DatabaseConnection>,
    pub cache: Arc<UnifiedCacheManager>,
    pub provider_config_manager: Arc<ProviderConfigManager>,
    pub proxy_service: ProxyService,
    pub trace_system: Option<Arc<UnifiedTraceSystem>>,
}
