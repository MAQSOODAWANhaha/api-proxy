use crate::app::context::AppContext;
use crate::auth::rate_limit_dist::RateLimiter;
use crate::collect::service::CollectService;
use crate::key_pool::ApiKeySchedulerService;
use crate::proxy::authentication_service::AuthenticationService;
use crate::proxy::request_transform_service::RequestTransformService;
use crate::proxy::response_transform_service::ResponseTransformService;
use crate::proxy::upstream_service::UpstreamService;
use crate::trace::TraceManager;
use std::ops::Deref;
use std::sync::Arc;

/// 代理服务集合
#[derive(Clone)]
pub struct ProxyServices {
    pub auth_service: Arc<AuthenticationService>,
    pub collect_service: Arc<CollectService>,
    pub trace_manager: Arc<TraceManager>,
    pub upstream_service: Arc<UpstreamService>,
    pub req_transform_service: Arc<RequestTransformService>,
    pub resp_transform_service: Arc<ResponseTransformService>,
    pub key_scheduler_service: Arc<ApiKeySchedulerService>,
    pub rate_limiter: Arc<RateLimiter>,
}

/// 代理服务的共享状态
///
/// 持有所有代理服务运行所需的依赖项。
#[derive(Clone)]
pub struct ProxyState {
    context: Arc<AppContext>,
    services: Arc<ProxyServices>,
}

impl ProxyState {
    #[must_use]
    pub fn new(context: Arc<AppContext>, services: ProxyServices) -> Self {
        Self {
            context,
            services: Arc::new(services),
        }
    }

    #[must_use]
    pub fn context(&self) -> Arc<AppContext> {
        Arc::clone(&self.context)
    }

    #[must_use]
    pub fn services(&self) -> Arc<ProxyServices> {
        Arc::clone(&self.services)
    }
}

impl Deref for ProxyState {
    type Target = ProxyServices;

    fn deref(&self) -> &Self::Target {
        &self.services
    }
}
