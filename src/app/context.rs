//! 简单的应用上下文（DI 容器）骨架
//!
//! 持有跨模块共享的服务实例（AuthManager/CacheManager/TraceSystem 等），便于在测试中注入替身实现。

use std::sync::Arc;

use crate::auth::AuthManager;
use crate::cache::CacheManager;
use crate::config::{AppConfig, ProviderConfigManager};
use crate::trace::TraceSystem;
use sea_orm::DatabaseConnection;

#[derive(Clone)]
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub db: Arc<DatabaseConnection>,
    pub cache: Arc<CacheManager>,
    pub provider_config: Arc<ProviderConfigManager>,
    pub auth: Arc<AuthManager>,
    pub trace: Arc<TraceSystem>,
}

impl AppContext {
    pub fn new(
        config: Arc<AppConfig>,
        db: Arc<DatabaseConnection>,
        cache: Arc<CacheManager>,
        provider_config: Arc<ProviderConfigManager>,
        auth: Arc<AuthManager>,
        trace: Arc<TraceSystem>,
    ) -> Self {
        Self {
            config,
            db,
            cache,
            provider_config,
            auth,
            trace,
        }
    }
}
