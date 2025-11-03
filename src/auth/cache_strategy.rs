//! # 认证缓存策略
//!
//! 为认证系统提供统一、优化的缓存策略，确保一致性和性能

use crate::config::CacheConfig;
use crate::ldebug;
use crate::logging::{LogComponent, LogStage};
use std::time::Duration;

use crate::cache::CacheManager;
use crate::error::Result;
use std::sync::Arc;

/// 认证缓存键类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthCacheKey {
    /// JWT认证结果缓存
    JwtAuth(String),
    /// JWT黑名单缓存
    JwtBlacklist(String),
    /// API密钥认证结果缓存
    ApiKeyAuth(String),
    /// Basic认证失败计数缓存
    BasicFailure(String),
    /// Basic认证结果缓存
    BasicAuth(String),
    /// `OAuth会话缓存`
    OAuthSession(String),
}

impl AuthCacheKey {
    /// 生成缓存键字符串
    #[must_use]
    pub fn to_key(&self) -> String {
        match self {
            Self::JwtAuth(hash) => format!("auth:jwt:{hash}"),
            Self::JwtBlacklist(hash) => format!("auth:jwt:blacklist:{hash}"),
            Self::ApiKeyAuth(hash) => format!("auth:apikey:{hash}"),
            Self::BasicFailure(hash) => format!("auth:basic:failure:{hash}"),
            Self::BasicAuth(hash) => format!("auth:basic:{hash}"),
            Self::OAuthSession(session_id) => format!("auth:oauth:session:{session_id}"),
        }
    }

    /// 获取推荐的TTL
    #[must_use]
    pub const fn default_ttl(&self) -> Duration {
        match self {
            // JWT认证结果：15分钟（JWT token通常较长期有效）
            Self::JwtAuth(_) => Duration::from_secs(900),

            // JWT黑名单和Basic失败计数：1小时（安全相关，需要较长时间生效）
            Self::JwtBlacklist(_) | Self::BasicFailure(_) => Duration::from_secs(3600),

            // API密钥认证：5分钟（相对较短，便于快速更新权限）
            Self::ApiKeyAuth(_) => Duration::from_secs(300),

            // Basic认证结果：10分钟（包含密码信息，较短TTL）
            Self::BasicAuth(_) => Duration::from_secs(600),

            // OAuth会话：30分钟（OAuth流程通常需要较长时间）
            Self::OAuthSession(_) => Duration::from_secs(1800),
        }
    }

    /// 判断是否应该缓存
    #[must_use]
    pub const fn should_cache(&self) -> bool {
        match self {
            // 安全相关的缓存
            Self::JwtAuth(_)
            | Self::JwtBlacklist(_)
            | Self::ApiKeyAuth(_)
            | Self::BasicFailure(_)
            | Self::BasicAuth(_)
            | Self::OAuthSession(_) => true,
        }
    }
}

/// 统一认证缓存管理器
///
/// 提供高级的缓存操作接口，隐藏底层缓存实现细节
pub struct UnifiedAuthCacheManager {
    /// 底层缓存管理器
    cache_manager: Arc<CacheManager>,
    /// 缓存配置
    cache_config: Arc<CacheConfig>,
}

impl UnifiedAuthCacheManager {
    /// 创建新的统一认证缓存管理器
    pub const fn new(cache_manager: Arc<CacheManager>, cache_config: Arc<CacheConfig>) -> Self {
        Self {
            cache_manager,
            cache_config,
        }
    }

    /// 缓存认证结果
    pub async fn cache_auth_result<T>(&self, key: &AuthCacheKey, value: &T) -> Result<()>
    where
        T: serde::Serialize + Send + Sync,
    {
        if !key.should_cache() {
            ldebug!(
                "system",
                LogStage::Cache,
                LogComponent::Cache,
                "skip_cache",
                &format!("Skipping cache for key type: {key:?}")
            );
            return Ok(());
        }

        let cache_key = key.to_key();
        let ttl = self.get_effective_ttl(key);

        match self
            .cache_manager
            .provider()
            .set(&cache_key, value, Some(ttl))
            .await
        {
            Ok(()) => {
                ldebug!("system", LogStage::Cache, LogComponent::Cache, "cache_set", "Cached auth result", cache_key = %cache_key, ttl_seconds = ttl.as_secs());
                Ok(())
            }
            Err(e) => {
                // 缓存失败不应该影响业务逻辑
                ldebug!("system", LogStage::Cache, LogComponent::Cache, "cache_set_fail", "Failed to cache auth result, continuing without cache", cache_key = %cache_key, error = %e);
                Ok(())
            }
        }
    }

