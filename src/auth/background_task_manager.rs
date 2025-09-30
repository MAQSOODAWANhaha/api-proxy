use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, oneshot};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use super::oauth_cleanup_task::{OAuthCleanupStats, OAuthCleanupTask};
use super::oauth_token_refresh_service::OAuthTokenRefreshService;
use super::oauth_token_refresh_task::{OAuthTokenRefreshTask, RefreshTaskConfig};
use crate::config::OAuthCleanupConfig;
use crate::error::{ProxyError, Result};

/// 后台任务类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BackgroundTaskType {
    /// OAuth 清理任务
    OAuthCleanup,
    /// OAuth Token 刷新任务
    OAuthTokenRefresh,
}

impl std::fmt::Display for BackgroundTaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackgroundTaskType::OAuthCleanup => write!(f, "oauth-cleanup"),
            BackgroundTaskType::OAuthTokenRefresh => write!(f, "oauth-token-refresh"),
        }
    }
}

/// 任务状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackgroundTaskStatus {
    NotStarted,
    Running,
    Paused,
    Stopped,
    Error(String),
}

/// 任务信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundTaskInfo {
    pub task_type: BackgroundTaskType,
    pub status: BackgroundTaskStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub run_count: u64,
    pub error_count: u64,
    pub last_error: Option<String>,
}

/// 后台任务管理器
pub struct BackgroundTaskManager {
    /// 数据库连接
    db: DatabaseConnection,

    /// OAuth 清理任务
    oauth_cleanup_task: Option<Arc<OAuthCleanupTask>>,
    oauth_cleanup_handle: Arc<RwLock<Option<JoinHandle<()>>>>,

    /// OAuth Token 刷新任务
    oauth_token_refresh_task: Option<Arc<OAuthTokenRefreshTask>>,

    /// 任务状态信息
    task_info: Arc<RwLock<HashMap<BackgroundTaskType, BackgroundTaskInfo>>>,

    /// 停止信号
    shutdown_tx: Arc<RwLock<Option<oneshot::Sender<()>>>>,
}

impl BackgroundTaskManager {
    /// 创建新的后台任务管理器
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            oauth_cleanup_task: None,
            oauth_cleanup_handle: Arc::new(RwLock::new(None)),
            oauth_token_refresh_task: None,
            task_info: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// 初始化 OAuth 清理任务
    pub async fn setup_oauth_cleanup_task(&mut self, config: OAuthCleanupConfig) -> Result<()> {
        // OAuth cleanup is always enabled - removed config.enabled check

        let cleanup_task = Arc::new(OAuthCleanupTask::new(self.db.clone(), config));
        self.oauth_cleanup_task = Some(cleanup_task);

        // 初始化任务信息
        let mut task_info = self.task_info.write().await;
        task_info.insert(
            BackgroundTaskType::OAuthCleanup,
            BackgroundTaskInfo {
                task_type: BackgroundTaskType::OAuthCleanup,
                status: BackgroundTaskStatus::NotStarted,
                started_at: None,
                last_run_at: None,
                next_run_at: None,
                run_count: 0,
                error_count: 0,
                last_error: None,
            },
        );

        info!(
            component = "background_task_manager",
            "OAuth cleanup task initialized"
        );
        Ok(())
    }

    /// 初始化 OAuth Token 刷新任务
    pub async fn setup_oauth_token_refresh_task(
        &mut self,
        refresh_service: Arc<OAuthTokenRefreshService>,
        config: RefreshTaskConfig,
    ) -> Result<()> {
        if !config.enabled {
            info!(
                component = "background_task_manager",
                "OAuth token refresh task is disabled"
            );
            return Ok(());
        }

        let token_refresh_task = Arc::new(OAuthTokenRefreshTask::new(refresh_service, config));
        self.oauth_token_refresh_task = Some(token_refresh_task);

        // 初始化任务信息
        let mut task_info = self.task_info.write().await;
        task_info.insert(
            BackgroundTaskType::OAuthTokenRefresh,
            BackgroundTaskInfo {
                task_type: BackgroundTaskType::OAuthTokenRefresh,
                status: BackgroundTaskStatus::NotStarted,
                started_at: None,
                last_run_at: None,
                next_run_at: None,
                run_count: 0,
                error_count: 0,
                last_error: None,
            },
        );

        info!(
            component = "background_task_manager",
            "OAuth token refresh task initialized"
        );
        Ok(())
    }

