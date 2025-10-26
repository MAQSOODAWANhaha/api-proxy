use crate::app::resources::AppResources;
use crate::auth::oauth_client::OAuthClient;
use crate::auth::{
    ApiKeyManager, AuthService, OAuthTokenRefreshService, SmartApiKeyProvider, jwt::JwtManager,
    rate_limit_dist::RateLimiter, types::AuthConfig,
};
use crate::error::{Context, Result};
use crate::key_pool::{KeyPoolService, api_key_health::ApiKeyHealthChecker};
use crate::trace::TraceSystem;
use std::sync::Arc;

/// 业务服务集合：封装身份、限流、追踪等核心服务实例
///
/// 职责：
/// - 管理核心业务逻辑服务（Service 层）
/// - 不包含任务调度逻辑（Task 层由 `AppTasks` 管理）
pub struct AppServices {
    auth_service: Arc<AuthService>,
    rate_limiter: Arc<RateLimiter>,
    trace_system: Arc<TraceSystem>,
    key_pool_service: Arc<KeyPoolService>,
    oauth_client: Arc<OAuthClient>,
    smart_api_key_provider: Arc<SmartApiKeyProvider>,
    oauth_refresh_service: Arc<OAuthTokenRefreshService>,
    api_key_health_checker: Arc<ApiKeyHealthChecker>,
}

impl AppServices {
    /// 根据基础资源初始化业务服务
    pub async fn initialize(resources: &Arc<AppResources>) -> Result<Arc<Self>> {
        let config = resources.config();
        let database = resources.database();
        let cache = resources.cache();

        let auth_config = Arc::new(AuthConfig::default());
        let jwt_manager =
            Arc::new(JwtManager::new(auth_config.clone()).context("JWT manager init failed")?);
        let api_key_manager = Arc::new(ApiKeyManager::new(
            database.clone(),
            auth_config.clone(),
            cache.clone(),
            Arc::new(config.cache.clone()),
        ));
        let auth_service = Arc::new(AuthService::new(
            jwt_manager,
            api_key_manager,
            database.clone(),
            auth_config,
        ));

        let rate_limiter = Arc::new(RateLimiter::new(cache.clone(), database.clone()));

        let trace_system = Arc::new(TraceSystem::new_immediate(database.clone()));

        let api_key_health_checker = Arc::new(ApiKeyHealthChecker::new(database.clone(), None));

        let key_pool_service = Arc::new(KeyPoolService::new(
            database.clone(),
            api_key_health_checker.clone(),
        ));

        let oauth_client = Arc::new(OAuthClient::new(database.clone()));
        let oauth_refresh_service = Arc::new(OAuthTokenRefreshService::new(
            database.clone(),
            oauth_client.clone(),
        ));

        let smart_api_key_provider = Arc::new(SmartApiKeyProvider::new(
            database.clone(),
            oauth_client.clone(),
            oauth_refresh_service.clone(),
        ));

        key_pool_service
            .set_smart_provider(smart_api_key_provider.clone())
            .await;

        Ok(Arc::new(Self {
            auth_service,
            rate_limiter,
            trace_system,
            key_pool_service,
            oauth_client,
            smart_api_key_provider,
            oauth_refresh_service,
            api_key_health_checker,
        }))
    }

    #[must_use]
    pub fn auth_service(&self) -> Arc<AuthService> {
        Arc::clone(&self.auth_service)
    }

    #[must_use]
    pub fn rate_limiter(&self) -> Arc<RateLimiter> {
        Arc::clone(&self.rate_limiter)
    }

    #[must_use]
    pub fn trace_system(&self) -> Arc<TraceSystem> {
        Arc::clone(&self.trace_system)
    }

    #[must_use]
    pub fn key_pool_service(&self) -> Arc<KeyPoolService> {
        Arc::clone(&self.key_pool_service)
    }

    #[must_use]
    pub fn oauth_client(&self) -> Arc<OAuthClient> {
        Arc::clone(&self.oauth_client)
    }

    #[must_use]
    pub fn smart_api_key_provider(&self) -> Arc<SmartApiKeyProvider> {
        Arc::clone(&self.smart_api_key_provider)
    }

    #[must_use]
    pub fn oauth_refresh_service(&self) -> Arc<OAuthTokenRefreshService> {
        Arc::clone(&self.oauth_refresh_service)
    }

    #[must_use]
    pub fn api_key_health_checker(&self) -> Arc<ApiKeyHealthChecker> {
        Arc::clone(&self.api_key_health_checker)
    }
}
