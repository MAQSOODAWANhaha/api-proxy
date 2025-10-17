//! 简单的应用上下文（DI 容器）骨架
//!
//! 持有跨模块共享的服务实例（AuthService/CacheManager/TraceSystem 等），便于在测试中注入替身实现。

use std::sync::Arc;

use crate::auth::{
    AuthService, oauth_client::OAuthClient, oauth_token_refresh_task::OAuthTokenRefreshTask,
    rate_limit_dist::DistributedRateLimiter, smart_api_key_provider::SmartApiKeyProvider,
};
use crate::cache::CacheManager;
use crate::config::AppConfig;
use crate::key_pool::api_key_health::ApiKeyHealthChecker;
use crate::trace::TraceSystem;
use sea_orm::DatabaseConnection;

#[derive(Clone)]
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub database: Arc<DatabaseConnection>,
    pub cache: Arc<CacheManager>,
    pub auth_service: Arc<AuthService>,
    pub rate_limiter: Arc<DistributedRateLimiter>,
    pub trace_system: Option<Arc<TraceSystem>>,
    pub api_key_health_checker: Option<Arc<ApiKeyHealthChecker>>,
    pub oauth_client: Option<Arc<OAuthClient>>,
    pub smart_api_key_provider: Option<Arc<SmartApiKeyProvider>>,
    pub oauth_token_refresh_task: Option<Arc<OAuthTokenRefreshTask>>,
}

impl AppContext {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        config: Arc<AppConfig>,
        database: Arc<DatabaseConnection>,
        cache: Arc<CacheManager>,
        auth_service: Arc<AuthService>,
        rate_limiter: Arc<DistributedRateLimiter>,
        trace_system: Option<Arc<TraceSystem>>,
        api_key_health_checker: Option<Arc<ApiKeyHealthChecker>>,
        oauth_client: Option<Arc<OAuthClient>>,
        smart_api_key_provider: Option<Arc<SmartApiKeyProvider>>,
        oauth_token_refresh_task: Option<Arc<OAuthTokenRefreshTask>>,
    ) -> Self {
        Self {
            config,
            database,
            cache,
            auth_service,
            rate_limiter,
            trace_system,
            api_key_health_checker,
            oauth_client,
            smart_api_key_provider,
            oauth_token_refresh_task,
        }
    }
}
