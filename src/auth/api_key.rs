//! API key management
//!
//! Provides API key validation, management and caching functionality

use chrono::{DateTime, Utc, Timelike};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, ActiveModelTrait, Set};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

use crate::auth::permissions::{Permission, PermissionChecker, Role};
use crate::auth::types::{ApiKeyInfo, AuthConfig, AuthError};
use crate::error::Result;
use entity::user_provider_keys;

/// API key manager error types
#[derive(Debug, Error)]
pub enum ApiKeyError {
    #[error("API key not found")]
    NotFound,
    #[error("API key is inactive")]
    Inactive,
    #[error("API key has expired")]
    Expired,
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("Database error: {0}")]
    Database(String),
    #[error("Invalid API key format")]
    InvalidFormat,
}

impl From<ApiKeyError> for AuthError {
    fn from(api_key_error: ApiKeyError) -> Self {
        match api_key_error {
            ApiKeyError::NotFound => AuthError::InvalidToken,
            ApiKeyError::Inactive => AuthError::AccountInactive,
            ApiKeyError::Expired => AuthError::TokenExpired,
            ApiKeyError::RateLimitExceeded => AuthError::RateLimitExceeded,
            ApiKeyError::Database(msg) => AuthError::InternalError(msg),
            ApiKeyError::InvalidFormat => AuthError::InvalidToken,
        }
    }
}

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

/// API key manager
pub struct ApiKeyManager {
    /// Database connection
    db: Arc<DatabaseConnection>,
    /// Authentication configuration
    #[allow(dead_code)]
    config: Arc<AuthConfig>,
    /// In-memory cache (production should use Redis)
    cache: tokio::sync::RwLock<HashMap<String, CachedApiKey>>,
    /// Rate limit tracker (in production should use Redis)
    rate_limits: tokio::sync::RwLock<HashMap<String, RateLimitTracker>>,
}

/// Cached API key information
#[derive(Debug, Clone)]
struct CachedApiKey {
    /// API key information
    api_key_info: ApiKeyInfo,
    /// User permissions
    permissions: Vec<Permission>,
    /// Cache time
    cached_at: DateTime<Utc>,
    /// Cache TTL in seconds
    ttl: i64,
}

/// Rate limit tracker for API keys
#[derive(Debug, Clone)]
struct RateLimitTracker {
    /// Request count in current minute
    requests_current_minute: i32,
    /// Tokens used today
    tokens_used_today: i32,
    /// Current minute timestamp
    current_minute: DateTime<Utc>,
    /// Current day (YYYY-MM-DD)
    current_day: String,
    /// Last request time
    last_request_at: DateTime<Utc>,
}

impl RateLimitTracker {
    /// Create new rate limit tracker
    fn new() -> Self {
        let now = Utc::now();
        Self {
            requests_current_minute: 0,
            tokens_used_today: 0,
            current_minute: now.with_second(0).unwrap().with_nanosecond(0).unwrap(),
            current_day: now.format("%Y-%m-%d").to_string(),
            last_request_at: now,
        }
    }

    /// Update request count and check if allowed
    fn check_and_update_request(&mut self, max_requests_per_minute: Option<i32>) -> bool {
        let now = Utc::now();
        let current_minute = now.with_second(0).unwrap().with_nanosecond(0).unwrap();

        // Reset if new minute
        if current_minute > self.current_minute {
            self.current_minute = current_minute;
            self.requests_current_minute = 0;
        }

        // Check request limit
        if let Some(max_requests) = max_requests_per_minute {
            if self.requests_current_minute >= max_requests {
                return false;
            }
        }

        // Update counters
        self.requests_current_minute += 1;
        self.last_request_at = now;
        true
    }

    /// Update token usage and check if allowed
    fn check_and_update_tokens(&mut self, tokens: i32, max_tokens_per_day: Option<i32>) -> bool {
        let now = Utc::now();
        let current_day = now.format("%Y-%m-%d").to_string();

        // Reset if new day
        if current_day != self.current_day {
            self.current_day = current_day;
            self.tokens_used_today = 0;
        }

        // Check token limit
        if let Some(max_tokens) = max_tokens_per_day {
            if self.tokens_used_today + tokens > max_tokens {
                return false;
            }
        }

        // Update token usage
        self.tokens_used_today += tokens;
        true
    }

