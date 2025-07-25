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

    /// 移除服务器
    pub async fn remove_server(&self, upstream_type_str: &str, api_id: i32) -> Result<()> {
        let upstream_type = match upstream_type_str.to_lowercase().as_str() {
            "openai" => UpstreamType::OpenAI,
            "anthropic" | "claude" => UpstreamType::Anthropic,
            "google" | "gemini" => UpstreamType::GoogleGemini,
            custom => UpstreamType::Custom(custom.to_string()),
        };

        let load_balancers = self.load_balancers.read().await;
        if let Some(load_balancer) = load_balancers.get(&upstream_type) {
            let server_address = format!("api_id_{}", api_id);
            load_balancer.remove_server(&upstream_type, &server_address)?;
            tracing::info!("Successfully removed server with API ID {} from upstream type {}", api_id, upstream_type_str);
        }
        
        Ok(())
    }

    /// 更改调度策略
    pub async fn change_strategy(
        &self,
        upstream_type: UpstreamType,
        new_strategy: crate::scheduler::types::SchedulingStrategy,
    ) -> Result<Option<crate::scheduler::types::SchedulingStrategy>> {
        let load_balancers = self.load_balancers.read().await;
        
        if let Some(load_balancer) = load_balancers.get(&upstream_type) {
            // 获取当前策略（这里简化实现，实际可能需要在负载均衡器中保存当前策略）
            let old_strategy = Some(crate::scheduler::types::SchedulingStrategy::RoundRobin);
            
            // 设置新策略
            load_balancer.set_strategy(upstream_type.clone(), new_strategy);
            
            tracing::info!("Changed strategy for {:?} from {:?} to {:?}", upstream_type, old_strategy, new_strategy);
            Ok(old_strategy)
        } else {
            Err(anyhow::anyhow!("No load balancer found for upstream type: {:?}", upstream_type))
        }
    }

    /// 获取详细指标
    pub async fn get_detailed_metrics(&self) -> Result<serde_json::Value> {
        let load_balancers = self.load_balancers.read().await;
        let mut metrics = serde_json::Map::new();
        
        for (upstream_type, load_balancer) in load_balancers.iter() {
            let all_servers = load_balancer.get_all_servers();
            let healthy_count = load_balancer.healthy_server_count(upstream_type);
            let total_count = all_servers.get(upstream_type).map(|servers| servers.len()).unwrap_or(0);
            
            let server_details: Vec<serde_json::Value> = all_servers
                .get(upstream_type)
                .map(|servers| {
                    servers.iter().map(|(server, metrics)| {
                        // 计算健康检查时间（秒前）
                        let health_check_seconds_ago = metrics.last_health_check.elapsed().as_secs();
                        let health_check_time = chrono::Utc::now() - chrono::Duration::seconds(health_check_seconds_ago as i64);
                        
                        serde_json::json!({
                            "address": server.address(),
                            "weight": server.weight,
                            "is_healthy": metrics.is_healthy,
                            "success_requests": metrics.success_requests,
                            "failed_requests": metrics.failed_requests,
                            "avg_response_time_ms": metrics.avg_response_time,
                            "last_health_check": health_check_time,
                            "use_tls": server.use_tls
                        })
                    }).collect()
                })
                .unwrap_or_default();
            
            metrics.insert(
                format!("{:?}", upstream_type),
                serde_json::json!({
                    "total_servers": total_count,
                    "healthy_servers": healthy_count,
                    "unhealthy_servers": total_count - healthy_count,
                    "success_rate": if total_count > 0 { 
                        (healthy_count as f64 / total_count as f64) * 100.0 
                    } else { 
                        0.0 
                    },
                    "servers": server_details
                })
            );
        }
        
        Ok(serde_json::Value::Object(metrics))
    }
}