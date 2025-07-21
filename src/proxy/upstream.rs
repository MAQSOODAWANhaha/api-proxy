//! # 上游服务管理
//!
//! 管理 AI 服务提供商的上游连接

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use pingora_core::upstreams::peer::HttpPeer;
use crate::config::AppConfig;
use crate::error::{ProxyError, Result};
use crate::scheduler::{LoadBalancer, SchedulingStrategy};
use crate::scheduler::balancer::LoadBalancerConfig;

/// 上游服务类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UpstreamType {
    OpenAI,
    Anthropic,
    GoogleGemini,
    Custom(String),
}

impl UpstreamType {
    /// 从路径判断上游类型
    pub fn from_path(path: &str) -> Option<Self> {
        if path.starts_with("/v1/") {
            // 标准 OpenAI API 路径
            Some(UpstreamType::OpenAI)
        } else if path.starts_with("/openai/") {
            Some(UpstreamType::OpenAI)
        } else if path.starts_with("/anthropic/") {
            Some(UpstreamType::Anthropic)
        } else if path.starts_with("/gemini/") || path.starts_with("/google/") {
            Some(UpstreamType::GoogleGemini)
        } else {
            None
        }
    }

    /// 获取默认的上游地址
    pub fn default_upstream(&self) -> &'static str {
        match self {
            UpstreamType::OpenAI => "api.openai.com:443",
            UpstreamType::Anthropic => "api.anthropic.com:443", 
            UpstreamType::GoogleGemini => "generativelanguage.googleapis.com:443",
            UpstreamType::Custom(_) => "localhost:8080",
        }
    }

    /// 判断是否使用 TLS
    pub fn use_tls(&self) -> bool {
        match self {
            UpstreamType::OpenAI | UpstreamType::Anthropic | UpstreamType::GoogleGemini => true,
            UpstreamType::Custom(_) => false,
        }
    }
}

/// 上游服务器信息
#[derive(Debug, Clone)]
pub struct UpstreamServer {
    pub host: String,
    pub port: u16,
    pub use_tls: bool,
    pub weight: u32,
    pub max_connections: Option<u32>,
    pub timeout_ms: u64,
    pub health_check_interval: u64,
    pub is_healthy: bool,
}

impl UpstreamServer {
    /// 创建新的上游服务器
    pub fn new(host: String, port: u16, use_tls: bool) -> Self {
        Self {
            host,
            port,
            use_tls,
            weight: 100,
            max_connections: None,
            timeout_ms: 30000,
            health_check_interval: 30000,
            is_healthy: true,
        }
    }

    /// 获取服务器地址
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// 创建 HttpPeer
    pub fn create_peer(&self, sni: String) -> HttpPeer {
        HttpPeer::new(&self.address(), self.use_tls, sni)
    }
}

/// 上游管理器
pub struct UpstreamManager {
    config: Arc<AppConfig>,
    load_balancer: LoadBalancer,
}

impl UpstreamManager {
    /// 创建新的上游管理器
    pub fn new(config: Arc<AppConfig>) -> Self {
        let lb_config = LoadBalancerConfig {
            default_strategy: SchedulingStrategy::RoundRobin,
            health_check_interval: Duration::from_secs(30),
            auto_failover: true,
            ..Default::default()
        };
        
        let mut manager = Self {
            config,
            load_balancer: LoadBalancer::new(lb_config),
        };
        
        manager.initialize_default_upstreams();
        manager
    }

    /// 使用自定义负载均衡配置创建管理器
    pub fn with_load_balancer_config(config: Arc<AppConfig>, lb_config: LoadBalancerConfig) -> Self {
        let mut manager = Self {
            config,
            load_balancer: LoadBalancer::new(lb_config),
        };
        
        manager.initialize_default_upstreams();
        manager
    }

    /// 初始化默认上游服务器
    fn initialize_default_upstreams(&mut self) {
        // OpenAI 上游
        let openai_server = UpstreamServer::new("api.openai.com".to_string(), 443, true);
        self.load_balancer.add_server(UpstreamType::OpenAI, openai_server).unwrap();

        // Anthropic 上游
        let anthropic_server = UpstreamServer::new("api.anthropic.com".to_string(), 443, true);
        self.load_balancer.add_server(UpstreamType::Anthropic, anthropic_server).unwrap();

        // Google Gemini 上游
        let gemini_server = UpstreamServer::new("generativelanguage.googleapis.com".to_string(), 443, true);
        self.load_balancer.add_server(UpstreamType::GoogleGemini, gemini_server).unwrap();

        tracing::info!("Initialized default upstream servers with load balancer");
    }

