//! # 健康检查服务

use crate::error::{ProxyError, Result};
use crate::proxy::upstream::ProviderId;
use super::types::{
    HealthCheckResult, HealthCheckConfig, ServerHealthStatus, HealthCheckTask, TaskStatus
};
use super::checker::HealthChecker;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};
use serde::Serialize;
use serde_with::{serde_as, DurationMilliSeconds};

/// 健康检查服务
pub struct HealthCheckService {
    /// 健康检查器
    checker: Arc<HealthChecker>,
    /// 服务器健康状态
    health_status: Arc<RwLock<HashMap<String, ServerHealthStatus>>>,
    /// 检查任务
    tasks: Arc<RwLock<HashMap<String, HealthCheckTask>>>,
    /// 全局配置
    global_config: HealthCheckConfig,
    /// 服务状态
    is_running: Arc<RwLock<bool>>,
}

impl HealthCheckService {
    /// 创建新的健康检查服务
    pub fn new(global_config: Option<HealthCheckConfig>) -> Self {
        Self {
            checker: Arc::new(HealthChecker::new()),
            health_status: Arc::new(RwLock::new(HashMap::new())),
            tasks: Arc::new(RwLock::new(HashMap::new())),
            global_config: global_config.unwrap_or_default(),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// 启动健康检查服务
    pub async fn start(&self) -> Result<()> {
        let mut running = self.is_running.write().await;
        if *running {
            return Err(ProxyError::server_init("Health check service already running".to_string()));
        }
        
        *running = true;
        tracing::info!("Health check service started");
        Ok(())
    }

    /// 停止健康检查服务
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.is_running.write().await;
        if !*running {
            return Ok(());
        }

        *running = false;
        
        // 停止所有任务
        let mut tasks = self.tasks.write().await;
        for task in tasks.values_mut() {
            task.set_status(TaskStatus::Stopped);
        }
        
        tracing::info!("Health check service stopped");
        Ok(())
    }

    /// 添加服务器到健康检查
    pub async fn add_server(
        &self,
        server_address: String,
        provider_id: ProviderId,
        config: Option<HealthCheckConfig>,
    ) -> Result<()> {
        let check_config = config.unwrap_or_else(|| self.global_config.clone());
        
        // 创建健康状态记录
        let health_status = ServerHealthStatus::new(server_address.clone(), provider_id.clone());
        self.health_status.write().await.insert(server_address.clone(), health_status);

        // 创建检查任务
        let task = HealthCheckTask::new(server_address.clone(), provider_id, check_config);
        self.tasks.write().await.insert(task.id.clone(), task);

        tracing::info!("Added server {} to health monitoring", server_address);
        Ok(())
    }

    /// 移除服务器的健康检查
    pub async fn remove_server(&self, server_address: &str) -> Result<()> {
        self.health_status.write().await.remove(server_address);
        
        // 找到并移除对应的任务
        let mut tasks = self.tasks.write().await;
        let task_ids: Vec<_> = tasks
            .values()
            .filter(|task| task.server_address == server_address)
            .map(|task| task.id.clone())
            .collect();
        
        for task_id in task_ids {
            tasks.remove(&task_id);
        }

        tracing::info!("Removed server {} from health monitoring", server_address);
        Ok(())
    }

    /// 获取服务器健康状态
    pub async fn get_server_health(&self, server_address: &str) -> Option<ServerHealthStatus> {
        self.health_status.read().await.get(server_address).cloned()
    }

    /// 获取所有服务器健康状态
    pub async fn get_all_health_status(&self) -> HashMap<String, ServerHealthStatus> {
        self.health_status.read().await.clone()
    }

    /// 获取健康的服务器列表
    pub async fn get_healthy_servers(&self, provider_id: &ProviderId) -> Vec<String> {
        self.health_status
            .read()
            .await
            .values()
            .filter(|status| status.provider_id == *provider_id && status.is_healthy)
            .map(|status| status.server_address.clone())
            .collect()
    }

    /// 获取不健康的服务器列表
    pub async fn get_unhealthy_servers(&self, provider_id: &ProviderId) -> Vec<String> {
        self.health_status
            .read()
            .await
            .values()
            .filter(|status| status.provider_id == *provider_id && !status.is_healthy)
            .map(|status| status.server_address.clone())
            .collect()
    }

    /// 获取整体健康统计信息
    pub async fn get_overall_health(&self) -> anyhow::Result<HealthCheckStatistics> {
        let health_status = self.health_status.read().await;
        let total_servers = health_status.len();
        let healthy_servers = health_status.values().filter(|s| s.is_healthy).count();
        let unhealthy_servers = total_servers - healthy_servers;
        
        let avg_response_time = if !health_status.is_empty() {
            let total_ms: u64 = health_status.values()
                .map(|s| s.avg_response_time.as_millis() as u64)
                .sum();
            Duration::from_millis(total_ms / health_status.len() as u64)
        } else {
            Duration::from_millis(0)
        };

        let active_tasks = self.tasks.read().await.len();
        let is_running = self.is_running().await;

        Ok(HealthCheckStatistics {
            total_servers,
            healthy_servers,
            unhealthy_servers,
            active_tasks,
            avg_response_time,
            is_running,
        })
    }

    /// 手动执行健康检查
    pub async fn manual_check(&self, server_address: &str) -> Result<HealthCheckResult> {
        let task = {
            let tasks = self.tasks.read().await;
            tasks
                .values()
                .find(|task| task.server_address == server_address)
                .cloned()
        };

        if let Some(task) = task {
            let result = self.checker.check_health(&task.server_address, &task.config).await?;
            self.update_health_status(&task.server_address, result.clone()).await?;
            Ok(result)
        } else {
            Err(ProxyError::server_init(format!("Server {} not found in health monitoring", server_address)))
        }
    }

    /// 执行所有待处理的健康检查
    pub async fn execute_pending_checks(&self) -> Result<usize> {
        if !*self.is_running.read().await {
            return Ok(0);
        }

        let pending_tasks: Vec<_> = {
            let tasks = self.tasks.read().await;
            tasks
                .values()
                .filter(|task| task.should_execute())
                .cloned()
                .collect()
        };

        if pending_tasks.is_empty() {
            return Ok(0);
        }

        let mut executed_count = 0;
        
        // 并发执行所有待处理的检查
        let check_futures: Vec<_> = pending_tasks
            .iter()
            .map(|task| {
                let checker = Arc::clone(&self.checker);
                let task = task.clone();
                async move {
                    let result = checker.check_health(&task.server_address, &task.config).await;
                    (task, result)
                }
            })
            .collect();

        let results = futures::future::join_all(check_futures).await;

        // 处理检查结果
        for (task, check_result) in results {
            match check_result {
                Ok(health_result) => {
                    // 更新健康状态
                    if let Err(e) = self.update_health_status(&task.server_address, health_result.clone()).await {
                        tracing::error!("Failed to update health status for {}: {}", task.server_address, e);
                        continue;
                    }

                    // 更新任务状态
                    self.update_task_status(&task.id).await?;
                    executed_count += 1;

                    tracing::debug!(
                        "Health check completed for {} - healthy: {}, response_time: {}ms",
                        task.server_address,
                        health_result.is_healthy,
                        health_result.response_time_ms
                    );
                }
                Err(e) => {
                    tracing::error!("Health check failed for {}: {}", task.server_address, e);
                    
                    // 创建错误结果并更新状态
                    let error_result = HealthCheckResult::failure(
                        format!("Check error: {}", e),
                        task.config.check_type,
                    );
                    
                    if let Err(update_err) = self.update_health_status(&task.server_address, error_result).await {
                        tracing::error!("Failed to update error status for {}: {}", task.server_address, update_err);
                    }
                }
            }
        }

        Ok(executed_count)
    }

    /// 更新健康状态
    async fn update_health_status(
        &self,
        server_address: &str,
        result: HealthCheckResult,
    ) -> Result<()> {
        let mut health_map = self.health_status.write().await;
        
        if let Some(status) = health_map.get_mut(server_address) {
            let was_healthy = status.is_healthy;
            status.update_status(result);
            
            // 记录状态变化
            if was_healthy != status.is_healthy {
                if status.is_healthy {
                    tracing::info!("Server {} recovered (healthy)", server_address);
                } else {
                    tracing::warn!("Server {} became unhealthy", server_address);
                }
            }
        } else {
            tracing::warn!("Attempted to update health status for unknown server: {}", server_address);
        }

        Ok(())
    }

    /// 更新任务状态
    async fn update_task_status(&self, task_id: &str) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        
        if let Some(task) = tasks.get_mut(task_id) {
            task.update_next_check();
            task.set_status(TaskStatus::Pending);
        }

        Ok(())
    }

