//! # Proxy Trace 追踪系统
//!
//! 负责收集代理请求的详细追踪数据，用于健康状态监控和性能分析

pub mod immediate;
pub mod manager;
pub mod models;
pub mod types;

pub use immediate::{ImmediateProxyTracer, ImmediateTracerConfig};
pub use manager::TraceManager;
pub use models::*;
pub use types::TraceStats;

use serde::{Deserialize, Serialize};
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
    pub fn new_immediate(
        db: Arc<sea_orm::DatabaseConnection>,
        config: ImmediateTracerConfig,
    ) -> Self {
        let tracer = Arc::new(ImmediateProxyTracer::new(db, config));

        Self { tracer }
    }

    /// 获取即时写入追踪器
    #[must_use]
    pub fn immediate_tracer(&self) -> Option<Arc<ImmediateProxyTracer>> {
        Some(self.tracer.clone())
    }
}

/// Trace 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceConfig {
    /// 是否启用 trace
    pub enabled: bool,
    /// 采样率 (0.0 - 1.0)
    pub sampling_rate: f64,
    /// 批量发送大小
    pub batch_size: usize,
    /// 批量发送间隔（秒）
    pub batch_interval_secs: u64,
    /// 数据保留天数
    pub retention_days: u32,
    /// 健康检查间隔（秒）
    pub health_check_interval_secs: u64,
}

impl Default for TraceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sampling_rate: 1.0,
            batch_size: 100,
            batch_interval_secs: 5,
            retention_days: 7,
            health_check_interval_secs: 60,
        }
    }
}
