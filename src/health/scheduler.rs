//! # 健康检查调度器

use crate::error::{ProxyError, Result};
use super::service::HealthCheckService;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::interval;

/// 健康检查调度器
pub struct HealthCheckScheduler {
    /// 健康检查服务
    health_service: Arc<HealthCheckService>,
    /// 主循环任务句柄
    main_task: Option<JoinHandle<()>>,
    /// 清理任务句柄
    cleanup_task: Option<JoinHandle<()>>,
    /// 检查间隔
    check_interval: Duration,
    /// 清理间隔
    cleanup_interval: Duration,
    /// 是否正在运行
    is_running: bool,
}

impl HealthCheckScheduler {
    /// 创建新的调度器
    pub fn new(
        health_service: Arc<HealthCheckService>,
        check_interval: Option<Duration>,
        cleanup_interval: Option<Duration>,
    ) -> Self {
        Self {
            health_service,
            main_task: None,
            cleanup_task: None,
            check_interval: check_interval.unwrap_or(Duration::from_secs(30)),
            cleanup_interval: cleanup_interval.unwrap_or(Duration::from_secs(300)), // 5分钟
            is_running: false,
        }
    }

    /// 启动调度器
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Err(ProxyError::server_init("Scheduler already running".to_string()));
        }

        // 启动健康检查服务
        self.health_service.start().await?;

        // 启动主检查循环
        self.start_main_loop().await?;

        // 启动清理任务
        self.start_cleanup_task().await?;

        self.is_running = true;
        tracing::info!("Health check scheduler started");
        Ok(())
    }

    /// 停止调度器
    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            return Ok(());
        }

        // 停止主循环
        if let Some(task) = self.main_task.take() {
            task.abort();
        }

        // 停止清理任务
        if let Some(task) = self.cleanup_task.take() {
            task.abort();
        }

        // 停止健康检查服务
        self.health_service.stop().await?;

        self.is_running = false;
        tracing::info!("Health check scheduler stopped");
        Ok(())
    }

    /// 启动主检查循环
    async fn start_main_loop(&mut self) -> Result<()> {
        let health_service = Arc::clone(&self.health_service);
        let interval_duration = self.check_interval;

        let task = tokio::spawn(async move {
            let mut interval = interval(interval_duration);
            
            loop {
                interval.tick().await;
                
                // 检查服务是否仍在运行
                if !health_service.is_running().await {
                    tracing::debug!("Health service stopped, exiting main loop");
                    break;
                }

                // 执行待处理的健康检查
                match health_service.execute_pending_checks().await {
                    Ok(executed_count) => {
                        if executed_count > 0 {
                            tracing::debug!("Executed {} health checks", executed_count);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error executing health checks: {}", e);
                    }
                }
            }
            
            tracing::debug!("Health check main loop ended");
        });

        self.main_task = Some(task);
        Ok(())
    }

    /// 启动清理任务
    async fn start_cleanup_task(&mut self) -> Result<()> {
        let health_service = Arc::clone(&self.health_service);
        let interval_duration = self.cleanup_interval;
        let max_age = Duration::from_secs(3600); // 1小时

        let task = tokio::spawn(async move {
            let mut interval = interval(interval_duration);
            
            loop {
                interval.tick().await;
                
                // 检查服务是否仍在运行
                if !health_service.is_running().await {
                    tracing::debug!("Health service stopped, exiting cleanup loop");
                    break;
                }

                // 清理过期的检查结果
                match health_service.cleanup_old_results(max_age).await {
                    Ok(cleaned_count) => {
                        if cleaned_count > 0 {
                            tracing::debug!("Cleaned {} old health check results", cleaned_count);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error cleaning old results: {}", e);
                    }
                }
            }
            
            tracing::debug!("Health check cleanup loop ended");
        });

        self.cleanup_task = Some(task);
        Ok(())
    }

    /// 检查调度器是否正在运行
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// 获取检查间隔
    pub fn check_interval(&self) -> Duration {
        self.check_interval
    }

    /// 设置检查间隔
    pub fn set_check_interval(&mut self, interval: Duration) {
        self.check_interval = interval;
    }

    /// 获取清理间隔
    pub fn cleanup_interval(&self) -> Duration {
        self.cleanup_interval
    }

    /// 设置清理间隔
    pub fn set_cleanup_interval(&mut self, interval: Duration) {
        self.cleanup_interval = interval;
    }

    /// 立即执行一轮健康检查
    pub async fn execute_immediate_check(&self) -> Result<usize> {
        if !self.is_running {
            return Err(ProxyError::server_init("Scheduler not running".to_string()));
        }

        self.health_service.execute_pending_checks().await
    }

    /// 获取健康服务引用
    pub fn health_service(&self) -> &Arc<HealthCheckService> {
        &self.health_service
    }
}

impl Drop for HealthCheckScheduler {
    fn drop(&mut self) {
        // 确保在析构时停止所有任务
        if let Some(task) = self.main_task.take() {
            task.abort();
        }
        if let Some(task) = self.cleanup_task.take() {
            task.abort();
        }
    }
}

/// 健康检查调度器构建器
pub struct HealthCheckSchedulerBuilder {
    health_service: Option<Arc<HealthCheckService>>,
    check_interval: Option<Duration>,
    cleanup_interval: Option<Duration>,
}

impl HealthCheckSchedulerBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            health_service: None,
            check_interval: None,
            cleanup_interval: None,
        }
    }

    /// 设置健康检查服务
    pub fn with_health_service(mut self, service: Arc<HealthCheckService>) -> Self {
        self.health_service = Some(service);
        self
    }

    /// 设置检查间隔
    pub fn with_check_interval(mut self, interval: Duration) -> Self {
        self.check_interval = Some(interval);
        self
    }

    /// 设置清理间隔
    pub fn with_cleanup_interval(mut self, interval: Duration) -> Self {
        self.cleanup_interval = Some(interval);
        self
    }

    /// 构建调度器
    pub fn build(self) -> Result<HealthCheckScheduler> {
        let health_service = self.health_service
            .ok_or_else(|| ProxyError::server_init("Health service is required".to_string()))?;

        Ok(HealthCheckScheduler::new(
            health_service,
            self.check_interval,
            self.cleanup_interval,
        ))
    }
}

