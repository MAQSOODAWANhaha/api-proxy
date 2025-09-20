//! # API密钥池管理器
//!
//! 专门管理用户API密钥池的选择和调度，替代传统的负载均衡器概念

use super::algorithms::{ApiKeySelectionResult, ApiKeySelector, SelectionContext};
use super::api_key_health::ApiKeyHealthChecker;
use super::types::SchedulingStrategy;
use crate::auth::{AuthCredentialType, CredentialResult, SmartApiKeyProvider};
use crate::error::{ProxyError, Result};
use entity::user_provider_keys;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// API密钥池管理器
/// 职责：管理用户的API密钥池，根据策略选择合适的密钥，并集成OAuth token智能刷新
pub struct ApiKeyPoolManager {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 缓存的密钥池
    key_pools: tokio::sync::RwLock<HashMap<String, Vec<user_provider_keys::Model>>>,
    /// 选择器缓存
    selectors: tokio::sync::RwLock<HashMap<SchedulingStrategy, Arc<dyn ApiKeySelector>>>,
    /// API密钥健康检查器
    health_checker: Arc<ApiKeyHealthChecker>,
    /// 智能API密钥提供者（支持OAuth token刷新）
    smart_provider: Option<Arc<SmartApiKeyProvider>>,
}

impl ApiKeyPoolManager {
    /// 创建新的API密钥池管理器
    pub fn new(db: Arc<DatabaseConnection>, health_checker: Arc<ApiKeyHealthChecker>) -> Self {
        Self {
            db,
            key_pools: tokio::sync::RwLock::new(HashMap::new()),
            selectors: tokio::sync::RwLock::new(HashMap::new()),
            health_checker,
            smart_provider: None,
        }
    }

    /// 创建带有智能密钥提供者的API密钥池管理器
    pub fn new_with_smart_provider(
        db: Arc<DatabaseConnection>,
        health_checker: Arc<ApiKeyHealthChecker>,
        smart_provider: Arc<SmartApiKeyProvider>,
    ) -> Self {
        Self {
            db,
            key_pools: tokio::sync::RwLock::new(HashMap::new()),
            selectors: tokio::sync::RwLock::new(HashMap::new()),
            health_checker,
            smart_provider: Some(smart_provider),
        }
    }

    /// 设置智能密钥提供者
    pub fn set_smart_provider(&mut self, smart_provider: Arc<SmartApiKeyProvider>) {
        self.smart_provider = Some(smart_provider);
    }

    /// 从用户服务API配置中获取API密钥池并选择密钥
    pub async fn select_api_key_from_service_api(
        &self,
        service_api: &entity::user_service_apis::Model,
        context: &SelectionContext,
    ) -> Result<ApiKeySelectionResult> {
        // 从service_api中解析user_provider_keys_ids JSON数组
        let provider_key_ids: Vec<i32> = match &service_api.user_provider_keys_ids {
            sea_orm::prelude::Json::Array(ids) => ids
                .iter()
                .filter_map(|id| id.as_i64().map(|i| i as i32))
                .collect(),
            _ => {
                return Err(ProxyError::internal(
                    "Invalid user_provider_keys_ids format",
                ));
            }
        };

        if provider_key_ids.is_empty() {
            return Err(ProxyError::internal(
                "No provider keys configured in service API",
            ));
        }

        // 查询指定的API密钥，并应用基础筛选条件
        let all_candidate_keys = entity::user_provider_keys::Entity::find()
            .filter(entity::user_provider_keys::Column::Id.is_in(provider_key_ids))
            .filter(entity::user_provider_keys::Column::IsActive.eq(true))
            .all(&*self.db)
            .await
            .map_err(|_| ProxyError::internal("Database error when loading API keys"))?;

        // 应用更智能的筛选逻辑，考虑认证状态和过期时间
        let user_keys = self.filter_valid_keys(&all_candidate_keys).await;

        if user_keys.is_empty() {
            return Err(ProxyError::internal(
                "No active provider keys found for configured IDs",
            ));
        }

        // 过滤健康的密钥
        let healthy_keys = self.filter_healthy_keys(&user_keys).await;

        // 记录密钥限制信息用于调试
        self.log_key_limits(&user_keys).await;

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
        let keys_to_use = if healthy_keys.is_empty() {
            &user_keys
        } else {
            &healthy_keys
        };

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
        let keys_to_use = if healthy_keys.is_empty() {
            keys
        } else {
            &healthy_keys
        };

        let selector = self.get_selector(strategy).await;
        selector.select_key(keys_to_use, context).await
    }

