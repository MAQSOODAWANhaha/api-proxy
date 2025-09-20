//! # OAuth Token刷新后台任务
//!
//! 提供定期执行的后台任务，实现OAuth token的主动刷新策略：
//! - 定期扫描即将过期的OAuth token并提前刷新
//! - 支持灵活的调度策略（固定间隔、cron表达式等）
//! - 监控和统计刷新任务的执行情况
//! - 提供任务控制接口（启动、停止、暂停）

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::sync::{RwLock, broadcast};
use tokio::task::JoinHandle;
use tokio::time::{Instant, interval};
use tracing::{debug, error, info, warn};

use crate::auth::oauth_token_refresh_service::{
    OAuthTokenRefreshService, RefreshStats, TokenRefreshResult,
};
use crate::error::{ProxyError, Result};

/// OAuth Token刷新后台任务
///
/// 核心功能：
/// 1. 定期执行主动刷新：扫描即将过期的token并提前刷新
/// 2. 任务调度管理：支持启动、停止、暂停、恢复
/// 3. 监控统计：记录任务执行情况和刷新结果
/// 4. 错误处理：任务失败时的重试和告警机制
pub struct OAuthTokenRefreshTask {
    refresh_service: Arc<OAuthTokenRefreshService>,
    config: RefreshTaskConfig,

    /// 任务状态
    task_state: Arc<RwLock<TaskState>>,

    /// 任务统计信息
    task_stats: Arc<RwLock<TaskStats>>,

    /// 控制信号发送器
    control_sender: broadcast::Sender<TaskControl>,

    /// 任务句柄
    task_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
}

/// 刷新任务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTaskConfig {
    /// 任务执行间隔（秒）
    pub interval_seconds: u64,

    /// 是否启用任务
    pub enabled: bool,

    /// 最大并发刷新数量
    pub max_concurrent_refreshes: usize,

    /// 任务超时时间（秒）
    pub task_timeout_seconds: u64,

    /// 错误重试次数
    pub max_error_retries: u32,

    /// 错误重试间隔（秒）
    pub error_retry_interval_seconds: u64,

    /// 静默时间段（不执行任务的时间段，格式：HH:MM-HH:MM）
    pub quiet_hours: Option<String>,
}

impl Default for RefreshTaskConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 600, // 默认10分钟执行一次
            enabled: true,
            max_concurrent_refreshes: 10,     // 最多同时刷新10个token
            task_timeout_seconds: 300,        // 任务超时5分钟
            max_error_retries: 3,             // 最多重试3次
            error_retry_interval_seconds: 60, // 重试间隔1分钟
            quiet_hours: None,                // 无静默时间
        }
    }
}

/// 任务状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// 重新加载配置
    ReloadConfig(RefreshTaskConfig),
}

/// 任务统计信息
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TaskStats {
    /// 任务启动时间
    pub started_at: Option<DateTime<Utc>>,

    /// 最后执行时间
    pub last_execution_time: Option<DateTime<Utc>>,

    /// 下次执行时间
    pub next_execution_time: Option<DateTime<Utc>>,

    /// 总执行次数
    pub total_executions: u64,

    /// 成功执行次数
    pub successful_executions: u64,

    /// 失败执行次数
    pub failed_executions: u64,

    /// 总刷新token数量
    pub total_tokens_refreshed: u64,

    /// 成功刷新token数量
    pub successful_token_refreshes: u64,

    /// 失败刷新token数量
    pub failed_token_refreshes: u64,

    /// 平均执行时长（毫秒）
    pub average_execution_duration_ms: f64,

    /// 最后一次错误
    pub last_error: Option<String>,

    /// 最后一次错误时间
    pub last_error_time: Option<DateTime<Utc>>,

    /// 连续错误次数
    pub consecutive_errors: u32,
}

