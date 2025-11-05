use crate::error::Result;
use crate::key_pool::api_key_health::ApiKeyHealthService;
use crate::logging::{LogComponent, LogStage};
use crate::{lerror, linfo};
use chrono::{DateTime, Utc};
use std::sync::{Arc, Weak};
use std::time::Duration as StdDuration;
use tokio::sync::{RwLock, mpsc};
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tokio_util::time::DelayQueue;

const COMMAND_CHANNEL_CAPACITY: usize = 128;

/// 限流重置命令：只需要添加新任务，取消操作通过延迟验证实现
#[derive(Debug, Copy, Clone)]
pub struct ScheduleResetCommand {
    pub key_id: i32,
    pub resets_at: chrono::NaiveDateTime,
}

#[derive(Clone)]
pub struct ApiKeyRateLimitResetTask {
    health_service: Weak<ApiKeyHealthService>,
    command_sender: Arc<RwLock<Option<mpsc::Sender<ScheduleResetCommand>>>>,
    task_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl ApiKeyRateLimitResetTask {
    #[must_use]
    pub fn new(health_service: &Arc<ApiKeyHealthService>) -> Self {
        Self {
            health_service: Arc::downgrade(health_service),
            command_sender: Arc::new(RwLock::new(None)),
            task_handle: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        let health_service = self.health_service.upgrade().ok_or_else(|| {
            crate::error!(
                Internal,
                "ApiKeyHealthService 已被释放，无法启动限流恢复任务"
            )
        })?;
        let (command_sender, command_receiver) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);

        // 从健康服务获取待恢复的限流任务
        let pending_resets = health_service.load_pending_resets_from_db().await?;

        let task_handle = tokio::spawn(run(
            health_service.clone(),
            command_receiver,
            pending_resets,
        ));
        *self.command_sender.write().await = Some(command_sender);
        *self.task_handle.write().await = Some(task_handle);

        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::HealthChecker,
            "rate_limit_reset_task_initialized",
            "Rate limit reset task initialized and restored pending resets"
        );

        Ok(())
    }

    pub async fn stop(&self) {
        let handle = {
            let mut guard = self.task_handle.write().await;
            guard.take()
        };

        if let Some(handle) = handle {
            handle.abort();
            let _ = handle.await;
        }

        *self.command_sender.write().await = None;
        linfo!(
            "system",
            LogStage::Shutdown,
            LogComponent::HealthChecker,
            "rate_limit_reset_task_stopped",
            "Rate limit reset task stopped"
        );
    }

    /// 获取命令发送器的克隆（用于外部发送命令）
    pub async fn get_command_sender(&self) -> Option<mpsc::Sender<ScheduleResetCommand>> {
        self.command_sender.read().await.clone()
    }

    /// 调度新的限流重置任务
    pub async fn schedule_reset(
        &self,
        key_id: i32,
        resets_at: chrono::NaiveDateTime,
    ) -> Result<()> {
        if let Some(sender) = self.command_sender.read().await.as_ref() {
            sender
                .send(ScheduleResetCommand { key_id, resets_at })
                .await
                .map_err(|e| crate::error!(Internal, "Failed to send schedule reset command", e))
        } else {
            Err(crate::error!(Internal, "Rate limit reset task not running"))
        }
    }
}

/// 主运行循环：简化版，移除复杂的 `key_map` 管理
async fn run(
    health_service: Arc<ApiKeyHealthService>,
    mut command_receiver: mpsc::Receiver<ScheduleResetCommand>,
    pending_resets: Vec<(i32, chrono::NaiveDateTime)>,
) {
    let mut queue: DelayQueue<i32> = DelayQueue::new();

    // 恢复待处理的重置任务
    for (key_id, resets_at) in pending_resets {
        let delay = calculate_delay(resets_at);
        queue.insert(key_id, delay);

        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::HealthChecker,
            "rate_limit_reset_restored",
            "Restored rate limit reset task to queue",
            key_id = key_id,
            delay_secs = delay.as_secs()
        );
    }

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::HealthChecker,
        "rate_limit_reset_task_started",
        "Rate limit reset task started"
    );

    loop {
        tokio::select! {
            Some(expired) = queue.next() => {
                let key_id = expired.into_inner();
                linfo!("system", LogStage::HealthCheck, LogComponent::HealthChecker, "rate_limit_expired", "Rate limit expired for key, attempting reset", key_id = key_id);

                // 异步执行重置，延迟验证：只有当 key 确实处于 rate_limited 状态时才重置
                let health_service = health_service.clone();
                tokio::spawn(async move {
                    if let Err(e) = health_service.reset_key_status(key_id).await {
                        lerror!("system", LogStage::HealthCheck, LogComponent::HealthChecker, "key_reset_failed", "Failed to reset key status", key_id = key_id, error = %e);
                    }
                });
            }
            Some(command) = command_receiver.recv() => {
                // 简化命令处理：直接插入队列，不维护映射表
                let delay = calculate_delay(command.resets_at);
                queue.insert(command.key_id, delay);

                linfo!(
                    "system",
                    LogStage::HealthCheck,
                    LogComponent::HealthChecker,
                    "key_reset_scheduled",
                    "Key status reset scheduled",
                    key_id = command.key_id,
                    delay_secs = delay.as_secs()
                );
            }
            else => {
                break;
            }
        }
    }
}

/// 计算延迟时间
fn calculate_delay(resets_at: chrono::NaiveDateTime) -> StdDuration {
    let now = Utc::now();
    let resets_at_utc = DateTime::<Utc>::from_naive_utc_and_offset(resets_at, Utc);
    if resets_at_utc > now {
        (resets_at_utc - now).to_std().unwrap_or_default()
    } else {
        StdDuration::from_secs(0)
    }
}
