//! # 负载均衡器核心实现

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;
use crate::error::{ProxyError, Result};
use crate::proxy::upstream::{UpstreamServer, UpstreamType};
use super::algorithms::{SchedulingAlgorithm, create_scheduler};
use super::types::{ServerMetrics, SchedulingResult, SchedulingStrategy};

/// 负载均衡器配置
#[derive(Debug, Clone)]
pub struct LoadBalancerConfig {
    /// 默认调度策略
    pub default_strategy: SchedulingStrategy,
    /// 健康检查间隔
    pub health_check_interval: Duration,
    /// 指标收集窗口大小
    pub metrics_window_size: usize,
    /// 服务器移除阈值（连续失败次数）
    pub failure_threshold: u32,
    /// 服务器恢复阈值（连续成功次数）
    pub recovery_threshold: u32,
    /// 自动故障转移
    pub auto_failover: bool,
}

impl Default for LoadBalancerConfig {
    fn default() -> Self {
        Self {
            default_strategy: SchedulingStrategy::RoundRobin,
            health_check_interval: Duration::from_secs(30),
            metrics_window_size: 100,
            failure_threshold: 3,
            recovery_threshold: 2,
            auto_failover: true,
        }
    }
}

/// 负载均衡器
pub struct LoadBalancer {
    /// 配置
    config: LoadBalancerConfig,
    /// 按类型分组的服务器
    servers: RwLock<HashMap<UpstreamType, Vec<UpstreamServer>>>,
    /// 服务器指标
    metrics: RwLock<HashMap<String, ServerMetrics>>,
    /// 调度算法
    schedulers: RwLock<HashMap<UpstreamType, Box<dyn SchedulingAlgorithm>>>,
    /// 故障计数器
    failure_counts: RwLock<HashMap<String, u32>>,
    /// 成功计数器
    success_counts: RwLock<HashMap<String, u32>>,
}

impl LoadBalancer {
    /// 创建新的负载均衡器
    pub fn new(config: LoadBalancerConfig) -> Self {
        Self {
            config,
            servers: RwLock::new(HashMap::new()),
            metrics: RwLock::new(HashMap::new()),
            schedulers: RwLock::new(HashMap::new()),
            failure_counts: RwLock::new(HashMap::new()),
            success_counts: RwLock::new(HashMap::new()),
        }
    }

    /// 使用默认配置创建负载均衡器
    pub fn with_default_config() -> Self {
        Self::new(LoadBalancerConfig::default())
    }

    /// 添加服务器
    pub fn add_server(&self, upstream_type: UpstreamType, server: UpstreamServer) -> Result<()> {
        let server_key = self.server_key(&upstream_type, &server);
        
        {
            let mut servers = self.servers.write().unwrap();
            servers.entry(upstream_type.clone()).or_insert_with(Vec::new).push(server);
        }

        {
            let mut metrics = self.metrics.write().unwrap();
            metrics.insert(server_key.clone(), ServerMetrics::new());
        }

        {
            let mut schedulers = self.schedulers.write().unwrap();
            if !schedulers.contains_key(&upstream_type) {
                schedulers.insert(upstream_type, create_scheduler(self.config.default_strategy));
            }
        }

        tracing::info!("Added server: {}", server_key);
        Ok(())
    }

    /// 移除服务器
    pub fn remove_server(&self, upstream_type: &UpstreamType, server_address: &str) -> Result<()> {
        let server_key = format!("{:?}:{}", upstream_type, server_address);

        {
            let mut servers = self.servers.write().unwrap();
            if let Some(server_list) = servers.get_mut(upstream_type) {
                server_list.retain(|s| s.address() != server_address);
                if server_list.is_empty() {
                    servers.remove(upstream_type);
                }
            }
        }

        {
            let mut metrics = self.metrics.write().unwrap();
            metrics.remove(&server_key);
        }

        {
            let mut failure_counts = self.failure_counts.write().unwrap();
            failure_counts.remove(&server_key);
        }

        {
            let mut success_counts = self.success_counts.write().unwrap();
            success_counts.remove(&server_key);
        }

        tracing::info!("Removed server: {}", server_key);
        Ok(())
    }