    /// 使用智能提供者获取有效的API凭证（支持OAuth token刷新）
    ///
    /// 这个方法集成了OAuth token的智能刷新功能：
    /// 1. 使用传统的密钥选择逻辑选择API密钥
    /// 2. 通过SmartApiKeyProvider获取有效凭证（自动处理OAuth token刷新）
    /// 3. 返回增强的选择结果，包含实际可用的凭证
    pub async fn select_smart_api_key_from_service_api(
        &self,
        service_api: &entity::user_service_apis::Model,
        context: &SelectionContext,
    ) -> Result<SmartApiKeySelectionResult> {
        // 先使用传统方法选择密钥
        let selection_result = self
            .select_api_key_from_service_api(service_api, context)
            .await?;

        // 如果有智能提供者，获取有效凭证
        if let Some(smart_provider) = &self.smart_provider {
            match smart_provider
                .get_valid_credential(selection_result.selected_key.id)
                .await
            {
                Ok(credential_result) => {
                    info!(
                        key_id = selection_result.selected_key.id,
                        auth_type = ?credential_result.auth_type,
                        refreshed = credential_result.refreshed,
                        "Successfully obtained smart API credential"
                    );

                    Ok(SmartApiKeySelectionResult {
                        selection_result,
                        credential: credential_result,
                        smart_enhanced: true,
                    })
                }
                Err(e) => {
                    error!(
                        key_id = selection_result.selected_key.id,
                        error = ?e,
                        "Failed to get smart API credential, falling back to raw key"
                    );

                    // 降级：使用原始API密钥
                    let fallback_credential = CredentialResult {
                        credential: selection_result.selected_key.api_key.clone(),
                        auth_type: AuthCredentialType::ApiKey,
                        refreshed: false,
                    };

                    Ok(SmartApiKeySelectionResult {
                        selection_result,
                        credential: fallback_credential,
                        smart_enhanced: false,
                    })
                }
            }
        } else {
            // 没有智能提供者，使用原始API密钥
            let basic_credential = CredentialResult {
                credential: selection_result.selected_key.api_key.clone(),
                auth_type: AuthCredentialType::ApiKey,
                refreshed: false,
            };

            Ok(SmartApiKeySelectionResult {
                selection_result,
                credential: basic_credential,
                smart_enhanced: false,
            })
        }
    }

