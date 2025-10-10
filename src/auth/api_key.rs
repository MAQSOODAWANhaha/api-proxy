//! # API密钥数据库操作与缓存工具
//!
//! 提供统一的API密钥数据库查询、缓存管理和格式验证功能
//! 供代理端认证和管理端认证共同使用

use crate::config::CacheConfig;
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, lwarn};
use chrono::{DateTime, Timelike, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::auth::cache_strategy::{AuthCacheKey, UnifiedAuthCacheManager, hash_token};
use crate::auth::permissions::{Permission, PermissionChecker, Role};
use crate::auth::rate_limit_dist::DistributedRateLimiter;
use crate::auth::types::{ApiKeyInfo, AuthConfig};
use crate::cache::CacheManager;
use crate::error::Result;
use entity::user_provider_keys;

/// API key validation result
#[derive(Debug, Clone)]
pub struct ApiKeyValidationResult {
    /// API key information
    pub api_key_info: ApiKeyInfo,
    /// User permissions
    pub permissions: Vec<Permission>,
    /// Permission checker
    pub permission_checker: PermissionChecker,
    /// Remaining requests per minute
    pub remaining_requests: Option<i32>,
    /// Remaining tokens per day
    pub remaining_tokens: Option<i32>,
}

/// Data structure for caching API key information.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiKeyCacheData {
    api_key_info: ApiKeyInfo,
    permissions: Vec<Permission>,
}

/// API key manager
pub struct ApiKeyManager {
    /// Database connection
    db: Arc<DatabaseConnection>,
    /// Authentication configuration
    #[allow(dead_code)]
    config: Arc<AuthConfig>,
    /// Unified cache manager
    cache: Arc<UnifiedAuthCacheManager>,
    /// Distributed rate limiter
    limiter: Arc<DistributedRateLimiter>,
    /// Raw cache manager for custom operations
    raw_cache: Arc<CacheManager>,
}

impl ApiKeyManager {
    /// Create new API key manager
    pub fn new(
        db: Arc<DatabaseConnection>,
        auth_config: Arc<AuthConfig>,
        cache_manager: Arc<CacheManager>,
        cache_config: Arc<CacheConfig>,
    ) -> Self {
        let auth_cache_manager = Arc::new(UnifiedAuthCacheManager::new(
            cache_manager.clone(),
            auth_config.clone(),
            cache_config,
        ));
        let rate_limiter = Arc::new(DistributedRateLimiter::new(cache_manager.clone()));
        Self {
            db,
            config: auth_config,
            cache: auth_cache_manager,
            limiter: rate_limiter,
            raw_cache: cache_manager,
        }
    }

