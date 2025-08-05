//! # Proxy Trace 追踪系统
//!
//! 负责收集代理请求的详细追踪数据，用于健康状态监控和性能分析

pub mod models;
pub mod unified;
pub mod immediate;

pub use models::*;
pub use unified::{UnifiedProxyTracer, UnifiedTracerConfig, UnifiedTrace};
pub use immediate::{ImmediateProxyTracer, ImmediateTracerConfig};

use std::sync::Arc;
use serde::{Deserialize, Serialize};

/// 统一追踪系统入口
pub enum UnifiedTraceSystem {
    /// 内存缓存模式（原有实现，将被废弃）
    Buffered {
        tracer: Arc<UnifiedProxyTracer>,
    },
    /// 即时写入模式（新的推荐实现）
    Immediate {
        tracer: Arc<ImmediateProxyTracer>,
    },
}

impl UnifiedTraceSystem {
    /// 创建新的统一追踪系统（内存缓存模式，将被废弃）
    #[deprecated(note = "Use new_immediate() instead for better performance with long-running requests")]
    pub fn new(
        db: Arc<sea_orm::DatabaseConnection>,
        config: UnifiedTracerConfig,
    ) -> Self {
        let tracer = Arc::new(UnifiedProxyTracer::new(db, config));
        
        Self::Buffered {
            tracer,
        }
    }
    
    /// 创建新的即时写入追踪系统（推荐）
    pub fn new_immediate(
        db: Arc<sea_orm::DatabaseConnection>,
        config: ImmediateTracerConfig,
    ) -> Self {
        let tracer = Arc::new(ImmediateProxyTracer::new(db, config));
        
        Self::Immediate {
            tracer,
        }
    }

    /// 获取内存缓存追踪器（已废弃）
    #[deprecated(note = "Use immediate_tracer() instead")]
    pub fn tracer(&self) -> Option<Arc<UnifiedProxyTracer>> {
        match self {
            Self::Buffered { tracer } => Some(tracer.clone()),
            Self::Immediate { .. } => None,
        }
    }
    
    /// 获取即时写入追踪器
    pub fn immediate_tracer(&self) -> Option<Arc<ImmediateProxyTracer>> {
        match self {
            Self::Buffered { .. } => None,
            Self::Immediate { tracer } => Some(tracer.clone()),
        }
    }
    
    /// 判断是否为即时写入模式
    pub fn is_immediate(&self) -> bool {
        matches!(self, Self::Immediate { .. })
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