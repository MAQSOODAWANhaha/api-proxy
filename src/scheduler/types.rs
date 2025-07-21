//! # 负载均衡调度器类型定义

use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// 调度策略枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulingStrategy {
    /// 轮询调度
    RoundRobin,
    /// 权重调度
    Weighted,
    /// 健康度最佳调度
    HealthBased,
}

impl Default for SchedulingStrategy {
    fn default() -> Self {
        Self::RoundRobin
    }
}

impl SchedulingStrategy {
    /// 从字符串解析调度策略
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "round_robin" | "roundrobin" | "rr" => Some(Self::RoundRobin),
            "weighted" | "weight" | "w" => Some(Self::Weighted),
            "health_based" | "healthbased" | "health" | "hb" => Some(Self::HealthBased),
            _ => None,
        }
    }

    /// 转换为字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RoundRobin => "round_robin",
            Self::Weighted => "weighted",
            Self::HealthBased => "health_based",
        }
    }
}

/// 服务器性能指标
#[derive(Debug, Clone)]
pub struct ServerMetrics {
    /// 平均响应时间（毫秒）
    pub avg_response_time: f64,
    /// 活跃连接数
    pub active_connections: u32,
    /// 最大连接数
    pub max_connections: u32,
    /// 成功请求数
    pub success_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 上次健康检查时间
    pub last_health_check: Instant,
    /// 健康状态
    pub is_healthy: bool,
    /// CPU使用率（百分比）
    pub cpu_usage: f32,
    /// 内存使用率（百分比）
    pub memory_usage: f32,
    /// 错误率（最近1分钟）
    pub error_rate: f32,
}

impl Default for ServerMetrics {
    fn default() -> Self {
        Self {
            avg_response_time: 0.0,
            active_connections: 0,
            max_connections: 1000,
            success_requests: 0,
            failed_requests: 0,
            last_health_check: Instant::now(),
            is_healthy: true,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            error_rate: 0.0,
        }
    }
}

impl ServerMetrics {
    /// 创建新的服务器指标
    pub fn new() -> Self {
        Self::default()
    }

    /// 计算健康分数（0-100）
    pub fn health_score(&self) -> f32 {
        if !self.is_healthy {
            return 0.0;
        }

        let mut score = 100.0f32;

        // 响应时间影响（越低越好）
        let response_penalty = (self.avg_response_time as f32 / 1000.0) * 10.0; // 每秒减10分
        score -= response_penalty.min(30.0); // 最多减30分

        // 连接使用率影响
        let connection_ratio = self.active_connections as f32 / self.max_connections as f32;
        let connection_penalty = connection_ratio * 20.0; // 满负载减20分
        score -= connection_penalty;

        // 错误率影响
        let error_penalty = self.error_rate * 30.0; // 100%错误率减30分
        score -= error_penalty;

        // CPU和内存使用率影响
        let resource_penalty = (self.cpu_usage + self.memory_usage) / 2.0 * 0.2; // 100%使用率减20分
        score -= resource_penalty;

        score.max(0.0).min(100.0)
    }

    /// 计算成功率
    pub fn success_rate(&self) -> f32 {
        let total = self.success_requests + self.failed_requests;
        if total == 0 {
            return 1.0;
        }
        self.success_requests as f32 / total as f32
    }

    /// 检查是否过载
    pub fn is_overloaded(&self) -> bool {
        let connection_ratio = self.active_connections as f32 / self.max_connections as f32;
        connection_ratio > 0.9 || self.cpu_usage > 90.0 || self.memory_usage > 90.0
    }

    /// 更新响应时间
    pub fn update_response_time(&mut self, response_time: Duration) {
        let new_time = response_time.as_millis() as f64;
        // 使用指数加权移动平均
        self.avg_response_time = self.avg_response_time * 0.9 + new_time * 0.1;
    }

    /// 记录成功请求
    pub fn record_success(&mut self) {
        self.success_requests += 1;
    }

