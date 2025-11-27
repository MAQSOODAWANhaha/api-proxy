//! # OAuth Token刷新后台任务
//!
//! 提供定期执行的后台任务，实现OAuth token的主动刷新策略：
//! - `定期扫描即将过期的OAuth` token并提前刷新
//! - 支持灵活的调度策略（固定间隔、cron表达式等）
//! - 监控和统计刷新任务的执行情况
//! - 提供任务控制接口（启动、停止、暂停）

use crate::auth::api_key_oauth_refresh_service::{
    ApiKeyOAuthRefreshResult, ApiKeyOAuthRefreshService,
};
use crate::auth::api_key_oauth_state_service::{ApiKeyOAuthStateService, ScheduledTokenRefresh};
use crate::error::{Context, ProxyError, Result, auth::AuthError};
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, lerror, linfo, lwarn};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tokio_util::time::{DelayQueue, delay_queue::Key};

const COMMAND_CHANNEL_CAPACITY: usize = 128;
const MAX_ERROR_RETRIES: u32 = 3;

/// OAuth Token刷新后台任务
///
/// 核心功能：
/// 1. 定期执行主动刷新：扫描即将过期的token并提前刷新
/// 2. 任务调度管理：支持启动、停止、暂停、恢复
/// 3. 监控统计：记录任务执行情况和刷新结果
/// 4. 错误处理：任务失败时的重试和告警机制
pub struct ApiKeyOAuthTokenRefreshTask {
    refresh_service: Arc<ApiKeyOAuthRefreshService>,
    oauth_state_service: Arc<ApiKeyOAuthStateService>,
    /// 任务状态
    task_state: Arc<RwLock<TaskState>>,

    /// 控制信号发送器
    control_sender: broadcast::Sender<TaskControl>,

    /// 调度命令通道
    command_sender: Arc<RwLock<Option<mpsc::Sender<RefreshCommand>>>>,

    /// 任务句柄
    task_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
}

/// 任务状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    /// 未启动
    NotStarted,
    /// 运行中
    Running,
    /// 暂停中
    Paused,
    /// 停止中
    Stopping,
    /// 已停止
    Stopped,
    /// 错误状态
    Error(String),
}

/// 任务控制信号
#[derive(Debug, Clone)]
pub enum TaskControl {
    /// 启动任务
    Start,
    /// 停止任务
    Stop,
    /// 暂停任务
    Pause,
    /// 恢复任务
    Resume,
    /// 立即执行一次刷新
    ExecuteNow,
}

#[derive(Debug, Clone)]
enum RefreshQueueItem {
    Session(String),
}

#[derive(Debug, Clone)]
enum RefreshCommand {
    Add(ScheduledTokenRefresh),
    Remove(String),
}

impl From<mpsc::error::SendError<RefreshCommand>> for ProxyError {
    fn from(error: mpsc::error::SendError<RefreshCommand>) -> Self {
        AuthError::Message(format!("Refresh command channel send failed: {error}")).into()
    }
}

