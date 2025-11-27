//! # API密钥池管理器
//!
//! 专门管理用户API密钥池的选择和调度，替代传统的负载均衡器概念

use super::algorithms::{ApiKeySelectionResult, ApiKeySelector, SelectionContext};
use super::api_key_health::ApiKeyHealthService;
use super::types::{ApiKeyHealthStatus, SchedulingStrategy};
use crate::auth::types::AuthStatus;
use crate::error::{Context, Result, key_pool::KeyPoolError};
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, linfo};
use entity::user_provider_keys;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use std::collections::HashMap;
use std::sync::Arc;

/// API 密钥池服务
/// 职责：管理用户的 API 密钥池，根据策略选择合适的密钥，并集成健康检查与 OAuth 智能刷新
pub struct ApiKeySchedulerService {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 选择器缓存
    selectors: tokio::sync::RwLock<HashMap<SchedulingStrategy, Arc<dyn ApiKeySelector>>>,
    /// API 密钥健康检查器
    api_key_health_service: Arc<ApiKeyHealthService>,
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
        }
    }

    #[must_use]
    pub const fn api_key_health_service(&self) -> &Arc<ApiKeyHealthService> {
        &self.api_key_health_service
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
        Self::log_key_limits(&user_keys);

        let keys_to_use = user_keys.as_slice();

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
            .with_context(|| "数据库查询 API Key 列表失败".to_string())?;

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
                return Err(KeyPoolError::InvalidProviderKeysFormat {
                    service_api_id: service_api.id,
                }
                .into());
            }
        };

        if ids.is_empty() {
            return Err(KeyPoolError::NoProviderKeysConfigured {
                service_api_id: service_api.id,
            }
            .into());
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
            return Err(KeyPoolError::NoActiveProviderKeys {
                service_api_id: context.user_service_api_id,
            }
            .into());
        }

        Ok(filtered)
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

    /// 手动标记API密钥为不健康
    pub async fn mark_key_unhealthy(&self, key_id: i32, reason: String) -> Result<()> {
        self.api_key_health_service
            .mark_key_unhealthy(key_id, reason)
            .await
            .context("标记 API key 不健康失败")
    }
}

impl std::fmt::Debug for ApiKeySchedulerService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeySchedulerService")
            .field("db", &"<Arc<DatabaseConnection>>")
            .field("selectors", &"<async>")
            .field("smart_provider", &"<async>")
            .field("health_checker", &"<opaque>")
            .finish_non_exhaustive()
    }
}
