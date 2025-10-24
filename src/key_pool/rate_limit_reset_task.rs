use crate::error::Result;
use crate::key_pool::api_key_health::ApiKeyHealth;
use crate::key_pool::types::ApiKeyHealthStatus;
use crate::logging::{LogComponent, LogStage};
use crate::{lerror, linfo, lwarn};
use chrono::{DateTime, Utc};
use entity::user_provider_keys;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::sync::{RwLock, mpsc};
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tokio_util::time::{DelayQueue, delay_queue::Key};

const COMMAND_CHANNEL_CAPACITY: usize = 128;

#[derive(Debug, Copy, Clone)]
pub enum ResetCommand {
    Add(i32, chrono::NaiveDateTime), // key_id, resets_at
    Remove(i32),                     // key_id
}

#[derive(Clone)]
pub struct RateLimitResetTask {
    db: Arc<DatabaseConnection>,
    health_status_cache: Arc<RwLock<HashMap<i32, ApiKeyHealth>>>,
    command_sender: Arc<RwLock<Option<mpsc::Sender<ResetCommand>>>>,
    task_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl RateLimitResetTask {
    pub fn new(
        db: Arc<DatabaseConnection>,
        health_status_cache: Arc<RwLock<HashMap<i32, ApiKeyHealth>>>,
    ) -> Self {
        Self {
            db,
            health_status_cache,
            command_sender: Arc::new(RwLock::new(None)),
            task_handle: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn start(&self) {
        let (command_sender, command_receiver) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
        let task_handle = tokio::spawn(run(
            self.db.clone(),
            self.health_status_cache.clone(),
            command_receiver,
        ));
        *self.command_sender.write().await = Some(command_sender);
        *self.task_handle.write().await = Some(task_handle);
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

    pub async fn schedule_reset(
        &self,
        key_id: i32,
        resets_at: chrono::NaiveDateTime,
    ) -> Result<()> {
        if let Some(sender) = self.command_sender.read().await.as_ref() {
            sender
                .send(ResetCommand::Add(key_id, resets_at))
                .await
                .map_err(|e| crate::error!(Internal, "Failed to send schedule reset command", e))
        } else {
            Err(crate::error!(Internal, "Rate limit reset task not running"))
        }
    }

    pub async fn cancel(&self, key_id: i32) -> Result<()> {
        if let Some(sender) = self.command_sender.read().await.as_ref() {
            sender
                .send(ResetCommand::Remove(key_id))
                .await
                .map_err(|e| crate::error!(Internal, "Failed to send cancel reset command", e))
        } else {
            Ok(())
        }
    }
}

async fn run(
    db: Arc<DatabaseConnection>,
    health_status_cache: Arc<RwLock<HashMap<i32, ApiKeyHealth>>>,
    mut command_receiver: mpsc::Receiver<ResetCommand>,
) {
    let mut queue: DelayQueue<i32> = DelayQueue::new();
    let mut key_map: HashMap<i32, Key> = HashMap::new();

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
                key_map.remove(&key_id);
                linfo!("system", LogStage::HealthCheck, LogComponent::HealthChecker, "rate_limit_expired", "Rate limit expired for key, attempting reset", key_id = key_id);
                let db_clone = db.clone();
                let cache_clone = health_status_cache.clone();
                tokio::spawn(async move {
                    if let Err(e) = reset_key_status(db_clone.as_ref(), &cache_clone, key_id).await {
                        lerror!("system", LogStage::HealthCheck, LogComponent::HealthChecker, "key_reset_failed", "Failed to reset key status", key_id = key_id, error = %e);
                    }
                });
            }
            Some(command) = command_receiver.recv() => {
                handle_command(command, &mut queue, &mut key_map);
            }
            else => {
                break;
            }
        }
    }
}

fn handle_command(
    command: ResetCommand,
    queue: &mut DelayQueue<i32>,
    key_map: &mut HashMap<i32, Key>,
) {
    match command {
        ResetCommand::Add(key_id, resets_at) => {
            let now = Utc::now();
            let resets_at_utc = DateTime::<Utc>::from_naive_utc_and_offset(resets_at, Utc);
            let delay = if resets_at_utc > now {
                (resets_at_utc - now).to_std().unwrap_or_default()
            } else {
                StdDuration::from_secs(0)
            };

            if let Some(existing_key) = key_map.get(&key_id) {
                queue.reset(existing_key, delay);
            } else {
                let key = queue.insert(key_id, delay);
                key_map.insert(key_id, key);
            }
            linfo!(
                "system",
                LogStage::HealthCheck,
                LogComponent::HealthChecker,
                "key_reset_scheduled",
                "Key status reset scheduled",
                key_id = key_id,
                delay_secs = delay.as_secs()
            );
        }
        ResetCommand::Remove(key_id) => {
            if let Some(key) = key_map.remove(&key_id) {
                queue.remove(&key);
                linfo!(
                    "system",
                    LogStage::HealthCheck,
                    LogComponent::HealthChecker,
                    "key_reset_cancelled",
                    "Key status reset cancelled",
                    key_id = key_id
                );
            }
        }
    }
}

async fn reset_key_status(
    db: &DatabaseConnection,
    health_status_cache: &RwLock<HashMap<i32, ApiKeyHealth>>,
    key_id: i32,
) -> Result<()> {
    let updated = reset_key_in_db(db, key_id).await?;
    if updated {
        reset_key_in_cache(health_status_cache, key_id).await;
    }
    Ok(())
}

async fn reset_key_in_db(db: &DatabaseConnection, key_id: i32) -> Result<bool> {
    let Some(key_to_update) = user_provider_keys::Entity::find_by_id(key_id)
        .one(db)
        .await?
    else {
        lwarn!(
            "system",
            LogStage::HealthCheck,
            LogComponent::HealthChecker,
            "key_not_found_for_reset",
            "Key not found for status reset, it might have been deleted.",
            key_id = key_id
        );
        return Ok(false);
    };

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
    Ok(false)
}

async fn reset_key_in_cache(health_status_cache: &RwLock<HashMap<i32, ApiKeyHealth>>, key_id: i32) {
    let mut health_map = health_status_cache.write().await;
    if let Some(status) = health_map.get_mut(&key_id) {
        status.is_healthy = true;
        status.health_score = 100.0;
        status.consecutive_failures = 0;
        status.last_error = None;
        linfo!(
            "system",
            LogStage::HealthCheck,
            LogComponent::HealthChecker,
            "in_memory_cache_reset",
            "In-memory health cache reset for key.",
            key_id = key_id
        );
    }
}