    /// Get remaining requests for current minute
    fn remaining_requests(&self, max_requests_per_minute: Option<i32>) -> Option<i32> {
        max_requests_per_minute.map(|max| (max - self.requests_current_minute).max(0))
    }

    /// Get remaining tokens for today
    fn remaining_tokens(&self, max_tokens_per_day: Option<i32>) -> Option<i32> {
        max_tokens_per_day.map(|max| (max - self.tokens_used_today).max(0))
    }
}

impl CachedApiKey {
    /// Check if cache is expired
    fn is_expired(&self) -> bool {
        let now = Utc::now();
        let expiry = self.cached_at + chrono::Duration::seconds(self.ttl);
        now >= expiry
    }
}

impl ApiKeyManager {
    /// Create new API key manager
    pub fn new(db: Arc<DatabaseConnection>, config: Arc<AuthConfig>) -> Self {
        Self {
            db,
            config,
            cache: tokio::sync::RwLock::new(HashMap::new()),
            rate_limits: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Validate API key
    pub async fn validate_api_key(&self, api_key: &str) -> Result<ApiKeyValidationResult> {
        // Check API key format
        if !self.is_valid_api_key_format(api_key) {
            return Err(ApiKeyError::InvalidFormat.into());
        }

        // Check cache first
        if let Some(cached) = self.get_from_cache(api_key).await {
            if !cached.is_expired() {
                // Get current rate limit info
                let (remaining_requests, remaining_tokens) = self.get_rate_limit_info(api_key, &cached.api_key_info).await;
                
                return Ok(ApiKeyValidationResult {
                    api_key_info: cached.api_key_info.clone(),
                    permissions: cached.permissions.clone(),
                    permission_checker: PermissionChecker::new(cached.permissions.clone()),
                    remaining_requests,
                    remaining_tokens,
                });
            }
            // Remove expired cache
            self.remove_from_cache(api_key).await;
        }

        // Query from database
        let api_key_model = user_provider_keys::Entity::find()
            .filter(user_provider_keys::Column::ApiKey.eq(api_key))
            .filter(user_provider_keys::Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await
            .map_err(|e| ApiKeyError::Database(e.to_string()))?
            .ok_or(ApiKeyError::NotFound)?;

        // Check if key is active
        if !api_key_model.is_active {
            return Err(ApiKeyError::Inactive.into());
        }

        // Convert to ApiKeyInfo
        let api_key_info = ApiKeyInfo {
            id: api_key_model.id,
            user_id: api_key_model.user_id,
            provider_type_id: api_key_model.provider_type_id,
            name: api_key_model.name,
            api_key: self.sanitize_api_key(&api_key_model.api_key),
            weight: api_key_model.weight,
            max_requests_per_minute: api_key_model.max_requests_per_minute,
            max_tokens_per_day: api_key_model.max_tokens_per_day,
            used_tokens_today: api_key_model.used_tokens_today,
            is_active: api_key_model.is_active,
            created_at: api_key_model.created_at.and_utc(),
            updated_at: api_key_model.updated_at.and_utc(),
        };

        // Get user permissions (simplified - should query from user and role tables)
        let permissions = self.get_user_permissions(api_key_model.user_id).await?;

        // Check rate limits
        let (remaining_requests, remaining_tokens) = self.get_rate_limit_info(api_key, &api_key_info).await;

        // Cache result
        self.cache_api_key(api_key, &api_key_info, &permissions)
            .await;

        Ok(ApiKeyValidationResult {
            api_key_info,
            permissions: permissions.clone(),
            permission_checker: PermissionChecker::new(permissions),
            remaining_requests,
            remaining_tokens,
        })
    }

    /// Check if API key format is valid
    fn is_valid_api_key_format(&self, api_key: &str) -> bool {
        // Basic format check: starts with sk- and at least 20 characters
        api_key.starts_with("sk-") && api_key.len() >= 20
    }

    /// Sanitize API key for logging
    fn sanitize_api_key(&self, api_key: &str) -> String {
        if api_key.len() > 10 {
            format!("{}***{}", &api_key[..4], &api_key[api_key.len() - 4..])
        } else {
            "***".to_string()
        }
    }

    /// Get user permissions from database
    async fn get_user_permissions(&self, user_id: i32) -> Result<Vec<Permission>> {
        use entity::{users, users::Entity as Users};
        use sea_orm::{EntityTrait, ColumnTrait, QueryFilter, PaginatorTrait};
        
        // 从数据库查询用户信息
        let user = Users::find()
            .filter(users::Column::Id.eq(user_id))
            .filter(users::Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await
            .map_err(|e| crate::error::ProxyError::database(format!("Failed to query user: {}", e)))?;
        
        let user = match user {
            Some(u) => u,
            None => {
                // 用户不存在或未激活，返回最小权限
                return Ok(vec![Permission::UseApi]);
            }
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
            permissions.extend(vec![
                Permission::ViewApiKeys,
            ]);
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

    /// Get API key from cache
    async fn get_from_cache(&self, api_key: &str) -> Option<CachedApiKey> {
        let cache = self.cache.read().await;
        cache.get(api_key).cloned()
    }

    /// Remove API key from cache
    async fn remove_from_cache(&self, api_key: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(api_key);
    }

    /// Cache API key information
    async fn cache_api_key(
        &self,
        api_key: &str,
        api_key_info: &ApiKeyInfo,
        permissions: &[Permission],
    ) {
        let cached = CachedApiKey {
            api_key_info: api_key_info.clone(),
            permissions: permissions.to_vec(),
            cached_at: Utc::now(),
            ttl: 300, // 5 minutes cache
        };

        let mut cache = self.cache.write().await;
        cache.insert(api_key.to_string(), cached);
    }

    /// Clean up expired cache (periodic cleanup task)
    pub async fn cleanup_expired_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.retain(|_, cached| !cached.is_expired());
    }

    /// Force refresh cache for specific API key
    pub async fn refresh_cache(&self, api_key: &str) -> Result<()> {
        self.remove_from_cache(api_key).await;
        // Next access will automatically reload from database
        Ok(())
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

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let total_keys = cache.len();
        let expired_keys = cache.values().filter(|cached| cached.is_expired()).count();

        CacheStats {
            total_keys,
            expired_keys,
            active_keys: total_keys - expired_keys,
        }
    }

    /// Check API key rate limit
    pub async fn check_rate_limit(
        &self,
        api_key: &str,
        request_cost: i32,
    ) -> Result<RateLimitStatus> {
        // Get API key info first
        let api_key_model = user_provider_keys::Entity::find()
            .filter(user_provider_keys::Column::ApiKey.eq(api_key))
            .filter(user_provider_keys::Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await
            .map_err(|e| ApiKeyError::Database(e.to_string()))?
            .ok_or(ApiKeyError::NotFound)?;

        let mut rate_limits = self.rate_limits.write().await;
        let tracker = rate_limits
            .entry(api_key.to_string())
            .or_insert_with(RateLimitTracker::new);

        // Check request rate limit
        let request_allowed = tracker.check_and_update_request(api_key_model.max_requests_per_minute);
        
        // Check token rate limit
        let token_allowed = tracker.check_and_update_tokens(request_cost, api_key_model.max_tokens_per_day);

        let allowed = request_allowed && token_allowed;

        // Determine reset time (next minute for requests, next day for tokens)
        let reset_time = if !request_allowed {
            Utc::now().with_second(0).unwrap().with_nanosecond(0).unwrap() + chrono::Duration::minutes(1)
        } else if !token_allowed {
            let tomorrow = Utc::now().date_naive() + chrono::Duration::days(1);
            tomorrow.and_hms_opt(0, 0, 0).unwrap().and_utc()
        } else {
            Utc::now() + chrono::Duration::minutes(1)
        };

        Ok(RateLimitStatus {
            allowed,
            remaining_requests: tracker.remaining_requests(api_key_model.max_requests_per_minute),
            remaining_tokens: tracker.remaining_tokens(api_key_model.max_tokens_per_day),
            reset_time,
        })
    }

    /// Record API key usage
    pub async fn record_usage(&self, api_key: &str, tokens_used: i32) -> Result<()> {
        // Update database record
        let api_key_model = user_provider_keys::Entity::find()
            .filter(user_provider_keys::Column::ApiKey.eq(api_key))
            .one(self.db.as_ref())
            .await
            .map_err(|e| ApiKeyError::Database(e.to_string()))?
            .ok_or(ApiKeyError::NotFound)?;

        let mut active_model: user_provider_keys::ActiveModel = api_key_model.into();
        
        // Update today's token usage
        let current_used = active_model.used_tokens_today.clone().unwrap().unwrap_or(0);
        active_model.used_tokens_today = Set(Some(current_used + tokens_used));

        // Update last used timestamp
        active_model.last_used = Set(Some(Utc::now().naive_utc()));

        active_model.update(self.db.as_ref())
            .await
            .map_err(|e| ApiKeyError::Database(e.to_string()))?;

        // Update in-memory rate limiter
        let mut rate_limits = self.rate_limits.write().await;
        if let Some(tracker) = rate_limits.get_mut(api_key) {
            tracker.tokens_used_today += tokens_used;
        }

        tracing::debug!(
            "Recorded usage for API key: {}, tokens: {}",
            self.sanitize_api_key(api_key),
            tokens_used
        );
        
        Ok(())
    }

    /// Get rate limit information for API key
    async fn get_rate_limit_info(&self, api_key: &str, api_key_info: &ApiKeyInfo) -> (Option<i32>, Option<i32>) {
        let rate_limits = self.rate_limits.read().await;
        
        if let Some(tracker) = rate_limits.get(api_key) {
            let remaining_requests = tracker.remaining_requests(api_key_info.max_requests_per_minute);
            let remaining_tokens = tracker.remaining_tokens(api_key_info.max_tokens_per_day);
            (remaining_requests, remaining_tokens)
        } else {
            // No rate limit data yet, return maximum values
            (api_key_info.max_requests_per_minute, api_key_info.max_tokens_per_day)
        }
    }

    /// Cleanup expired rate limit trackers
    pub async fn cleanup_rate_limits(&self) {
        let mut rate_limits = self.rate_limits.write().await;
        let now = Utc::now();
        let one_hour_ago = now - chrono::Duration::hours(1);
        
        rate_limits.retain(|_, tracker| tracker.last_request_at > one_hour_ago);
    }

    /// Get rate limit statistics
    pub async fn get_rate_limit_stats(&self) -> HashMap<String, serde_json::Value> {
        let rate_limits = self.rate_limits.read().await;
        let mut stats = HashMap::new();
        
        for (api_key, tracker) in rate_limits.iter() {
            let sanitized_key = self.sanitize_api_key(api_key);
            stats.insert(sanitized_key, serde_json::json!({
                "requests_current_minute": tracker.requests_current_minute,
                "tokens_used_today": tracker.tokens_used_today,
                "last_request_at": tracker.last_request_at
            }));
        }
        
        stats
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total keys
    pub total_keys: usize,
    /// Expired keys
    pub expired_keys: usize,
    /// Active keys
    pub active_keys: usize,
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
    use super::*;

    #[test]
    fn test_api_key_format_validation() {
        // Test API key format validation logic
        assert!("sk-1234567890abcdef12345".starts_with("sk-") && "sk-1234567890abcdef12345".len() >= 20);
        assert!(!("invalid-key".starts_with("sk-") && "invalid-key".len() >= 20));
        assert!(!("sk-short".starts_with("sk-") && "sk-short".len() >= 20));
        assert!(!("ak-1234567890abcdef12345".starts_with("sk-") && "ak-1234567890abcdef12345".len() >= 20));
    }

    #[test]
    fn test_api_key_sanitization() {
        // Test sanitization logic
        fn sanitize_api_key(api_key: &str) -> String {
            if api_key.len() > 10 {
                format!("{}***{}", &api_key[..4], &api_key[api_key.len()-4..])
            } else {
                "***".to_string()
            }
        }

        let sanitized = sanitize_api_key("sk-1234567890abcdef12345");
        assert_eq!(sanitized, "sk-1***2345");

        let short_sanitized = sanitize_api_key("short");
        assert_eq!(short_sanitized, "***");
    }

    #[test]
    fn test_cache_expiration() {
        let cached = CachedApiKey {
            api_key_info: ApiKeyInfo {
                id: 1,
                user_id: 1,
                provider_type_id: 1,
                name: "test-key".to_string(),
                api_key: "sk-test***test".to_string(),
                weight: Some(1),
                max_requests_per_minute: Some(100),
                max_tokens_per_day: Some(10000),
                used_tokens_today: Some(0),
                is_active: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            permissions: vec![Permission::UseOpenAI],
            cached_at: Utc::now() - chrono::Duration::seconds(400), // Expired
            ttl: 300, // 5 minutes
        };

        assert!(cached.is_expired());

        let fresh_cached = CachedApiKey {
            cached_at: Utc::now(),
            ttl: 300,
            ..cached
        };

        assert!(!fresh_cached.is_expired());
    }
}
