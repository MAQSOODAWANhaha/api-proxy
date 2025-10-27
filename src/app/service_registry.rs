use crate::app::resources::AppResources;
use crate::auth::oauth_client::OAuthClient;
use crate::auth::{
    ApiKeyManager, ApiKeyRefreshService, AuthService, SmartApiKeyProvider, jwt::JwtManager,
    rate_limit_dist::RateLimiter, types::AuthConfig,
};
use crate::error::{Context, Result};
use crate::key_pool::{ApiKeyHealthService, ApiKeySchedulerService};
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
    oauth_client: Arc<OAuthClient>,
    smart_api_key_provider: Arc<SmartApiKeyProvider>,
    api_key_scheduler_service: Arc<ApiKeySchedulerService>,
    api_key_refresh_service: Arc<ApiKeyRefreshService>,
    api_key_health_service: Arc<ApiKeyHealthService>,
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

        let api_key_health_service = Arc::new(ApiKeyHealthService::new(database.clone(), None));

        let api_key_scheduler_service = Arc::new(ApiKeySchedulerService::new(
            database.clone(),
            api_key_health_service.clone(),
        ));

        let oauth_client = Arc::new(OAuthClient::new(database.clone()));
        let api_key_refresh_service = Arc::new(ApiKeyRefreshService::new(
            database.clone(),
            oauth_client.clone(),
        ));

        let smart_api_key_provider = Arc::new(SmartApiKeyProvider::new(
            database.clone(),
            oauth_client.clone(),
            api_key_refresh_service.clone(),
        ));

        api_key_scheduler_service
            .set_smart_provider(smart_api_key_provider.clone())
            .await;

        Ok(Arc::new(Self {
            auth_service,
            rate_limiter,
            trace_system,
            oauth_client,
            smart_api_key_provider,
            api_key_scheduler_service,
            api_key_refresh_service,
            api_key_health_service,
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
    pub fn api_key_scheduler_service(&self) -> Arc<ApiKeySchedulerService> {
        Arc::clone(&self.api_key_scheduler_service)
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
    pub fn api_key_refresh_service(&self) -> Arc<ApiKeyRefreshService> {
        Arc::clone(&self.api_key_refresh_service)
    }

    #[must_use]
    pub fn api_key_health_service(&self) -> Arc<ApiKeyHealthService> {
        Arc::clone(&self.api_key_health_service)
    }
}