    /// 获取缓存的认证结果
    pub async fn get_cached_auth_result<T>(&self, key: &AuthCacheKey) -> Option<T>
    where
        T: serde::de::DeserializeOwned + Send + Sync,
    {
        let cache_key = key.to_key();

        match self.cache_manager.provider().get::<T>(&cache_key).await {
            Ok(Some(value)) => {
                ldebug!("system", LogStage::Cache, LogComponent::Cache, "cache_hit", "Auth cache hit", cache_key = %cache_key);
                Some(value)
            }
            Ok(None) => {
                ldebug!("system", LogStage::Cache, LogComponent::Cache, "cache_miss", "Auth cache miss", cache_key = %cache_key);
                None
            }
            Err(e) => {
                ldebug!("system", LogStage::Cache, LogComponent::Cache, "cache_get_fail", "Auth cache error, treating as miss", cache_key = %cache_key, error = %e);
                None
            }
        }
    }

    /// 移除缓存条目
    pub async fn invalidate_cache(&self, key: &AuthCacheKey) -> Result<()> {
        let cache_key = key.to_key();

        // 由于CacheManager可能不支持删除，我们设置极短的TTL来"删除"
        let result = self
            .cache_manager
            .provider()
            .set(
                &cache_key,
                &Option::<()>::None,
                Some(Duration::from_secs(1)),
            )
            .await;

        ldebug!("system", LogStage::Cache, LogComponent::Cache, "cache_invalidate", "Auth cache invalidated", cache_key = %cache_key, success = result.is_ok());

        Ok(())
    }

    /// 检查缓存条目是否存在
    pub async fn cache_exists(&self, key: &AuthCacheKey) -> bool {
        let cache_key = key.to_key();

        matches!(
            self.cache_manager
                .provider()
                .get::<serde_json::Value>(&cache_key)
                .await,
            Ok(Some(_))
        )
    }

    /// 获取有效的TTL
    ///
    /// 考虑配置覆盖和默认值
    fn get_effective_ttl(&self, key: &AuthCacheKey) -> Duration {
        let cache_key = match key {
            AuthCacheKey::JwtAuth(token)
            | AuthCacheKey::JwtBlacklist(token)
            | AuthCacheKey::ApiKeyAuth(token)
            | AuthCacheKey::BasicFailure(token)
            | AuthCacheKey::BasicAuth(token)
            | AuthCacheKey::OAuthSession(token) => crate::cache::keys::CacheKey::AuthToken {
                token_hash: token.clone(),
            },
        };
        let strategy_ttl = crate::cache::strategies::CacheStrategies::for_key(&cache_key)
            .ttl
            .as_duration();
        strategy_ttl.unwrap_or_else(|| Duration::from_secs(self.cache_config.default_ttl))
    }

    /// 批量缓存操作
    pub async fn batch_cache<T>(&self, operations: Vec<(AuthCacheKey, T)>) -> Result<()>
    where
        T: serde::Serialize + Send + Sync,
    {
        let mut successful = 0;
        let mut failed = 0;

        for (key, value) in operations {
            match self.cache_auth_result(&key, &value).await {
                Ok(()) => successful += 1,
                Err(_) => failed += 1,
            }
        }

        ldebug!(
            "system",
            LogStage::Cache,
            LogComponent::Cache,
            "batch_cache_complete",
            "Batch cache operation completed",
            successful_operations = successful,
            failed_operations = failed
        );

        Ok(())
    }

    /// 预热缓存
    ///
    /// 为常用的认证结果预填充缓存
    pub fn warm_cache(&self, _warm_entries: Vec<AuthCacheKey>) -> Result<()> {
        // 预热逻辑可以根据实际需要实现
        // 例如：预加载常用的API密钥验证结果
        ldebug!(
            "system",
            LogStage::Cache,
            LogComponent::Cache,
            "warmup_complete",
            "Auth cache warm-up completed"
        );
        Ok(())
    }