    /// Validate API key
    pub async fn validate_api_key(&self, api_key: &str) -> Result<ApiKeyValidationResult> {
        // Check API key format
        if !self.is_valid_api_key_format(api_key) {
            return Err(crate::proxy_err!(auth, "API 密钥格式无效"));
        }

        // Check cache first
        let cache_key = AuthCacheKey::ApiKeyAuth(hash_token(api_key));
        if let Some(cached) = self
            .cache
            .get_cached_auth_result::<ApiKeyCacheData>(&cache_key)
            .await
        {
            // Get current rate limit info
            let (remaining_requests, remaining_tokens) = self
                .get_rate_limit_info(api_key, &cached.api_key_info)
                .await?;

            return Ok(ApiKeyValidationResult {
                api_key_info: cached.api_key_info.clone(),
                permissions: cached.permissions.clone(),
                permission_checker: PermissionChecker::new(cached.permissions.clone()),
                remaining_requests,
                remaining_tokens,
            });
        }

        // Query from database
        let api_key_model = self
            .find_api_key_record(api_key)
            .await?
            .ok_or_else(|| crate::proxy_err!(auth, "API 密钥不存在"))?;

        // Check if key is active
        if !api_key_model.is_active {
            return Err(crate::proxy_err!(auth, "API 密钥未激活"));
        }

        // Convert to ApiKeyInfo
        let api_key_info = ApiKeyInfo {
            id: api_key_model.id,
            user_id: api_key_model.user_id,
            provider_type_id: api_key_model.provider_type_id,
            auth_type: api_key_model.auth_type.clone(),
            name: api_key_model.name,
            api_key: Self::sanitize_api_key(&api_key_model.api_key),
            weight: api_key_model.weight,
            max_requests_per_minute: api_key_model.max_requests_per_minute,
            max_tokens_prompt_per_minute: api_key_model.max_tokens_prompt_per_minute,
            max_requests_per_day: api_key_model.max_requests_per_day,
            is_active: api_key_model.is_active,
            created_at: api_key_model.created_at.and_utc(),
            updated_at: api_key_model.updated_at.and_utc(),
        };

        // Get user permissions
        let permissions = self.get_user_permissions(api_key_model.user_id).await?;

        // Check rate limits
        let (remaining_requests, remaining_tokens) =
            self.get_rate_limit_info(api_key, &api_key_info).await?;

        // Cache result
        let cache_data = ApiKeyCacheData {
            api_key_info: api_key_info.clone(),
            permissions: permissions.clone(),
        };
        // Use a reasonable TTL for API key info, e.g., 5 minutes
        if let Err(e) = self.cache.cache_auth_result(&cache_key, &cache_data).await {
            lwarn!(
                "system",
                LogStage::Cache,
                LogComponent::ApiKey,
                "cache_fail",
                &format!("Failed to cache API key info: {e}")
            );
        }

        Ok(ApiKeyValidationResult {
            api_key_info,
            permissions: permissions.clone(),
            permission_checker: PermissionChecker::new(permissions),
            remaining_requests,
            remaining_tokens,
        })
    }

    /// Check if API key format is valid
    #[must_use]
    pub fn is_valid_api_key_format(&self, api_key: &str) -> bool {
        // Basic format check: starts with sk- and at least 20 characters
        api_key.starts_with("sk-") && api_key.len() >= 20
    }

    /// Sanitize API key for logging（委托统一工具，避免重复实现）
    fn sanitize_api_key(api_key: &str) -> String {
        crate::auth::AuthUtils::sanitize_api_key(api_key)
    }

