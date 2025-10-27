//! # API密钥池管理器
//!
//! 专门管理用户API密钥池的选择和调度，替代传统的负载均衡器概念

use super::algorithms::{ApiKeySelectionResult, ApiKeySelector, SelectionContext};
use super::api_key_health::ApiKeyHealthService;
use super::types::{ApiKeyHealthStatus, SchedulingStrategy};
use crate::auth::{ApiKeySelectService, types::AuthStatus};
use crate::error::{ProxyError, Result};
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, lerror, linfo, lwarn};
use entity::user_provider_keys;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// API 密钥池服务
/// 职责：管理用户的 API 密钥池，根据策略选择合适的密钥，并集成健康检查与 OAuth 智能刷新
pub struct ApiKeySchedulerService {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 选择器缓存
    selectors: tokio::sync::RwLock<HashMap<SchedulingStrategy, Arc<dyn ApiKeySelector>>>,
    /// API 密钥健康检查器
    api_key_health_service: Arc<ApiKeyHealthService>,
    /// 智能 API 密钥提供者（支持 OAuth token 刷新）
    api_key_select_service: tokio::sync::RwLock<Option<Arc<ApiKeySelectService>>>,
    /// 是否已完成启动预热
    ready: AtomicBool,
}

impl ApiKeySchedulerService {
    /// 创建新的API密钥池管理器
    #[must_use]
    pub fn new(
        db: Arc<DatabaseConnection>,
        api_key_health_service: Arc<ApiKeyHealthService>,
    ) -> Self {
        Self {
            db,
            selectors: tokio::sync::RwLock::new(HashMap::new()),
            api_key_health_service,
            api_key_select_service: tokio::sync::RwLock::new(None),
            ready: AtomicBool::new(false),
        }
    }

    /// 设置智能密钥提供者
    pub async fn set_smart_provider(&self, smart_provider: Arc<ApiKeySelectService>) {
        let mut guard = self.api_key_select_service.write().await;
        *guard = Some(smart_provider);
    }

    /// 启动并预热密钥池服务
    pub async fn start(&self) -> Result<()> {
        self.api_key_health_service
            .start()
            .await
            .map_err(|e| ProxyError::internal_with_source("Failed to start health checker", e))?;
        self.ready.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// 停止健康检查服务
    pub async fn stop(&self) -> Result<()> {
        self.api_key_health_service
            .stop()
            .await
            .map_err(|e| ProxyError::internal_with_source("Failed to stop health checker", e))?;
        self.ready.store(false, Ordering::SeqCst);
        Ok(())
    }

    /// 当前服务是否已准备就绪
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    /// 获取健康状态缓存的引用
    #[must_use]
    pub fn health_status_cache(
        &self,
    ) -> Arc<tokio::sync::RwLock<HashMap<i32, super::api_key_health::ApiKeyHealth>>> {
        self.api_key_health_service.get_health_status_cache()
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
            LogComponent::KeyPool,
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
            LogComponent::KeyPool,
            "keys_for_selection_count",
            "Final number of keys passed to the selection algorithm",
            count = keys_to_use.len()
        );

        let scheduling_strategy = Self::resolve_strategy(service_api);
        let selector = self.get_selector(scheduling_strategy).await;

        selector.select_key(keys_to_use, context).await
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
        key.auth_status.as_deref().is_none_or(|auth_status| {
            let status = AuthStatus::from(auth_status);
            Self::auth_status_allows_usage(key, &status)
        })
    }

