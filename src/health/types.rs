//! # 健康检查类型定义

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime};
use std::collections::HashMap;
use crate::proxy::upstream::ProviderId;

/// 健康检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// 检查时间
    #[serde(skip, default = "Instant::now")]
    pub timestamp: Instant,
    /// 系统时间（用于序列化）
    pub system_time: SystemTime,
    /// 是否健康
    pub is_healthy: bool,
    /// 响应时间（毫秒）
    pub response_time_ms: u64,
    /// 状态码
    pub status_code: Option<u16>,
    /// 错误信息
    pub error_message: Option<String>,
    /// 检查类型
    pub check_type: HealthCheckType,
}

/// 健康检查类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthCheckType {
    /// HTTP GET请求
    Http,
    /// TCP连接检查
    Tcp,
    /// HTTPS SSL证书检查
    Https,
    /// 自定义检查
    Custom,
}

/// 服务器健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHealthStatus {
    /// 服务器地址
    pub server_address: String,
    /// 提供商ID
    pub provider_id: ProviderId,
    /// 当前健康状态
    pub is_healthy: bool,
    /// 最后检查时间
    #[serde(skip, default)]
    pub last_check: Option<Instant>,
    /// 最后健康时间
    #[serde(skip, default)]
    pub last_healthy: Option<Instant>,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 连续成功次数
    pub consecutive_successes: u32,
    /// 最近的检查结果
    pub recent_results: Vec<HealthCheckResult>,
    /// 平均响应时间
    pub avg_response_time: Duration,
    /// 健康分数 (0-100)
    pub health_score: f32,
}

/// 健康检查配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// 检查间隔
    pub interval: Duration,
    /// 超时时间
    pub timeout: Duration,
    /// 失败阈值
    pub failure_threshold: u32,
    /// 成功阈值
    pub success_threshold: u32,
    /// 检查类型
    pub check_type: HealthCheckType,
    /// 检查路径（HTTP检查用）
    pub path: Option<String>,
    /// 期望的状态码
    pub expected_status: Vec<u16>,
    /// 检查体内容（POST请求用）
    pub body: Option<String>,
    /// 请求头
    pub headers: HashMap<String, String>,
    /// 是否启用
    pub enabled: bool,
}

/// 健康检查任务
#[derive(Debug, Clone)]
pub struct HealthCheckTask {
    /// 任务ID
    pub id: String,
    /// 服务器地址
    pub server_address: String,
    /// 提供商ID
    pub provider_id: ProviderId,
    /// 检查配置
    pub config: HealthCheckConfig,
    /// 下次检查时间
    pub next_check: Instant,
    /// 任务状态
    pub status: TaskStatus,
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// 等待中
    Pending,
    /// 运行中
    Running,
    /// 已暂停
    Paused,
    /// 已停止
    Stopped,
    /// 已完成
    Completed,
    /// 失败
    Failed,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(10),
            failure_threshold: 3,
            success_threshold: 2,
            check_type: HealthCheckType::Http,
            path: Some("/health".to_string()),
            expected_status: vec![200, 201, 204],
            body: None,
            headers: HashMap::new(),
            enabled: true,
        }
    }
}

impl ServerHealthStatus {
    /// 创建新的健康状态
    pub fn new(server_address: String, provider_id: ProviderId) -> Self {
        Self {
            server_address,
            provider_id,
            is_healthy: true, // 默认假设健康
            last_check: None,
            last_healthy: None,
            consecutive_failures: 0,
            consecutive_successes: 0,
            recent_results: Vec::new(),
            avg_response_time: Duration::from_millis(0),
            health_score: 100.0,
        }
    }

    /// 更新健康状态
    pub fn update_status(&mut self, result: HealthCheckResult) {
        self.last_check = Some(result.timestamp);
        
        // 保留最近50个结果
        self.recent_results.push(result.clone());
        if self.recent_results.len() > 50 {
            self.recent_results.remove(0);
        }

        if result.is_healthy {
            self.consecutive_successes += 1;
            self.consecutive_failures = 0;
            self.last_healthy = Some(result.timestamp);
        } else {
            self.consecutive_failures += 1;
            self.consecutive_successes = 0;
        }

        // 重新计算健康状态
        self.recalculate_health();
    }

    /// 重新计算健康状态和分数
    fn recalculate_health(&mut self) {
        // 根据连续失败次数决定健康状态
        let config = HealthCheckConfig::default();
        self.is_healthy = self.consecutive_failures < config.failure_threshold;

        // 计算平均响应时间
        if !self.recent_results.is_empty() {
            let total_response_time: u64 = self.recent_results
                .iter()
                .filter(|r| r.is_healthy)
                .map(|r| r.response_time_ms)
                .sum();
            let healthy_count = self.recent_results
                .iter()
                .filter(|r| r.is_healthy)
                .count();
            
            if healthy_count > 0 {
                self.avg_response_time = Duration::from_millis(total_response_time / healthy_count as u64);
            }
        }

        // 计算健康分数
        self.health_score = self.calculate_health_score();
    }