    /// 清理过期的检查结果
    pub async fn cleanup_old_results(&self, max_age: Duration) -> Result<usize> {
        let cutoff_time = Instant::now() - max_age;
        let mut cleaned_count = 0;

        let mut health_map = self.health_status.write().await;
        
        for status in health_map.values_mut() {
            let initial_count = status.recent_results.len();
            status.recent_results.retain(|result| result.timestamp > cutoff_time);
            cleaned_count += initial_count - status.recent_results.len();
        }

        if cleaned_count > 0 {
            tracing::debug!("Cleaned {} old health check results", cleaned_count);
        }

        Ok(cleaned_count)
    }

    /// 获取健康检查统计信息
    pub async fn get_statistics(&self) -> HealthCheckStatistics {
        let health_map = self.health_status.read().await;
        let tasks_map = self.tasks.read().await;

        let total_servers = health_map.len();
        let healthy_servers = health_map.values().filter(|s| s.is_healthy).count();
        let unhealthy_servers = total_servers - healthy_servers;
        
        let active_tasks = tasks_map.values()
            .filter(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::Running))
            .count();

        // 计算平均响应时间
        let total_response_time: u64 = health_map.values()
            .map(|s| s.avg_response_time.as_millis() as u64)
            .sum();
        
        let avg_response_time = if total_servers > 0 {
            Duration::from_millis(total_response_time / total_servers as u64)
        } else {
            Duration::from_millis(0)
        };

