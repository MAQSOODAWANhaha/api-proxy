//! 简单的应用上下文（DI 容器）骨架
//!
//! 持有跨模块共享的服务实例（AuthService/CacheManager/TraceSystem 等），便于在测试中注入替身实现。

use crate::auth::{
    AuthService, oauth_client::OAuthClient, oauth_token_refresh_task::OAuthTokenRefreshTask,
    rate_limit_dist::DistributedRateLimiter, smart_api_key_provider::SmartApiKeyProvider,
};
use crate::cache::CacheManager;
use crate::config::AppConfig;
use crate::error::{ProxyError, Result};
use crate::key_pool::KeyPoolService;
use crate::trace::TraceSystem;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub database: Arc<DatabaseConnection>,
    pub cache: Arc<CacheManager>,
    pub auth_service: Arc<AuthService>,
    pub rate_limiter: Arc<DistributedRateLimiter>,
    pub trace_system: Arc<TraceSystem>,
    pub key_pool_service: Arc<KeyPoolService>,
    pub oauth_client: Arc<OAuthClient>,
    pub smart_api_key_provider: Arc<SmartApiKeyProvider>,
    pub oauth_token_refresh_task: Arc<OAuthTokenRefreshTask>,
}

impl AppContext {
    /// 返回一个新的 `AppContextBuilder` 实例
    #[must_use]
    pub fn builder() -> AppContextBuilder {
        AppContextBuilder::new()
    }
}

/// `AppContext` 的构建器（链式调用风格）
#[derive(Default)]
pub struct AppContextBuilder {
    config: Option<Arc<AppConfig>>,
    database: Option<Arc<DatabaseConnection>>,
    cache: Option<Arc<CacheManager>>,
    auth_service: Option<Arc<AuthService>>,
    rate_limiter: Option<Arc<DistributedRateLimiter>>,
    trace_system: Option<Arc<TraceSystem>>,
    key_pool_service: Option<Arc<KeyPoolService>>,
    oauth_client: Option<Arc<OAuthClient>>,
    smart_api_key_provider: Option<Arc<SmartApiKeyProvider>>,
    oauth_token_refresh_task: Option<Arc<OAuthTokenRefreshTask>>,
}

impl AppContextBuilder {
    /// 创建一个新的 `AppContextBuilder`
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_config(mut self, config: Arc<AppConfig>) -> Self {
        self.config = Some(config);
        self
    }

    #[must_use]
    pub fn with_database(mut self, database: Arc<DatabaseConnection>) -> Self {
        self.database = Some(database);
        self
    }

    #[must_use]
    pub fn with_cache(mut self, cache: Arc<CacheManager>) -> Self {
        self.cache = Some(cache);
        self
    }

    #[must_use]
    pub fn with_auth_service(mut self, auth_service: Arc<AuthService>) -> Self {
        self.auth_service = Some(auth_service);
        self
    }

    #[must_use]
    pub fn with_rate_limiter(mut self, rate_limiter: Arc<DistributedRateLimiter>) -> Self {
        self.rate_limiter = Some(rate_limiter);
        self
    }

    #[must_use]
    pub fn with_trace_system(mut self, trace_system: Arc<TraceSystem>) -> Self {
        self.trace_system = Some(trace_system);
        self
    }

    #[must_use]
    pub fn with_key_pool_service(mut self, key_pool_service: Arc<KeyPoolService>) -> Self {
        self.key_pool_service = Some(key_pool_service);
        self
    }

    #[must_use]
    pub fn with_oauth_client(mut self, oauth_client: Arc<OAuthClient>) -> Self {
        self.oauth_client = Some(oauth_client);
        self
    }

    #[must_use]
    pub fn with_smart_api_key_provider(
        mut self,
        smart_api_key_provider: Arc<SmartApiKeyProvider>,
    ) -> Self {
        self.smart_api_key_provider = Some(smart_api_key_provider);
        self
    }

    #[must_use]
    pub fn with_oauth_token_refresh_task(
        mut self,
        oauth_token_refresh_task: Arc<OAuthTokenRefreshTask>,
    ) -> Self {
        self.oauth_token_refresh_task = Some(oauth_token_refresh_task);
        self
    }

    /// 构建最终的 `AppContext`
    ///
    /// # Errors
    ///
    /// 如果有任何必要的服务未被设置，则返回 `ProxyError::BuilderIncomplete`。
    pub fn build(self) -> Result<AppContext> {
        Ok(AppContext {
            config: self.config.ok_or(ProxyError::BuilderContext("config"))?,
            database: self
                .database
                .ok_or(ProxyError::BuilderContext("database"))?,
            cache: self.cache.ok_or(ProxyError::BuilderContext("cache"))?,
            auth_service: self
                .auth_service
                .ok_or(ProxyError::BuilderContext("auth_service"))?,
            rate_limiter: self
                .rate_limiter
                .ok_or(ProxyError::BuilderContext("rate_limiter"))?,
            trace_system: self
                .trace_system
                .ok_or(ProxyError::BuilderContext("trace_system"))?,
            key_pool_service: self
                .key_pool_service
                .ok_or(ProxyError::BuilderContext("key_pool_service"))?,
            oauth_client: self
                .oauth_client
                .ok_or(ProxyError::BuilderContext("oauth_client"))?,
            smart_api_key_provider: self
                .smart_api_key_provider
                .ok_or(ProxyError::BuilderContext("smart_api_key_provider"))?,
            oauth_token_refresh_task: self
                .oauth_token_refresh_task
                .ok_or(ProxyError::BuilderContext("oauth_token_refresh_task"))?,
        })
    }
}