    #[allow(clippy::cognitive_complexity)]
    fn auth_status_allows_usage(key: &user_provider_keys::Model, status: &AuthStatus) -> bool {
        match *status {
            AuthStatus::Authorized => true,
            AuthStatus::Pending => {
                ldebug!(
                    "system",
                    LogStage::Scheduling,
                    LogComponent::KeyPool,
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
                    LogComponent::KeyPool,
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
                    LogComponent::KeyPool,
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
                    LogComponent::KeyPool,
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
        if let Some(expires_at) = key.expires_at.as_ref()
            && now >= expires_at
        {
            ldebug!(
                "system",
                LogStage::Scheduling,
                LogComponent::KeyPool,
                "key_expired",
                "API key has expired, skipping",
                key_id = key.id,
                key_name = %key.name,
                expires_at = %expires_at,
            );
            return false;
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
                    LogComponent::KeyPool,
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
                    LogComponent::KeyPool,
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
                    LogComponent::KeyPool,
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
                    LogComponent::KeyPool,
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
            LogComponent::KeyPool,
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
            LogComponent::KeyPool,
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
            LogComponent::KeyPool,
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
            LogComponent::KeyPool,
            "healthy_keys_count",
            "Keys remaining after health filter",
            count = healthy_keys.len()
        );

        if healthy_keys.is_empty() {
            lwarn!(
                "system",
                LogStage::Scheduling,
                LogComponent::KeyPool,
                "no_healthy_keys",
                "No healthy API keys available, using all keys in degraded mode",
                service_api_id = service_api.id,
                total_keys = user_keys.len(),
            );
        } else if healthy_keys.len() != user_keys.len() {
            ldebug!(
                "system",
                LogStage::Scheduling,
                LogComponent::KeyPool,
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
                LogComponent::KeyPool,
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
        let healthy_key_ids = self.api_key_health_service.get_healthy_keys().await;

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
                        LogComponent::KeyPool,
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

    /// 手动标记API密钥为不健康
    pub async fn mark_key_unhealthy(&self, key_id: i32, reason: String) -> Result<()> {
        self.api_key_health_service
            .mark_key_unhealthy(key_id, reason)
            .await
            .map_err(|e| ProxyError::internal_with_source("Failed to mark key unhealthy", e))
    }

    /// 获取API密钥的健康状态
    pub async fn get_key_health_status(
        &self,
        key_id: i32,
    ) -> Option<super::api_key_health::ApiKeyHealth> {
        self.api_key_health_service
            .get_key_health_status(key_id)
            .await
    }

    /// 处理新增密钥：拉取最新信息并立即执行一次健康检测
    pub async fn register_new_key(&self, key_id: i32) -> Result<()> {
        let key = self
            .load_key_model(key_id)
            .await?
            .ok_or_else(|| ProxyError::internal(format!("Provider key {key_id} not found")))?;
        self.api_key_health_service
            .check_api_key(&key)
            .await
            .map_err(|e| ProxyError::internal_with_source("Failed to register provider key", e))?;

        linfo!(
            "system",
            LogStage::HealthCheck,
            LogComponent::KeyPool,
            "provider_key_registered",
            "Registered new provider key for health tracking",
            key_id = key_id,
            provider_type_id = key.provider_type_id
        );

        Ok(())
    }

    /// 处理密钥更新：重新获取信息并刷新健康状态
    pub async fn refresh_key(&self, key_id: i32) -> Result<()> {
        let key = self
            .load_key_model(key_id)
            .await?
            .ok_or_else(|| ProxyError::internal(format!("Provider key {key_id} not found")))?;
        self.api_key_health_service
            .check_api_key(&key)
            .await
            .map_err(|e| ProxyError::internal_with_source("Failed to refresh provider key", e))?;

        linfo!(
            "system",
            LogStage::HealthCheck,
            LogComponent::KeyPool,
            "provider_key_refreshed",
            "Refreshed provider key after management update",
            key_id = key_id,
            provider_type_id = key.provider_type_id
        );

        Ok(())
    }

    /// 处理密钥删除：移除健康状态（延迟验证会自动处理限流任务取消）
    pub async fn remove_key(&self, key_id: i32) -> Result<()> {
        // 注意：不再需要显式取消限流重置任务
        // 延迟验证机制会在任务执行时检查密钥是否存在，自动跳过已删除的密钥

        // 刷新健康状态缓存
        self.api_key_health_service
            .load_health_status_from_database()
            .await
            .map_err(|e| {
                ProxyError::internal_with_source("Failed to refresh health status cache", e)
            })?;

        linfo!(
            "system",
            LogStage::HealthCheck,
            LogComponent::KeyPool,
            "provider_key_removed",
            "Removed provider key from health tracking",
            key_id = key_id
        );

        Ok(())
    }

    async fn load_key_model(&self, key_id: i32) -> Result<Option<user_provider_keys::Model>> {
        entity::user_provider_keys::Entity::find_by_id(key_id)
            .one(&*self.db)
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "load_key_model_failed",
                    &format!("Failed to load provider key {key_id}: {err}")
                );
                ProxyError::internal("Failed to load provider key")
            })
    }

    #[must_use]
    pub const fn health_checker(&self) -> &Arc<ApiKeyHealthService> {
        &self.api_key_health_service
    }
}

impl std::fmt::Debug for ApiKeySchedulerService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeySchedulerService")
            .field("db", &"<Arc<DatabaseConnection>>")
            .field("selectors", &"<async>")
            .field("smart_provider", &"<async>")
            .field("health_checker", &"<opaque>")
            .field("ready", &self.ready.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}
