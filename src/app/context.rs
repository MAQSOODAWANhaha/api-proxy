//! # 应用上下文
//!
//! 负责统一持有基础资源、业务服务与后台任务调度器，提供便捷的依赖访问接口。

use crate::app::{AppResources, AppServices, AppTasks};
use crate::config::AppConfig;
use crate::error::Result;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppContext {
    resources: Arc<AppResources>,
    services: Arc<AppServices>,
    tasks: Arc<AppTasks>,
}

impl AppContext {
    /// 根据配置和数据库连接初始化应用上下文
    pub async fn bootstrap(
        config: Arc<AppConfig>,
        database: Arc<DatabaseConnection>,
    ) -> Result<Arc<Self>> {
        let resources = AppResources::build(config, database)?;
        let services = AppServices::initialize(&resources)?;
        let tasks = AppTasks::initialize(&services).await?;

        Ok(Arc::new(Self {
            resources,
            services,
            tasks,
        }))
    }

    #[must_use]
    pub const fn resources(&self) -> &Arc<AppResources> {
        &self.resources
    }

    #[must_use]
    pub const fn services(&self) -> &Arc<AppServices> {
        &self.services
    }

    #[must_use]
    pub const fn tasks(&self) -> &Arc<AppTasks> {
        &self.tasks
    }

    #[must_use]
    pub fn config(&self) -> Arc<AppConfig> {
        self.resources.config()
    }

    #[must_use]
    pub fn database(&self) -> Arc<DatabaseConnection> {
        self.resources.database()
    }

    #[must_use]
    pub fn cache(&self) -> Arc<crate::cache::CacheManager> {
        self.resources.cache()
    }
}