impl OAuthTokenRefreshTask {
    /// 创建新的OAuth Token刷新后台任务
    pub fn new(refresh_service: Arc<OAuthTokenRefreshService>, config: RefreshTaskConfig) -> Self {
        let (control_sender, _) = broadcast::channel(10);

        Self {
            refresh_service,
            config,
            task_state: Arc::new(RwLock::new(TaskState::NotStarted)),
            task_stats: Arc::new(RwLock::new(TaskStats::default())),
            control_sender,
            task_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// 使用默认配置创建任务
    pub fn new_with_defaults(refresh_service: Arc<OAuthTokenRefreshService>) -> Self {
        Self::new(refresh_service, RefreshTaskConfig::default())
    }

    /// 启动后台任务
    pub async fn start(&self) -> Result<()> {
        let mut state = self.task_state.write().await;

        if matches!(*state, TaskState::Running) {
            return Err(ProxyError::business("Task is already running"));
        }

        if !self.config.enabled {
            return Err(ProxyError::config("Task is disabled in config"));
        }

        // 启动任务
        *state = TaskState::Running;

        // 更新统计信息
        let mut stats = self.task_stats.write().await;
        stats.started_at = Some(Utc::now());
        stats.next_execution_time =
            Some(Utc::now() + Duration::seconds(self.config.interval_seconds as i64));
        drop(stats);

        // 启动任务循环
        let task_handle = self.spawn_task_loop().await;
        *self.task_handle.write().await = Some(task_handle);

        info!(
            "OAuth Token refresh task started with interval {} seconds",
            self.config.interval_seconds
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
        if let Some(handle) = self.task_handle.write().await.take() {
            let _ = handle.await;
        }

        *state = TaskState::Stopped;
        info!("OAuth Token refresh task stopped");
        Ok(())
    }

    /// 暂停任务
    pub async fn pause(&self) -> Result<()> {
        let mut state = self.task_state.write().await;

        if !matches!(*state, TaskState::Running) {
            return Err(ProxyError::business("Task is not running"));
        }

        *state = TaskState::Paused;
        let _ = self.control_sender.send(TaskControl::Pause);

        info!("OAuth Token refresh task paused");
        Ok(())
    }

    /// 恢复任务
    pub async fn resume(&self) -> Result<()> {
        let mut state = self.task_state.write().await;

        if !matches!(*state, TaskState::Paused) {
            return Err(ProxyError::business("Task is not paused"));
        }

        *state = TaskState::Running;
        let _ = self.control_sender.send(TaskControl::Resume);

        info!("OAuth Token refresh task resumed");
        Ok(())
    }

    /// 立即执行一次刷新
    pub async fn execute_now(&self) -> Result<()> {
        let _ = self.control_sender.send(TaskControl::ExecuteNow);
        info!("OAuth Token refresh task triggered for immediate execution");
        Ok(())
    }

    /// 重新加载配置
    pub async fn reload_config(&self, new_config: RefreshTaskConfig) -> Result<()> {
        let _ = self
            .control_sender
            .send(TaskControl::ReloadConfig(new_config));
        info!("OAuth Token refresh task config reload requested");
        Ok(())
    }

    /// 获取任务状态
    pub async fn get_state(&self) -> TaskState {
        self.task_state.read().await.clone()
    }

    /// 获取任务统计信息
    pub async fn get_stats(&self) -> TaskStats {
        self.task_stats.read().await.clone()
    }

    /// 获取刷新服务统计信息
    pub async fn get_refresh_stats(&self) -> RefreshStats {
        self.refresh_service.get_refresh_stats().await
    }

    /// 生成任务循环
    async fn spawn_task_loop(&self) -> JoinHandle<()> {
        let refresh_service = Arc::clone(&self.refresh_service);
        let task_state = Arc::clone(&self.task_state);
        let task_stats = Arc::clone(&self.task_stats);
        let mut control_receiver = self.control_sender.subscribe();
        let mut config = self.config.clone();

        tokio::spawn(async move {
            let mut ticker = interval(StdDuration::from_secs(config.interval_seconds));
            let mut consecutive_errors = 0u32;

            info!("OAuth Token refresh task loop started");

            loop {
                // 检查控制信号
                tokio::select! {
                    // 定时器触发
                    _ = ticker.tick() => {
                        // 检查任务状态
                        let current_state = { task_state.read().await.clone() };
                        match current_state {
                            TaskState::Running => {
                                // 执行刷新任务
                                let execution_start = Instant::now();
                                let execution_result: Result<Vec<TokenRefreshResult>> = Self::execute_refresh_task(&refresh_service).await;
                                let execution_duration = execution_start.elapsed();

                                // 更新统计信息
                                Self::update_task_stats(
                                    &task_stats,
                                    &execution_result,
                                    execution_duration,
                                    &mut consecutive_errors
                                ).await;

                                // 更新下次执行时间
                                {
                                    let mut stats = task_stats.write().await;
                                    stats.next_execution_time = Some(Utc::now() + Duration::seconds(config.interval_seconds as i64));
                                }

                                match execution_result {
                                    Ok(refresh_results) => {
                                        debug!("OAuth Token refresh task executed successfully, processed {} tokens",
                                               refresh_results.len());
                                        consecutive_errors = 0;
                                    },
                                    Err(e) => {
                                        error!("OAuth Token refresh task execution failed: {:?}", e);
                                        consecutive_errors += 1;

                                        // 如果连续错误次数超过阈值，暂停任务
                                        if consecutive_errors >= config.max_error_retries {
                                            warn!("Too many consecutive errors, pausing task");
                                            *task_state.write().await = TaskState::Error(format!("Too many consecutive errors: {}", e));
                                        }
                                    }
                                }
                            },
                            TaskState::Paused => {
                                // 暂停状态，跳过这次执行
                                debug!("OAuth Token refresh task is paused, skipping execution");
                            },
                            TaskState::Stopping => {
                                // 收到停止信号，退出循环
                                info!("OAuth Token refresh task stopping");
                                break;
                            },
                            _ => {
                                // 其他状态，等待
                                debug!("OAuth Token refresh task in state: {:?}, waiting", current_state);
                            }
                        }
                    },

                    // 控制信号
                    Ok(control) = control_receiver.recv() => {
                        match control {
                            TaskControl::Stop => {
                                info!("Received stop signal, exiting task loop");
                                break;
                            },
                            TaskControl::Pause => {
                                debug!("Received pause signal");
                                // 状态已在外部更新
                            },
                            TaskControl::Resume => {
                                debug!("Received resume signal");
                                // 状态已在外部更新
                            },
                            TaskControl::ExecuteNow => {
                                info!("Received execute now signal, running immediate refresh");
                                let execution_result = Self::execute_refresh_task(&refresh_service).await;
                                match execution_result {
                                    Ok(refresh_results) => {
                                        info!("Immediate OAuth Token refresh completed, processed {} tokens",
                                              refresh_results.len());
                                    },
                                    Err(e) => {
                                        error!("Immediate OAuth Token refresh failed: {:?}", e);
                                    }
                                }
                            },
                            TaskControl::ReloadConfig(new_config) => {
                                info!("Reloading task configuration");
                                config = new_config;
                                ticker = interval(StdDuration::from_secs(config.interval_seconds));
                            },
                            TaskControl::Start => {
                                // 启动信号，通常不在这里处理
                                debug!("Received start signal in task loop");
                            }
                        }
                    }
                }
            }

            info!("OAuth Token refresh task loop ended");
        })
    }

    /// 执行刷新任务
    async fn execute_refresh_task(
        refresh_service: &Arc<OAuthTokenRefreshService>,
    ) -> Result<Vec<TokenRefreshResult>> {
        debug!("Executing OAuth Token refresh task");

        let start_time = Instant::now();
        let result = refresh_service.active_refresh_expiring_tokens().await;
        let duration = start_time.elapsed();

        match &result {
            Ok(refresh_results) => {
                let successful_count = refresh_results.iter().filter(|r| r.success).count();
                let failed_count = refresh_results.len() - successful_count;

                info!(
                    "OAuth Token refresh task completed in {:?}: {} successful, {} failed",
                    duration, successful_count, failed_count
                );
            }
            Err(e) => {
                error!(
                    "OAuth Token refresh task failed after {:?}: {:?}",
                    duration, e
                );
            }
        }

        result
    }

    /// 更新任务统计信息
    async fn update_task_stats(
        task_stats: &Arc<RwLock<TaskStats>>,
        execution_result: &Result<Vec<TokenRefreshResult>>,
        execution_duration: StdDuration,
        consecutive_errors: &mut u32,
    ) {
        let mut stats = task_stats.write().await;

        stats.last_execution_time = Some(Utc::now());
        stats.total_executions += 1;

        // 更新平均执行时长
        let duration_ms = execution_duration.as_millis() as f64;
        if stats.total_executions == 1 {
            stats.average_execution_duration_ms = duration_ms;
        } else {
            stats.average_execution_duration_ms = (stats.average_execution_duration_ms
                * (stats.total_executions - 1) as f64
                + duration_ms)
                / stats.total_executions as f64;
        }

        match execution_result {
            Ok(refresh_results) => {
                stats.successful_executions += 1;

                // 统计刷新结果
                let successful_refreshes =
                    refresh_results.iter().filter(|r| r.success).count() as u64;
                let failed_refreshes = refresh_results.len() as u64 - successful_refreshes;

                stats.total_tokens_refreshed += refresh_results.len() as u64;
                stats.successful_token_refreshes += successful_refreshes;
                stats.failed_token_refreshes += failed_refreshes;
            }
            Err(e) => {
                stats.failed_executions += 1;
                stats.last_error = Some(e.to_string());
                stats.last_error_time = Some(Utc::now());
                stats.consecutive_errors = *consecutive_errors;
            }
        }
    }

    /// 检查是否在静默时间段内
    fn is_in_quiet_hours(&self) -> bool {
        // TODO: 实现静默时间段检查逻辑
        // 解析 config.quiet_hours (格式: "22:00-06:00") 并检查当前时间
        false
    }
}

/// 任务构建器
pub struct OAuthTokenRefreshTaskBuilder {
    config: RefreshTaskConfig,
}

impl OAuthTokenRefreshTaskBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            config: RefreshTaskConfig::default(),
        }
    }

