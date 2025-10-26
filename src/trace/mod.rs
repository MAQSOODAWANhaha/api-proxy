pub mod immediate;
pub mod manager;

pub use immediate::ImmediateProxyTracer;
pub use manager::TraceManager;
use std::sync::Arc;

/// 追踪系统入口（TraceSystem）
///
/// 现在只支持即时写入模式，确保长时间请求不会导致内存泄漏
pub struct TraceSystem {
    tracer: Arc<ImmediateProxyTracer>,
}

impl TraceSystem {
    /// 创建新的即时写入追踪系统
    #[must_use]
    pub fn new_immediate(db: Arc<sea_orm::DatabaseConnection>) -> Self {
        let tracer = Arc::new(ImmediateProxyTracer::new(db));
        Self { tracer }
    }

    /// 获取即时写入追踪器
    #[must_use]
    pub fn immediate_tracer(&self) -> Option<Arc<ImmediateProxyTracer>> {
        Some(self.tracer.clone())
    }
}
