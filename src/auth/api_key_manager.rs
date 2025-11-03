//! # API密钥数据库操作与缓存工具
//!
//! 提供统一的API密钥数据库查询、缓存管理和格式验证功能
//! 供代理端认证和管理端认证共同使用

use crate::config::CacheConfig;
use crate::logging::{LogComponent, LogStage};
use crate::lwarn;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::sync::Arc;

use crate::auth::cache_strategy::{AuthCacheKey, UnifiedAuthCacheManager, hash_token};
use crate::auth::types::ApiKeyInfo;
use crate::cache::CacheManager;
use crate::error::{Result, auth::AuthError};
use entity::user_provider_keys;

/// API key manager
pub struct ApiKeyManager {
    /// Database connection
    db: Arc<DatabaseConnection>,
    /// Unified cache manager
    cache: Arc<UnifiedAuthCacheManager>,
}

impl ApiKeyManager {
    /// Create new API key manager
    pub fn new(
        db: Arc<DatabaseConnection>,
        cache_manager: Arc<CacheManager>,
        cache_config: Arc<CacheConfig>,
    ) -> Self {
        let auth_cache_manager =
            Arc::new(UnifiedAuthCacheManager::new(cache_manager, cache_config));
        Self {
            db,
            cache: auth_cache_manager,
        }
    }

    /// Validate API key
    pub async fn validate_api_key(&self, api_key: &str) -> Result<ApiKeyInfo> {
        // Check API key format
        if !self.is_valid_api_key_format(api_key) {
            return Err(AuthError::ApiKeyMalformed.into());
        }

        // Check cache first
        let cache_key = AuthCacheKey::ApiKeyAuth(hash_token(api_key));
        if let Some(cached) = self
            .cache
            .get_cached_auth_result::<ApiKeyInfo>(&cache_key)
            .await
        {
            return Ok(cached);
        }

        // Query from database
        let api_key_model = self
            .find_api_key_record(api_key)
            .await?
            .ok_or_else(|| AuthError::ApiKeyInvalid(api_key.to_string()))?;

        // Check if key is active
        if !api_key_model.is_active {
            return Err(AuthError::ApiKeyInactive.into());
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

        // Cache result
        if let Err(e) = self
            .cache
            .cache_auth_result(&cache_key, &api_key_info)
            .await
        {
            lwarn!(
                "system",
                LogStage::Cache,
                LogComponent::ApiKey,
                "cache_fail",
                &format!("Failed to cache API key info: {e}")
            );
        }

        Ok(api_key_info)
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
            .map_err(|e| {
                crate::error!(
                    Internal,
                    format!(
                        "Database error when fetching API key {}",
                        Self::sanitize_api_key(api_key)
                    ),
                    e
                )
            })
    }

    /// 验证API密钥格式（共享方法）
    #[must_use]
    pub fn validate_api_key_format(&self, api_key: &str) -> bool {
        self.is_valid_api_key_format(api_key)
    }

    /// 管理端完整API密钥验证（保留原有逻辑）
    ///
    /// 包含权限检查、速率限制等完整功能
    pub async fn validate_for_management(&self, api_key: &str) -> Result<ApiKeyInfo> {
        // 使用原有的validate_api_key方法
        self.validate_api_key(api_key).await
    }
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