    /// 设置执行间隔
    pub fn interval_seconds(mut self, seconds: u64) -> Self {
        self.config.interval_seconds = seconds;
        self
    }

    /// 设置是否启用
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// 设置最大并发数
    pub fn max_concurrent_refreshes(mut self, max: usize) -> Self {
        self.config.max_concurrent_refreshes = max;
        self
    }

    /// 设置任务超时时间
    pub fn task_timeout_seconds(mut self, seconds: u64) -> Self {
        self.config.task_timeout_seconds = seconds;
        self
    }

    /// 设置最大重试次数
    pub fn max_error_retries(mut self, retries: u32) -> Self {
        self.config.max_error_retries = retries;
        self
    }

    /// 设置重试间隔
    pub fn error_retry_interval_seconds(mut self, seconds: u64) -> Self {
        self.config.error_retry_interval_seconds = seconds;
        self
    }

    /// 设置静默时间
    pub fn quiet_hours(mut self, hours: Option<String>) -> Self {
        self.config.quiet_hours = hours;
        self
    }

    /// 构建任务
    pub fn build(self, refresh_service: Arc<OAuthTokenRefreshService>) -> OAuthTokenRefreshTask {
        OAuthTokenRefreshTask::new(refresh_service, self.config)
    }
}

impl Default for OAuthTokenRefreshTaskBuilder {
    fn default() -> Self {
        Self::new()
    }
}