impl Default for HealthCheckSchedulerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 调度器管理器 - 管理多个调度器实例
pub struct SchedulerManager {
    schedulers: Vec<HealthCheckScheduler>,
}

impl SchedulerManager {
    /// 创建新的管理器
    pub fn new() -> Self {
        Self {
            schedulers: Vec::new(),
        }
    }

    /// 添加调度器
    pub fn add_scheduler(&mut self, scheduler: HealthCheckScheduler) {
        self.schedulers.push(scheduler);
    }

    /// 启动所有调度器
    pub async fn start_all(&mut self) -> Result<()> {
        for scheduler in &mut self.schedulers {
            scheduler.start().await?;
        }
        tracing::info!("Started {} health check schedulers", self.schedulers.len());
        Ok(())
    }

    /// 停止所有调度器
    pub async fn stop_all(&mut self) -> Result<()> {
        for scheduler in &mut self.schedulers {
            scheduler.stop().await?;
        }
        tracing::info!("Stopped {} health check schedulers", self.schedulers.len());
        Ok(())
    }

    /// 获取运行中的调度器数量
    pub fn running_count(&self) -> usize {
        self.schedulers.iter().filter(|s| s.is_running()).count()
    }

    /// 执行所有调度器的立即检查
    pub async fn execute_immediate_checks(&self) -> Result<usize> {
        let mut total_executed = 0;
        
        for scheduler in &self.schedulers {
            if scheduler.is_running() {
                match scheduler.execute_immediate_check().await {
                    Ok(count) => total_executed += count,
                    Err(e) => tracing::error!("Failed to execute immediate check: {}", e),
                }
            }
        }
        
        Ok(total_executed)
    }
}

impl Default for SchedulerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::service::HealthCheckService;

    #[tokio::test]
    async fn test_scheduler_creation() {
        let health_service = Arc::new(HealthCheckService::new(None));
        let scheduler = HealthCheckScheduler::new(
            health_service,
            Some(Duration::from_millis(100)),
            Some(Duration::from_millis(500)),
        );
        
        assert!(!scheduler.is_running());
        assert_eq!(scheduler.check_interval(), Duration::from_millis(100));
        assert_eq!(scheduler.cleanup_interval(), Duration::from_millis(500));
    }

    #[tokio::test]
    async fn test_scheduler_builder() {
        let health_service = Arc::new(HealthCheckService::new(None));
        
        let scheduler = HealthCheckSchedulerBuilder::new()
            .with_health_service(health_service)
            .with_check_interval(Duration::from_millis(200))
            .with_cleanup_interval(Duration::from_secs(1))
            .build();
        
        assert!(scheduler.is_ok());
        let scheduler = scheduler.unwrap();
        assert_eq!(scheduler.check_interval(), Duration::from_millis(200));
        assert_eq!(scheduler.cleanup_interval(), Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_scheduler_start_stop() {
        let health_service = Arc::new(HealthCheckService::new(None));
        let mut scheduler = HealthCheckScheduler::new(
            health_service,
            Some(Duration::from_millis(50)),
            Some(Duration::from_millis(100)),
        );
        
        assert!(scheduler.start().await.is_ok());
        assert!(scheduler.is_running());
        
        // 等待一小段时间确保任务启动
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        assert!(scheduler.stop().await.is_ok());
        assert!(!scheduler.is_running());
    }

    #[tokio::test]
    async fn test_scheduler_manager() {
        let health_service1 = Arc::new(HealthCheckService::new(None));
        let health_service2 = Arc::new(HealthCheckService::new(None));
        
        let scheduler1 = HealthCheckScheduler::new(
            health_service1,
            Some(Duration::from_millis(100)),
            None,
        );
        let scheduler2 = HealthCheckScheduler::new(
            health_service2,
            Some(Duration::from_millis(100)),
            None,
        );
        
        let mut manager = SchedulerManager::new();
        manager.add_scheduler(scheduler1);
        manager.add_scheduler(scheduler2);
        
        assert_eq!(manager.running_count(), 0);
        
        assert!(manager.start_all().await.is_ok());
        assert_eq!(manager.running_count(), 2);
        
        assert!(manager.stop_all().await.is_ok());
        assert_eq!(manager.running_count(), 0);
    }

    #[tokio::test]
    async fn test_immediate_check() {
        let health_service = Arc::new(HealthCheckService::new(None));
        let mut scheduler = HealthCheckScheduler::new(
            health_service,
            Some(Duration::from_secs(1)),
            None,
        );
        
        // 在未启动时应该失败
        assert!(scheduler.execute_immediate_check().await.is_err());
        
        // 启动后应该成功（即使没有服务器）
        assert!(scheduler.start().await.is_ok());
        let result = scheduler.execute_immediate_check().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0); // 没有服务器，执行0个检查
        
        assert!(scheduler.stop().await.is_ok());
    }
}