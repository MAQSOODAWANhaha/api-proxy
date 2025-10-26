//! # 后台任务调度器
//!
//! 提供统一的任务注册、启动与停止能力，避免在各个模块中分散管理后台任务。

use crate::app::tasks::TaskType;
use crate::error::Result;
use crate::logging::{LogComponent, LogStage};
use crate::{lerror, linfo, lwarn};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

type TaskFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;
type TaskAction = Arc<dyn Fn() -> TaskFuture + Send + Sync>;

/// 调度任务定义
#[derive(Clone)]
pub struct ScheduledTask {
    task_type: TaskType,
    start: TaskAction,
    stop: Option<TaskAction>,
}

impl ScheduledTask {
    /// 创建任务构建器
    #[must_use]
    pub fn builder(task_type: TaskType) -> ScheduledTaskBuilder {
        ScheduledTaskBuilder {
            task_type,
            start: None,
            stop: None,
        }
    }

    /// 启动任务
    async fn start(&self) -> Result<()> {
        linfo!(
            "system",
            LogStage::BackgroundTask,
            LogComponent::ServerSetup,
            "task_start",
            "Starting background task",
            task = ?self.task_type
        );
        (self.start)().await
    }

    /// 停止任务
    async fn stop(&self) -> Result<()> {
        if let Some(action) = &self.stop {
            linfo!(
                "system",
                LogStage::Shutdown,
                LogComponent::ServerSetup,
                "task_stop",
                "Stopping background task",
                task = ?self.task_type
            );
            action().await
        } else {
            lwarn!(
                "system",
                LogStage::Shutdown,
                LogComponent::ServerSetup,
                "task_stop_skipped",
                "No shutdown hook registered for background task",
                task = ?self.task_type
            );
            Ok(())
        }
    }
}

/// 任务构建器
pub struct ScheduledTaskBuilder {
    task_type: TaskType,
    start: Option<TaskAction>,
    stop: Option<TaskAction>,
}

impl ScheduledTaskBuilder {
    /// 注册启动逻辑
    #[must_use]
    pub fn on_start<F, Fut>(mut self, action: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.start = Some(Arc::new(move || Box::pin(action())));
        self
    }

    /// 注册停止逻辑
    #[must_use]
    pub fn on_stop<F, Fut>(mut self, action: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.stop = Some(Arc::new(move || Box::pin(action())));
        self
    }

    /// 构建最终任务
    #[must_use]
    pub fn build(self) -> ScheduledTask {
        let start = self.start.expect("ScheduledTask requires a start action");
        ScheduledTask {
            task_type: self.task_type,
            start,
            stop: self.stop,
        }
    }
}

/// 后台任务调度器
#[derive(Default)]
pub struct TaskScheduler {
    tasks: RwLock<Vec<ScheduledTask>>,
}

impl TaskScheduler {
    /// 创建新的调度器
    #[must_use]
    pub fn new() -> Self {
        Self {
            tasks: RwLock::new(Vec::new()),
        }
    }

    /// 注册任务
    pub async fn register(&self, task: ScheduledTask) {
        let mut guard = self.tasks.write().await;
        guard.push(task);
    }

    /// 批量注册任务
    pub async fn register_many(&self, tasks: Vec<ScheduledTask>) {
        let mut guard = self.tasks.write().await;
        guard.extend(tasks);
    }

    /// 启动所有任务
    pub async fn start_all(&self) -> Result<()> {
        let tasks = { self.tasks.read().await.clone() };
        for task in tasks {
            if let Err(err) = task.start().await {
                lerror!(
                    "system",
                    LogStage::BackgroundTask,
                    LogComponent::ServerSetup,
                    "task_start_failed",
                    "Background task failed to start",
                    task = ?task.task_type,
                    error = %err
                );
                return Err(err);
            }
        }
        Ok(())
    }

    /// 停止所有任务（逆序执行）
    pub async fn shutdown(&self) -> Result<()> {
        let tasks = { self.tasks.read().await.clone() };
        for task in tasks.into_iter().rev() {
            if let Err(err) = task.stop().await {
                lerror!(
                    "system",
                    LogStage::Shutdown,
                    LogComponent::ServerSetup,
                    "task_stop_failed",
                    "Background task failed to stop cleanly",
                    task = ?task.task_type,
                    error = %err
                );
                return Err(err);
            }
        }
        Ok(())
    }
}
