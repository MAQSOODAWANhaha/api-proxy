//! # Pingora 代理服务器构建器
//!
//! 提供统一的服务器初始化逻辑，避免代码重复

use crate::auth::{AuthManager, AuthService};
use crate::cache::CacheManager;
use crate::config::{AppConfig, ProviderConfigManager};
use crate::error::{ProxyError, Result};
use crate::linfo;
use crate::logging::{LogComponent, LogStage};
use crate::pricing::PricingCalculatorService;
use crate::proxy::{
    AuthenticationService, RequestTransformService, ResponseTransformService, StatisticsService,
    TracingService, UpstreamService, service::ProxyService,
};
use crate::scheduler::ApiKeyPoolManager;
use crate::scheduler::api_key_health::ApiKeyHealthChecker;
use crate::trace::TraceSystem;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// 代理服务器组件构建器
///
/// 统一管理数据库连接、缓存管理器、服务商配置等组件的创建逻辑
pub struct ProxyServerBuilder {
    config: Arc<AppConfig>,
    db: Option<Arc<DatabaseConnection>>,
    cache: Option<Arc<CacheManager>>,
    provider_config_manager: Option<Arc<ProviderConfigManager>>,
    trace_system: Option<Arc<TraceSystem>>,
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
    pub fn with_trace_system(mut self, trace_system: Arc<TraceSystem>) -> Self {
        self.trace_system = Some(trace_system);
        self
    }

    /// 创建或获取数据库连接
    pub async fn ensure_database(&mut self) -> Result<Arc<DatabaseConnection>> {
        if let Some(db) = &self.db {
            return Ok(db.clone());
        }
        self.config
            .database
            .ensure_database_path()
            .map_err(|e| ProxyError::server_init(format!("数据库路径设置失败: {}", e)))?;
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
    pub fn ensure_cache(&mut self) -> Result<Arc<CacheManager>> {
        if let Some(cache) = &self.cache {
            return Ok(cache.clone());
        }
        let cache = Arc::new(
            CacheManager::new(&self.config.cache)
                .map_err(|e| ProxyError::cache(format!("缓存管理器创建失败: {}", e)))?,
        );
        self.cache = Some(cache.clone());
        Ok(cache)
    }

    /// 创建或获取服务商配置管理器
    pub fn ensure_provider_config_manager(
        &mut self,
        db: Arc<DatabaseConnection>,
        cache: Arc<CacheManager>,
    ) -> Arc<ProviderConfigManager> {
        if let Some(manager) = &self.provider_config_manager {
            return manager.clone();
        }
        let manager = Arc::new(ProviderConfigManager::new(db, cache));
        self.provider_config_manager = Some(manager.clone());
        manager
    }

    /// 创建统一认证管理器
    async fn create_auth_manager(
        &self,
        db: Arc<DatabaseConnection>,
        cache: Arc<CacheManager>,
    ) -> Result<Arc<AuthManager>> {
        let auth_config = Arc::new(crate::auth::types::AuthConfig::default());
        let jwt_manager = Arc::new(
            crate::auth::JwtManager::new(auth_config.clone())
                .map_err(|e| ProxyError::server_init(format!("JWT管理器创建失败: {}", e)))?,
        );
        let api_key_manager = Arc::new(crate::auth::ApiKeyManager::new(
            db.clone(),
            auth_config.clone(),
            cache.clone(),
            Arc::new(self.config.cache.clone()),
        ));
        let auth_service = Arc::new(AuthService::new(
            jwt_manager,
            api_key_manager,
            db.clone(),
            auth_config.clone(),
        ));
        let auth_manager = AuthManager::new(auth_service, auth_config, db, cache).await?;
        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::Builder,
            "auth_manager_created",
            "统一认证管理器创建完成"
        );
        Ok(Arc::new(auth_manager))
    }

    /// 创建代理服务实例
    pub async fn create_proxy_service(
        &self,
        db: Arc<DatabaseConnection>,
        cache: Arc<CacheManager>,
        _provider_config_manager: Arc<ProviderConfigManager>,
    ) -> pingora_core::Result<ProxyService> {
        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::Builder,
            "creating_ai_proxy_service",
            "创建AI代理服务"
        );

        let auth_manager = self
            .create_auth_manager(db.clone(), cache.clone())
            .await
            .map_err(|_| pingora_core::Error::new_str("认证管理器创建失败"))?;

        // --- 服务依赖组装 ---
        let health_checker = Arc::new(ApiKeyHealthChecker::new(db.clone(), None));
        let api_key_pool = Arc::new(ApiKeyPoolManager::new(db.clone(), health_checker));
        let pricing_calculator = Arc::new(PricingCalculatorService::new(db.clone()));

        let auth_service = Arc::new(AuthenticationService::new(
            auth_manager,
            db.clone(),
            cache.clone(),
            api_key_pool,
        ));
        let stats_service = Arc::new(StatisticsService::new(pricing_calculator));
        let trace_service = Arc::new(TracingService::new(
            self.trace_system
                .as_ref()
                .and_then(|ts| ts.immediate_tracer()),
        ));
        let upstream_service = Arc::new(UpstreamService::new(db.clone()));
        let req_transform_service = Arc::new(RequestTransformService::new(db.clone()));
        let resp_transform_service = Arc::new(ResponseTransformService::new());

        ProxyService::new(
            db.clone(), // 直接注入DB
            auth_service,
            stats_service,
            trace_service,
            upstream_service,
            req_transform_service,
            resp_transform_service,
        )
    }

    /// 构建完整的组件集合
    pub async fn build_components(&mut self) -> Result<ProxyServerComponents> {
        let db = self.ensure_database().await?;
        let cache = self.ensure_cache()?;
        let provider_config_manager =
            self.ensure_provider_config_manager(db.clone(), cache.clone());

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

    /// 获取代理服务器监听地址
    pub fn get_server_address(&self) -> String {
        let proxy_port = self.config.get_proxy_port();
        let host = self
            .config
            .dual_port
            .as_ref()
            .map_or("0.0.0.0", |d| &d.proxy.http.host);
        format!("{}:{}", host, proxy_port)
    }
}

/// 代理服务器组件集合
pub struct ProxyServerComponents {
    pub config: Arc<AppConfig>,
    pub db: Arc<DatabaseConnection>,
    pub cache: Arc<CacheManager>,
    pub provider_config_manager: Arc<ProviderConfigManager>,
    pub proxy_service: ProxyService,
    pub trace_system: Option<Arc<TraceSystem>>,
}
