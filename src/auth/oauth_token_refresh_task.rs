//! # OAuth Token刷新后台任务
//!
//! 提供定期执行的后台任务，实现OAuth token的主动刷新策略：
//! - `定期扫描即将过期的OAuth` token并提前刷新
//! - 支持灵活的调度策略（固定间隔、cron表达式等）
//! - 监控和统计刷新任务的执行情况
//! - 提供任务控制接口（启动、停止、暂停）

use crate::auth::oauth_token_refresh_service::{
    OAuthTokenRefreshService, RefreshStats, RefreshType, ScheduledTokenRefresh, TokenRefreshResult,
};
use crate::error::{ProxyError, Result};
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, lerror, linfo, lwarn};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tokio_util::time::{DelayQueue, delay_queue::Key};

const FALLBACK_RESCAN_INTERVAL_SECS: u64 = 600;
const COMMAND_CHANNEL_CAPACITY: usize = 128;
const MAX_ERROR_RETRIES: u32 = 3;
const ERROR_RETRY_INTERVAL_SECS: u64 = 60;

/// OAuth Token刷新后台任务
///
/// 核心功能：
/// 1. 定期执行主动刷新：扫描即将过期的token并提前刷新
/// 2. 任务调度管理：支持启动、停止、暂停、恢复
/// 3. 监控统计：记录任务执行情况和刷新结果
/// 4. 错误处理：任务失败时的重试和告警机制
pub struct OAuthTokenRefreshTask {
    refresh_service: Arc<OAuthTokenRefreshService>,
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
    Rescan,
}

#[derive(Debug, Clone)]
enum RefreshCommand {
    Add(ScheduledTokenRefresh),
    Remove(String),
}

