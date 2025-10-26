use crate::app::service_registry::AppServices;
use crate::app::task_scheduler::{ScheduledTask, TaskScheduler};
use crate::auth::oauth_token_refresh_task::OAuthTokenRefreshTask;
use crate::error::Result;
use crate::key_pool::rate_limit_reset_task::RateLimitResetTask;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

/// 后台任务类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskType {
    /// 速率限制缓存预热
    RateLimitCacheWarmup,
    /// 密钥池健康检查
    KeyPoolHealthChecker,
    /// OAuth Token 刷新
    OAuthTokenRefresh,
    /// 限流重置任务（常驻服务）
    RateLimitReset,
}

/// 后台任务集合：调度器及任务实例统一管理
///
/// 职责：
/// - 创建和管理所有后台任务（Task 层）
/// - Task 依赖 Service，从 `AppServices` 获取
/// - 统一的任务生命周期管理
pub struct AppTasks {
    scheduler: Arc<TaskScheduler>,
    /// 任务实例注册表：通过任务类型查找具体的任务实例
    task_instances: HashMap<TaskType, Arc<dyn Any + Send + Sync>>,
}

impl AppTasks {
    /// 初始化调度器并注册所有后台任务
    pub async fn initialize(
        resources: &Arc<crate::app::resources::AppResources>,
        services: &Arc<AppServices>,
    ) -> Result<Arc<Self>> {
        let scheduler = Arc::new(TaskScheduler::new());
        let mut task_instances: HashMap<TaskType, Arc<dyn Any + Send + Sync>> = HashMap::new();

        // 从 services 获取核心服务
        let database = resources.database();
        let rate_limiter = services.rate_limiter();
        let key_pool = services.key_pool_service();
        let oauth_refresh_service = services.oauth_refresh_service();
        let api_key_health_checker = services.api_key_health_checker();

        // 在 AppTasks 中创建任务实例（Task 依赖 Service）
        let oauth_token_refresh_task =
            Arc::new(OAuthTokenRefreshTask::new(oauth_refresh_service.clone()));

        let rate_limit_reset_task = Arc::new(RateLimitResetTask::new(
            database.clone(),
            api_key_health_checker.get_health_status_cache(),
        ));

        // 建立 Service 与 Task 的双向通信
        // 将 rate_limit_reset_task 的 sender 注入到 health_checker
        if let Some(sender) = rate_limit_reset_task.get_command_sender().await {
            api_key_health_checker
                .set_rate_limit_reset_sender(sender)
                .await;
        }

        // 注册需要通过 get_task() 访问的任务实例
        task_instances.insert(
            TaskType::OAuthTokenRefresh,
            oauth_token_refresh_task.clone(),
        );
        task_instances.insert(TaskType::RateLimitReset, rate_limit_reset_task.clone());

        // 注册任务到调度器
        scheduler
            .register_many(vec![
                ScheduledTask::builder(TaskType::RateLimitCacheWarmup)
                    .on_start(move || {
                        let rate_limiter = rate_limiter.clone();
                        async move { rate_limiter.warmup_daily_usage_cache().await }
                    })
                    .build(),
                ScheduledTask::builder(TaskType::KeyPoolHealthChecker)
                    .on_start({
                        let key_pool = key_pool.clone();
                        move || {
                            let service = key_pool.clone();
                            async move { service.start().await }
                        }
                    })
                    .on_stop(move || {
                        let service = key_pool.clone();
                        async move { service.stop().await }
                    })
                    .build(),
                ScheduledTask::builder(TaskType::OAuthTokenRefresh)
                    .on_start({
                        let task = oauth_token_refresh_task.clone();
                        move || {
                            let task = task.clone();
                            async move { task.start().await }
                        }
                    })
                    .on_stop(move || {
                        let task = oauth_token_refresh_task.clone();
                        async move { task.stop().await }
                    })
                    .build(),
                ScheduledTask::builder(TaskType::RateLimitReset)
                    .on_start({
                        let task = rate_limit_reset_task.clone();
                        move || {
                            let task = task.clone();
                            async move { task.start().await }
                        }
                    })
                    .on_stop(move || {
                        let task = rate_limit_reset_task.clone();
                        async move {
                            task.stop().await;
                            Ok(())
                        }
                    })
                    .build(),
            ])
            .await;

        Ok(Arc::new(Self {
            scheduler,
            task_instances,
        }))
    }

    #[must_use]
    pub fn scheduler(&self) -> Arc<TaskScheduler> {
        Arc::clone(&self.scheduler)
    }

    /// 获取指定类型的任务实例
    ///
    /// # Example
    /// ```ignore
    /// let oauth_task = app_tasks.get_task::<OAuthTokenRefreshTask>(TaskType::OAuthTokenRefresh);
    /// ```
    #[must_use]
    pub fn get_task<T: Send + Sync + 'static>(&self, task_type: TaskType) -> Option<Arc<T>> {
        self.task_instances
            .get(&task_type)
            .and_then(|any| Arc::clone(any).downcast::<T>().ok())
    }
}
