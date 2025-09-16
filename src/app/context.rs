//! 简单的应用上下文（DI 容器）骨架
//!
//! 统一持有跨模块共享的服务实例，便于在测试中注入替身实现。

use std::sync::Arc;

use crate::auth::RefactoredUnifiedAuthManager;
use crate::cache::UnifiedCacheManager;
use crate::config::{AppConfig, ProviderConfigManager};
use crate::trace::UnifiedTraceSystem;
use sea_orm::DatabaseConnection;

#[derive(Clone)]
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub db: Arc<DatabaseConnection>,
    pub cache: Arc<UnifiedCacheManager>,
    pub provider_config: Arc<ProviderConfigManager>,
    pub auth: Arc<RefactoredUnifiedAuthManager>,
    pub trace: Arc<UnifiedTraceSystem>,
}

impl AppContext {
    pub fn new(
        config: Arc<AppConfig>,
        db: Arc<DatabaseConnection>,
        cache: Arc<UnifiedCacheManager>,
        provider_config: Arc<ProviderConfigManager>,
        auth: Arc<RefactoredUnifiedAuthManager>,
        trace: Arc<UnifiedTraceSystem>,
    ) -> Self {
        Self { config, db, cache, provider_config, auth, trace }
    }
}
