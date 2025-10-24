use crate::app::context::AppContext;
use crate::auth::rate_limit_dist::DistributedRateLimiter;
use crate::collect::service::CollectService;
use crate::key_pool::KeyPoolService;
use crate::proxy::authentication_service::AuthenticationService;
use crate::proxy::request_transform_service::RequestTransformService;
use crate::proxy::response_transform_service::ResponseTransformService;
use crate::proxy::upstream_service::UpstreamService;
use crate::trace::TraceManager;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// 代理服务的共享状态
///
/// 持有所有代理服务运行所需的依赖项。
#[derive(Clone)]
pub struct ProxyState {
    pub context: Arc<AppContext>,
    pub db: Arc<DatabaseConnection>,
    pub auth_service: Arc<AuthenticationService>,
    pub collect_service: Arc<CollectService>,
    pub trace_manager: Arc<TraceManager>,
    pub upstream_service: Arc<UpstreamService>,
    pub req_transform_service: Arc<RequestTransformService>,
    pub resp_transform_service: Arc<ResponseTransformService>,
    pub key_pool_service: Arc<KeyPoolService>,
    pub rate_limiter: Arc<DistributedRateLimiter>,
}