impl ApiKeyOAuthTokenRefreshTask {
    /// `创建新的OAuth` Token刷新后台任务
    #[must_use]
    pub fn new(
        refresh_service: Arc<ApiKeyOAuthRefreshService>,
        oauth_state_service: Arc<ApiKeyOAuthStateService>,
    ) -> Self {
        let (control_sender, _) = broadcast::channel(10);

        Self {
            refresh_service,
            oauth_state_service,
            task_state: Arc::new(RwLock::new(TaskState::NotStarted)),
            control_sender,
            command_sender: Arc::new(RwLock::new(None)),
            task_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// 启动后台任务
    pub async fn start(&self) -> Result<()> {
        let mut state = self.task_state.write().await;

        if matches!(*state, TaskState::Running) {
            crate::bail!("Task is already running");
        }

        // 启动前执行一次全局扫描
        let initial_schedule = match self
            .oauth_state_service
            .load_initial_plans(Utc::now())
            .await
        {
            Ok(entries) => entries,
            Err(e) => {
                lerror!(
                    "system",
                    LogStage::Startup,
                    LogComponent::OAuth,
                    "init_schedule_fail",
                    &format!("Failed to initialize OAuth token refresh schedule: {e:?}")
                );
                Vec::new()
            }
        };

        // 启动任务
        *state = TaskState::Running;
        drop(state);

        // 启动任务循环
        let (command_sender, command_receiver) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
        let task_handle = self.spawn_task_loop(initial_schedule, command_receiver);
        *self.command_sender.write().await = Some(command_sender);
        *self.task_handle.write().await = Some(task_handle);

        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::OAuth,
            "task_started",
            "OAuth Token refresh task started"
        );
        Ok(())
    }

    /// 停止后台任务
    pub async fn stop(&self) -> Result<()> {
        let mut state = self.task_state.write().await;

        if matches!(*state, TaskState::NotStarted | TaskState::Stopped) {
            crate::bail!("Task is not running");
        }

        // 发送停止信号
        *state = TaskState::Stopping;
        let _ = self.control_sender.send(TaskControl::Stop);

        // 等待任务结束
        let handle = self.task_handle.write().await.take();
        if let Some(handle) = handle {
            let _ = handle.await;
        }

        *self.command_sender.write().await = None;
        *state = TaskState::Stopped;
        drop(state);
        linfo!(
            "system",
            LogStage::Shutdown,
            LogComponent::OAuth,
            "task_stopped",
            "OAuth Token refresh task stopped"
        );
        Ok(())
    }

    /// 暂停任务
    pub async fn pause(&self) -> Result<()> {
        let mut state = self.task_state.write().await;

        if !matches!(*state, TaskState::Running) {
            crate::bail!("Task is not running");
        }

        *state = TaskState::Paused;
        drop(state);
        let _ = self.control_sender.send(TaskControl::Pause);

        linfo!(
            "system",
            LogStage::BackgroundTask,
            LogComponent::OAuth,
            "task_paused",
            "OAuth Token refresh task paused"
        );
        Ok(())
    }

    /// 恢复任务
    pub async fn resume(&self) -> Result<()> {
        let mut state = self.task_state.write().await;

        if !matches!(*state, TaskState::Paused) {
            crate::bail!("Task is not paused");
        }

        *state = TaskState::Running;
        drop(state);
        let _ = self.control_sender.send(TaskControl::Resume);

        linfo!(
            "system",
            LogStage::BackgroundTask,
            LogComponent::OAuth,
            "task_resumed",
            "OAuth Token refresh task resumed"
        );
        Ok(())
    }

    /// 立即执行一次刷新
    pub fn execute_now(&self) -> Result<()> {
        let _ = self.control_sender.send(TaskControl::ExecuteNow);
        linfo!(
            "system",
            LogStage::BackgroundTask,
            LogComponent::OAuth,
            "task_triggered",
            "OAuth Token refresh task triggered for immediate execution"
        );
        Ok(())
    }

    /// 预计算会话的刷新计划，不立即入队
    pub async fn prepare_schedule(&self, session_id: &str) -> Result<ScheduledTokenRefresh> {
        self.oauth_state_service
            .schedule_session_refresh(session_id, Utc::now())
            .await
    }

    /// 将计算好的刷新计划推送给调度器
    pub async fn enqueue_schedule(&self, schedule: ScheduledTokenRefresh) -> Result<()> {
        let sender = {
            let guard = self.command_sender.read().await;
            guard.as_ref().cloned().ok_or_else(|| {
                crate::error::auth::AuthError::Message("Refresh task is not running".to_string())
            })?
        };

        sender
            .send(RefreshCommand::Add(schedule))
            .await
            .context("Failed to enqueue refresh schedule")
    }

    /// 注册会话刷新（计算计划并入队）
    pub async fn register_session(&self, session_id: &str) -> Result<()> {
        let schedule = self
            .oauth_state_service
            .create_refresh_plan(session_id, Utc::now())
            .await?;
        self.enqueue_schedule(schedule).await
    }

    /// 从调度器中移除会话
    pub async fn remove_session(&self, session_id: &str) -> Result<()> {
        self.oauth_state_service
            .delete_refresh_plan(session_id)
            .await?;
        let sender = {
            let guard = self.command_sender.read().await;
            guard.as_ref().cloned().ok_or_else(|| {
                crate::error::auth::AuthError::Message("Refresh task is not running".to_string())
            })?
        };

        sender
            .send(RefreshCommand::Remove(session_id.to_string()))
            .await
            .context("Failed to remove refresh schedule")
    }

    /// 获取任务状态
    pub async fn get_state(&self) -> TaskState {
        self.task_state.read().await.clone()
    }

    /// 获取刷新服务统计信息
    /// 生成任务循环
    #[allow(clippy::too_many_lines)]
    fn spawn_task_loop(
        &self,
        initial_schedule: Vec<ScheduledTokenRefresh>,
        command_receiver: mpsc::Receiver<RefreshCommand>,
    ) -> JoinHandle<()> {
        let refresh_service = Arc::clone(&self.refresh_service);
        let oauth_state_service = Arc::clone(&self.oauth_state_service);
        let task_state = Arc::clone(&self.task_state);
        let mut control_receiver = self.control_sender.subscribe();

        tokio::spawn(async move {
            let mut command_receiver = command_receiver;
            let mut queue = DelayQueue::new();
            let mut session_keys: HashMap<String, Key> = HashMap::new();
            let mut session_schedules: HashMap<String, ScheduledTokenRefresh> = HashMap::new();
            let mut consecutive_errors = 0u32;

            for entry in initial_schedule {
                Self::insert_or_update_entry(
                    &mut queue,
                    &mut session_keys,
                    &mut session_schedules,
                    &entry,
                );
            }

            linfo!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "task_loop_started",
                "OAuth Token refresh task loop started"
            );

            loop {
                tokio::select! {
                    maybe_expired = queue.next() => {
                        let Some(expired) = maybe_expired else {
                            continue;
                        };
                        let RefreshQueueItem::Session(session_id) = expired.into_inner();
                        let current_state = { task_state.read().await.clone() };
                        if !matches!(current_state, TaskState::Running) {
                            if let Some(schedule) = session_schedules.get(&session_id).cloned() {
                                Self::insert_or_update_entry(
                                    &mut queue,
                                    &mut session_keys,
                                    &mut session_schedules,
                                    &schedule,
                                );
                            }
                            continue;
                        }

                        let Some(schedule) = session_schedules.remove(&session_id) else {
                            session_keys.remove(&session_id);
                            continue;
                        };
                        session_keys.remove(&session_id);

                        let success = Self::process_session_entry(
                            &refresh_service,
                            &oauth_state_service,
                            schedule,
                            &mut queue,
                            &mut session_keys,
                            &mut session_schedules,
                        )
                        .await;

                        if success {
                            consecutive_errors = 0;
                        } else {
                            consecutive_errors = consecutive_errors.saturating_add(1);
                            if MAX_ERROR_RETRIES > 0 && consecutive_errors >= MAX_ERROR_RETRIES {
                                lwarn!(
                                    "system",
                                    LogStage::BackgroundTask,
                                    LogComponent::OAuth,
                                    "too_many_errors",
                                    "Too many consecutive errors, pausing task"
                                );
                                *task_state.write().await = TaskState::Error(format!(
                                    "Too many consecutive errors: {consecutive_errors}"
                                ));
                            }
                        }
                    }
                    command = command_receiver.recv() => {
                        match command {
                            Some(RefreshCommand::Add(schedule)) => {
                                let session_id = schedule.session_id.clone();
                                Self::insert_or_update_entry(
                                    &mut queue,
                                    &mut session_keys,
                                    &mut session_schedules,
                                    &schedule,
                                );
                                ldebug!(
                                    "system",
                                    LogStage::BackgroundTask,
                                    LogComponent::OAuth,
                                    "session_scheduled",
                                    &format!("Scheduled OAuth session {session_id} for refresh")
                                );
                            }
                            Some(RefreshCommand::Remove(session_id)) => {
                                if let Some(key) = session_keys.remove(&session_id) {
                                    let _ = queue.remove(&key);
                                }
                                session_schedules.remove(&session_id);
                                ldebug!(
                                    "system",
                                    LogStage::BackgroundTask,
                                    LogComponent::OAuth,
                                    "session_removed",
                                    &format!("Removed OAuth session {session_id} from refresh queue")
                                );
                            }
                            None => {
                                // 命令通道关闭，等待现有任务处理完成
                            }
                        }
                    }
                    Ok(control) = control_receiver.recv() => match control {
                        TaskControl::Stop => {
                            linfo!(
                                "system",
                                LogStage::Shutdown,
                                LogComponent::OAuth,
                                "stop_signal",
                                "Received stop signal, exiting task loop"
                            );
                            break;
                        }
                        TaskControl::Pause => {
                            ldebug!(
                                "system",
                                LogStage::BackgroundTask,
                                LogComponent::OAuth,
                                "pause_signal",
                                "Received pause signal"
                            );
                        }
                        TaskControl::Resume => {
                            ldebug!(
                                "system",
                                LogStage::BackgroundTask,
                                LogComponent::OAuth,
                                "resume_signal",
                                "Received resume signal"
                            );
                            consecutive_errors = 0;
                        }
                        TaskControl::ExecuteNow => {
                            let now = Utc::now();
                            let session_ids: Vec<String> =
                                session_schedules.keys().cloned().collect();
                            for session_id in session_ids {
                                if let Some(schedule) = session_schedules.get_mut(&session_id) {
                                    schedule.next_refresh_at = now;
                                    let updated = schedule.clone();
                                    Self::insert_or_update_entry(
                                        &mut queue,
                                        &mut session_keys,
                                        &mut session_schedules,
                                        &updated,
                                    );
                                }
                            }
                            consecutive_errors = 0;
                        }
                        TaskControl::Start => {
                            ldebug!(
                                "system",
                                LogStage::Startup,
                                LogComponent::OAuth,
                                "start_signal",
                                "Received start signal in task loop"
                            );
                        }
                    }
                }
            }

            linfo!(
                "system",
                LogStage::Shutdown,
                LogComponent::OAuth,
                "task_loop_ended",
                "OAuth Token refresh task loop ended"
            );
        })
    }