    /// 记录失败请求
    pub fn record_failure(&mut self) {
        self.failed_requests += 1;
    }

    /// 更新健康状态
    pub fn update_health(&mut self, is_healthy: bool) {
        self.is_healthy = is_healthy;
        self.last_health_check = Instant::now();
    }

    /// 增加活跃连接
    pub fn add_connection(&mut self) {
        self.active_connections += 1;
    }

    /// 减少活跃连接
    pub fn remove_connection(&mut self) {
        if self.active_connections > 0 {
            self.active_connections -= 1;
        }
    }

    /// 检查健康检查是否过期
    pub fn is_health_check_expired(&self, ttl: Duration) -> bool {
        self.last_health_check.elapsed() > ttl
    }
}

/// 调度结果
#[derive(Debug, Clone)]
pub struct SchedulingResult {
    /// 选中的服务器索引
    pub server_index: usize,
    /// 选择原因
    pub reason: String,
    /// 健康分数（如果适用）
    pub health_score: Option<f32>,
    /// 调度策略
    pub strategy: SchedulingStrategy,
    /// 调度时间
    pub timestamp: Instant,
}

impl SchedulingResult {
    pub fn new(server_index: usize, reason: String, strategy: SchedulingStrategy) -> Self {
        Self {
            server_index,
            reason,
            health_score: None,
            strategy,
            timestamp: Instant::now(),
        }
    }

    pub fn with_health_score(mut self, score: f32) -> Self {
        self.health_score = Some(score);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_scheduling_strategy_parsing() {
        assert_eq!(SchedulingStrategy::from_str("round_robin"), Some(SchedulingStrategy::RoundRobin));
        assert_eq!(SchedulingStrategy::from_str("weighted"), Some(SchedulingStrategy::Weighted));
        assert_eq!(SchedulingStrategy::from_str("health_based"), Some(SchedulingStrategy::HealthBased));
        assert_eq!(SchedulingStrategy::from_str("unknown"), None);
    }

    #[test]
    fn test_server_metrics_health_score() {
        let mut metrics = ServerMetrics::new();
        
        // 健康服务器应该有高分数
        assert!(metrics.health_score() > 90.0);
        
        // 不健康服务器应该得0分
        metrics.is_healthy = false;
        assert_eq!(metrics.health_score(), 0.0);
        
        // 高延迟应该降低分数
        metrics.is_healthy = true;
        metrics.avg_response_time = 5000.0; // 5秒
        assert!(metrics.health_score() < 80.0);
    }

    #[test]
    fn test_server_metrics_success_rate() {
        let mut metrics = ServerMetrics::new();
        
        // 初始成功率应该是100%
        assert_eq!(metrics.success_rate(), 1.0);
        
        // 添加一些请求
        metrics.record_success();
        metrics.record_success();
        metrics.record_failure();
        
        // 成功率应该是2/3
        assert!((metrics.success_rate() - 2.0/3.0).abs() < 0.001);
    }

    #[test]
    fn test_server_metrics_overload_detection() {
        let mut metrics = ServerMetrics::new();
        
        // 正常情况下不应该过载
        assert!(!metrics.is_overloaded());
        
        // 连接数过多应该检测为过载
        metrics.active_connections = 950; // 95%
        assert!(metrics.is_overloaded());
        
        // CPU过高应该检测为过载
        metrics.active_connections = 100;
        metrics.cpu_usage = 95.0;
        assert!(metrics.is_overloaded());
    }

    #[test]
    fn test_response_time_update() {
        let mut metrics = ServerMetrics::new();
        
        // 更新响应时间
        metrics.update_response_time(Duration::from_millis(100));
        assert!(metrics.avg_response_time > 0.0);
        
        // 再次更新，应该使用移动平均
        let first_avg = metrics.avg_response_time;
        metrics.update_response_time(Duration::from_millis(200));
        assert!(metrics.avg_response_time > first_avg);
        assert!(metrics.avg_response_time < 200.0); // 应该小于最新值，因为使用了移动平均
    }
}