//! # API密钥池管理器
//!
//! 专门管理用户API密钥池的选择和调度，替代传统的负载均衡器概念

use super::algorithms::{ApiKeySelectionResult, ApiKeySelector, SelectionContext};
use super::api_key_health::ApiKeyHealthChecker;
use super::types::{ApiKeyHealthStatus, SchedulingStrategy};
use crate::auth::{AuthCredentialType, CredentialResult, SmartApiKeyProvider, types::AuthStatus};
use crate::error::{ProxyError, Result};
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, lerror, linfo, lwarn};
use entity::user_provider_keys;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use std::collections::HashMap;
use std::sync::Arc;

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
    #[must_use]
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
    #[must_use]
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
        linfo!(
            &context.request_id,
            LogStage::Scheduling,
            LogComponent::Scheduler,
            "start_key_selection",
            "Starting API key selection for service API",
            service_api_id = service_api.id,
            route_group = %context.route_group
        );

        let provider_key_ids = Self::get_provider_key_ids(service_api, context)?;
        let all_candidate_keys = self
            .load_active_provider_keys(&provider_key_ids, context)
            .await?;
        let user_keys = Self::filter_valid_keys_with_logging(&all_candidate_keys, context)?;
        let healthy_keys = self
            .filter_healthy_keys_with_logging(&user_keys, service_api, context)
            .await;
        Self::log_key_limits(&user_keys);

        let keys_to_use = if healthy_keys.is_empty() {
            user_keys.as_slice()
        } else {
            healthy_keys.as_slice()
        };

        linfo!(
            &context.request_id,
            LogStage::Scheduling,
            LogComponent::Scheduler,
            "keys_for_selection_count",
            "Final number of keys passed to the selection algorithm",
            count = keys_to_use.len()
        );

        let scheduling_strategy = Self::resolve_strategy(service_api);
        let selector = self.get_selector(scheduling_strategy).await;

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
    /// `这个方法集成了OAuth` token的智能刷新功能：
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
                    linfo!(
                        "system",
                        LogStage::Scheduling,
                        LogComponent::Scheduler,
                        "smart_credential_ok",
                        "Successfully obtained smart API credential",
                        key_id = selection_result.selected_key.id,
                        auth_type = ?credential_result.auth_type,
                        refreshed = credential_result.refreshed,
                    );

                    Ok(SmartApiKeySelectionResult {
                        selection_result,
                        credential: credential_result,
                        smart_enhanced: true,
                    })
                }
                Err(e) => {
                    lerror!(
                        "system",
                        LogStage::Scheduling,
                        LogComponent::Scheduler,
                        "smart_credential_fail",
                        "Failed to get smart API credential, falling back to raw key",
                        key_id = selection_result.selected_key.id,
                        error = ?e,
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
        let cache_key = format!("user_{user_id}_{provider_type_id}");

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
        let cache_key = format!("user_{user_id}_{provider_type_id}");
        let pools = self.key_pools.read().await;
        pools.get(&cache_key).cloned()
    }

    /// 清理指定用户的密钥池缓存
    pub async fn invalidate_user_cache(&self, user_id: i32) {
        let mut pools = self.key_pools.write().await;
        let keys_to_remove: Vec<String> = pools
            .keys()
            .filter(|key| key.starts_with(&format!("user_{user_id}_")))
            .cloned()
            .collect();

        for key in keys_to_remove {
            pools.remove(&key);
        }
    }

    /// 清理所有缓存
    pub async fn clear_cache(&self) {
        self.key_pools.write().await.clear();
        self.selectors.write().await.clear();
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
    fn filter_valid_keys(keys: &[user_provider_keys::Model]) -> Vec<user_provider_keys::Model> {
        let now = chrono::Utc::now().naive_utc();

        keys.iter()
            .filter(|key| Self::is_key_valid(key, &now))
            .cloned()
            .collect()
    }

    fn is_key_valid(key: &user_provider_keys::Model, now: &chrono::NaiveDateTime) -> bool {
        Self::passes_auth_checks(key)
            && Self::is_not_expired(key, now)
            && Self::passes_health_checks(key, now)
    }

    fn passes_auth_checks(key: &user_provider_keys::Model) -> bool {
        key.auth_status.as_deref().map_or(true, |auth_status| {
            let status = AuthStatus::from(auth_status);
            Self::auth_status_allows_usage(key, status)
        })
    }

    fn auth_status_allows_usage(key: &user_provider_keys::Model, status: AuthStatus) -> bool {
        match status {
            AuthStatus::Authorized => true,
            AuthStatus::Pending => {
                ldebug!(
                    "system",
                    LogStage::Scheduling,
                    LogComponent::Scheduler,
                    "key_pending",
                    "API key is pending authorization, skipping",
                    key_id = key.id,
                    key_name = %key.name,
                );
                false
            }
            AuthStatus::Expired => {
                ldebug!(
                    "system",
                    LogStage::Scheduling,
                    LogComponent::Scheduler,
                    "key_expired",
                    "API key authorization has expired, skipping",
                    key_id = key.id,
                    key_name = %key.name,
                );
                false
            }
            AuthStatus::Error => {
                ldebug!(
                    "system",
                    LogStage::Scheduling,
                    LogComponent::Scheduler,
                    "key_auth_error",
                    "API key has authorization error, skipping",
                    key_id = key.id,
                    key_name = %key.name,
                );
                false
            }
            AuthStatus::Revoked => {
                ldebug!(
                    "system",
                    LogStage::Scheduling,
                    LogComponent::Scheduler,
                    "key_revoked",
                    "API key has been revoked, skipping",
                    key_id = key.id,
                    key_name = %key.name,
                );
                false
            }
        }
    }

    fn is_not_expired(key: &user_provider_keys::Model, now: &chrono::NaiveDateTime) -> bool {
        if let Some(expires_at) = key.expires_at.as_ref() {
            if now >= expires_at {
                ldebug!(
                    "system",
                    LogStage::Scheduling,
                    LogComponent::Scheduler,
                    "key_expired",
                    "API key has expired, skipping",
                    key_id = key.id,
                    key_name = %key.name,
                    expires_at = %expires_at,
                );
                return false;
            }
        }
        true
    }

    fn passes_health_checks(key: &user_provider_keys::Model, now: &chrono::NaiveDateTime) -> bool {
        match key.health_status.as_str().parse::<ApiKeyHealthStatus>() {
            Ok(ApiKeyHealthStatus::Healthy) => true,
            Ok(ApiKeyHealthStatus::RateLimited) => Self::is_rate_limit_recovered(key, now),
            Ok(ApiKeyHealthStatus::Unhealthy) => {
                ldebug!(
                    "system",
                    LogStage::Scheduling,
                    LogComponent::Scheduler,
                    "key_unhealthy",
                    "API key is unhealthy, skipping",
                    key_id = key.id,
                    key_name = %key.name,
                    health_status = %key.health_status,
                );
                false
            }
            Err(_) => {
                ldebug!(
                    "system",
                    LogStage::Scheduling,
                    LogComponent::Scheduler,
                    "key_unknown_health",
                    "Unknown health status, treating as unhealthy",
                    key_id = key.id,
                    key_name = %key.name,
                    unknown_health = %key.health_status,
                );
                false
            }
        }
    }

    fn is_rate_limit_recovered(
        key: &user_provider_keys::Model,
        now: &chrono::NaiveDateTime,
    ) -> bool {
        match key.rate_limit_resets_at.as_ref() {
            Some(resets_at) if now > resets_at => true,
            Some(resets_at) => {
                ldebug!(
                    "system",
                    LogStage::Scheduling,
                    LogComponent::Scheduler,
                    "key_rate_limited",
                    "API key is still rate limited, skipping",
                    key_id = key.id,
                    key_name = %key.name,
                    health_status = %key.health_status,
                    rate_limit_resets_at = ?resets_at,
                );
                false
            }
            None => {
                ldebug!(
                    "system",
                    LogStage::Scheduling,
                    LogComponent::Scheduler,
                    "key_rate_limited_no_reset",
                    "API key is rate limited without reset time, skipping",
                    key_id = key.id,
                    key_name = %key.name,
                    health_status = %key.health_status,
                );
                false
            }
        }
    }

    async fn load_active_provider_keys(
        &self,
        provider_key_ids: &[i32],
        context: &SelectionContext,
    ) -> Result<Vec<user_provider_keys::Model>> {
        let keys = entity::user_provider_keys::Entity::find()
            .filter(entity::user_provider_keys::Column::Id.is_in(provider_key_ids.to_vec()))
            .filter(entity::user_provider_keys::Column::IsActive.eq(true))
            .order_by_asc(entity::user_provider_keys::Column::Id)
            .all(&*self.db)
            .await
            .map_err(|_| ProxyError::internal("Database error when loading API keys"))?;

        ldebug!(
            &context.request_id,
            LogStage::Scheduling,
            LogComponent::Scheduler,
            "candidate_keys_count",
            "Retrieved candidate keys from DB",
            count = keys.len()
        );

        Ok(keys)
    }

    fn get_provider_key_ids(
        service_api: &entity::user_service_apis::Model,
        context: &SelectionContext,
    ) -> Result<Vec<i32>> {
        let ids = match &service_api.user_provider_keys_ids {
            sea_orm::prelude::Json::Array(values) => values
                .iter()
                .filter_map(|id| id.as_i64().and_then(|i| i32::try_from(i).ok()))
                .collect::<Vec<_>>(),
            _ => {
                return Err(ProxyError::internal(
                    "Invalid user_provider_keys_ids format",
                ));
            }
        };

        if ids.is_empty() {
            return Err(ProxyError::internal(
                "No provider keys configured in service API",
            ));
        }

        ldebug!(
            &context.request_id,
            LogStage::Scheduling,
            LogComponent::Scheduler,
            "configured_keys",
            "Found configured provider key IDs",
            key_ids = ?ids
        );

        Ok(ids)
    }

    fn filter_valid_keys_with_logging(
        candidate_keys: &[user_provider_keys::Model],
        context: &SelectionContext,
    ) -> Result<Vec<user_provider_keys::Model>> {
        let filtered = Self::filter_valid_keys(candidate_keys);

        ldebug!(
            &context.request_id,
            LogStage::Scheduling,
            LogComponent::Scheduler,
            "valid_keys_count",
            "Keys remaining after initial validation filter",
            count = filtered.len()
        );

        if filtered.is_empty() {
            Err(ProxyError::internal(
                "No active provider keys found for configured IDs",
            ))
        } else {
            Ok(filtered)
        }
    }

    async fn filter_healthy_keys_with_logging(
        &self,
        user_keys: &[user_provider_keys::Model],
        service_api: &entity::user_service_apis::Model,
        context: &SelectionContext,
    ) -> Vec<user_provider_keys::Model> {
        let healthy_keys = self.filter_healthy_keys(user_keys).await;

        ldebug!(
            &context.request_id,
            LogStage::Scheduling,
            LogComponent::Scheduler,
            "healthy_keys_count",
            "Keys remaining after health filter",
            count = healthy_keys.len()
        );

        if healthy_keys.is_empty() {
            lwarn!(
                "system",
                LogStage::Scheduling,
                LogComponent::Scheduler,
                "no_healthy_keys",
                "No healthy API keys available, using all keys in degraded mode",
                service_api_id = service_api.id,
                total_keys = user_keys.len(),
            );
        } else if healthy_keys.len() != user_keys.len() {
            ldebug!(
                "system",
                LogStage::Scheduling,
                LogComponent::Scheduler,
                "unhealthy_keys_filtered",
                &format!(
                    "Filtered out {} unhealthy API keys",
                    user_keys.len() - healthy_keys.len()
                ),
                service_api_id = service_api.id,
                total_keys = user_keys.len(),
                healthy_keys = healthy_keys.len(),
            );
        }

        healthy_keys
    }

    fn resolve_strategy(service_api: &entity::user_service_apis::Model) -> SchedulingStrategy {
        service_api
            .scheduling_strategy
            .as_deref()
            .and_then(SchedulingStrategy::parse)
            .unwrap_or_default()
    }

    /// 记录密钥限制信息
    fn log_key_limits(keys: &[user_provider_keys::Model]) {
        for key in keys {
            ldebug!(
                "system",
                LogStage::Scheduling,
                LogComponent::Scheduler,
                "key_info",
                "API key limits and status information",
                key_id = key.id,
                key_name = %key.name,
                weight = ?key.weight,
                max_requests_per_minute = ?key.max_requests_per_minute,
                max_tokens_prompt_per_minute = ?key.max_tokens_prompt_per_minute,
                max_requests_per_day = ?key.max_requests_per_day,
                auth_status = ?key.auth_status,
                expires_at = ?key.expires_at,
                health_status = %key.health_status,
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
                let is_locally_healthy =
                    match key.health_status.as_str().parse::<ApiKeyHealthStatus>() {
                        Ok(ApiKeyHealthStatus::Healthy) => true,
                        Ok(ApiKeyHealthStatus::RateLimited) => {
                            key.rate_limit_resets_at.is_some_and(|resets_at| {
                                let now = chrono::Utc::now().naive_utc();
                                now > resets_at
                            })
                        }
                        Ok(ApiKeyHealthStatus::Unhealthy) | Err(_) => false,
                    };

                let final_result = is_healthy_by_checker || is_locally_healthy;

                if !final_result {
                    ldebug!(
                        "system",
                        LogStage::Scheduling,
                        LogComponent::Scheduler,
                        "key_unhealthy_filtered",
                        "Key filtered out due to health status",
                        key_id = key.id,
                        key_name = %key.name,
                        health_status = %key.health_status,
                        is_healthy_by_checker,
                        is_locally_healthy,
                        rate_limit_resets_at = ?key.rate_limit_resets_at,
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
            .map_err(|e| ProxyError::internal(format!("Health check failed: {e}")))
    }

    /// 批量检查多个API密钥的健康状态
    pub async fn batch_check_keys_health(
        &self,
        keys: Vec<user_provider_keys::Model>,
    ) -> Result<HashMap<i32, super::api_key_health::ApiKeyCheckResult>> {
        self.health_checker
            .batch_check_keys(keys)
            .await
            .map_err(|e| ProxyError::internal(format!("Batch health check failed: {e}")))
    }

    /// 手动标记API密钥为不健康
    pub async fn mark_key_unhealthy(&self, key_id: i32, reason: String) -> Result<()> {
        self.health_checker
            .mark_key_unhealthy(key_id, reason)
            .await
            .map_err(|e| ProxyError::internal(format!("Failed to mark key unhealthy: {e}")))
    }

    /// 获取API密钥的健康状态
    pub async fn get_key_health_status(
        &self,
        key_id: i32,
    ) -> Option<super::api_key_health::ApiKeyHealth> {
        self.health_checker.get_key_health_status(key_id).await
    }

    /// 获取所有API密钥的健康状态
    pub async fn get_all_health_status(&self) -> HashMap<i32, super::api_key_health::ApiKeyHealth> {
        self.health_checker.get_all_health_status().await
    }

    /// 启动健康检查服务
    pub async fn start_health_checking(&self) -> Result<()> {
        self.health_checker
            .start()
            .await
            .map_err(|e| ProxyError::internal(format!("Failed to start health checker: {e}")))
    }

    /// 停止健康检查服务
    pub async fn stop_health_checking(&self) -> Result<()> {
        self.health_checker
            .stop()
            .await
            .map_err(|e| ProxyError::internal(format!("Failed to stop health checker: {e}")))
    }

    /// 获取密钥池统计信息
    pub async fn get_pool_stats(&self) -> PoolStats {
        let pools = self.key_pools.read().await;
        let selectors = self.selectors.read().await;
        let healthy_key_ids = self.health_checker.get_healthy_keys().await;
        let all_health_status = self.health_checker.get_all_health_status().await;

        PoolStats {
            cached_pools: pools.len(),
            total_keys: pools.values().map(std::vec::Vec::len).sum(),
            active_selectors: selectors.len(),
            available_strategies: vec![
                SchedulingStrategy::RoundRobin,
                SchedulingStrategy::Weighted,
                SchedulingStrategy::HealthBest,
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
    #[must_use]
    pub fn get_credential(&self) -> &str {
        &self.credential.credential
    }

    /// `检查凭证是否是OAuth` token
    #[must_use]
    pub const fn is_oauth_token(&self) -> bool {
        matches!(
            self.credential.auth_type,
            AuthCredentialType::OAuthToken { .. }
        )
    }

    /// 检查凭证是否刚刚刷新过
    #[must_use]
    pub const fn is_refreshed(&self) -> bool {
        self.credential.refreshed
    }

    /// 获取选中的密钥ID
    #[must_use]
    pub const fn get_key_id(&self) -> i32 {
        self.selection_result.selected_key.id
    }

    /// 获取选中的密钥名称
    #[must_use]
    pub fn get_key_name(&self) -> &str {
        &self.selection_result.selected_key.name
    }

    /// 获取用户ID
    #[must_use]
    pub const fn get_user_id(&self) -> i32 {
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
