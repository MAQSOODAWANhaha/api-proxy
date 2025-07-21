//! # 统计模块
//!
//! 收集和分析系统统计信息

pub mod service;

pub use service::{StatisticsService, RequestStats, TimeRangeQuery};