    /// Get user permissions from database
    async fn get_user_permissions(&self, user_id: i32) -> Result<Vec<Permission>> {
        use entity::{users, users::Entity as Users};
        use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

        // 从数据库查询用户信息
        let user = Users::find()
            .filter(users::Column::Id.eq(user_id))
            .filter(users::Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await
            .map_err(|e| {
                crate::error::ProxyError::database(format!("Failed to query user: {e}"))
            })?;

        let Some(user) = user else {
            // 用户不存在或未激活，返回最小权限
            return Ok(vec![Permission::UseApi]);
        };

        // 根据用户类型确定权限
        let mut permissions = Vec::new();

        // 基础权限：所有激活用户都有的权限
        permissions.push(Permission::UseApi);

        // 管理员权限
        if user.is_admin {
            permissions.extend(Role::Admin.permissions());
            return Ok(permissions);
        }

        // 检查用户是否有活跃的API密钥（表示是付费用户）
        let api_count = entity::user_service_apis::Entity::find()
            .filter(entity::user_service_apis::Column::UserId.eq(user_id))
            .filter(entity::user_service_apis::Column::IsActive.eq(true))
            .count(self.db.as_ref())
            .await
            .unwrap_or(0);

        if api_count > 0 {
            // 有活跃API密钥的用户，给予更多权限
            permissions.extend(vec![
                Permission::ViewApiKeys,
                Permission::UseOpenAI,
                Permission::UseAnthropic,
                Permission::UseGemini,
            ]);

            // 根据API密钥数量给予不同权限等级
            if api_count >= 5 {
                // 高级用户
                permissions.push(Permission::ViewStatistics);
                permissions.push(Permission::UseAllProviders);
            }
        } else {
            // 没有API密钥的用户，只有基础权限
            permissions.extend(vec![Permission::ViewApiKeys]);
        }

        // 根据用户注册时间给予一些额外权限
        let now = chrono::Utc::now().naive_utc();
        let user_age_days = (now - user.created_at).num_days();

        if user_age_days >= 30 {
            // 注册超过30天的用户，给予查看健康状态权限
            permissions.push(Permission::ViewHealth);
        }

        Ok(permissions)
    }

    /// Force refresh cache for specific API key
    pub async fn refresh_cache(&self, api_key: &str) -> Result<()> {
        let cache_key = AuthCacheKey::ApiKeyAuth(hash_token(api_key));
        self.cache.invalidate_cache(&cache_key).await
    }

    /// Batch validate API keys (for cache preloading)
    pub async fn preload_api_keys(&self, api_keys: &[String]) -> HashMap<String, bool> {
        let mut results = HashMap::new();

        for api_key in api_keys {
            let is_valid = self.validate_api_key(api_key).await.is_ok();
            results.insert(api_key.clone(), is_valid);
        }

        results
    }

    /// Check API key rate limit (requests per minute only)
    #[allow(clippy::cast_possible_truncation)]
    pub async fn check_rate_limit(&self, api_key: &str) -> Result<RateLimitStatus> {
        let api_key_model = self
            .find_api_key_record(api_key)
            .await?
            .ok_or_else(|| crate::proxy_err!(auth, "API 密钥不存在"))?;

        // Check request rate limit
        let rpm_limit = i64::from(api_key_model.max_requests_per_minute.unwrap_or(i32::MAX));
        let rpm_outcome = self
            .limiter
            .check_per_minute(api_key_model.user_id, "proxy", rpm_limit)
            .await
            .map_err(|e| crate::proxy_err!(internal, "Rate limit check failed: {}", e))?;

        // Get current token usage
        let date = chrono::Utc::now().format("%Y%m%d").to_string();
        let token_key = format!("rate_limit:token:{}:{}", api_key_model.id, date);
        let current_tokens: i64 = self
            .raw_cache
            .provider()
            .get(&token_key)
            .await
            .map_err(|e| crate::proxy_err!(internal, "Cache error: {}", e))?
            .unwrap_or(0);

        let remaining_tokens = api_key_model
            .max_requests_per_day
            .map(|max| (i64::from(max) - current_tokens).max(0));

        let reset_time = Utc::now()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap()
            + chrono::Duration::minutes(1);

        Ok(RateLimitStatus {
            allowed: rpm_outcome.allowed,
            remaining_requests: Some((rpm_outcome.limit - rpm_outcome.current).max(0) as i32),
            remaining_tokens: remaining_tokens.map(|t| t as i32),
            reset_time,
        })
    }

    /// Record API key usage (tokens)
    #[allow(clippy::cast_sign_loss)]
    pub async fn record_usage(&self, api_key: &str, tokens_used: i32) -> Result<()> {
        let api_key_model = self
            .find_api_key_record(api_key)
            .await?
            .ok_or_else(|| crate::proxy_err!(auth, "API 密钥不存在"))?;

        // Update database record for `updated_at`
        let mut active_model: user_provider_keys::ActiveModel = api_key_model.clone().into();
        active_model.updated_at = Set(Utc::now().naive_utc());
        active_model
            .update(self.db.as_ref())
            .await
            .map_err(|e| crate::proxy_err!(internal, "Database error: {}", e))?;

        // Update token usage in cache
        if tokens_used > 0 {
            let date = chrono::Utc::now().format("%Y%m%d").to_string();
            let token_key = format!("rate_limit:token:{}:{}", api_key_model.id, date);
            let new_total = self
                .raw_cache
                .provider()
                .incr(&token_key, i64::from(tokens_used))
                .await
                .map_err(|e| crate::proxy_err!(internal, "Cache error: {}", e))?;

            // Set TTL on first increment of the day
            if new_total == i64::from(tokens_used) {
                let now = Utc::now();
                let tomorrow = (now.date_naive() + chrono::Duration::days(1))
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                let ttl = (tomorrow.and_utc() - now).num_seconds().max(60) as u64;
                self.raw_cache
                    .provider()
                    .expire(&token_key, Duration::from_secs(ttl))
                    .await
                    .map_err(|e| crate::proxy_err!(internal, "Cache error: {}", e))?;
            }
        }

        ldebug!(
            "system",
            LogStage::Internal,
            LogComponent::ApiKey,
            "usage_recorded",
                                            &format!("Recorded usage for API key: {}, tokens: {tokens_used}",
                                                Self::sanitize_api_key(api_key),
                                            )        );

        Ok(())
    }

    /// Get rate limit information for API key
    #[allow(clippy::cast_possible_truncation)]
    async fn get_rate_limit_info(
        &self,
        _api_key: &str,
        api_key_info: &ApiKeyInfo,
    ) -> Result<(Option<i32>, Option<i32>)> {
        // Get RPM from distributed limiter
        let rpm_key =
            crate::cache::keys::CacheKeyBuilder::rate_limit(api_key_info.user_id, "proxy").build();
        let current_requests: i64 = self
            .raw_cache
            .provider()
            .get(&rpm_key)
            .await
            .map_err(|e| crate::proxy_err!(internal, "Cache error: {}", e))?
            .unwrap_or(0);
        let remaining_requests = api_key_info
            .max_requests_per_minute
            .map(|max| (i64::from(max) - current_requests).max(0) as i32);

        // Get TPD from cache
        let date = chrono::Utc::now().format("%Y%m%d").to_string();
        let token_key = format!("rate_limit:token:{}:{}", api_key_info.id, date);
        let current_tokens: i64 = self
            .raw_cache
            .provider()
            .get(&token_key)
            .await
            .map_err(|e| crate::proxy_err!(internal, "Cache error: {}", e))?
            .unwrap_or(0);
        let remaining_tokens = api_key_info
            .max_requests_per_day
            .map(|max| (i64::from(max) - current_tokens).max(0) as i32);

        Ok((remaining_requests, remaining_tokens))
    }

    // ==================== 共享数据库操作方法 ====================

    /// 根据API密钥查询数据库记录（不包含认证逻辑）
    ///
    /// 返回原始的数据库记录，供不同认证场景使用
    pub async fn find_api_key_record(
        &self,
        api_key: &str,
    ) -> Result<Option<user_provider_keys::Model>> {
        user_provider_keys::Entity::find()
            .filter(user_provider_keys::Column::ApiKey.eq(api_key))
            .filter(user_provider_keys::Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await
            .map_err(|e| crate::proxy_err!(internal, "Database error: {}", e))
    }

    /// 验证API密钥格式（共享方法）
    #[must_use]
    pub fn validate_api_key_format(&self, api_key: &str) -> bool {
        self.is_valid_api_key_format(api_key)
    }

    /// 清理指定API密钥的缓存（供外部调用）
    pub async fn invalidate_api_key_cache(&self, api_key: &str) {
        let cache_key = AuthCacheKey::ApiKeyAuth(hash_token(api_key));
        let _ = self.cache.invalidate_cache(&cache_key).await;
    }

    /// 获取API密钥基本信息（不含权限和速率限制）
    ///
    /// 用于代理端轻量级认证
    pub async fn get_api_key_info(&self, api_key: &str) -> Result<Option<ApiKeyInfo>> {
        // 检查缓存
        let cache_key = AuthCacheKey::ApiKeyAuth(hash_token(api_key));
        if let Some(cached) = self
            .cache
            .get_cached_auth_result::<ApiKeyCacheData>(&cache_key)
            .await
        {
            return Ok(Some(cached.api_key_info));
        }

        // 查询数据库
        if let Some(record) = self.find_api_key_record(api_key).await? {
            let api_key_info = ApiKeyInfo {
                id: record.id,
                user_id: record.user_id,
                provider_type_id: record.provider_type_id,
                auth_type: record.auth_type.clone(),
                name: record.name,
                api_key: Self::sanitize_api_key(&record.api_key),
                weight: record.weight,
                max_requests_per_minute: record.max_requests_per_minute,
                max_tokens_prompt_per_minute: record.max_tokens_prompt_per_minute,
                max_requests_per_day: record.max_requests_per_day,
                is_active: record.is_active,
                created_at: record.created_at.and_utc(),
                updated_at: record.updated_at.and_utc(),
            };

            // 获取权限并缓存
            let permissions = self.get_user_permissions(record.user_id).await?;
            let cache_data = ApiKeyCacheData {
                api_key_info: api_key_info.clone(),
                permissions,
            };
            if let Err(e) = self.cache.cache_auth_result(&cache_key, &cache_data).await {
                lwarn!(
                    "system",
                    LogStage::Cache,
                    LogComponent::ApiKey,
                    "cache_fail",
                    &format!("Failed to cache API key info: {e}")
                );
            }

            Ok(Some(api_key_info))
        } else {
            Ok(None)
        }
    }

    /// 代理端轻量级API密钥验证
    ///
    /// 只验证密钥存在性和激活状态，不包含权限检查
    pub async fn validate_for_proxy(&self, api_key: &str) -> Result<ApiKeyInfo> {
        if !self.is_valid_api_key_format(api_key) {
            return Err(crate::proxy_err!(auth, "API 密钥格式无效"));
        }

        match self.get_api_key_info(api_key).await? {
            Some(info) => {
                if info.is_active {
                    Ok(info)
                } else {
                    Err(crate::proxy_err!(auth, "API 密钥未激活"))
                }
            }
            None => Err(crate::proxy_err!(auth, "API 密钥不存在")),
        }
    }

    /// 管理端完整API密钥验证（保留原有逻辑）
    ///
    /// 包含权限检查、速率限制等完整功能
    pub async fn validate_for_management(&self, api_key: &str) -> Result<ApiKeyValidationResult> {
        // 使用原有的validate_api_key方法
        self.validate_api_key(api_key).await
    }
}

/// Rate limit status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    /// Whether request is allowed
    pub allowed: bool,
    /// Remaining requests
    pub remaining_requests: Option<i32>,
    /// Remaining tokens
    pub remaining_tokens: Option<i32>,
    /// Reset time
    pub reset_time: DateTime<Utc>,
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_api_key_format_validation() {
        // Test API key format validation logic
        assert!(
            "sk-1234567890abcdef12345".starts_with("sk-") && "sk-1234567890abcdef12345".len() >= 20
        );
        assert!(!("invalid-key".starts_with("sk-") && "invalid-key".len() >= 20));
        assert!(!("sk-short".starts_with("sk-") && "sk-short".len() >= 20));
        assert!(
            !("ak-1234567890abcdef12345".starts_with("sk-")
                && "ak-1234567890abcdef12345".len() >= 20)
        );
    }

    #[test]
    fn test_api_key_sanitization() {
        // Test sanitization logic
        fn sanitize_api_key(api_key: &str) -> String {
            if api_key.len() > 10 {
                format!("{}***{}", &api_key[..4], &api_key[api_key.len() - 4..])
            } else {
                "***".to_string()
            }
        }

        let sanitized = sanitize_api_key("sk-1234567890abcdef12345");
        assert_eq!(sanitized, "sk-1***2345");

        let short_sanitized = sanitize_api_key("short");
        assert_eq!(short_sanitized, "***");
    }
}