    fn duration_until(target: DateTime<Utc>) -> StdDuration {
        let now = Utc::now();
        if target <= now {
            StdDuration::from_secs(0)
        } else {
            (target - now)
                .to_std()
                .unwrap_or_else(|_| StdDuration::from_secs(0))
        }
    }

    fn retry_delay() -> Duration {
        Duration::seconds(
            i64::try_from(ApiKeyOAuthStateService::retry_interval_secs()).unwrap_or(60),
        )
    }

    #[allow(clippy::too_many_arguments, clippy::cognitive_complexity)]
    async fn handle_success(
        request_id: &str,
        stage: LogStage,
        component: LogComponent,
        session_id: &str,
        result: ApiKeyOAuthRefreshResult,
        mut entry: ScheduledTokenRefresh,
        oauth_state_service: &Arc<ApiKeyOAuthStateService>,
        queue: &mut DelayQueue<RefreshQueueItem>,
        session_keys: &mut HashMap<String, Key>,
        session_schedules: &mut HashMap<String, ScheduledTokenRefresh>,
    ) -> bool {
        match oauth_state_service.refresh_target_exists(session_id).await {
            Ok(true) => {}
            Ok(false) => {
                oauth_state_service.release_refresh_slot(session_id).await;
                ldebug!(
                    request_id,
                    stage,
                    component,
                    "refresh_target_removed",
                    "Refresh target removed before completion",
                    session_id = %session_id
                );
                return true;
            }
            Err(err) => {
                err.log();
                oauth_state_service.release_refresh_slot(session_id).await;
                lwarn!(
                    request_id,
                    stage,
                    component,
                    "refresh_target_check_failed",
                    "Failed to verify refresh target state",
                    session_id = %session_id
                );
                return false;
            }
        }

        match oauth_state_service.complete_refresh(&result).await {
            Ok(next_schedule) => {
                ldebug!(
                    request_id,
                    stage,
                    component,
                    "refresh_success",
                    "Token refresh completed successfully",
                    session_id = %session_id,
                    next_refresh_at = %next_schedule.next_refresh_at
                );
                Self::insert_or_update_entry(
                    queue,
                    session_keys,
                    session_schedules,
                    &next_schedule,
                );
                true
            }
            Err(err) => {
                err.log();
                oauth_state_service.release_refresh_slot(session_id).await;
                lwarn!(
                    request_id,
                    stage,
                    component,
                    "complete_refresh_failed",
                    "Failed to finalize refresh result, scheduling retry",
                    session_id = %session_id,
                    error = %err
                );
                entry.retry_attempts = entry.retry_attempts.saturating_add(1);
                entry.next_refresh_at = Utc::now() + Self::retry_delay();
                Self::insert_or_update_entry(queue, session_keys, session_schedules, &entry);
                false
            }
        }
    }

