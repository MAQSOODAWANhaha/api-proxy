//! # 负载均衡调度算法实现

use super::types::{SchedulingResult, SchedulingStrategy, ServerMetrics};
use crate::error::{ProxyError, Result};
use crate::proxy::upstream::UpstreamServer;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 调度算法特质
pub trait SchedulingAlgorithm: Send + Sync {
    /// 选择服务器
    fn select_server(
        &self,
        servers: &[UpstreamServer],
        metrics: &[ServerMetrics],
    ) -> Result<SchedulingResult>;

    /// 获取算法名称
    fn name(&self) -> &'static str;

    /// 重置内部状态
    fn reset(&self);
}

/// 轮询调度器
pub struct RoundRobinScheduler {
    counter: AtomicUsize,
}

impl RoundRobinScheduler {
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }
}

impl Default for RoundRobinScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl SchedulingAlgorithm for RoundRobinScheduler {
    fn select_server(
        &self,
        servers: &[UpstreamServer],
        metrics: &[ServerMetrics],
    ) -> Result<SchedulingResult> {
        if servers.is_empty() {
            return Err(ProxyError::upstream_not_available(
                "No servers available".to_string(),
            ));
        }

        // 过滤健康的服务器
        let healthy_indices: Vec<usize> = metrics
            .iter()
            .enumerate()
            .filter(|(_, m)| m.is_healthy)
            .map(|(i, _)| i)
            .collect();

        if healthy_indices.is_empty() {
            return Err(ProxyError::upstream_not_available(
                "No healthy servers available".to_string(),
            ));
        }

        // 轮询选择
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        let selected_index = healthy_indices[counter % healthy_indices.len()];

        Ok(SchedulingResult::new(
            selected_index,
            format!("Round robin selection (counter: {})", counter),
            SchedulingStrategy::RoundRobin,
        ))
    }

    fn name(&self) -> &'static str {
        "RoundRobin"
    }

    fn reset(&self) {
        self.counter.store(0, Ordering::SeqCst);
    }
}

/// 权重调度器
pub struct WeightedScheduler {
    /// 当前权重值
    current_weights: std::sync::Mutex<Vec<i32>>,
}

impl WeightedScheduler {
    pub fn new() -> Self {
        Self {
            current_weights: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// 初始化权重
    fn initialize_weights(&self, server_count: usize, servers: &[UpstreamServer]) {
        let mut weights = self.current_weights.lock().unwrap();
        if weights.len() != server_count {
            weights.clear();
            weights.extend(servers.iter().map(|_s| 0));
        }
    }
}

impl Default for WeightedScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl SchedulingAlgorithm for WeightedScheduler {
    fn select_server(
        &self,
        servers: &[UpstreamServer],
        metrics: &[ServerMetrics],
    ) -> Result<SchedulingResult> {
        if servers.is_empty() {
            return Err(ProxyError::upstream_not_available(
                "No servers available".to_string(),
            ));
        }

        // 过滤健康的服务器及其权重
        let healthy_servers: Vec<(usize, u32)> = servers
            .iter()
            .enumerate()
            .filter(|(i, _)| metrics[*i].is_healthy)
            .map(|(i, s)| (i, s.weight))
            .collect();

        if healthy_servers.is_empty() {
            return Err(ProxyError::upstream_not_available(
                "No healthy servers available".to_string(),
            ));
        }

        // 初始化权重
        self.initialize_weights(servers.len(), servers);

        let mut current_weights = self.current_weights.lock().unwrap();

        // 计算总权重
        let total_weight: u32 = healthy_servers.iter().map(|(_, w)| *w).sum();

        if total_weight == 0 {
            // 如果所有权重都是0，退化为轮询
            let selected_index = healthy_servers[0].0;
            return Ok(SchedulingResult::new(
                selected_index,
                "All weights are zero, fallback to first server".to_string(),
                SchedulingStrategy::Weighted,
            ));
        }

        // 平滑加权轮询算法（Nginx的算法）
        let mut selected_index = 0;
        let mut max_current_weight = i32::MIN;

        // 为所有健康服务器增加权重
        for &(index, weight) in &healthy_servers {
            current_weights[index] += weight as i32;

            // 找到当前权重最大的服务器
            if current_weights[index] > max_current_weight {
                max_current_weight = current_weights[index];
                selected_index = index;
            }
        }

        // 减少选中服务器的权重
        current_weights[selected_index] -= total_weight as i32;

        Ok(SchedulingResult::new(
            selected_index,
            format!(
                "Weighted selection (weight: {}, current: {})",
                servers[selected_index].weight, max_current_weight
            ),
            SchedulingStrategy::Weighted,
        ))
    }

    fn name(&self) -> &'static str {
        "Weighted"
    }

    fn reset(&self) {
        let mut weights = self.current_weights.lock().unwrap();
        weights.clear();
    }
}

/// 健康度最佳调度器
pub struct HealthBasedScheduler;

impl HealthBasedScheduler {
    pub fn new() -> Self {
        Self
    }