    /// 选择服务器
    pub fn select_server(&self, upstream_type: &UpstreamType) -> Result<(UpstreamServer, SchedulingResult)> {
        let servers = {
            let servers_guard = self.servers.read().unwrap();
            servers_guard.get(upstream_type)
                .ok_or_else(|| ProxyError::upstream_not_found(format!("No servers for type: {:?}", upstream_type)))?
                .clone()
        };

        if servers.is_empty() {
            return Err(ProxyError::upstream_not_available(format!("No servers available for type: {:?}", upstream_type)));
        }

        // 获取服务器指标
        let metrics: Vec<ServerMetrics> = {
            let metrics_guard = self.metrics.read().unwrap();
            servers.iter()
                .map(|server| {
                    let key = self.server_key(upstream_type, server);
                    metrics_guard.get(&key).cloned().unwrap_or_default()
                })
                .collect()
        };

        // 获取调度器
        let result = {
            let schedulers_guard = self.schedulers.read().unwrap();
            let scheduler = schedulers_guard.get(upstream_type)
                .ok_or_else(|| ProxyError::upstream_not_found(format!("No scheduler for type: {:?}", upstream_type)))?;
            
            scheduler.select_server(&servers, &metrics)?
        };

        let selected_server = servers[result.server_index].clone();
        
        tracing::debug!("Selected server: {} for type: {:?}, reason: {}", 
                       selected_server.address(), upstream_type, result.reason);

        Ok((selected_server, result))
    }

    /// 设置调度策略
    pub fn set_strategy(&self, upstream_type: UpstreamType, strategy: SchedulingStrategy) {
        let mut schedulers = self.schedulers.write().unwrap();
        schedulers.insert(upstream_type, create_scheduler(strategy));
    }

    /// 记录请求成功
    pub fn record_success(&self, upstream_type: &UpstreamType, server_address: &str, response_time: Duration) {
        let server_key = format!("{:?}:{}", upstream_type, server_address);
        
        {
            let mut metrics = self.metrics.write().unwrap();
            if let Some(metric) = metrics.get_mut(&server_key) {
                metric.record_success();
                metric.update_response_time(response_time);
            }
        }

        {
            let mut success_counts = self.success_counts.write().unwrap();
            let count = success_counts.entry(server_key.clone()).or_insert(0);
            *count += 1;

            // 检查是否需要恢复健康状态
            if *count >= self.config.recovery_threshold {
                self.mark_server_healthy(upstream_type, server_address, true);
                *count = 0; // 重置计数器
            }
        }

        // 重置失败计数器
        {
            let mut failure_counts = self.failure_counts.write().unwrap();
            failure_counts.insert(server_key, 0);
        }
    }

    /// 记录请求失败
    pub fn record_failure(&self, upstream_type: &UpstreamType, server_address: &str) {
        let server_key = format!("{:?}:{}", upstream_type, server_address);
        
        {
            let mut metrics = self.metrics.write().unwrap();
            if let Some(metric) = metrics.get_mut(&server_key) {
                metric.record_failure();
            }
        }

        {
            let mut failure_counts = self.failure_counts.write().unwrap();
            let count = failure_counts.entry(server_key.clone()).or_insert(0);
            *count += 1;

            // 检查是否需要标记为不健康
            if *count >= self.config.failure_threshold && self.config.auto_failover {
                self.mark_server_healthy(upstream_type, server_address, false);
                tracing::warn!("Marked server {} as unhealthy after {} failures", server_address, count);
            }
        }

        // 重置成功计数器
        {
            let mut success_counts = self.success_counts.write().unwrap();
            success_counts.insert(server_key, 0);
        }
    }

    /// 标记服务器健康状态
    pub fn mark_server_healthy(&self, upstream_type: &UpstreamType, server_address: &str, is_healthy: bool) {
        let server_key = format!("{:?}:{}", upstream_type, server_address);
        
        {
            let mut metrics = self.metrics.write().unwrap();
            if let Some(metric) = metrics.get_mut(&server_key) {
                metric.update_health(is_healthy);
            }
        }

        {
            let mut servers = self.servers.write().unwrap();
            if let Some(server_list) = servers.get_mut(upstream_type) {
                for server in server_list {
                    if server.address() == server_address {
                        server.is_healthy = is_healthy;
                        break;
                    }
                }
            }
        }

        tracing::info!("Updated server {} health status: {}", server_address, is_healthy);
    }

