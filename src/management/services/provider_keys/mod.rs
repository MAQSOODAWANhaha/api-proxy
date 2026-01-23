//! # 提供商密钥服务模块
//!
//! 聚合管理端提供商密钥的业务逻辑，采用模块化设计：
//! - `models`: 数据结构定义
//! - `crud`: 基本 CRUD 操作
//! - `validation`: 数据验证
//! - `oauth`: OAuth 辅助功能
//! - `gemini`: Gemini 特定逻辑
//! - `statistics`: 统计查询
//! - `service`: 核心服务编排

mod crud;
mod gemini;
mod models;
mod oauth;
mod service;
mod statistics;
mod validation;

// 重新导出公共接口
pub use models::{
    CreateProviderKeyRequest, DailyStats, PrepareGeminiContext, ProviderKeyUsageStats,
    ProviderKeysListQuery, TrendData, TrendDataPoint, TrendQuery, UpdateProviderKeyRequest,
    UserProviderKeyQuery,
};

pub use service::ProviderKeyService;
