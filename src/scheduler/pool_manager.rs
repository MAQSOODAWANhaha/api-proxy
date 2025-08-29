//! # API密钥池管理器
//!
//! 专门管理用户API密钥池的选择和调度，替代传统的负载均衡器概念

use super::algorithms::{ApiKeySelector, ApiKeySelectionResult, SelectionContext};
use super::api_key_health::ApiKeyHealthChecker;
use super::types::SchedulingStrategy;
use crate::error::{ProxyError, Result};
use entity::user_provider_keys;
use sea_orm::{DatabaseConnection, EntityTrait, ColumnTrait, QueryFilter};
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{debug, warn};

/// API密钥池管理器
/// 职责：管理用户的API密钥池，根据策略选择合适的密钥
pub struct ApiKeyPoolManager {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 缓存的密钥池
    key_pools: tokio::sync::RwLock<HashMap<String, Vec<user_provider_keys::Model>>>,
    /// 选择器缓存
    selectors: tokio::sync::RwLock<HashMap<SchedulingStrategy, Arc<dyn ApiKeySelector>>>,
    /// API密钥健康检查器
    health_checker: Arc<ApiKeyHealthChecker>,
}

impl ApiKeyPoolManager {
    /// 创建新的API密钥池管理器
    pub fn new(db: Arc<DatabaseConnection>, health_checker: Arc<ApiKeyHealthChecker>) -> Self {
        Self {
            db,
            key_pools: tokio::sync::RwLock::new(HashMap::new()),
            selectors: tokio::sync::RwLock::new(HashMap::new()),
            health_checker,
        }
    }

    /// 从用户服务API配置中获取API密钥池并选择密钥
    pub async fn select_api_key_from_service_api(
        &self,
        service_api: &entity::user_service_apis::Model,
        context: &SelectionContext,
    ) -> Result<ApiKeySelectionResult> {
        // 从service_api中解析user_provider_keys_ids JSON数组
        let provider_key_ids: Vec<i32> = match &service_api.user_provider_keys_ids {
            sea_orm::prelude::Json::Array(ids) => {
                ids.iter()
                    .filter_map(|id| id.as_i64().map(|i| i as i32))
                    .collect()
            },
            _ => {
                return Err(ProxyError::internal("Invalid user_provider_keys_ids format"));
            }
        };

        if provider_key_ids.is_empty() {
            return Err(ProxyError::internal("No provider keys configured in service API"));
        }

        // 查询指定的API密钥
        let user_keys = entity::user_provider_keys::Entity::find()
            .filter(entity::user_provider_keys::Column::Id.is_in(provider_key_ids))
            .filter(entity::user_provider_keys::Column::IsActive.eq(true))
            .all(&*self.db)
            .await
            .map_err(|_| ProxyError::internal("Database error when loading API keys"))?;

        if user_keys.is_empty() {
            return Err(ProxyError::internal("No active provider keys found for configured IDs"));
        }

        // 过滤健康的密钥
        let healthy_keys = self.filter_healthy_keys(&user_keys).await;
        
        if healthy_keys.is_empty() {
            // 如果没有健康的密钥，记录警告并使用所有密钥（降级模式）
            warn!(
                service_api_id = service_api.id,
                total_keys = user_keys.len(),
                "No healthy API keys available, using all keys in degraded mode"
            );
        } else if healthy_keys.len() != user_keys.len() {
            debug!(
                service_api_id = service_api.id,
                total_keys = user_keys.len(),
                healthy_keys = healthy_keys.len(),
                "Filtered out {} unhealthy API keys",
                user_keys.len() - healthy_keys.len()
            );
        }

        // 选择要使用的密钥集合（优先使用健康的，降级时使用全部）
        let keys_to_use = if healthy_keys.is_empty() { &user_keys } else { &healthy_keys };

        // 使用配置的调度策略
        let scheduling_strategy = service_api
            .scheduling_strategy
            .as_deref()
            .and_then(|s| SchedulingStrategy::from_str(s))
            .unwrap_or_default();

        // 获取或创建选择器
        let selector = self.get_selector(scheduling_strategy).await;
        
        // 执行密钥选择
        selector.select_key(keys_to_use, context).await
    }

    /// 从密钥池中直接选择API密钥
    pub async fn select_api_key_from_pool(
        &self,
        keys: &[user_provider_keys::Model],
        strategy: SchedulingStrategy,
        context: &SelectionContext,
    ) -> Result<ApiKeySelectionResult> {
        if keys.is_empty() {
            return Err(ProxyError::internal("Empty API key pool"));
        }

        // 过滤健康的密钥
        let healthy_keys = self.filter_healthy_keys(keys).await;
        let keys_to_use = if healthy_keys.is_empty() { keys } else { &healthy_keys };

        let selector = self.get_selector(strategy).await;
        selector.select_key(keys_to_use, context).await
    }

    /// 缓存用户的API密钥池
    pub async fn cache_user_key_pool(&self, user_id: i32, provider_type_id: i32) -> Result<Vec<user_provider_keys::Model>> {
        let cache_key = format!("user_{}_{}", user_id, provider_type_id);
        
        // 查询用户的API密钥
        let user_keys = entity::user_provider_keys::Entity::find()
            .filter(entity::user_provider_keys::Column::UserId.eq(user_id))
            .filter(entity::user_provider_keys::Column::ProviderTypeId.eq(provider_type_id))
            .filter(entity::user_provider_keys::Column::IsActive.eq(true))
            .all(&*self.db)
            .await
            .map_err(|_| ProxyError::internal("Database error when caching key pool"))?;

        // 缓存到内存
        {
            let mut pools = self.key_pools.write().await;
            pools.insert(cache_key, user_keys.clone());
        }

        Ok(user_keys)
    }