    /// 获取服务器指标
    pub fn get_server_metrics(&self, upstream_type: &UpstreamType, server_address: &str) -> Option<ServerMetrics> {
        let server_key = format!("{:?}:{}", upstream_type, server_address);
        let metrics = self.metrics.read().unwrap();
        metrics.get(&server_key).cloned()
    }

    /// 获取所有服务器状态
    pub fn get_all_servers(&self) -> HashMap<UpstreamType, Vec<(UpstreamServer, ServerMetrics)>> {
        let servers_guard = self.servers.read().unwrap();
        let metrics_guard = self.metrics.read().unwrap();
        
        let mut result = HashMap::new();
        
        for (upstream_type, servers) in servers_guard.iter() {
            let server_metrics: Vec<(UpstreamServer, ServerMetrics)> = servers
                .iter()
                .map(|server| {
                    let key = self.server_key(upstream_type, server);
                    let metrics = metrics_guard.get(&key).cloned().unwrap_or_default();
                    (server.clone(), metrics)
                })
                .collect();
            
            result.insert(upstream_type.clone(), server_metrics);
        }
        
        result
    }

    /// 获取健康服务器数量
    pub fn healthy_server_count(&self, upstream_type: &UpstreamType) -> usize {
        let servers_guard = self.servers.read().unwrap();
        let metrics_guard = self.metrics.read().unwrap();
        
        if let Some(servers) = servers_guard.get(upstream_type) {
            servers.iter()
                .filter(|server| {
                    let key = self.server_key(upstream_type, server);
                    metrics_guard.get(&key)
                        .map(|m| m.is_healthy)
                        .unwrap_or(false)
                })
                .count()
        } else {
            0
        }
    }

    /// 重置所有调度器状态
    pub fn reset_schedulers(&self) {
        let schedulers = self.schedulers.read().unwrap();
        for scheduler in schedulers.values() {
            scheduler.reset();
        }
    }

    /// 生成服务器键
    fn server_key(&self, upstream_type: &UpstreamType, server: &UpstreamServer) -> String {
        format!("{:?}:{}", upstream_type, server.address())
    }

    /// 清理过期指标
    pub fn cleanup_expired_metrics(&self) {
        let mut metrics = self.metrics.write().unwrap();
        let health_check_ttl = self.config.health_check_interval * 3; // 3倍健康检查间隔作为TTL
        
        metrics.retain(|key, metric| {
            if metric.is_health_check_expired(health_check_ttl) {
                tracing::debug!("Removing expired metrics for server: {}", key);
                false
            } else {
                true
            }
        });
    }
}

impl Clone for LoadBalancer {
    fn clone(&self) -> Self {
        let config = self.config.clone();
        let new_balancer = LoadBalancer::new(config.clone());
        
        // 复制服务器配置
        {
            let servers = self.servers.read().unwrap();
            let metrics = self.metrics.read().unwrap();
            let failure_counts = self.failure_counts.read().unwrap();
            let success_counts = self.success_counts.read().unwrap();
            
            *new_balancer.servers.write().unwrap() = servers.clone();
            *new_balancer.metrics.write().unwrap() = metrics.clone();
            *new_balancer.failure_counts.write().unwrap() = failure_counts.clone();
            *new_balancer.success_counts.write().unwrap() = success_counts.clone();
        }
        
        // 重建调度器
        {
            let servers = new_balancer.servers.read().unwrap();
            let mut schedulers = new_balancer.schedulers.write().unwrap();
            for upstream_type in servers.keys() {
                schedulers.insert(upstream_type.clone(), crate::scheduler::algorithms::create_scheduler(config.default_strategy));
            }
        }
        
        new_balancer
    }
}