    #[allow(clippy::too_many_arguments, clippy::cognitive_complexity)]
    async fn handle_failure(
        request_id: &str,
        stage: LogStage,
        component: LogComponent,
        session_id: &str,
        error_message: String,
        mut entry: ScheduledTokenRefresh,
        oauth_state_service: &Arc<ApiKeyOAuthStateService>,
        queue: &mut DelayQueue<RefreshQueueItem>,
        session_keys: &mut HashMap<String, Key>,
        session_schedules: &mut HashMap<String, ScheduledTokenRefresh>,
    ) -> bool {
        match oauth_state_service.refresh_target_exists(session_id).await {
            Ok(true) => {}
            Ok(false) => {
                oauth_state_service.release_refresh_slot(session_id).await;
                ldebug!(
                    request_id,
                    stage,
                    component,
                    "refresh_target_removed",
                    "Refresh target removed before failure handling",
                    session_id = %session_id
                );
                return true;
            }
            Err(err) => {
                err.log();
                oauth_state_service.release_refresh_slot(session_id).await;
                lwarn!(
                    request_id,
                    stage,
                    component,
                    "refresh_target_check_failed",
                    "Failed to verify refresh target state",
                    session_id = %session_id
                );
                return false;
            }
        }

        match oauth_state_service
            .fail_refresh(session_id, entry.retry_attempts, &error_message)
            .await
        {
            Ok(Some(schedule)) => {
                ldebug!(
                    request_id,
                    stage,
                    component,
                    "refresh_retry_scheduled",
                    "Scheduled retry after refresh failure",
                    session_id = %session_id,
                    next_refresh_at = %schedule.next_refresh_at,
                    attempts = schedule.retry_attempts
                );
                Self::insert_or_update_entry(queue, session_keys, session_schedules, &schedule);
            }
            Ok(None) => {
                linfo!(
                    request_id,
                    stage,
                    component,
                    "refresh_disabled",
                    "Refresh disabled after exceeding retry attempts",
                    session_id = %session_id
                );
            }
            Err(state_err) => {
                state_err.log();
                oauth_state_service.release_refresh_slot(session_id).await;
                lwarn!(
                    request_id,
                    stage,
                    component,
                    "refresh_state_update_failed",
                    "Failed to update refresh state, scheduling fallback retry",
                    session_id = %session_id,
                    error = %state_err
                );
                entry.retry_attempts = entry.retry_attempts.saturating_add(1);
                entry.next_refresh_at = Utc::now() + Self::retry_delay();
                Self::insert_or_update_entry(queue, session_keys, session_schedules, &entry);
            }
        }
        false
    }
    fn insert_or_update_entry(
        queue: &mut DelayQueue<RefreshQueueItem>,
        session_keys: &mut HashMap<String, Key>,
        session_schedules: &mut HashMap<String, ScheduledTokenRefresh>,
        entry: &ScheduledTokenRefresh,
    ) {
        let delay = Self::duration_until(entry.next_refresh_at);
        let session_id = entry.session_id.clone();
        session_schedules.insert(session_id.clone(), entry.clone());
        if let Some(existing_key) = session_keys.remove(&session_id) {
            let _ = queue.remove(&existing_key);
        }
        let key = queue.insert(RefreshQueueItem::Session(session_id.clone()), delay);
        session_keys.insert(session_id, key);
    }

