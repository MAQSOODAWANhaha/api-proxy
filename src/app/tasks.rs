use crate::app::service_registry::AppServices;
use crate::app::task_scheduler::{ScheduledTask, TaskScheduler};
use crate::auth::api_key_oauth_token_refresh_task::ApiKeyOAuthTokenRefreshTask;
use crate::error::Result;
use crate::key_pool::ApiKeyRateLimitResetTask;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

/// 后台任务类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskType {
    /// 速率限制缓存预热
    ApiKeyRateLimitCache,
    /// 限流状态自动恢复
    ApiKeyRateLimitReset,
    /// OAuth Token 刷新
    ApiKeyOAuthTokenRefresh,
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
    pub async fn initialize(services: &Arc<AppServices>) -> Result<Arc<Self>> {
        let scheduler = Arc::new(TaskScheduler::new());
        let mut task_instances: HashMap<TaskType, Arc<dyn Any + Send + Sync>> = HashMap::new();

        // 从 services 获取核心服务
        let rate_limiter = services.api_key_rate_limit_service();
        let api_refresh: Arc<crate::auth::ApiKeyOAuthRefreshService> =
            services.api_key_refresh_service();
        let api_oauth_state: Arc<crate::auth::ApiKeyOAuthStateService> =
            services.api_key_oauth_state_service();
        let api_key_health_service = services.api_key_health_service();

        // 在 AppTasks 中创建任务实例（Task 依赖 Service）
        let refresh = Arc::new(ApiKeyOAuthTokenRefreshTask::new(
            api_refresh.clone(),
            api_oauth_state.clone(),
        ));
        let reset = Arc::new(ApiKeyRateLimitResetTask::new(&api_key_health_service));

        // 将恢复任务注册到健康服务，内部通过弱引用避免循环依赖
        api_key_health_service.set_reset_task(&reset).await;

        // 注册需要通过 get_task() 访问的任务实例
        task_instances.insert(TaskType::ApiKeyOAuthTokenRefresh, refresh.clone());
        task_instances.insert(TaskType::ApiKeyRateLimitReset, reset.clone());

        // 注册任务到调度器
        scheduler
            .register_many(vec![
                ScheduledTask::builder(TaskType::ApiKeyRateLimitCache)
                    .on_start(move || {
                        let rate_limiter = rate_limiter.clone();
                        async move { rate_limiter.warmup_daily_usage_cache().await }
                    })
                    .build(),
                ScheduledTask::builder(TaskType::ApiKeyRateLimitReset)
                    .on_start({
                        let task = reset.clone();
                        move || {
                            let task = task.clone();
                            async move { task.start().await }
                        }
                    })
                    .on_stop({
                        let task = reset.clone();
                        move || {
                            let task = task.clone();
                            async move {
                                task.stop().await;
                                Ok(())
                            }
                        }
                    })
                    .build(),
                ScheduledTask::builder(TaskType::ApiKeyOAuthTokenRefresh)
                    .on_start({
                        let task = refresh.clone();
                        move || {
                            let task = task.clone();
                            async move { task.start().await }
                        }
                    })
                    .on_stop(move || {
                        let task = refresh.clone();
                        async move { task.stop().await }
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
    /// let oauth_task = app_tasks.get_task::<OAuthTokenRefreshTask>(TaskType::ApiKeyOAuthTokenRefresh);
    /// ```
    #[must_use]
    pub fn get_task<T: Send + Sync + 'static>(&self, task_type: TaskType) -> Option<Arc<T>> {
        self.task_instances
            .get(&task_type)
            .and_then(|any| Arc::clone(any).downcast::<T>().ok())
    }
}
