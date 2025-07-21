//! # 负载均衡管理器
//!
//! 管理多个负载均衡器实例

use crate::config::AppConfig;
use crate::proxy::upstream::UpstreamType;
use crate::scheduler::balancer::LoadBalancer;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 负载均衡管理器
pub struct LoadBalancerManager {
    /// 应用配置
    config: Arc<AppConfig>,
    /// 负载均衡器映射
    load_balancers: Arc<RwLock<HashMap<UpstreamType, LoadBalancer>>>,
}

impl LoadBalancerManager {
    /// 创建新的负载均衡管理器
    pub fn new(config: Arc<AppConfig>) -> Result<Self> {
        Ok(Self {
            config,
            load_balancers: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 获取指定上游类型的负载均衡器
    pub async fn get_load_balancer(&self, upstream_type: &UpstreamType) -> Option<LoadBalancer> {
        let load_balancers = self.load_balancers.read().await;
        load_balancers.get(upstream_type).cloned()
    }

    /// 添加负载均衡器
    pub async fn add_load_balancer(&self, upstream_type: UpstreamType, load_balancer: LoadBalancer) {
        let mut load_balancers = self.load_balancers.write().await;
        load_balancers.insert(upstream_type, load_balancer);
    }

    /// 获取所有负载均衡器的状态
    pub async fn get_all_status(&self) -> HashMap<UpstreamType, String> {
        let load_balancers = self.load_balancers.read().await;
        let mut status = HashMap::new();
        
        for (upstream_type, _) in load_balancers.iter() {
            status.insert(upstream_type.clone(), "active".to_string());
        }
        
        status
    }

    /// 向指定上游类型添加服务器
    pub async fn add_server(
        &self,
        upstream_type_str: &str,
        host: &str,
        port: u16,
        weight: u32,
        use_tls: bool
    ) -> Result<()> {
        // 解析上游类型
        let upstream_type = match upstream_type_str.to_lowercase().as_str() {
            "openai" => UpstreamType::OpenAI,
            "anthropic" => UpstreamType::Anthropic,
            "google" | "gemini" => UpstreamType::GoogleGemini,
            _ => UpstreamType::Custom(upstream_type_str.to_string()),
        };

        // 构建服务器信息
        use crate::proxy::upstream::UpstreamServer;
        let server = UpstreamServer {
            host: host.to_string(),
            port,
            use_tls,
            weight,
            max_connections: Some(100), // 默认最大连接数
            timeout_ms: 30000, // 默认30秒超时
            health_check_interval: 60, // 默认60秒健康检查间隔
            is_healthy: true, // 初始状态为健康
        };

        // 获取或创建负载均衡器
        let mut load_balancers = self.load_balancers.write().await;
        
        let load_balancer = match load_balancers.get(&upstream_type) {
            Some(lb) => lb.clone(),
            None => {
                // 创建新的负载均衡器
                use crate::scheduler::balancer::{LoadBalancer, LoadBalancerConfig};
                let config = LoadBalancerConfig::default();
                let new_lb = LoadBalancer::new(config);
                load_balancers.insert(upstream_type.clone(), new_lb.clone());
                new_lb
            }
        };

        // 添加服务器到负载均衡器
        load_balancer.add_server(upstream_type, server)?;
        
        tracing::info!("Successfully added server {}:{} to upstream type {}", host, port, upstream_type_str);
        Ok(())
    }
}