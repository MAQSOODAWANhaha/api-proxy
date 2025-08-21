//! # 上游服务管理
//!
//! 基于数据库驱动的 AI 服务提供商上游连接管理

use crate::config::{AppConfig, ProviderConfig, ProviderConfigManager};
use crate::error::{ProxyError, Result};
use crate::scheduler::balancer::LoadBalancerConfig;
use crate::scheduler::{LoadBalancer, SchedulingStrategy};
use pingora_core::upstreams::peer::HttpPeer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// 提供商标识符 - 基于数据库主键的动态标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderId(pub i32);

impl ProviderId {
    /// 从数据库ID创建提供商标识
    pub fn from_database_id(id: i32) -> Self {
        Self(id)
    }

    /// 获取数据库ID
    pub fn id(&self) -> i32 {
        self.0
    }

    /// 转换为字符串形式（用于日志等）
    pub fn as_string(&self) -> String {
        format!("provider_{}", self.0)
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "provider_{}", self.0)
    }
}

impl From<i32> for ProviderId {
    fn from(id: i32) -> Self {
        ProviderId(id)
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

/// 上游管理器 - 完全数据库驱动
pub struct UpstreamManager {
    config: Arc<AppConfig>,
    load_balancer: LoadBalancer,
    provider_config_manager: Arc<ProviderConfigManager>,
}

impl UpstreamManager {
    /// 创建基于数据库驱动的上游管理器（推荐使用）
    pub fn new_database_driven(
        config: Arc<AppConfig>,
        provider_config_manager: Arc<ProviderConfigManager>,
    ) -> Self {
        let lb_config = LoadBalancerConfig {
            default_strategy: SchedulingStrategy::RoundRobin,
            health_check_interval: Duration::from_secs(30),
            auto_failover: true,
            ..Default::default()
        };

        Self {
            config,
            load_balancer: LoadBalancer::new(lb_config),
            provider_config_manager,
        }
    }

    /// 使用自定义负载均衡配置创建数据库驱动的管理器
    pub fn with_load_balancer_config(
        config: Arc<AppConfig>,
        provider_config_manager: Arc<ProviderConfigManager>,
        lb_config: LoadBalancerConfig,
    ) -> Self {
        Self {
            config,
            load_balancer: LoadBalancer::new(lb_config),
            provider_config_manager,
        }
    }

    /// 从数据库动态初始化上游服务器 - 完全数据库驱动
    pub async fn initialize_dynamic_upstreams(&mut self) -> Result<()> {
        match self.provider_config_manager.get_active_providers().await {
            Ok(providers) => {
                let providers_count = providers.len();
                tracing::info!("Loading {} active providers from database", providers_count);

                for provider in providers {
                    let provider_id = ProviderId::from_database_id(provider.id);
                    let upstream_server = self.provider_config_to_upstream_server(&provider)?;

                    // 添加服务器到负载均衡器
                    if let Err(e) = self
                        .load_balancer
                        .add_server(provider_id.clone(), upstream_server)
                    {
                        tracing::warn!(
                            "Failed to add upstream server for provider {}: {}",
                            provider_id,
                            e
                        );
                        continue;
                    }

                    tracing::info!(
                        "Added upstream server: {} (ID:{}) -> {} ({})",
                        provider.name,
                        provider_id,
                        provider.upstream_address,
                        if provider.base_url.contains("443") {
                            "TLS"
                        } else {
                            "HTTP"
                        }
                    );
                }

                tracing::info!(
                    "Successfully initialized {} database-driven upstream servers",
                    providers_count
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to load providers from database: {}", e);
                Err(e)
            }
        }
    }

    /// 将ProviderConfig转换为UpstreamServer
    fn provider_config_to_upstream_server(
        &self,
        config: &ProviderConfig,
    ) -> Result<UpstreamServer> {
        // 解析地址和端口
        let (host, port) = if config.upstream_address.contains(':') {
            let parts: Vec<&str> = config.upstream_address.splitn(2, ':').collect();
            if parts.len() != 2 {
                return Err(ProxyError::config(&format!(
                    "Invalid upstream address format: {}",
                    config.upstream_address
                )));
            }
            let port = parts[1].parse::<u16>().map_err(|_| {
                ProxyError::config(&format!(
                    "Invalid port in upstream address: {}",
                    config.upstream_address
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
            if let Some(max_connections) =
                json_config.get("max_connections").and_then(|v| v.as_u64())
            {
                server.max_connections = Some(max_connections as u32);
            }
            if let Some(health_check_interval) = json_config
                .get("health_check_interval")
                .and_then(|v| v.as_u64())
            {
                server.health_check_interval = health_check_interval * 1000; // 转换为毫秒
            }
        }

        Ok(server)
    }

    /// 刷新上游服务器配置（重新从数据库加载）
    pub async fn refresh_upstreams(&mut self) -> Result<()> {
        // 刷新提供商配置缓存
        if let Err(e) = self.provider_config_manager.refresh_cache().await {
            tracing::warn!("Failed to refresh provider config cache: {}", e);
        }

        // 重新初始化上游服务器
        self.initialize_dynamic_upstreams().await
    }

    /// 使用负载均衡选择上游服务器
    pub fn select_upstream(&self, provider_id: &ProviderId) -> Result<UpstreamServer> {
        let (server, result) = self.load_balancer.select_server(provider_id)?;
        tracing::debug!(
            "Selected upstream: {} using strategy: {:?}, reason: {}",
            server.address(),
            result.strategy,
            result.reason
        );
        Ok(server)
    }

    /// 根据提供商ID选择上游服务器
    pub fn select_upstream_by_id(&self, provider_id: i32) -> Result<UpstreamServer> {
        let provider_id = ProviderId::from_database_id(provider_id);
        self.select_upstream(&provider_id)
    }

    /// 创建用于指定提供商的 HttpPeer
    pub fn create_peer_for_provider(&self, provider_id: i32) -> Result<HttpPeer> {
        let upstream = self.select_upstream_by_id(provider_id)?;
        let sni = upstream.host.clone();
        Ok(upstream.create_peer(sni))
    }

    /// 添加上游服务器
    pub fn add_upstream(&self, provider_id: ProviderId, server: UpstreamServer) -> Result<()> {
        self.load_balancer.add_server(provider_id, server)
    }

    /// 移除上游服务器
    pub fn remove_upstream(&self, provider_id: &ProviderId, server_address: &str) -> Result<()> {
        self.load_balancer
            .remove_server(provider_id, server_address)
    }

    /// 更新服务器健康状态
    pub fn update_server_health(
        &self,
        provider_id: &ProviderId,
        server_address: &str,
        is_healthy: bool,
    ) {
        self.load_balancer
            .mark_server_healthy(provider_id, server_address, is_healthy);
    }

    /// 记录请求成功
    pub fn record_success(
        &self,
        provider_id: &ProviderId,
        server_address: &str,
        response_time: Duration,
    ) {
        self.load_balancer
            .record_success(provider_id, server_address, response_time);
    }

    /// 记录请求失败
    pub fn record_failure(&self, provider_id: &ProviderId, server_address: &str) {
        self.load_balancer
            .record_failure(provider_id, server_address);
    }

    /// 设置负载均衡策略
    pub fn set_load_balancing_strategy(
        &self,
        provider_id: ProviderId,
        strategy: SchedulingStrategy,
    ) {
        self.load_balancer.set_strategy(provider_id, strategy);
    }

    /// 获取所有上游服务器状态
    pub fn get_all_upstreams(
        &self,
    ) -> HashMap<ProviderId, Vec<(UpstreamServer, crate::scheduler::ServerMetrics)>> {
        self.load_balancer.get_all_servers()
    }

    /// 获取健康的上游服务器数量
    pub fn healthy_server_count(&self, provider_id: &ProviderId) -> usize {
        self.load_balancer.healthy_server_count(provider_id)
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
            .field("provider_count", &all_servers.keys().len())
            .field("total_servers", &total_servers)
            .finish()
    }
}