impl OAuthTokenRefreshTask {
    /// `创建新的OAuth` Token刷新后台任务
    #[must_use]
    pub fn new(refresh_service: Arc<OAuthTokenRefreshService>) -> Self {
        let (control_sender, _) = broadcast::channel(10);

        Self {
            refresh_service,
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
            return Err(ProxyError::business("Task is already running"));
        }

        // 启动前执行一次全局扫描
        let initial_schedule = match self.refresh_service.initialize_refresh_schedule().await {
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
        let task_handle = self
            .spawn_task_loop(initial_schedule, command_receiver);
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
            return Err(ProxyError::business("Task is not running"));
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
            return Err(ProxyError::business("Task is not running"));
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
            return Err(ProxyError::business("Task is not paused"));
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
        self.refresh_service
            .register_session_for_refresh(session_id)
            .await
    }

    /// 将计算好的刷新计划推送给调度器
    pub async fn enqueue_schedule(&self, schedule: ScheduledTokenRefresh) -> Result<()> {
        let sender = {
            let guard = self.command_sender.read().await;
            guard
                .as_ref()
                .cloned()
                .ok_or_else(|| ProxyError::business("Refresh task is not running"))?
        };

        sender
            .send(RefreshCommand::Add(schedule))
            .await
            .map_err(|e| ProxyError::internal(format!("Failed to enqueue refresh schedule: {e}")))
    }

    /// 注册会话刷新（计算计划并入队）
    pub async fn register_session(&self, session_id: &str) -> Result<()> {
        let schedule = self.prepare_schedule(session_id).await?;
        self.enqueue_schedule(schedule).await
    }

    /// 从调度器中移除会话
    pub async fn remove_session(&self, session_id: &str) -> Result<()> {
        let sender = {
            let guard = self.command_sender.read().await;
            guard
                .as_ref()
                .cloned()
                .ok_or_else(|| ProxyError::business("Refresh task is not running"))?
        };

        sender
            .send(RefreshCommand::Remove(session_id.to_string()))
            .await
            .map_err(|e| ProxyError::internal(format!("Failed to remove refresh schedule: {e}")))
    }

    /// 获取任务状态
    pub async fn get_state(&self) -> TaskState {
        self.task_state.read().await.clone()
    }

    /// 获取刷新服务统计信息
    pub async fn get_refresh_stats(&self) -> RefreshStats {
        self.refresh_service.get_refresh_stats().await
    }

    /// 生成任务循环
    #[allow(clippy::too_many_lines)]
    fn spawn_task_loop(
        &self,
        initial_schedule: Vec<ScheduledTokenRefresh>,
        command_receiver: mpsc::Receiver<RefreshCommand>,
    ) -> JoinHandle<()> {
        let refresh_service = Arc::clone(&self.refresh_service);
        let task_state = Arc::clone(&self.task_state);
        let mut control_receiver = self.control_sender.subscribe();

        tokio::spawn(async move {
            let mut command_receiver = command_receiver;
            let mut queue: DelayQueue<RefreshQueueItem> = DelayQueue::new();
            let mut session_keys: HashMap<String, Key> = HashMap::new();
            let mut session_schedules: HashMap<String, ScheduledTokenRefresh> = HashMap::new();
            let mut rescan_key: Option<Key> = None;

            for entry in initial_schedule {
                Self::insert_or_update_entry(
                    &mut queue,
                    &mut session_keys,
                    &mut session_schedules,
                    &entry,
                );
            }

            Self::schedule_rescan(&mut queue, &mut rescan_key);

            let mut consecutive_errors = 0u32;

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
                            break;
                        };

                        match expired.into_inner() {
                            RefreshQueueItem::Session(session_id) => {
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

                                let Some(schedule) = session_schedules.get(&session_id).cloned() else {
                                    session_keys.remove(&session_id);
                                    continue;
                                };

                                let results = Self::process_session_entry(
                                    &refresh_service,
                                    schedule,
                                    &mut queue,
                                    &mut session_keys,
                                    &mut session_schedules,
                                )
                                .await;
                                let had_error = results.iter().any(|r| !r.success);
                                if had_error {
                                    consecutive_errors = consecutive_errors.saturating_add(1);
                                } else {
                                    consecutive_errors = 0;
                                }

                                if had_error
                                    && MAX_ERROR_RETRIES > 0
                                    && consecutive_errors >= MAX_ERROR_RETRIES
                                {
                                    lwarn!("system", LogStage::BackgroundTask, LogComponent::OAuth, "too_many_errors", "Too many consecutive errors, pausing task");
                                    *task_state.write().await =
                                        TaskState::Error(format!(
                                            "Too many consecutive errors: {consecutive_errors}"
                                        ));
                                }
                            }
                            RefreshQueueItem::Rescan => {
                                rescan_key = None;
                                let rescan_result =
                                    Self::resync_schedule(
                                        &refresh_service,
                                        &mut queue,
                                        &mut session_keys,
                                        &mut session_schedules,
                                    )
                                    .await;
                                let had_error = rescan_result.is_err();
                                if let Err(e) = rescan_result {
                                    lerror!("system", LogStage::BackgroundTask, LogComponent::OAuth, "resync_failed", &format!("Failed to resync OAuth refresh schedule: {e:?}"));
                                }

                                Self::schedule_rescan(&mut queue, &mut rescan_key);

                                if had_error {
                                    consecutive_errors = consecutive_errors.saturating_add(1);
                                } else {
                                    consecutive_errors = 0;
                                }

                                if had_error
                                    && MAX_ERROR_RETRIES > 0
                                    && consecutive_errors >= MAX_ERROR_RETRIES
                                {
                                    lwarn!("system", LogStage::BackgroundTask, LogComponent::OAuth, "too_many_errors", "Too many consecutive errors, pausing task");
                                    *task_state.write().await =
                                        TaskState::Error(format!(
                                            "Too many consecutive errors: {consecutive_errors}"
                                        ));
                                }
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
                                ldebug!("system", LogStage::BackgroundTask, LogComponent::OAuth, "session_scheduled", &format!("Scheduled OAuth session {session_id} for refresh"));
                            }
                            Some(RefreshCommand::Remove(session_id)) => {
                                if let Some(key) = session_keys.remove(&session_id) {
                                    let _ = queue.remove(&key);
                                }
                                session_schedules.remove(&session_id);
                                ldebug!("system", LogStage::BackgroundTask, LogComponent::OAuth, "session_removed", &format!("Removed OAuth session {session_id} from refresh queue"));
                            }
                            None => {
                                // 命令通道已关闭，继续处理现有队列
                            }
                        }
                    }
                    Ok(control) = control_receiver.recv() => {
                        match control {
                            TaskControl::Stop => {
                                linfo!("system", LogStage::Shutdown, LogComponent::OAuth, "stop_signal", "Received stop signal, exiting task loop");
                                break;
                            }
                            TaskControl::Pause => {
                                ldebug!("system", LogStage::BackgroundTask, LogComponent::OAuth, "pause_signal", "Received pause signal");
                            }
                            TaskControl::Resume => {
                                ldebug!("system", LogStage::BackgroundTask, LogComponent::OAuth, "resume_signal", "Received resume signal");
                                consecutive_errors = 0;
                            }
                            TaskControl::ExecuteNow => {
                                linfo!("system", LogStage::BackgroundTask, LogComponent::OAuth, "execute_now_signal", "Received execute now signal, triggering immediate rescan");
                                if let Err(e) = Self::resync_schedule(
                                    &refresh_service,
                                    &mut queue,
                                    &mut session_keys,
                                    &mut session_schedules,
                                )
                                .await {
                                    lerror!("system", LogStage::BackgroundTask, LogComponent::OAuth, "rescan_failed", &format!("Immediate rescan failed: {e:?}"));
                                }
                                Self::schedule_rescan(&mut queue, &mut rescan_key);
                                consecutive_errors = 0;
                            }
                            TaskControl::Start => {
                                ldebug!("system", LogStage::Startup, LogComponent::OAuth, "start_signal", "Received start signal in task loop");
                            }
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

    fn schedule_rescan(queue: &mut DelayQueue<RefreshQueueItem>, rescan_key: &mut Option<Key>) {
        let next_rescan_at = Utc::now() + Duration::seconds(i64::try_from(FALLBACK_RESCAN_INTERVAL_SECS).unwrap_or(i64::MAX));
        let delay = Self::duration_until(next_rescan_at);
        if let Some(key) = rescan_key.as_ref() {
            queue.reset(key, delay);
        } else {
            let key = queue.insert(RefreshQueueItem::Rescan, delay);
            *rescan_key = Some(key);
        }
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
        if let Some(existing_key) = session_keys.get(&session_id).copied() {
            queue.reset(&existing_key, delay);
            session_keys.insert(session_id, existing_key);
        } else {
            let key = queue.insert(RefreshQueueItem::Session(session_id.clone()), delay);
            session_keys.insert(session_id, key);
        }
    }

    async fn resync_schedule(
        refresh_service: &Arc<OAuthTokenRefreshService>,
        queue: &mut DelayQueue<RefreshQueueItem>,
        session_keys: &mut HashMap<String, Key>,
        session_schedules: &mut HashMap<String, ScheduledTokenRefresh>,
    ) -> Result<()> {
        if let Err(err) = refresh_service.cleanup_stale_sessions().await {
            lwarn!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "cleanup_failed",
                &format!("Failed to cleanup stale OAuth sessions: {err:?}")
            );
        }

        let sessions = refresh_service.list_authorized_sessions().await?;
        let mut active_ids = HashSet::new();

        for session in sessions {
            let session_id = session.session_id.clone();
            active_ids.insert(session_id.clone());
            if let Some(schedule) = refresh_service.build_schedule_for_session(&session) {
                Self::insert_or_update_entry(queue, session_keys, session_schedules, &schedule);
            } else if let Some(key) = session_keys.remove(&session_id) {
                queue.remove(&key);
                session_schedules.remove(&session_id);
            }
        }

        let obsolete: Vec<String> = session_keys
            .keys()
            .filter(|id| !active_ids.contains(*id))
            .cloned()
            .collect();
        for session_id in obsolete {
            if let Some(key) = session_keys.remove(&session_id) {
                queue.remove(&key);
            }
            session_schedules.remove(&session_id);
        }

        Ok(())
    }

    async fn process_session_entry(
        refresh_service: &Arc<OAuthTokenRefreshService>,
        entry: ScheduledTokenRefresh,
        queue: &mut DelayQueue<RefreshQueueItem>,
        session_keys: &mut HashMap<String, Key>,
        session_schedules: &mut HashMap<String, ScheduledTokenRefresh>,
    ) -> Vec<TokenRefreshResult> {
        let session_id = entry.session_id.clone();
        session_keys.remove(&session_id);
        session_schedules.remove(&session_id);

        let mut results = Vec::new();

        let result = match refresh_service
            .refresh_session(&session_id, RefreshType::Active)
            .await
        {
            Ok(result) => result,
            Err(e) => {
                lerror!(
                    "system",
                    LogStage::BackgroundTask,
                    LogComponent::OAuth,
                    "refresh_failed",
                    &format!(
                        "Failed to refresh token for session {session_id}: {e:?}"
                    )
                );
                let failure = TokenRefreshResult {
                    success: false,
                    new_access_token: None,
                    new_expires_at: None,
                    error_message: Some(e.to_string()),
                    should_retry: true,
                    refresh_type: RefreshType::Active,
                };
                Self::schedule_next_for_result(
                    refresh_service,
                    &session_id,
                    &failure,
                    queue,
                    session_keys,
                    session_schedules,
                )
                .await;
                results.push(failure);
                return results;
            }
        };

        Self::schedule_next_for_result(
            refresh_service,
            &session_id,
            &result,
            queue,
            session_keys,
            session_schedules,
        )
        .await;

        results.push(result);
        results
    }

    async fn schedule_next_for_result(
        refresh_service: &Arc<OAuthTokenRefreshService>,
        session_id: &str,
        result: &TokenRefreshResult,
        queue: &mut DelayQueue<RefreshQueueItem>,
        session_keys: &mut HashMap<String, Key>,
        session_schedules: &mut HashMap<String, ScheduledTokenRefresh>,
    ) {
        match refresh_service
            .determine_next_refresh_after(session_id, result)
            .await
        {
            Ok(Some(schedule)) => {
                Self::insert_or_update_entry(queue, session_keys, session_schedules, &schedule);
            }
            Ok(None) => {
                session_keys.remove(session_id);
                session_schedules.remove(session_id);
            }
            Err(e) => {
                lwarn!(
                    "system",
                    LogStage::BackgroundTask,
                    LogComponent::OAuth,
                    "next_refresh_fail",
                    &format!(
                        "Failed to determine next refresh for session {session_id}: {e:?}"
                    )
                );
                if result.should_retry {
                    let retry_at = Utc::now()
                        + Duration::seconds(i64::try_from(OAuthTokenRefreshService::retry_interval_seconds()).unwrap_or(i64::MAX));
                    let schedule = ScheduledTokenRefresh {
                        session_id: session_id.to_string(),
                        next_refresh_at: retry_at,
                        expires_at: retry_at,
                    };
                    Self::insert_or_update_entry(queue, session_keys, session_schedules, &schedule);
                } else {
                    let retry_at = Utc::now() + Duration::seconds(i64::try_from(ERROR_RETRY_INTERVAL_SECS).unwrap_or(i64::MAX));
                    let schedule = ScheduledTokenRefresh {
                        session_id: session_id.to_string(),
                        next_refresh_at: retry_at,
                        expires_at: retry_at,
                    };
                    Self::insert_or_update_entry(queue, session_keys, session_schedules, &schedule);
                }
            }
        }
    }
}