impl std::fmt::Debug for LoadBalancer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let servers = self.servers.read().unwrap();
        let metrics = self.metrics.read().unwrap();
        
        f.debug_struct("LoadBalancer")
            .field("config", &self.config)
            .field("server_types", &servers.keys().collect::<Vec<_>>())
            .field("total_servers", &servers.values().map(|v| v.len()).sum::<usize>())
            .field("total_metrics", &metrics.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::upstream::UpstreamServer;
    use std::time::Duration;

    fn create_test_server(host: &str, port: u16, weight: u32) -> UpstreamServer {
        UpstreamServer {
            host: host.to_string(),
            port,
            use_tls: true,
            weight,
            max_connections: Some(1000),
            timeout_ms: 30000,
            health_check_interval: 30000,
            is_healthy: true,
        }
    }

    #[test]
    fn test_load_balancer_creation() {
        let balancer = LoadBalancer::with_default_config();
        assert!(balancer.get_all_servers().is_empty());
    }

    #[test]
    fn test_add_and_remove_server() {
        let balancer = LoadBalancer::with_default_config();
        let server = create_test_server("example.com", 443, 100);
        let upstream_type = UpstreamType::OpenAI;

        // 添加服务器
        balancer.add_server(upstream_type.clone(), server.clone()).unwrap();
        
        let all_servers = balancer.get_all_servers();
        assert_eq!(all_servers.len(), 1);
        assert!(all_servers.contains_key(&upstream_type));

        // 移除服务器
        balancer.remove_server(&upstream_type, &server.address()).unwrap();
        
        let all_servers = balancer.get_all_servers();
        assert!(all_servers.is_empty() || all_servers.get(&upstream_type).unwrap().is_empty());
    }

    #[test]
    fn test_server_selection() {
        let balancer = LoadBalancer::with_default_config();
        let upstream_type = UpstreamType::OpenAI;
        
        // 添加多个服务器
        let server1 = create_test_server("server1.example.com", 443, 100);
        let server2 = create_test_server("server2.example.com", 443, 200);
        
        balancer.add_server(upstream_type.clone(), server1).unwrap();
        balancer.add_server(upstream_type.clone(), server2).unwrap();

        // 选择服务器
        let (selected_server, result) = balancer.select_server(&upstream_type).unwrap();
        assert!(selected_server.host.contains("example.com"));
        assert_eq!(result.strategy, SchedulingStrategy::RoundRobin);
    }

    #[test]
    fn test_success_and_failure_recording() {
        let balancer = LoadBalancer::with_default_config();
        let upstream_type = UpstreamType::OpenAI;
        let server = create_test_server("example.com", 443, 100);
        let server_address = server.address();
        
        balancer.add_server(upstream_type.clone(), server).unwrap();

        // 记录成功
        balancer.record_success(&upstream_type, &server_address, Duration::from_millis(100));
        
        let metrics = balancer.get_server_metrics(&upstream_type, &server_address).unwrap();
        assert_eq!(metrics.success_requests, 1);
        assert!(metrics.avg_response_time > 0.0);

        // 记录失败
        balancer.record_failure(&upstream_type, &server_address);
        
        let metrics = balancer.get_server_metrics(&upstream_type, &server_address).unwrap();
        assert_eq!(metrics.failed_requests, 1);
    }

    #[test]
    fn test_health_status_management() {
        let balancer = LoadBalancer::with_default_config();
        let upstream_type = UpstreamType::OpenAI;
        let server = create_test_server("example.com", 443, 100);
        let server_address = server.address();
        
        balancer.add_server(upstream_type.clone(), server).unwrap();

        // 初始状态应该是健康的
        assert_eq!(balancer.healthy_server_count(&upstream_type), 1);

        // 标记为不健康
        balancer.mark_server_healthy(&upstream_type, &server_address, false);
        assert_eq!(balancer.healthy_server_count(&upstream_type), 0);

        // 恢复健康
        balancer.mark_server_healthy(&upstream_type, &server_address, true);
        assert_eq!(balancer.healthy_server_count(&upstream_type), 1);
    }

    #[test]
    fn test_strategy_switching() {
        let balancer = LoadBalancer::with_default_config();
        let upstream_type = UpstreamType::OpenAI;
        let server = create_test_server("example.com", 443, 100);
        
        balancer.add_server(upstream_type.clone(), server).unwrap();

        // 更换为权重调度策略
        balancer.set_strategy(upstream_type.clone(), SchedulingStrategy::Weighted);
        
        let (_, result) = balancer.select_server(&upstream_type).unwrap();
        assert_eq!(result.strategy, SchedulingStrategy::Weighted);
    }

    #[test]
    fn test_no_servers_error() {
        let balancer = LoadBalancer::with_default_config();
        let upstream_type = UpstreamType::OpenAI;
        
        let result = balancer.select_server(&upstream_type);
        assert!(result.is_err());
    }
}