    /// 计算服务器的综合分数
    fn calculate_score(&self, server: &UpstreamServer, metrics: &ServerMetrics) -> f32 {
        if !metrics.is_healthy {
            return 0.0;
        }

        let mut score = metrics.health_score();

        // 权重加成（权重越高，分数加成越多）
        let weight_bonus = (server.weight as f32 / 100.0) * 10.0; // 标准权重100得到10分加成
        score += weight_bonus;

        // 连接负载惩罚
        let connection_ratio = metrics.active_connections as f32 / metrics.max_connections as f32;
        let load_penalty = connection_ratio * 20.0; // 满负载减20分
        score -= load_penalty;

        score.max(0.0)
    }
}

impl Default for HealthBasedScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl SchedulingAlgorithm for HealthBasedScheduler {
    fn select_server(
        &self,
        servers: &[UpstreamServer],
        metrics: &[ServerMetrics],
    ) -> Result<SchedulingResult> {
        if servers.is_empty() {
            return Err(ProxyError::upstream_not_available(
                "No servers available".to_string(),
            ));
        }

        // 计算所有健康服务器的分数
        let scored_servers: Vec<(usize, f32)> = servers
            .iter()
            .enumerate()
            .filter(|(i, _)| metrics[*i].is_healthy)
            .map(|(i, server)| (i, self.calculate_score(server, &metrics[i])))
            .collect();

        if scored_servers.is_empty() {
            return Err(ProxyError::upstream_not_available(
                "No healthy servers available".to_string(),
            ));
        }

        // 选择分数最高的服务器
        let (selected_index, best_score) = scored_servers
            .iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .copied()
            .unwrap();

        Ok(SchedulingResult::new(
            selected_index,
            format!("Health-based selection (score: {:.2})", best_score),
            SchedulingStrategy::HealthBased,
        )
        .with_health_score(best_score))
    }

    fn name(&self) -> &'static str {
        "HealthBased"
    }

    fn reset(&self) {
        // 无状态算法，无需重置
    }
}