    #[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
    async fn process_session_entry(
        refresh_service: &Arc<ApiKeyOAuthRefreshService>,
        oauth_state_service: &Arc<ApiKeyOAuthStateService>,
        mut entry: ScheduledTokenRefresh,
        queue: &mut DelayQueue<RefreshQueueItem>,
        session_keys: &mut HashMap<String, Key>,
        session_schedules: &mut HashMap<String, ScheduledTokenRefresh>,
    ) -> bool {
        let session_id = entry.session_id.clone();
        let stage = LogStage::BackgroundTask;
        let component = LogComponent::OAuth;
        let request_id = uuid::Uuid::new_v4().to_string();
        let acquired = oauth_state_service.acquire_refresh_slot(&session_id).await;
        if !acquired {
            lwarn!(
                &request_id,
                stage,
                component,
                "slot_busy",
                "Refresh slot already acquired by another worker, rescheduling",
                session_id = %session_id
            );
            entry.next_refresh_at = Utc::now() + Self::retry_delay();
            Self::insert_or_update_entry(queue, session_keys, session_schedules, &entry);
            return true;
        }

        match refresh_service
            .execute_token_refresh(request_id.clone(), &session_id)
            .await
        {
            Ok(result) => {
                Self::handle_success(
                    &request_id,
                    stage,
                    component,
                    &session_id,
                    result,
                    entry.clone(),
                    oauth_state_service,
                    queue,
                    session_keys,
                    session_schedules,
                )
                .await
            }
            Err(err) => {
                err.log();
                lerror!(
                    &request_id,
                    stage,
                    component,
                    "refresh_execution_failed",
                    "Token refresh execution returned error",
                    session_id = %session_id,
                    error = %err
                );
                Self::handle_failure(
                    &request_id,
                    stage,
                    component,
                    &session_id,
                    err.to_string(),
                    entry,
                    oauth_state_service,
                    queue,
                    session_keys,
                    session_schedules,
                )
                .await
            }
        }
    }
}