        HealthCheckStatistics {
            total_servers,
            healthy_servers,
            unhealthy_servers,
            active_tasks,
            avg_response_time,
            is_running: *self.is_running.read().await,
        }
    }

    /// 检查服务是否正在运行
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// 强制标记服务器为不健康
    pub async fn mark_server_unhealthy(&self, server_address: &str, reason: String) -> Result<()> {
        let mut health_map = self.health_status.write().await;
        
        if let Some(status) = health_map.get_mut(server_address) {
            status.is_healthy = false;
            status.consecutive_failures += 1;
            
            // 添加强制失败结果
            let failure_result = HealthCheckResult::failure(
                format!("Manually marked unhealthy: {}", reason),
                self.global_config.check_type,
            );
            status.update_status(failure_result);
            
            tracing::warn!("Manually marked server {} as unhealthy: {}", server_address, reason);
            Ok(())
        } else {
            Err(ProxyError::server_init(format!("Server {} not found", server_address)))
        }
    }

    /// 强制标记服务器为健康
    pub async fn mark_server_healthy(&self, server_address: &str) -> Result<()> {
        let mut health_map = self.health_status.write().await;
        
        if let Some(status) = health_map.get_mut(server_address) {
            status.is_healthy = true;
            status.consecutive_successes += 1;
            status.consecutive_failures = 0;
            
            tracing::info!("Manually marked server {} as healthy", server_address);
            Ok(())
        } else {
            Err(ProxyError::server_init(format!("Server {} not found", server_address)))
        }
    }
}

/// 健康检查统计信息
#[serde_as]
#[derive(Debug, Clone, Serialize)]
pub struct HealthCheckStatistics {
    pub total_servers: usize,
    pub healthy_servers: usize,
    pub unhealthy_servers: usize,
    pub active_tasks: usize,
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub avg_response_time: Duration,
    pub is_running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_service_creation() {
        let service = HealthCheckService::new(None);
        assert!(!service.is_running().await);
    }

    #[tokio::test]
    async fn test_start_stop_service() {
        let service = HealthCheckService::new(None);
        
        assert!(service.start().await.is_ok());
        assert!(service.is_running().await);
        
        assert!(service.stop().await.is_ok());
        assert!(!service.is_running().await);
    }

    #[tokio::test]
    async fn test_add_remove_server() {
        let service = HealthCheckService::new(None);
        let server_addr = "127.0.0.1:8080".to_string();
        
        let provider_id = ProviderId::from_database_id(1);
        assert!(service.add_server(server_addr.clone(), provider_id, None).await.is_ok());
        assert!(service.get_server_health(&server_addr).await.is_some());
        
        assert!(service.remove_server(&server_addr).await.is_ok());
        assert!(service.get_server_health(&server_addr).await.is_none());
    }

    #[tokio::test]
    async fn test_health_statistics() {
        let service = HealthCheckService::new(None);
        
        let stats = service.get_statistics().await;
        assert_eq!(stats.total_servers, 0);
        assert_eq!(stats.healthy_servers, 0);
        assert_eq!(stats.unhealthy_servers, 0);
    }

    #[tokio::test]
    async fn test_manual_health_marking() {
        let service = HealthCheckService::new(None);
        let server_addr = "127.0.0.1:8080".to_string();
        
        let provider_id = ProviderId::from_database_id(1);
        service.add_server(server_addr.clone(), provider_id, None).await.unwrap();
        
        // 测试手动标记为不健康
        assert!(service.mark_server_unhealthy(&server_addr, "Test".to_string()).await.is_ok());
        let status = service.get_server_health(&server_addr).await.unwrap();
        assert!(!status.is_healthy);
        
        // 测试手动标记为健康
        assert!(service.mark_server_healthy(&server_addr).await.is_ok());
        let status = service.get_server_health(&server_addr).await.unwrap();
        assert!(status.is_healthy);
    }

    #[tokio::test]
    async fn test_get_healthy_unhealthy_servers() {
        let service = HealthCheckService::new(None);
        
        let provider_id = ProviderId::from_database_id(1);
        service.add_server("server1:8080".to_string(), provider_id.clone(), None).await.unwrap();
        service.add_server("server2:8080".to_string(), provider_id.clone(), None).await.unwrap();
        
        // 标记一个为不健康
        service.mark_server_unhealthy("server1:8080", "Test".to_string()).await.unwrap();
        
        let healthy = service.get_healthy_servers(&provider_id).await;
        let unhealthy = service.get_unhealthy_servers(&provider_id).await;
        
        assert_eq!(healthy.len(), 1);
        assert_eq!(unhealthy.len(), 1);
        assert!(healthy.contains(&"server2:8080".to_string()));
        assert!(unhealthy.contains(&"server1:8080".to_string()));
    }
}