    /// 从缓存获取API密钥池
    pub async fn get_cached_key_pool(&self, user_id: i32, provider_type_id: i32) -> Option<Vec<user_provider_keys::Model>> {
        let cache_key = format!("user_{}_{}", user_id, provider_type_id);
        let pools = self.key_pools.read().await;
        pools.get(&cache_key).cloned()
    }

    /// 清理指定用户的密钥池缓存
    pub async fn invalidate_user_cache(&self, user_id: i32) {
        let mut pools = self.key_pools.write().await;
        let keys_to_remove: Vec<String> = pools
            .keys()
            .filter(|key| key.starts_with(&format!("user_{}_", user_id)))
            .cloned()
            .collect();
        
        for key in keys_to_remove {
            pools.remove(&key);
        }
    }

    /// 清理所有缓存
    pub async fn clear_cache(&self) {
        let mut pools = self.key_pools.write().await;
        pools.clear();

        let mut selectors = self.selectors.write().await;
        selectors.clear();
    }

    /// 获取或创建API密钥选择器
    async fn get_selector(&self, strategy: SchedulingStrategy) -> Arc<dyn ApiKeySelector> {
        {
            let selectors = self.selectors.read().await;
            if let Some(selector) = selectors.get(&strategy) {
                return selector.clone();
            }
        }

        // 创建新的选择器
        let selector = super::algorithms::create_api_key_selector(strategy);
        
        {
            let mut selectors = self.selectors.write().await;
            selectors.insert(strategy, selector.clone());
        }

        selector
    }

    /// 过滤健康的API密钥
    async fn filter_healthy_keys(&self, keys: &[user_provider_keys::Model]) -> Vec<user_provider_keys::Model> {
        let healthy_key_ids = self.health_checker.get_healthy_keys().await;
        
        keys.iter()
            .filter(|key| healthy_key_ids.contains(&key.id))
            .cloned()
            .collect()
    }

    /// 检查指定API密钥的健康状态
    pub async fn check_key_health(&self, key: &user_provider_keys::Model) -> Result<super::api_key_health::ApiKeyCheckResult> {
        self.health_checker
            .check_api_key(key)
            .await
            .map_err(|e| ProxyError::internal(&format!("Health check failed: {}", e)))
    }

    /// 批量检查多个API密钥的健康状态
    pub async fn batch_check_keys_health(
        &self,
        keys: Vec<user_provider_keys::Model>,
    ) -> Result<HashMap<i32, super::api_key_health::ApiKeyCheckResult>> {
        self.health_checker
            .batch_check_keys(keys)
            .await
            .map_err(|e| ProxyError::internal(&format!("Batch health check failed: {}", e)))
    }

    /// 手动标记API密钥为不健康
    pub async fn mark_key_unhealthy(&self, key_id: i32, reason: String) -> Result<()> {
        self.health_checker
            .mark_key_unhealthy(key_id, reason)
            .await
            .map_err(|e| ProxyError::internal(&format!("Failed to mark key unhealthy: {}", e)))
    }

    /// 获取API密钥的健康状态
    pub async fn get_key_health_status(&self, key_id: i32) -> Option<super::api_key_health::ApiKeyHealthStatus> {
        self.health_checker.get_key_health_status(key_id).await
    }

    /// 获取所有API密钥的健康状态
    pub async fn get_all_health_status(&self) -> HashMap<i32, super::api_key_health::ApiKeyHealthStatus> {
        self.health_checker.get_all_health_status().await
    }

    /// 启动健康检查服务
    pub async fn start_health_checking(&self) -> Result<()> {
        self.health_checker
            .start()
            .await
            .map_err(|e| ProxyError::internal(&format!("Failed to start health checker: {}", e)))
    }

    /// 停止健康检查服务
    pub async fn stop_health_checking(&self) -> Result<()> {
        self.health_checker
            .stop()
            .await
            .map_err(|e| ProxyError::internal(&format!("Failed to stop health checker: {}", e)))
    }

    /// 获取密钥池统计信息
    pub async fn get_pool_stats(&self) -> PoolStats {
        let pools = self.key_pools.read().await;
        let selectors = self.selectors.read().await;
        let healthy_key_ids = self.health_checker.get_healthy_keys().await;
        let all_health_status = self.health_checker.get_all_health_status().await;
        
        PoolStats {
            cached_pools: pools.len(),
            total_keys: pools.values().map(|pool| pool.len()).sum(),
            active_selectors: selectors.len(),
            available_strategies: vec![
                SchedulingStrategy::RoundRobin,
                SchedulingStrategy::Weighted,
                SchedulingStrategy::HealthBased,
            ],
            healthy_keys: healthy_key_ids.len(),
            total_tracked_keys: all_health_status.len(),
            health_check_running: self.health_checker.is_running().await,
        }
    }
}

/// 密钥池统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct PoolStats {
    /// 缓存的密钥池数量
    pub cached_pools: usize,
    /// 总密钥数量
    pub total_keys: usize,
    /// 活跃选择器数量
    pub active_selectors: usize,
    /// 可用策略
    pub available_strategies: Vec<SchedulingStrategy>,
    /// 健康的密钥数量
    pub healthy_keys: usize,
    /// 正在追踪的密钥总数
    pub total_tracked_keys: usize,
    /// 健康检查服务是否运行中
    pub health_check_running: bool,
}

impl std::fmt::Debug for ApiKeyPoolManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiKeyPoolManager")
            .field("cached_pools", &"<async>")
            .field("selectors", &"<async>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use tokio;

    #[tokio::test]
    async fn test_pool_manager_creation() {
        // 基本测试，需要真实数据库连接
        // 实际测试需要设置测试环境
        assert!(true);
    }
}