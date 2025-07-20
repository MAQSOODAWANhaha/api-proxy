//! # 上游服务管理
//!
//! 管理 AI 服务提供商的上游连接

use std::collections::HashMap;
use std::sync::Arc;
use pingora_core::upstreams::peer::HttpPeer;
use crate::config::AppConfig;
use crate::error::{ProxyError, Result};

/// 上游服务类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    upstreams: HashMap<UpstreamType, Vec<UpstreamServer>>,
}

impl UpstreamManager {
    /// 创建新的上游管理器
    pub fn new(config: Arc<AppConfig>) -> Self {
        let mut manager = Self {
            config,
            upstreams: HashMap::new(),
        };
        
        manager.initialize_default_upstreams();
        manager
    }

    /// 初始化默认上游服务器
    fn initialize_default_upstreams(&mut self) {
        // OpenAI 上游
        let openai_servers = vec![
            UpstreamServer::new("api.openai.com".to_string(), 443, true),
        ];
        self.upstreams.insert(UpstreamType::OpenAI, openai_servers);

        // Anthropic 上游
        let anthropic_servers = vec![
            UpstreamServer::new("api.anthropic.com".to_string(), 443, true),
        ];
        self.upstreams.insert(UpstreamType::Anthropic, anthropic_servers);

        // Google Gemini 上游
        let gemini_servers = vec![
            UpstreamServer::new("generativelanguage.googleapis.com".to_string(), 443, true),
        ];
        self.upstreams.insert(UpstreamType::GoogleGemini, gemini_servers);

        tracing::info!("Initialized default upstream servers");
    }

    /// 获取指定类型的上游服务器
    pub fn get_upstream(&self, upstream_type: &UpstreamType) -> Result<&UpstreamServer> {
        let servers = self.upstreams.get(upstream_type)
            .ok_or_else(|| ProxyError::upstream_not_found(format!("No upstream servers for {:?}", upstream_type)))?;
        
        // 简单轮询选择（后续可以实现更复杂的负载均衡）
        let healthy_servers: Vec<_> = servers.iter().filter(|s| s.is_healthy).collect();
        
        if healthy_servers.is_empty() {
            return Err(ProxyError::upstream_not_available(format!("No healthy servers for {:?}", upstream_type)));
        }

        // 简单选择第一个健康的服务器
        Ok(healthy_servers[0])
    }

    /// 根据请求路径选择上游服务器
    pub fn select_upstream_for_path(&self, path: &str) -> Result<&UpstreamServer> {
        let upstream_type = UpstreamType::from_path(path)
            .ok_or_else(|| ProxyError::upstream_not_found(format!("Cannot determine upstream for path: {}", path)))?;
        
        self.get_upstream(&upstream_type)
    }

    /// 创建用于指定路径的 HttpPeer
    pub fn create_peer_for_path(&self, path: &str) -> Result<HttpPeer> {
        let upstream = self.select_upstream_for_path(path)?;
        let sni = upstream.host.clone();
        
        Ok(upstream.create_peer(sni))
    }

    /// 添加自定义上游服务器
    pub fn add_upstream(&mut self, upstream_type: UpstreamType, server: UpstreamServer) {
        self.upstreams.entry(upstream_type).or_insert_with(Vec::new).push(server);
    }

    /// 移除上游服务器
    pub fn remove_upstream(&mut self, upstream_type: &UpstreamType) -> Option<Vec<UpstreamServer>> {
        self.upstreams.remove(upstream_type)
    }

    /// 更新服务器健康状态
    pub fn update_server_health(&mut self, upstream_type: &UpstreamType, server_address: &str, is_healthy: bool) {
        if let Some(servers) = self.upstreams.get_mut(upstream_type) {
            for server in servers {
                if server.address() == server_address {
                    server.is_healthy = is_healthy;
                    tracing::info!("Updated health status for {}: {}", server_address, is_healthy);
                    break;
                }
            }
        }
    }

    /// 获取所有上游服务器状态
    pub fn get_all_upstreams(&self) -> &HashMap<UpstreamType, Vec<UpstreamServer>> {
        &self.upstreams
    }

    /// 获取健康的上游服务器数量
    pub fn healthy_server_count(&self, upstream_type: &UpstreamType) -> usize {
        self.upstreams.get(upstream_type)
            .map(|servers| servers.iter().filter(|s| s.is_healthy).count())
            .unwrap_or(0)
    }
}

impl std::fmt::Debug for UpstreamManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpstreamManager")
            .field("upstream_count", &self.upstreams.len())
            .field("upstreams", &self.upstreams.keys().collect::<Vec<_>>())
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
        assert!(manager.get_upstream(&UpstreamType::OpenAI).is_ok());
        assert!(manager.get_upstream(&UpstreamType::Anthropic).is_ok());
        assert!(manager.get_upstream(&UpstreamType::GoogleGemini).is_ok());
        
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
        let mut manager = UpstreamManager::new(config);
        
        // 初始状态应该是健康的
        assert_eq!(manager.healthy_server_count(&UpstreamType::OpenAI), 1);
        
        // 更新健康状态
        manager.update_server_health(&UpstreamType::OpenAI, "api.openai.com:443", false);
        assert_eq!(manager.healthy_server_count(&UpstreamType::OpenAI), 0);
        
        // 恢复健康状态
        manager.update_server_health(&UpstreamType::OpenAI, "api.openai.com:443", true);
        assert_eq!(manager.healthy_server_count(&UpstreamType::OpenAI), 1);
    }
}