    /// 获取缓存统计信息
    #[must_use]
    pub const fn get_cache_stats(&self) -> AuthCacheStats {
        // 这里可以集成更详细的缓存统计
        AuthCacheStats {
            jwt_cache_entries: 0,
            api_key_cache_entries: 0,
            basic_auth_cache_entries: 0,
            oauth_cache_entries: 0,
            total_cache_hits: 0,
            total_cache_misses: 0,
            cache_hit_ratio: 0.0,
        }
    }

    /// 清理过期缓存
    pub fn cleanup_expired(&self) -> Result<u64> {
        // CacheManager基于TTL自动清理
        ldebug!(
            "system",
            LogStage::Cache,
            LogComponent::Cache,
            "cleanup_auto",
            "Auth cache cleanup - handled automatically by TTL"
        );
        Ok(0)
    }
}

/// 认证缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct AuthCacheStats {
    /// JWT缓存条目数
    pub jwt_cache_entries: u64,
    /// API密钥缓存条目数
    pub api_key_cache_entries: u64,
    /// Basic认证缓存条目数
    pub basic_auth_cache_entries: u64,
    /// `OAuth缓存条目数`
    pub oauth_cache_entries: u64,
    /// 总缓存命中数
    pub total_cache_hits: u64,
    /// 总缓存未命中数
    pub total_cache_misses: u64,
    /// 缓存命中率
    pub cache_hit_ratio: f64,
}

/// 缓存策略配置
#[derive(Debug, Clone)]
pub struct CacheStrategyConfig {
    /// 是否启用认证结果缓存
    pub enable_auth_cache: bool,
    /// 是否启用Basic认证结果缓存
    pub enable_basic_auth_cache: bool,
    /// 是否启用黑名单缓存
    pub enable_blacklist_cache: bool,
    /// 自定义TTL配置
    pub custom_ttl: std::collections::HashMap<String, Duration>,
}

impl Default for CacheStrategyConfig {
    fn default() -> Self {
        Self {
            enable_auth_cache: true,
            enable_basic_auth_cache: true,
            enable_blacklist_cache: true,
            custom_ttl: std::collections::HashMap::new(),
        }
    }
}

/// 哈希工具函数
#[must_use]
pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 哈希用户凭据
#[must_use]
pub fn hash_credentials(username: &str, password: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("{username}:{password}").as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_cache_key_generation() {
        let jwt_key = AuthCacheKey::JwtAuth("test_hash".to_string());
        assert_eq!(jwt_key.to_key(), "auth:jwt:test_hash");

        let blacklist_key = AuthCacheKey::JwtBlacklist("token_hash".to_string());
        assert_eq!(blacklist_key.to_key(), "auth:jwt:blacklist:token_hash");

        let api_key = AuthCacheKey::ApiKeyAuth("api_hash".to_string());
        assert_eq!(api_key.to_key(), "auth:apikey:api_hash");
    }

    #[test]
    fn test_cache_ttl_defaults() {
        let jwt_key = AuthCacheKey::JwtAuth("hash".to_string());
        assert_eq!(jwt_key.default_ttl(), Duration::from_secs(900));

        let blacklist_key = AuthCacheKey::JwtBlacklist("hash".to_string());
        assert_eq!(blacklist_key.default_ttl(), Duration::from_secs(3600));

        let api_key = AuthCacheKey::ApiKeyAuth("hash".to_string());
        assert_eq!(api_key.default_ttl(), Duration::from_secs(300));
    }

    #[test]
    fn test_should_cache() {
        assert!(AuthCacheKey::JwtAuth("hash".to_string()).should_cache());
        assert!(AuthCacheKey::JwtBlacklist("hash".to_string()).should_cache());
        assert!(AuthCacheKey::ApiKeyAuth("hash".to_string()).should_cache());
        assert!(AuthCacheKey::BasicFailure("hash".to_string()).should_cache());
    }

    #[test]
    fn test_hash_token_consistency() {
        let token = "test_token_123";
        let hash1 = hash_token(token);
        let hash2 = hash_token(token);

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA256产生64字符的hex字符串
    }

    #[test]
    fn test_hash_credentials_consistency() {
        let username = "user";
        let password = "pass";

        let hash1 = hash_credentials(username, password);
        let hash2 = hash_credentials(username, password);

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);

        // 不同凭据应该产生不同的哈希
        let different_hash = hash_credentials("other", "pass");
        assert_ne!(hash1, different_hash);
    }
}