    /// 启动所有任务
    pub async fn start_all_tasks(&self) -> Result<()> {
        info!(
            component = "background_task_manager",
            "Starting all background tasks"
        );

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        *self.shutdown_tx.write().await = Some(shutdown_tx);

        // 启动 OAuth 清理任务
        if let Some(cleanup_task) = &self.oauth_cleanup_task {
            let task = Arc::clone(cleanup_task);
            let task_info = Arc::clone(&self.task_info);

            let handle = tokio::spawn(async move {
                let mut shutdown_rx = shutdown_rx;

                // 更新任务状态
                {
                    let mut info = task_info.write().await;
                    if let Some(task_info) = info.get_mut(&BackgroundTaskType::OAuthCleanup) {
                        task_info.status = BackgroundTaskStatus::Running;
                        task_info.started_at = Some(Utc::now());
                    }
                }

                tokio::select! {
                    _ = task.start() => {
                        // 任务正常结束
                        let mut info = task_info.write().await;
                        if let Some(task_info) = info.get_mut(&BackgroundTaskType::OAuthCleanup) {
                            task_info.status = BackgroundTaskStatus::Stopped;
                        }
                    }
                    _ = &mut shutdown_rx => {
                        // 收到停止信号
                        let mut info = task_info.write().await;
                        if let Some(task_info) = info.get_mut(&BackgroundTaskType::OAuthCleanup) {
                            task_info.status = BackgroundTaskStatus::Stopped;
                        }
                        info!(component = "background_task_manager", "OAuth cleanup task stopped by shutdown signal");
                    }
                }
            });

            *self.oauth_cleanup_handle.write().await = Some(handle);
        }

        // 启动 OAuth Token 刷新任务
        if let Some(token_refresh_task) = &self.oauth_token_refresh_task {
            if let Err(e) = token_refresh_task.start().await {
                error!(
                    component = "background_task_manager",
                    "Failed to start OAuth token refresh task: {:?}", e
                );
                return Err(e);
            }

            // 更新任务状态
            let mut task_info = self.task_info.write().await;
            if let Some(info) = task_info.get_mut(&BackgroundTaskType::OAuthTokenRefresh) {
                info.status = BackgroundTaskStatus::Running;
                info.started_at = Some(Utc::now());
            }
        }

        info!(
            component = "background_task_manager",
            "All background tasks started successfully"
        );
        Ok(())
    }

    /// 停止所有任务
    pub async fn stop_all_tasks(&self) -> Result<()> {
        info!(
            component = "background_task_manager",
            "Stopping all background tasks"
        );

        // 发送停止信号给 OAuth 清理任务
        if let Some(shutdown_tx) = self.shutdown_tx.write().await.take() {
            let _ = shutdown_tx.send(());
        }

        // 等待 OAuth 清理任务停止
        if let Some(handle) = self.oauth_cleanup_handle.write().await.take() {
            if let Err(e) = handle.await {
                error!(
                    component = "background_task_manager",
                    "OAuth cleanup task handle error: {:?}", e
                );
            }
        }

        // 停止 OAuth Token 刷新任务
        if let Some(token_refresh_task) = &self.oauth_token_refresh_task {
            if let Err(e) = token_refresh_task.stop().await {
                error!(
                    component = "background_task_manager",
                    "Failed to stop OAuth token refresh task: {:?}", e
                );
            }
        }

        // 更新所有任务状态
        let mut task_info = self.task_info.write().await;
        for (_, info) in task_info.iter_mut() {
            info.status = BackgroundTaskStatus::Stopped;
        }

        info!(
            component = "background_task_manager",
            "All background tasks stopped"
        );
        Ok(())
    }

    /// 获取任务状态
    pub async fn get_task_status(
        &self,
        task_type: BackgroundTaskType,
    ) -> Option<BackgroundTaskInfo> {
        self.task_info.read().await.get(&task_type).cloned()
    }

    /// 获取所有任务状态
    pub async fn get_all_task_status(&self) -> Vec<BackgroundTaskInfo> {
        self.task_info.read().await.values().cloned().collect()
    }

