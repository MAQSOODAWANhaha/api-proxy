//! # Proxy Trace 追踪系统
//!
//! 负责收集代理请求的详细追踪数据，用于健康状态监控和性能分析

pub mod models;
pub mod unified;

pub use models::*;
pub use unified::{UnifiedProxyTracer, UnifiedTracerConfig, UnifiedTrace};

use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 统一追踪系统入口
pub struct UnifiedTraceSystem {
    tracer: Arc<UnifiedProxyTracer>,
}

impl UnifiedTraceSystem {
    /// 创建新的统一追踪系统
    pub fn new(
        db: Arc<sea_orm::DatabaseConnection>,
        config: UnifiedTracerConfig,
    ) -> Self {
        let tracer = Arc::new(UnifiedProxyTracer::new(db, config));
        
        Self {
            tracer,
        }
    }

    /// 获取追踪器
    pub fn tracer(&self) -> Arc<UnifiedProxyTracer> {
        self.tracer.clone()
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