use crate::error::Result;
use crate::key_pool::types::ApiKeyHealthStatus;
use crate::logging::{LogComponent, LogStage};
use crate::{lerror, linfo};
use chrono::{DateTime, Utc};
use entity::user_provider_keys;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use std::sync::Arc;
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
    db: Arc<DatabaseConnection>,
    command_sender: Arc<RwLock<Option<mpsc::Sender<ScheduleResetCommand>>>>,
    task_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl ApiKeyRateLimitResetTask {
    #[must_use]
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            command_sender: Arc::new(RwLock::new(None)),
            task_handle: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        let (command_sender, command_receiver) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);

        // 从数据库恢复未过期的限流任务
        let pending_resets = self.load_pending_resets_from_db().await?;

        let task_handle = tokio::spawn(run(self.db.clone(), command_receiver, pending_resets));
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

    /// 从数据库加载所有未过期的限流重置任务
    #[allow(clippy::cognitive_complexity)] // 包含数据恢复逻辑，复杂度合理
    async fn load_pending_resets_from_db(&self) -> Result<Vec<(i32, chrono::NaiveDateTime)>> {
        let now = Utc::now().naive_utc();

        let pending_keys = user_provider_keys::Entity::find()
            .filter(
                user_provider_keys::Column::HealthStatus
                    .eq(ApiKeyHealthStatus::RateLimited.to_string()),
            )
            .filter(user_provider_keys::Column::RateLimitResetsAt.is_not_null())
            .all(self.db.as_ref())
            .await?;

        let mut pending_resets = Vec::new();
        for key in pending_keys {
            if let Some(resets_at) = key.rate_limit_resets_at {
                // 只恢复未过期的任务
                if resets_at > now {
                    pending_resets.push((key.id, resets_at));
                    linfo!(
                        "system",
                        LogStage::Startup,
                        LogComponent::HealthChecker,
                        "restored_rate_limit_reset",
                        "Restored pending rate limit reset task",
                        key_id = key.id,
                        resets_at = %resets_at
                    );
                } else {
                    // 已过期但状态未更新，立即重置
                    linfo!(
                        "system",
                        LogStage::Startup,
                        LogComponent::HealthChecker,
                        "expired_rate_limit_found",
                        "Found expired rate limit on startup, resetting immediately",
                        key_id = key.id
                    );
                    // 异步重置，不阻塞启动
                    let db_clone = self.db.clone();
                    let key_id = key.id;
                    tokio::spawn(async move {
                        if let Err(e) = reset_key_status(db_clone.as_ref(), key_id).await {
                            lerror!(
                                "system",
                                LogStage::Startup,
                                LogComponent::HealthChecker,
                                "immediate_reset_failed",
                                "Failed to reset expired key on startup",
                                key_id = key_id,
                                error = %e
                            );
                        }
                    });
                }
            }
        }

        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::HealthChecker,
            "pending_resets_loaded",
            "Loaded pending rate limit reset tasks from database",
            count = pending_resets.len()
        );

        Ok(pending_resets)
    }

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
#[allow(clippy::cognitive_complexity)] // 核心调度逻辑，已经是最简化版本
async fn run(
    db: Arc<DatabaseConnection>,
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
                let db_clone = db.clone();
                tokio::spawn(async move {
                    if let Err(e) = reset_key_status(db_clone.as_ref(), key_id).await {
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

/// 重置密钥状态（带延迟验证）
async fn reset_key_status(db: &DatabaseConnection, key_id: i32) -> Result<()> {
    let updated = reset_key_in_db(db, key_id).await?;
    if updated {
        linfo!(
            "system",
            LogStage::HealthCheck,
            LogComponent::HealthChecker,
            "key_status_reset",
            "Key status reset to healthy",
            key_id = key_id
        );
    }
    Ok(())
}

/// 数据库中重置密钥状态（延迟验证：只有真正处于 `rate_limited` 状态时才重置）
async fn reset_key_in_db(db: &DatabaseConnection, key_id: i32) -> Result<bool> {
    let Some(key_to_update) = user_provider_keys::Entity::find_by_id(key_id)
        .one(db)
        .await?
    else {
        // 密钥已被删除，静默忽略
        return Ok(false);
    };

    // 延迟验证：只有确实处于 rate_limited 状态时才重置
    if key_to_update.health_status == ApiKeyHealthStatus::RateLimited.to_string() {
        linfo!(
            "system",
            LogStage::HealthCheck,
            LogComponent::HealthChecker,
            "resetting_key_status_db",
            "Resetting key status to healthy in DB.",
            key_id = key_id
        );
        let mut active_model: user_provider_keys::ActiveModel = key_to_update.into();
        active_model.health_status = Set(ApiKeyHealthStatus::Healthy.to_string());
        active_model.health_status_detail = Set(None);
        active_model.rate_limit_resets_at = Set(None);
        active_model.updated_at = Set(Utc::now().naive_utc());
        active_model.update(db).await?;
        return Ok(true);
    }

    // 已经不是 rate_limited 状态，无需重置（可能已被其他流程处理）
    Ok(false)
}