    /// 获取 OAuth 清理统计信息
    pub async fn get_oauth_cleanup_stats(&self) -> Result<Option<OAuthCleanupStats>> {
        if let Some(cleanup_task) = &self.oauth_cleanup_task {
            let stats = cleanup_task
                .get_cleanup_stats()
                .await
                .map_err(|e| ProxyError::business(format!("Failed to get cleanup stats: {}", e)))?;
            Ok(Some(stats))
        } else {
            Ok(None)
        }
    }

    /// 手动执行 OAuth 清理
    pub async fn execute_oauth_cleanup_now(&self) -> Result<()> {
        if let Some(cleanup_task) = &self.oauth_cleanup_task {
            cleanup_task
                .cleanup_expired_sessions()
                .await
                .map_err(|e| ProxyError::business(format!("Manual cleanup failed: {}", e)))?;

            // 更新运行统计
            let mut task_info = self.task_info.write().await;
            if let Some(info) = task_info.get_mut(&BackgroundTaskType::OAuthCleanup) {
                info.run_count += 1;
                info.last_run_at = Some(Utc::now());
            }

            info!(
                component = "background_task_manager",
                "Manual OAuth cleanup executed successfully"
            );
            Ok(())
        } else {
            Err(ProxyError::business("OAuth cleanup task not initialized"))
        }
    }

    /// 手动执行 OAuth Token 刷新
    pub async fn execute_oauth_token_refresh_now(&self) -> Result<()> {
        if let Some(token_refresh_task) = &self.oauth_token_refresh_task {
            token_refresh_task.execute_now().await?;
            info!(
                component = "background_task_manager",
                "Manual OAuth token refresh triggered"
            );
            Ok(())
        } else {
            Err(ProxyError::business(
                "OAuth token refresh task not initialized",
            ))
        }
    }

    /// 暂停指定任务
    pub async fn pause_task(&self, task_type: BackgroundTaskType) -> Result<()> {
        match task_type {
            BackgroundTaskType::OAuthCleanup => {
                // OAuth 清理任务暂时不支持暂停，因为它是简单的定时器
                warn!(
                    component = "background_task_manager",
                    "OAuth cleanup task does not support pause operation"
                );
                Ok(())
            }
            BackgroundTaskType::OAuthTokenRefresh => {
                if let Some(task) = &self.oauth_token_refresh_task {
                    task.pause().await?;

                    // 更新状态
                    let mut task_info = self.task_info.write().await;
                    if let Some(info) = task_info.get_mut(&task_type) {
                        info.status = BackgroundTaskStatus::Paused;
                    }

                    Ok(())
                } else {
                    Err(ProxyError::business(
                        "OAuth token refresh task not initialized",
                    ))
                }
            }
        }
    }

    /// 恢复指定任务
    pub async fn resume_task(&self, task_type: BackgroundTaskType) -> Result<()> {
        match task_type {
            BackgroundTaskType::OAuthCleanup => {
                // OAuth 清理任务暂时不支持恢复
                warn!(
                    component = "background_task_manager",
                    "OAuth cleanup task does not support resume operation"
                );
                Ok(())
            }
            BackgroundTaskType::OAuthTokenRefresh => {
                if let Some(task) = &self.oauth_token_refresh_task {
                    task.resume().await?;

                    // 更新状态
                    let mut task_info = self.task_info.write().await;
                    if let Some(info) = task_info.get_mut(&task_type) {
                        info.status = BackgroundTaskStatus::Running;
                    }

                    Ok(())
                } else {
                    Err(ProxyError::business(
                        "OAuth token refresh task not initialized",
                    ))
                }
            }
        }
    }
}

impl Drop for BackgroundTaskManager {
    fn drop(&mut self) {
        // 这里可以添加清理逻辑，但不能使用 async
        // 实际的清理工作应该在 stop_all_tasks 中完成
        tracing::debug!("BackgroundTaskManager dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 注意：由于当前 sea-orm 版本不支持 MockDatabase，这些测试被注释掉
    // 完整的功能测试将在集成测试中进行

    #[test]
    fn test_background_task_type_display() {
        assert_eq!(
            BackgroundTaskType::OAuthCleanup.to_string(),
            "oauth-cleanup"
        );
        assert_eq!(
            BackgroundTaskType::OAuthTokenRefresh.to_string(),
            "oauth-token-refresh"
        );
    }
}