    /// 计算健康分数
    fn calculate_health_score(&self) -> f32 {
        if self.recent_results.is_empty() {
            return 100.0;
        }

        let recent_count = std::cmp::min(self.recent_results.len(), 10);
        let recent_results = &self.recent_results[self.recent_results.len() - recent_count..];
        
        // 基础健康率
        let healthy_count = recent_results.iter().filter(|r| r.is_healthy).count();
        let health_ratio = healthy_count as f32 / recent_results.len() as f32;
        let mut score = health_ratio * 100.0;

        // 响应时间惩罚
        if let Some(avg_time) = recent_results
            .iter()
            .filter(|r| r.is_healthy)
            .map(|r| r.response_time_ms)
            .collect::<Vec<_>>()
            .get(0)
        {
            let avg_ms = *avg_time as f32;
            // 响应时间超过1秒开始惩罚
            if avg_ms > 1000.0 {
                let penalty = ((avg_ms - 1000.0) / 1000.0) * 10.0;
                score -= penalty.min(30.0); // 最多扣30分
            }
        }

        // 连续失败惩罚
        if self.consecutive_failures > 0 {
            score -= (self.consecutive_failures as f32 * 15.0).min(50.0);
        }

        score.max(0.0).min(100.0)
    }

    /// 检查是否需要标记为不健康
    pub fn should_mark_unhealthy(&self, threshold: u32) -> bool {
        self.consecutive_failures >= threshold
    }

    /// 检查是否可以恢复为健康
    pub fn should_mark_healthy(&self, threshold: u32) -> bool {
        self.consecutive_successes >= threshold && !self.is_healthy
    }
}

impl HealthCheckResult {
    /// 创建成功的检查结果
    pub fn success(response_time_ms: u64, status_code: u16, check_type: HealthCheckType) -> Self {
        Self {
            timestamp: Instant::now(),
            system_time: SystemTime::now(),
            is_healthy: true,
            response_time_ms,
            status_code: Some(status_code),
            error_message: None,
            check_type,
        }
    }

    /// 创建失败的检查结果
    pub fn failure(error_message: String, check_type: HealthCheckType) -> Self {
        Self {
            timestamp: Instant::now(),
            system_time: SystemTime::now(),
            is_healthy: false,
            response_time_ms: 0,
            status_code: None,
            error_message: Some(error_message),
            check_type,
        }
    }

    /// 创建超时的检查结果
    pub fn timeout(timeout_ms: u64, check_type: HealthCheckType) -> Self {
        Self {
            timestamp: Instant::now(),
            system_time: SystemTime::now(),
            is_healthy: false,
            response_time_ms: timeout_ms,
            status_code: None,
            error_message: Some("Request timeout".to_string()),
            check_type,
        }
    }
}

impl HealthCheckTask {
    /// 创建新的健康检查任务
    pub fn new(
        server_address: String,
        provider_id: ProviderId,
        config: HealthCheckConfig,
    ) -> Self {
        let id = format!("{}:{}", provider_id.to_string(), server_address);
        Self {
            id,
            server_address,
            provider_id,
            config,
            next_check: Instant::now(),
            status: TaskStatus::Pending,
        }
    }

    /// 检查是否应该执行
    pub fn should_execute(&self) -> bool {
        self.status == TaskStatus::Pending && Instant::now() >= self.next_check
    }

    /// 更新下次检查时间
    pub fn update_next_check(&mut self) {
        self.next_check = Instant::now() + self.config.interval;
    }

    /// 设置任务状态
    pub fn set_status(&mut self, status: TaskStatus) {
        self.status = status;
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_result_creation() {
        let success = HealthCheckResult::success(100, 200, HealthCheckType::Http);
        assert!(success.is_healthy);
        assert_eq!(success.response_time_ms, 100);
        assert_eq!(success.status_code, Some(200));

        let failure = HealthCheckResult::failure("Connection refused".to_string(), HealthCheckType::Tcp);
        assert!(!failure.is_healthy);
        assert!(failure.error_message.is_some());
    }

    #[test]
    fn test_server_health_status_update() {
        let provider_id = ProviderId::from_database_id(1);
        let mut status = ServerHealthStatus::new("127.0.0.1:8080".to_string(), provider_id);
        
        // 模拟失败
        let failure = HealthCheckResult::failure("Connection timeout".to_string(), HealthCheckType::Http);
        status.update_status(failure);
        
        assert_eq!(status.consecutive_failures, 1);
        assert_eq!(status.consecutive_successes, 0);
    }

    #[test]
    fn test_health_score_calculation() {
        let provider_id = ProviderId::from_database_id(1);
        let mut status = ServerHealthStatus::new("127.0.0.1:8080".to_string(), provider_id);
        
        // 添加成功结果
        for _ in 0..5 {
            let success = HealthCheckResult::success(100, 200, HealthCheckType::Http);
            status.update_status(success);
        }
        
        assert!(status.health_score > 90.0);
        assert!(status.is_healthy);
    }

    #[test]
    fn test_task_scheduling() {
        let config = HealthCheckConfig::default();
        let provider_id = ProviderId::from_database_id(1);
        let mut task = HealthCheckTask::new(
            "127.0.0.1:8080".to_string(),
            provider_id,
            config,
        );
        
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.should_execute());
        
        task.update_next_check();
        assert!(!task.should_execute());
    }
}