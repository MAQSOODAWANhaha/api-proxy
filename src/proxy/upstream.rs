//! # 上游服务管理
//!
//! 管理 AI 服务提供商的上游连接

use crate::config::{AppConfig, ProviderConfigManager, ProviderConfig};
use crate::error::{ProxyError, Result};
use crate::scheduler::balancer::LoadBalancerConfig;
use crate::scheduler::{LoadBalancer, SchedulingStrategy};
use pingora_core::upstreams::peer::HttpPeer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

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
    provider_config_manager: Option<Arc<ProviderConfigManager>>,
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
            provider_config_manager: None,
        };

        manager.initialize_default_upstreams();
        manager
    }

    /// 使用自定义负载均衡配置创建管理器
    pub fn with_load_balancer_config(
        config: Arc<AppConfig>,
        lb_config: LoadBalancerConfig,
    ) -> Self {
        let mut manager = Self {
            config,
            load_balancer: LoadBalancer::new(lb_config),
            provider_config_manager: None,
        };

        manager.initialize_default_upstreams();
        manager
    }

    /// 使用动态服务商配置创建管理器（推荐）
    pub fn with_provider_config(
        config: Arc<AppConfig>,
        provider_config_manager: Arc<ProviderConfigManager>,
    ) -> Self {
        let lb_config = LoadBalancerConfig {
            default_strategy: SchedulingStrategy::RoundRobin,
            health_check_interval: Duration::from_secs(30),
            auto_failover: true,
            ..Default::default()
        };

        let mut manager = Self {
            config,
            load_balancer: LoadBalancer::new(lb_config),
            provider_config_manager: Some(provider_config_manager),
        };

        // 使用异步初始化，这里先用默认配置，实际应该在异步环境中调用 initialize_dynamic_upstreams
        manager.initialize_default_upstreams();
        manager
    }

    /// 使用动态服务商配置和自定义负载均衡配置创建管理器
    pub fn with_provider_and_lb_config(
        config: Arc<AppConfig>,
        provider_config_manager: Arc<ProviderConfigManager>,
        lb_config: LoadBalancerConfig,
    ) -> Self {
        let mut manager = Self {
            config,
            load_balancer: LoadBalancer::new(lb_config),
            provider_config_manager: Some(provider_config_manager),
        };

        // 使用异步初始化，这里先用默认配置，实际应该在异步环境中调用 initialize_dynamic_upstreams
        manager.initialize_default_upstreams();
        manager
    }

    /// 初始化默认上游服务器
    fn initialize_default_upstreams(&mut self) {
        // OpenAI 上游
        let openai_server = UpstreamServer::new("api.openai.com".to_string(), 443, true);
        self.load_balancer
            .add_server(UpstreamType::OpenAI, openai_server)
            .unwrap();

        // Anthropic 上游
        let anthropic_server = UpstreamServer::new("api.anthropic.com".to_string(), 443, true);
        self.load_balancer
            .add_server(UpstreamType::Anthropic, anthropic_server)
            .unwrap();

        // Google Gemini 上游
        let gemini_server =
            UpstreamServer::new("generativelanguage.googleapis.com".to_string(), 443, true);
        self.load_balancer
            .add_server(UpstreamType::GoogleGemini, gemini_server)
            .unwrap();

        tracing::info!("Initialized default upstream servers with load balancer");
    }

    /// 从数据库动态初始化上游服务器（替代硬编码）
    pub async fn initialize_dynamic_upstreams(&mut self) -> Result<()> {
        if let Some(ref provider_manager) = self.provider_config_manager {
            match provider_manager.get_active_providers().await {
                Ok(providers) => {
                    tracing::info!("Loading {} active providers from database", providers.len());
                    
                    for provider in providers {
                        let upstream_type = self.provider_config_to_upstream_type(&provider);
                        let upstream_server = self.provider_config_to_upstream_server(&provider)?;
                        
                        // 清除旧的服务器配置（如果存在）
                        self.load_balancer.remove_all_servers(&upstream_type);
                        
                        // 添加新的服务器配置
                        if let Err(e) = self.load_balancer.add_server(upstream_type.clone(), upstream_server) {
                            tracing::warn!("Failed to add upstream server for {}: {}", provider.name, e);
                            continue;
                        }
                        
                        tracing::debug!(
                            "Added upstream server: {} -> {} ({})",
                            provider.name,
                            provider.upstream_address,
                            if provider.base_url.contains("443") { "TLS" } else { "HTTP" }
                        );
                    }
                    
                    tracing::info!("Successfully initialized dynamic upstream servers");
                    Ok(())
                }
                Err(e) => {
                    tracing::error!("Failed to load providers from database: {}", e);
                    tracing::warn!("Falling back to default hardcoded configuration");
                    self.initialize_default_upstreams();
                    Err(e)
                }
            }
        } else {
            tracing::warn!("No provider config manager available, using default configuration");
            self.initialize_default_upstreams();
            Ok(())
        }
    }

    /// 将ProviderConfig转换为UpstreamType
    fn provider_config_to_upstream_type(&self, config: &ProviderConfig) -> UpstreamType {
        match config.name.to_lowercase().as_str() {
            "openai" => UpstreamType::OpenAI,
            "anthropic" | "claude" => UpstreamType::Anthropic,
            "gemini" | "google" => UpstreamType::GoogleGemini,
            _ => UpstreamType::Custom(config.name.clone()),
        }
    }

    /// 将ProviderConfig转换为UpstreamServer
    fn provider_config_to_upstream_server(&self, config: &ProviderConfig) -> Result<UpstreamServer> {
        // 解析地址和端口
        let (host, port) = if config.upstream_address.contains(':') {
            let parts: Vec<&str> = config.upstream_address.splitn(2, ':').collect();
            if parts.len() != 2 {
                return Err(ProxyError::config(&format!(
                    "Invalid upstream address format: {}", config.upstream_address
                )));
            }
            let port = parts[1].parse::<u16>().map_err(|_| {
                ProxyError::config(&format!(
                    "Invalid port in upstream address: {}", config.upstream_address
                ))
            })?;
            (parts[0].to_string(), port)
        } else {
            (config.upstream_address.clone(), 443) // 默认HTTPS端口
        };

        // 判断是否使用TLS（通常端口443或明确配置）
        let use_tls = port == 443 || config.base_url.starts_with("https");

        let mut server = UpstreamServer::new(host, port, use_tls);
        
        // 应用配置中的超时设置
        if let Some(timeout_seconds) = config.timeout_seconds {
            server.timeout_ms = (timeout_seconds as u64) * 1000;
        }

        // 从配置JSON中提取其他设置
        if let Some(ref json_config) = config.config_json {
            if let Some(weight) = json_config.get("weight").and_then(|v| v.as_u64()) {
                server.weight = weight as u32;
            }
            if let Some(max_connections) = json_config.get("max_connections").and_then(|v| v.as_u64()) {
                server.max_connections = Some(max_connections as u32);
            }
            if let Some(health_check_interval) = json_config.get("health_check_interval").and_then(|v| v.as_u64()) {
                server.health_check_interval = health_check_interval * 1000; // 转换为毫秒
            }
        }

        Ok(server)
    }

    /// 刷新上游服务器配置（重新从数据库加载）
    pub async fn refresh_upstreams(&mut self) -> Result<()> {
        if let Some(ref provider_manager) = self.provider_config_manager {
            // 刷新提供商配置缓存
            if let Err(e) = provider_manager.refresh_cache().await {
                tracing::warn!("Failed to refresh provider config cache: {}", e);
            }
            
            // 重新初始化上游服务器
            self.initialize_dynamic_upstreams().await
        } else {
            tracing::warn!("Cannot refresh upstreams: no provider config manager");
            Ok(())
        }
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
        tracing::debug!(
            "Selected upstream: {} using strategy: {:?}, reason: {}",
            server.address(),
            result.strategy,
            result.reason
        );
        Ok(server)
    }

    /// 根据请求路径选择上游服务器
    pub fn select_upstream_for_path(&self, path: &str) -> Result<UpstreamServer> {
        let upstream_type = UpstreamType::from_path(path).ok_or_else(|| {
            ProxyError::upstream_not_found(format!("Cannot determine upstream for path: {}", path))
        })?;

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
    pub fn remove_upstream(
        &self,
        upstream_type: &UpstreamType,
        server_address: &str,
    ) -> Result<()> {
        self.load_balancer
            .remove_server(upstream_type, server_address)
    }

    /// 更新服务器健康状态
    pub fn update_server_health(
        &self,
        upstream_type: &UpstreamType,
        server_address: &str,
        is_healthy: bool,
    ) {
        self.load_balancer
            .mark_server_healthy(upstream_type, server_address, is_healthy);
    }

    /// 记录请求成功
    pub fn record_success(
        &self,
        upstream_type: &UpstreamType,
        server_address: &str,
        response_time: Duration,
    ) {
        self.load_balancer
            .record_success(upstream_type, server_address, response_time);
    }

    /// 记录请求失败
    pub fn record_failure(&self, upstream_type: &UpstreamType, server_address: &str) {
        self.load_balancer
            .record_failure(upstream_type, server_address);
    }

    /// 设置负载均衡策略
    pub fn set_load_balancing_strategy(
        &self,
        upstream_type: UpstreamType,
        strategy: SchedulingStrategy,
    ) {
        self.load_balancer.set_strategy(upstream_type, strategy);
    }

    /// 获取所有上游服务器状态
    pub fn get_all_upstreams(
        &self,
    ) -> HashMap<UpstreamType, Vec<(UpstreamServer, crate::scheduler::ServerMetrics)>> {
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