    /// 缓存用户的API密钥池
    pub async fn cache_user_key_pool(
        &self,
        user_id: i32,
        provider_type_id: i32,
    ) -> Result<Vec<user_provider_keys::Model>> {
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
    pub async fn get_cached_key_pool(
        &self,
        user_id: i32,
        provider_type_id: i32,
    ) -> Option<Vec<user_provider_keys::Model>> {
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

    /// 过滤有效的API密钥 - 综合考虑认证状态、过期时间等条件
    async fn filter_valid_keys(
        &self,
        keys: &[user_provider_keys::Model],
    ) -> Vec<user_provider_keys::Model> {
        let now = chrono::Utc::now().naive_utc();

        keys.iter()
            .filter(|key| {
                // 1. 检查认证状态
                if let Some(auth_status) = &key.auth_status {
                    match auth_status.as_str() {
                        "authorized" => {} // 认证成功，继续检查其他条件
                        "pending" => {
                            debug!(
                                key_id = key.id,
                                key_name = %key.name,
                                "API key is pending authorization, skipping"
                            );
                            return false;
                        }
                        "expired" => {
                            debug!(
                                key_id = key.id,
                                key_name = %key.name,
                                "API key authorization has expired, skipping"
                            );
                            return false;
                        }
                        "error" => {
                            debug!(
                                key_id = key.id,
                                key_name = %key.name,
                                "API key has authorization error, skipping"
                            );
                            return false;
                        }
                        _ => {
                            // 未知状态，保守地允许通过
                            debug!(
                                key_id = key.id,
                                key_name = %key.name,
                                unknown_status = %auth_status,
                                "Unknown auth status, allowing key"
                            );
                        }
                    }
                }

                // 2. 检查过期时间
                if let Some(expires_at) = key.expires_at {
                    if now >= expires_at {
                        debug!(
                            key_id = key.id,
                            key_name = %key.name,
                            expires_at = %expires_at,
                            "API key has expired, skipping"
                        );
                        return false;
                    }
                }

                // 3. 检查健康状态（现在有4个状态：healthy、rate_limited、unhealthy、error）
                match key.health_status.as_str() {
                    "healthy" | "unknown" => {
                        // 健康或未知状态允许通过
                        true
                    }
                    "rate_limited" => {
                        // 限流状态：检查是否已经解除
                        if let Some(resets_at) = key.rate_limit_resets_at {
                            let now = chrono::Utc::now().naive_utc();
                            if now > resets_at {
                                // 限流已解除，允许通过
                                true
                            } else {
                                debug!(
                                    key_id = key.id,
                                    key_name = %key.name,
                                    health_status = %key.health_status,
                                    rate_limit_resets_at = ?resets_at,
                                    "API key is still rate limited, skipping"
                                );
                                false
                            }
                        } else {
                            // 没有重置时间，保守地认为不健康
                            debug!(
                                key_id = key.id,
                                key_name = %key.name,
                                health_status = %key.health_status,
                                "API key is rate limited without reset time, skipping"
                            );
                            false
                        }
                    }
                    "unhealthy" | "error" => {
                        debug!(
                            key_id = key.id,
                            key_name = %key.name,
                            health_status = %key.health_status,
                            "API key is unhealthy or in error state, skipping"
                        );
                        false
                    }
                    _ => {
                        // 其他状态保守地允许通过
                        debug!(
                            key_id = key.id,
                            key_name = %key.name,
                            unknown_health = %key.health_status,
                            "Unknown health status, allowing key"
                        );
                        true
                    }
                }
            })
            .cloned()
            .collect()
    }

    /// 记录密钥限制信息
    async fn log_key_limits(&self, keys: &[user_provider_keys::Model]) {
        for key in keys {
            debug!(
                key_id = key.id,
                key_name = %key.name,
                weight = ?key.weight,
                max_requests_per_minute = ?key.max_requests_per_minute,
                max_tokens_prompt_per_minute = ?key.max_tokens_prompt_per_minute,
                max_requests_per_day = ?key.max_requests_per_day,
                auth_status = ?key.auth_status,
                expires_at = ?key.expires_at,
                health_status = %key.health_status,
                "API key limits and status information"
            );
        }
    }

    /// 过滤健康的API密钥
    async fn filter_healthy_keys(
        &self,
        keys: &[user_provider_keys::Model],
    ) -> Vec<user_provider_keys::Model> {
        let healthy_key_ids = self.health_checker.get_healthy_keys().await;

        keys.iter()
            .filter(|key| {
                // 首先检查健康检查器中的状态
                let is_healthy_by_checker = healthy_key_ids.contains(&key.id);

                // 然后检查本地的健康状态字段，考虑限流状态的自动恢复
                let is_locally_healthy = match key.health_status.as_str() {
                    "healthy" | "unknown" => true,
                    "rate_limited" => {
                        // 检查限流是否已经解除
                        if let Some(resets_at) = key.rate_limit_resets_at {
                            let now = chrono::Utc::now().naive_utc();
                            now > resets_at
                        } else {
                            // 没有重置时间，保守地认为不健康
                            false
                        }
                    }
                    "unhealthy" | "error" => false,
                    _ => true, // 其他状态保守地允许通过
                };

                let final_result = is_healthy_by_checker && is_locally_healthy;

                if !final_result {
                    debug!(
                        key_id = key.id,
                        key_name = %key.name,
                        health_status = %key.health_status,
                        is_healthy_by_checker,
                        is_locally_healthy,
                        rate_limit_resets_at = ?key.rate_limit_resets_at,
                        "Key filtered out due to health status"
                    );
                }

                final_result
            })
            .cloned()
            .collect()
    }

    /// 检查指定API密钥的健康状态
    pub async fn check_key_health(
        &self,
        key: &user_provider_keys::Model,
    ) -> Result<super::api_key_health::ApiKeyCheckResult> {
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
    pub async fn get_key_health_status(
        &self,
        key_id: i32,
    ) -> Option<super::api_key_health::ApiKeyHealthStatus> {
        self.health_checker.get_key_health_status(key_id).await
    }

    /// 获取所有API密钥的健康状态
    pub async fn get_all_health_status(
        &self,
    ) -> HashMap<i32, super::api_key_health::ApiKeyHealthStatus> {
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

/// 智能API密钥选择结果
///
/// 扩展了传统的ApiKeySelectionResult，增加了OAuth token智能刷新支持
#[derive(Debug, Clone)]
pub struct SmartApiKeySelectionResult {
    /// 传统的密钥选择结果
    pub selection_result: ApiKeySelectionResult,

    /// 智能凭证（可能是刷新后的OAuth token或原始API密钥）
    pub credential: CredentialResult,

    /// 是否启用了智能增强（即是否使用了SmartApiKeyProvider）
    pub smart_enhanced: bool,
}

impl SmartApiKeySelectionResult {
    /// 获取实际可用的API凭证
    pub fn get_credential(&self) -> &str {
        &self.credential.credential
    }

    /// 检查凭证是否是OAuth token
    pub fn is_oauth_token(&self) -> bool {
        matches!(
            self.credential.auth_type,
            AuthCredentialType::OAuthToken { .. }
        )
    }

    /// 检查凭证是否刚刚刷新过
    pub fn is_refreshed(&self) -> bool {
        self.credential.refreshed
    }

    /// 获取选中的密钥ID
    pub fn get_key_id(&self) -> i32 {
        self.selection_result.selected_key.id
    }

    /// 获取选中的密钥名称
    pub fn get_key_name(&self) -> &str {
        &self.selection_result.selected_key.name
    }

    /// 获取用户ID
    pub fn get_user_id(&self) -> i32 {
        self.selection_result.selected_key.user_id
    }
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
