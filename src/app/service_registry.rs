use crate::app::resources::AppResources;
use crate::auth::oauth_client::ApiKeyOauthService;
use crate::auth::{
    ApiKeyAuthenticationService, ApiKeyManager, ApiKeyOAuthRefreshService, ApiKeyOAuthStateService,
    api_key_usage_limit_service::ApiKeyUsageLimitService, jwt::JwtManager,
};
use crate::error::{Context, Result};
use crate::key_pool::{ApiKeyHealthService, ApiKeySchedulerService};
use crate::trace::ApiKeyTraceService;
use std::sync::Arc;

/// 业务服务集合:封装身份、限流、追踪等核心服务实例
///
/// 职责：
/// - 管理核心业务逻辑服务（Service 层）
/// - 不包含任务调度逻辑（Task 层由 `AppTasks` 管理）
///
/// 认证架构说明：
/// - **管理端 (Management)**：使用 JWT 进行用户身份验证（登录、权限控制）
/// - **代理端 (Proxy)**：使用 API Key 进行服务认证，不涉及 JWT
/// - `ApiKeyAuthenticationService` 被两端共享，但代理端仅使用其 API Key 验证功能
pub struct AppServices {
    authentication: Arc<ApiKeyAuthenticationService>,
    usage_limit: Arc<ApiKeyUsageLimitService>,
    trace: Arc<ApiKeyTraceService>,
    oauth: Arc<ApiKeyOauthService>,
    oauth_state: Arc<ApiKeyOAuthStateService>,
    scheduler: Arc<ApiKeySchedulerService>,
    refresh: Arc<ApiKeyOAuthRefreshService>,
    health: Arc<ApiKeyHealthService>,
}

impl AppServices {
    /// 根据基础资源初始化业务服务
    pub fn initialize(resources: &Arc<AppResources>) -> Result<Arc<Self>> {
        let config = resources.config();
        let database = resources.database();
        let cache = resources.cache();

        // 从配置加载认证配置（安全字段通过环境变量提供）
        let jwt_manager =
            Arc::new(JwtManager::new(&config.auth).context("JWT manager init failed")?);
        let api_key_manager = Arc::new(ApiKeyManager::new(
            database.clone(),
            cache.clone(),
            Arc::new(config.cache.clone()),
        ));
        let authentication = Arc::new(ApiKeyAuthenticationService::new(
            jwt_manager,
            api_key_manager,
            database.clone(),
        ));

        let rate_limiter = Arc::new(ApiKeyUsageLimitService::new(cache, database.clone()));

        let trace_system = Arc::new(ApiKeyTraceService::new_immediate(database.clone()));

        let api_key_health = Arc::new(ApiKeyHealthService::new(database.clone()));

        let api_key_scheduler = Arc::new(ApiKeySchedulerService::new(
            database.clone(),
            api_key_health.clone(),
        ));

        let oauth_client = Arc::new(ApiKeyOauthService::new(database));
        let api_key_oauth_state_service = oauth_client.api_key_oauth_state_service();
        let api_key_refresh_service = oauth_client.api_key_oauth_refresh_service();

        Ok(Arc::new(Self {
            authentication,
            usage_limit: rate_limiter,
            trace: trace_system,
            oauth: oauth_client,
            oauth_state: api_key_oauth_state_service,
            scheduler: api_key_scheduler,
            refresh: api_key_refresh_service,
            health: api_key_health,
        }))
    }

    #[must_use]
    pub fn api_key_authentication_service(&self) -> Arc<ApiKeyAuthenticationService> {
        Arc::clone(&self.authentication)
    }

    #[must_use]
    pub fn api_key_rate_limit_service(&self) -> Arc<ApiKeyUsageLimitService> {
        Arc::clone(&self.usage_limit)
    }

    #[must_use]
    pub fn api_key_trace_service(&self) -> Arc<ApiKeyTraceService> {
        Arc::clone(&self.trace)
    }

    #[must_use]
    pub fn api_key_scheduler_service(&self) -> Arc<ApiKeySchedulerService> {
        Arc::clone(&self.scheduler)
    }

    #[must_use]
    pub fn api_key_oauth_service(&self) -> Arc<ApiKeyOauthService> {
        Arc::clone(&self.oauth)
    }

    #[must_use]
    pub fn api_key_oauth_state_service(&self) -> Arc<ApiKeyOAuthStateService> {
        Arc::clone(&self.oauth_state)
    }

    #[must_use]
    pub fn api_key_refresh_service(&self) -> Arc<ApiKeyOAuthRefreshService> {
        Arc::clone(&self.refresh)
    }

    #[must_use]
    pub fn api_key_health_service(&self) -> Arc<ApiKeyHealthService> {
        Arc::clone(&self.health)
    }
}
