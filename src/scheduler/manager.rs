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
}