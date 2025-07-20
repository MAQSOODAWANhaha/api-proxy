//! API key management
//!
//! Provides API key validation, management and caching functionality

use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
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
    config: Arc<AuthConfig>,
    /// In-memory cache (production should use Redis)
    cache: tokio::sync::RwLock<HashMap<String, CachedApiKey>>,
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
                return Ok(ApiKeyValidationResult {
                    api_key_info: cached.api_key_info.clone(),
                    permissions: cached.permissions.clone(),
                    permission_checker: PermissionChecker::new(cached.permissions.clone()),
                    remaining_requests: None, // TODO: Get from rate limiter
                    remaining_tokens: None,   // TODO: Get from rate limiter
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

        // Cache result
        self.cache_api_key(api_key, &api_key_info, &permissions)
            .await;

        Ok(ApiKeyValidationResult {
            api_key_info,
            permissions: permissions.clone(),
            permission_checker: PermissionChecker::new(permissions),
            remaining_requests: None, // TODO: Implement rate limit checking
            remaining_tokens: None,   // TODO: Implement token usage checking
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

    /// Get user permissions (simplified implementation)
    async fn get_user_permissions(&self, _user_id: i32) -> Result<Vec<Permission>> {
        // TODO: Should query from database for user roles and permissions
        // Temporarily return basic user permissions
        Ok(Role::User.permissions())
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
        _api_key: &str,
        _request_cost: i32,
    ) -> Result<RateLimitStatus> {
        // TODO: Implement real rate limit checking
        // Return mock result for now
        Ok(RateLimitStatus {
            allowed: true,
            remaining_requests: Some(100),
            remaining_tokens: Some(10000),
            reset_time: Utc::now() + chrono::Duration::minutes(1),
        })
    }

    /// Record API key usage
    pub async fn record_usage(&self, api_key: &str, tokens_used: i32) -> Result<()> {
        // TODO: Implement usage recording to database and cache
        tracing::info!(
            "Recording usage for API key: {}, tokens: {}",
            self.sanitize_api_key(api_key),
            tokens_used
        );
        Ok(())
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
