use crate::app::resources::AppResources;
use crate::auth::oauth_client::ApiKeyAuthentication;
use crate::auth::{
    ApiKeyManager, ApiKeyOAuthRefreshService, ApiKeyOAuthStateService, AuthService,
    api_key_usage_limit::UsageLimiter, jwt::JwtManager,
};
use crate::error::{Context, Result};
use crate::key_pool::{ApiKeyHealthService, ApiKeySchedulerService};
use crate::trace::TraceSystem;
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
/// - `AuthService` 被两端共享，但代理端仅使用其 API Key 验证功能
pub struct AppServices {
    auth_service: Arc<AuthService>,
    rate_limiter: Arc<UsageLimiter>,
    trace_system: Arc<TraceSystem>,
    oauth_client: Arc<ApiKeyAuthentication>,
    api_key_oauth_state_service: Arc<ApiKeyOAuthStateService>,
    api_key_scheduler_service: Arc<ApiKeySchedulerService>,
    api_key_refresh_service: Arc<ApiKeyOAuthRefreshService>,
    api_key_health_service: Arc<ApiKeyHealthService>,
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
        let auth_service = Arc::new(AuthService::new(
            jwt_manager,
            api_key_manager,
            database.clone(),
        ));

        let rate_limiter = Arc::new(UsageLimiter::new(cache, database.clone()));

        let trace_system = Arc::new(TraceSystem::new_immediate(database.clone()));

        let api_key_health_service = Arc::new(ApiKeyHealthService::new(database.clone()));

        let api_key_scheduler_service = Arc::new(ApiKeySchedulerService::new(
            database.clone(),
            api_key_health_service.clone(),
        ));

        let oauth_client = Arc::new(ApiKeyAuthentication::new(database));
        let api_key_oauth_state_service = oauth_client.api_key_oauth_state_service();
        let api_key_refresh_service = oauth_client.api_key_oauth_refresh_service();

        Ok(Arc::new(Self {
            auth_service,
            rate_limiter,
            trace_system,
            oauth_client,
            api_key_oauth_state_service,
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
    pub fn rate_limiter(&self) -> Arc<UsageLimiter> {
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
    pub fn oauth_client(&self) -> Arc<ApiKeyAuthentication> {
        Arc::clone(&self.oauth_client)
    }

    #[must_use]
    pub fn api_key_oauth_state_service(&self) -> Arc<ApiKeyOAuthStateService> {
        Arc::clone(&self.api_key_oauth_state_service)
    }

    #[must_use]
    pub fn api_key_refresh_service(&self) -> Arc<ApiKeyOAuthRefreshService> {
        Arc::clone(&self.api_key_refresh_service)
    }

    #[must_use]
    pub fn api_key_health_service(&self) -> Arc<ApiKeyHealthService> {
        Arc::clone(&self.api_key_health_service)
    }
}
