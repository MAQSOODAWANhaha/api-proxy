use crate::app::context::AppContext;
use crate::management::server::ManagementState;
use crate::proxy::state::ProxyState;
use std::sync::Arc;

/// 运行时需要共享的核心上下文与状态封装
pub struct SharedServices {
    pub app_context: Arc<AppContext>,
    pub proxy_state: Arc<ProxyState>,
    pub management_state: Arc<ManagementState>,
}
