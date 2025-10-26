use crate::cache::CacheManager;
use crate::config::AppConfig;
use crate::error::Result;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// 应用基础资源：配置、数据库、缓存等底层依赖
pub struct AppResources {
    config: Arc<AppConfig>,
    database: Arc<DatabaseConnection>,
    cache: Arc<CacheManager>,
}

impl AppResources {
    /// 根据配置与数据库连接构建资源层
    pub fn build(config: Arc<AppConfig>, database: Arc<DatabaseConnection>) -> Result<Arc<Self>> {
        let cache = Arc::new(CacheManager::new(&config.cache)?);
        Ok(Arc::new(Self {
            config,
            database,
            cache,
        }))
    }

    #[must_use]
    pub fn config(&self) -> Arc<AppConfig> {
        Arc::clone(&self.config)
    }

    #[must_use]
    pub fn database(&self) -> Arc<DatabaseConnection> {
        Arc::clone(&self.database)
    }

    #[must_use]
    pub fn cache(&self) -> Arc<CacheManager> {
        Arc::clone(&self.cache)
    }
}