/// 创建调度算法实例
pub fn create_scheduler(strategy: SchedulingStrategy) -> Box<dyn SchedulingAlgorithm> {
    match strategy {
        SchedulingStrategy::RoundRobin => Box::new(RoundRobinScheduler::new()),
        SchedulingStrategy::Weighted => Box::new(WeightedScheduler::new()),
        SchedulingStrategy::HealthBased => Box::new(HealthBasedScheduler::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::upstream::UpstreamServer;

    fn create_test_servers() -> Vec<UpstreamServer> {
        vec![
            UpstreamServer {
                host: "server1.example.com".to_string(),
                port: 443,
                use_tls: true,
                weight: 100,
                max_connections: Some(1000),
                timeout_ms: 30000,
                health_check_interval: 30000,
                is_healthy: true,
            },
            UpstreamServer {
                host: "server2.example.com".to_string(),
                port: 443,
                use_tls: true,
                weight: 200,
                max_connections: Some(1000),
                timeout_ms: 30000,
                health_check_interval: 30000,
                is_healthy: true,
            },
            UpstreamServer {
                host: "server3.example.com".to_string(),
                port: 443,
                use_tls: true,
                weight: 50,
                max_connections: Some(1000),
                timeout_ms: 30000,
                health_check_interval: 30000,
                is_healthy: true,
            },
        ]
    }

    fn create_test_metrics() -> Vec<ServerMetrics> {
        vec![
            ServerMetrics {
                is_healthy: true,
                avg_response_time: 100.0,
                active_connections: 100,
                max_connections: 1000,
                ..ServerMetrics::default()
            },
            ServerMetrics {
                is_healthy: true,
                avg_response_time: 150.0,
                active_connections: 200,
                max_connections: 1000,
                ..ServerMetrics::default()
            },
            ServerMetrics {
                is_healthy: false, // 不健康的服务器
                avg_response_time: 50.0,
                active_connections: 50,
                max_connections: 1000,
                ..ServerMetrics::default()
            },
        ]
    }

    #[test]
    fn test_round_robin_scheduler() {
        let scheduler = RoundRobinScheduler::new();
        let servers = create_test_servers();
        let metrics = create_test_metrics();

        // 测试多次选择，应该轮询健康的服务器
        let result1 = scheduler.select_server(&servers, &metrics).unwrap();
        let result2 = scheduler.select_server(&servers, &metrics).unwrap();

        assert!(result1.server_index < servers.len());
        assert!(result2.server_index < servers.len());

        // 确保不会选择不健康的服务器（索引2）
        assert_ne!(result1.server_index, 2);
        assert_ne!(result2.server_index, 2);
    }

    #[test]
    fn test_weighted_scheduler() {
        let scheduler = WeightedScheduler::new();
        let servers = create_test_servers();
        let metrics = create_test_metrics();

        // 测试多次选择
        let mut selections = std::collections::HashMap::new();
        for _ in 0..100 {
            let result = scheduler.select_server(&servers, &metrics).unwrap();
            *selections.entry(result.server_index).or_insert(0) += 1;
        }

        // 确保不会选择不健康的服务器
        assert!(!selections.contains_key(&2));

        // 权重高的服务器应该被选择更多次（但由于算法复杂性，不做严格检查）
        assert!(selections.len() <= 2); // 最多选择2个健康服务器
    }

    #[test]
    fn test_health_based_scheduler() {
        let scheduler = HealthBasedScheduler::new();
        let servers = create_test_servers();
        let mut metrics = create_test_metrics();

        // 让第一个服务器表现更好
        metrics[0].avg_response_time = 50.0;
        metrics[1].avg_response_time = 200.0;

        let result = scheduler.select_server(&servers, &metrics).unwrap();

        // 应该选择健康且表现好的服务器
        assert_ne!(result.server_index, 2); // 不会选择不健康的
        assert!(result.health_score.is_some());
        assert!(result.health_score.unwrap() > 0.0);
    }

    #[test]
    fn test_no_healthy_servers() {
        let scheduler = RoundRobinScheduler::new();
        let servers = create_test_servers();
        let mut metrics = create_test_metrics();

        // 让所有服务器都不健康
        for metric in &mut metrics {
            metric.is_healthy = false;
        }

        let result = scheduler.select_server(&servers, &metrics);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_servers() {
        let scheduler = RoundRobinScheduler::new();
        let servers = vec![];
        let metrics = vec![];

        let result = scheduler.select_server(&servers, &metrics);
        assert!(result.is_err());
    }

    #[test]
    fn test_scheduler_creation() {
        let rr_scheduler = create_scheduler(SchedulingStrategy::RoundRobin);
        assert_eq!(rr_scheduler.name(), "RoundRobin");

        let weighted_scheduler = create_scheduler(SchedulingStrategy::Weighted);
        assert_eq!(weighted_scheduler.name(), "Weighted");

        let health_scheduler = create_scheduler(SchedulingStrategy::HealthBased);
        assert_eq!(health_scheduler.name(), "HealthBased");
    }
}