    /// 获取指定类型的上游服务器（已弃用，使用select_upstream代替）
    #[deprecated(note = "Use select_upstream for load balancing")]
    pub fn get_upstream(&self, upstream_type: &UpstreamType) -> Result<UpstreamServer> {
        let (server, _) = self.load_balancer.select_server(upstream_type)?;
        Ok(server)
    }

    /// 使用负载均衡选择上游服务器
    pub fn select_upstream(&self, upstream_type: &UpstreamType) -> Result<UpstreamServer> {
        let (server, result) = self.load_balancer.select_server(upstream_type)?;
        tracing::debug!("Selected upstream: {} using strategy: {:?}, reason: {}", 
                       server.address(), result.strategy, result.reason);
        Ok(server)
    }

    /// 根据请求路径选择上游服务器
    pub fn select_upstream_for_path(&self, path: &str) -> Result<UpstreamServer> {
        let upstream_type = UpstreamType::from_path(path)
            .ok_or_else(|| ProxyError::upstream_not_found(format!("Cannot determine upstream for path: {}", path)))?;
        
        self.select_upstream(&upstream_type)
    }

    /// 创建用于指定路径的 HttpPeer
    pub fn create_peer_for_path(&self, path: &str) -> Result<HttpPeer> {
        let upstream = self.select_upstream_for_path(path)?;
        let sni = upstream.host.clone();
        
        Ok(upstream.create_peer(sni))
    }

    /// 添加自定义上游服务器
    pub fn add_upstream(&self, upstream_type: UpstreamType, server: UpstreamServer) -> Result<()> {
        self.load_balancer.add_server(upstream_type, server)
    }

    /// 移除上游服务器
    pub fn remove_upstream(&self, upstream_type: &UpstreamType, server_address: &str) -> Result<()> {
        self.load_balancer.remove_server(upstream_type, server_address)
    }

    /// 更新服务器健康状态
    pub fn update_server_health(&self, upstream_type: &UpstreamType, server_address: &str, is_healthy: bool) {
        self.load_balancer.mark_server_healthy(upstream_type, server_address, is_healthy);
    }

    /// 记录请求成功
    pub fn record_success(&self, upstream_type: &UpstreamType, server_address: &str, response_time: Duration) {
        self.load_balancer.record_success(upstream_type, server_address, response_time);
    }

    /// 记录请求失败
    pub fn record_failure(&self, upstream_type: &UpstreamType, server_address: &str) {
        self.load_balancer.record_failure(upstream_type, server_address);
    }

    /// 设置负载均衡策略
    pub fn set_load_balancing_strategy(&self, upstream_type: UpstreamType, strategy: SchedulingStrategy) {
        self.load_balancer.set_strategy(upstream_type, strategy);
    }

    /// 获取所有上游服务器状态
    pub fn get_all_upstreams(&self) -> HashMap<UpstreamType, Vec<(UpstreamServer, crate::scheduler::ServerMetrics)>> {
        self.load_balancer.get_all_servers()
    }

    /// 获取健康的上游服务器数量
    pub fn healthy_server_count(&self, upstream_type: &UpstreamType) -> usize {
        self.load_balancer.healthy_server_count(upstream_type)
    }

    /// 获取负载均衡器引用
    pub fn load_balancer(&self) -> &LoadBalancer {
        &self.load_balancer
    }
}

