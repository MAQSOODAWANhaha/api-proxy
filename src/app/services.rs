use crate::{app::context::AppContext, proxy::state::ProxyState};
use std::sync::Arc;

/// 所有共享服务的统一载体
pub struct SharedServices {
    pub app_context: Arc<AppContext>,
    pub proxy_state: Arc<ProxyState>,
}