impl std::fmt::Debug for UpstreamManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let all_servers = self.load_balancer.get_all_servers();
        let total_servers: usize = all_servers.values().map(|v| v.len()).sum();
        
        f.debug_struct("UpstreamManager")
            .field("config", &"AppConfig")
            .field("load_balancer", &self.load_balancer)
            .field("server_types", &all_servers.keys().collect::<Vec<_>>())
            .field("total_servers", &total_servers)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::helpers::init_test_env;
    use crate::testing::fixtures::TestConfig;

    #[test]
    fn test_upstream_type_from_path() {
        assert_eq!(UpstreamType::from_path("/v1/chat/completions"), Some(UpstreamType::OpenAI));
        assert_eq!(UpstreamType::from_path("/openai/v1/completions"), Some(UpstreamType::OpenAI));
        assert_eq!(UpstreamType::from_path("/anthropic/v1/messages"), Some(UpstreamType::Anthropic));
        assert_eq!(UpstreamType::from_path("/gemini/v1/chat"), Some(UpstreamType::GoogleGemini));
        assert_eq!(UpstreamType::from_path("/unknown/path"), None);
    }

    #[test]
    fn test_upstream_server_creation() {
        let server = UpstreamServer::new("api.openai.com".to_string(), 443, true);
        
        assert_eq!(server.host, "api.openai.com");
        assert_eq!(server.port, 443);
        assert!(server.use_tls);
        assert!(server.is_healthy);
        assert_eq!(server.address(), "api.openai.com:443");
    }

    #[test]
    fn test_upstream_manager() {
        init_test_env();
        
        let config = Arc::new(TestConfig::app_config());
        let manager = UpstreamManager::new(config);
        
        // 检查默认上游服务器
        assert!(manager.select_upstream(&UpstreamType::OpenAI).is_ok());
        assert!(manager.select_upstream(&UpstreamType::Anthropic).is_ok());
        assert!(manager.select_upstream(&UpstreamType::GoogleGemini).is_ok());
        
        // 测试路径选择
        assert!(manager.select_upstream_for_path("/v1/chat/completions").is_ok());
        assert!(manager.select_upstream_for_path("/anthropic/v1/messages").is_ok());
        
        // 测试未知路径
        assert!(manager.select_upstream_for_path("/unknown/path").is_err());
    }

    #[test]
    fn test_upstream_manager_peer_creation() {
        init_test_env();
        
        let config = Arc::new(TestConfig::app_config());
        let manager = UpstreamManager::new(config);
        
        let peer = manager.create_peer_for_path("/v1/chat/completions");
        assert!(peer.is_ok());
    }

    #[test]
    fn test_upstream_health_management() {
        init_test_env();
        
        let config = Arc::new(TestConfig::app_config());
        let manager = UpstreamManager::new(config);
        
        // 初始状态应该是健康的
        assert_eq!(manager.healthy_server_count(&UpstreamType::OpenAI), 1);
        
        // 更新健康状态
        manager.update_server_health(&UpstreamType::OpenAI, "api.openai.com:443", false);
        assert_eq!(manager.healthy_server_count(&UpstreamType::OpenAI), 0);
        
        // 恢复健康状态
        manager.update_server_health(&UpstreamType::OpenAI, "api.openai.com:443", true);
        assert_eq!(manager.healthy_server_count(&UpstreamType::OpenAI), 1);
    }

    #[test]
    fn test_load_balancing_strategies() {
        init_test_env();
        
        let config = Arc::new(TestConfig::app_config());
        let manager = UpstreamManager::new(config);
        
        // 添加多个服务器进行负载均衡测试
        let server1 = UpstreamServer::new("server1.example.com".to_string(), 443, true);
        let server2 = UpstreamServer::new("server2.example.com".to_string(), 443, true);
        
        manager.add_upstream(UpstreamType::OpenAI, server1).unwrap();
        manager.add_upstream(UpstreamType::OpenAI, server2).unwrap();
        
        // 测试轮询策略
        manager.set_load_balancing_strategy(UpstreamType::OpenAI, SchedulingStrategy::RoundRobin);
        let result1 = manager.select_upstream(&UpstreamType::OpenAI).unwrap();
        let result2 = manager.select_upstream(&UpstreamType::OpenAI).unwrap();
        
        // 应该选择不同的服务器（或相同的，取决于轮询状态）
        assert!(result1.host.contains("example.com"));
        assert!(result2.host.contains("example.com"));
        
        // 测试权重策略
        manager.set_load_balancing_strategy(UpstreamType::OpenAI, SchedulingStrategy::Weighted);
        let result3 = manager.select_upstream(&UpstreamType::OpenAI).unwrap();
        assert!(result3.host.contains("example.com"));
        
        // 测试健康度最佳策略
        manager.set_load_balancing_strategy(UpstreamType::OpenAI, SchedulingStrategy::HealthBased);
        let result4 = manager.select_upstream(&UpstreamType::OpenAI).unwrap();
        assert!(result4.host.contains("example.com"));
    }

    #[test]
    fn test_request_metrics_recording() {
        init_test_env();
        
        let config = Arc::new(TestConfig::app_config());
        let manager = UpstreamManager::new(config);
        
        let server_address = "api.openai.com:443";
        
        // 记录成功请求
        manager.record_success(&UpstreamType::OpenAI, server_address, Duration::from_millis(100));
        
        // 记录失败请求
        manager.record_failure(&UpstreamType::OpenAI, server_address);
        
        // 验证指标已更新
        let all_servers = manager.get_all_upstreams();
        assert!(all_servers.contains_key(&UpstreamType::OpenAI));
        
        let openai_servers = &all_servers[&UpstreamType::OpenAI];
        assert!(!openai_servers.is_empty());
        
        let (_, metrics) = &openai_servers[0];
        assert_eq!(metrics.success_requests, 1);
        assert_eq!(metrics.failed_requests, 1);
        assert!(metrics.avg_response_time > 0.0);
    }
}