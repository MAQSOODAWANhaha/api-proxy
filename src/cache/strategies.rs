//! # 缓存策略
//!
//! 定义不同类型数据的缓存策略和TTL管理

use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::keys::CacheKey;

/// 缓存 TTL 策略
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CacheTtl {
    /// 短期缓存（5分钟）- 用于会话、令牌等
    Short,
    /// 中期缓存（1小时）- 用于统计、健康状态等
    Medium,
    /// 长期缓存（24小时）- 用于配置、用户信息等
    Long,
    /// 自定义 TTL（秒）
    Custom(u64),
    /// 永不过期（需要手动删除）
    Never,
}

impl CacheTtl {
    /// 获取TTL秒数
    #[must_use]
    pub const fn as_seconds(&self) -> Option<u64> {
        match self {
            Self::Short => Some(300),   // 5分钟
            Self::Medium => Some(3600), // 1小时
            Self::Long => Some(86400),  // 24小时
            Self::Custom(seconds) => Some(*seconds),
            Self::Never => None,
        }
    }

    /// 获取 Duration
    #[must_use]
    pub fn as_duration(&self) -> Option<Duration> {
        self.as_seconds().map(Duration::from_secs)
    }

    /// 从秒数创建自定义 TTL
    #[must_use]
    pub const fn from_seconds(seconds: u64) -> Self { Self::Custom(seconds) }

    /// 从分钟创建自定义 TTL
    #[must_use]
    pub const fn from_minutes(minutes: u64) -> Self { Self::Custom(minutes * 60) }

    /// 从小时创建自定义 TTL
    #[must_use]
    pub const fn from_hours(hours: u64) -> Self { Self::Custom(hours * 3600) }

    /// 从天创建自定义 TTL
    #[must_use]
    pub const fn from_days(days: u64) -> Self { Self::Custom(days * 86400) }
}

/// 缓存策略
#[derive(Debug, Clone)]
pub struct CacheStrategy {
    /// TTL 策略
    pub ttl: CacheTtl,
    /// 是否允许缓存空值
    pub cache_null_values: bool,
    /// 是否启用压缩
    pub compression_enabled: bool,
    /// 最大值大小（字节）
    pub max_value_size: usize,
    /// 是否启用预热
    pub warmup_enabled: bool,
}

impl Default for CacheStrategy {
    fn default() -> Self {
        Self {
            ttl: CacheTtl::Medium,
            cache_null_values: false,
            compression_enabled: false,
            max_value_size: 1024 * 1024, // 1MB
            warmup_enabled: false,
        }
    }
}

impl CacheStrategy {
    /// 创建短期缓存策略
    #[must_use]
    pub const fn short_term() -> Self {
        Self {
            ttl: CacheTtl::Short,
            cache_null_values: false,
            compression_enabled: false,
            max_value_size: 64 * 1024, // 64KB
            warmup_enabled: false,
        }
    }

    /// 创建中期缓存策略
    #[must_use]
    pub const fn medium_term() -> Self {
        Self {
            ttl: CacheTtl::Medium,
            cache_null_values: true,
            compression_enabled: false,
            max_value_size: 256 * 1024, // 256KB
            warmup_enabled: false,
        }
    }

    /// 创建长期缓存策略
    #[must_use]
    pub const fn long_term() -> Self {
        Self {
            ttl: CacheTtl::Long,
            cache_null_values: true,
            compression_enabled: true,
            max_value_size: 1024 * 1024, // 1MB
            warmup_enabled: true,
        }
    }

    /// 创建自定义缓存策略
    #[must_use]
    pub fn custom(ttl: CacheTtl) -> Self {
        Self {
            ttl,
            ..Default::default()
        }
    }

    /// 根据缓存键自动选择策略
    #[must_use]
    pub fn for_key(key: &CacheKey) -> Self {
        if key.is_temporary() {
            Self::short_term()
        } else if key.is_config() {
            Self::long_term()
        } else if key.is_stats() {
            Self::medium_term()
        } else {
            Self::default()
        }
    }

    /// 设置 TTL
    #[must_use]
    pub fn with_ttl(mut self, ttl: CacheTtl) -> Self {
        self.ttl = ttl;
        self
    }

    /// 设置是否允许缓存空值
    #[must_use]
    pub fn with_null_values(mut self, cache_null_values: bool) -> Self {
        self.cache_null_values = cache_null_values;
        self
    }

    /// 设置是否启用压缩
    #[must_use]
    pub fn with_compression(mut self, compression_enabled: bool) -> Self {
        self.compression_enabled = compression_enabled;
        self
    }

    /// 设置最大值大小
    #[must_use]
    pub fn with_max_value_size(mut self, max_value_size: usize) -> Self {
        self.max_value_size = max_value_size;
        self
    }

    /// 设置是否启用预热
    #[must_use]
    pub fn with_warmup(mut self, warmup_enabled: bool) -> Self {
        self.warmup_enabled = warmup_enabled;
        self
    }

    /// 验证值是否符合策略要求
    pub fn validate_value(&self, value: &str) -> bool {
        if value.is_empty() && !self.cache_null_values {
            return false;
        }

        if value.len() > self.max_value_size {
            return false;
        }

        true
    }
}

/// 预定义的缓存策略
pub struct CacheStrategies;

impl CacheStrategies {
    /// 用户会话缓存策略（30分钟）
    pub fn user_session() -> CacheStrategy {
        CacheStrategy::short_term()
            .with_ttl(CacheTtl::from_minutes(30))
            .with_null_values(false)
            .with_compression(false)
    }

    /// 认证令牌缓存策略（15分钟）
    pub fn auth_token() -> CacheStrategy {
        CacheStrategy::short_term()
            .with_ttl(CacheTtl::from_minutes(15))
            .with_null_values(false)
            .with_compression(false)
    }

    /// API健康状态缓存策略（5分钟）
    pub fn api_health() -> CacheStrategy {
        CacheStrategy::short_term()
            .with_ttl(CacheTtl::from_minutes(5))
            .with_null_values(true)
            .with_compression(false)
    }

    /// 用户API密钥缓存策略（2小时）
    pub fn user_api_key() -> CacheStrategy {
        CacheStrategy::medium_term()
            .with_ttl(CacheTtl::from_hours(2))
            .with_null_values(false)
            .with_compression(false)
    }

    /// 请求统计缓存策略（6小时）
    pub fn request_stats() -> CacheStrategy {
        CacheStrategy::medium_term()
            .with_ttl(CacheTtl::from_hours(6))
            .with_null_values(true)
            .with_compression(true)
    }

    /// 每日统计缓存策略（24小时）
    pub fn daily_stats() -> CacheStrategy {
        CacheStrategy::long_term()
            .with_ttl(CacheTtl::from_hours(24))
            .with_null_values(true)
            .with_compression(true)
    }

    /// 配置缓存策略（12小时）
    pub fn config() -> CacheStrategy {
        CacheStrategy::long_term()
            .with_ttl(CacheTtl::from_hours(12))
            .with_null_values(false)
            .with_compression(false)
            .with_warmup(true)
    }

    /// 提供商配置缓存策略（6小时）
    pub fn provider_config() -> CacheStrategy {
        CacheStrategy::medium_term()
            .with_ttl(CacheTtl::from_hours(6))
            .with_null_values(false)
            .with_compression(false)
            .with_warmup(true)
    }

    /// 速率限制缓存策略（1分钟）
    pub fn rate_limit() -> CacheStrategy {
        CacheStrategy::short_term()
            .with_ttl(CacheTtl::from_minutes(1))
            .with_null_values(false)
            .with_compression(false)
    }

    /// 根据缓存键获取推荐策略
    pub fn for_key(key: &CacheKey) -> CacheStrategy {
        match key {
            CacheKey::UserSession { .. } => Self::user_session(),
            CacheKey::UserApiKey { .. } => Self::user_api_key(),
            CacheKey::UserApiConfig { .. } => Self::user_api_key(), // 用户API配置使用与API密钥相同的策略
            CacheKey::ApiHealth { .. } => Self::api_health(),
            CacheKey::RequestStats { .. } => Self::request_stats(),
            CacheKey::DailyStats { .. } => Self::daily_stats(),
            CacheKey::Config { .. } => Self::config(),
            CacheKey::RateLimit { .. } => Self::rate_limit(),
            CacheKey::ProviderConfig { .. } => Self::provider_config(),
            CacheKey::AuthToken { .. } => Self::auth_token(),
            CacheKey::Custom { .. } => CacheStrategy::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_ttl_conversion() {
        assert_eq!(CacheTtl::Short.as_seconds(), Some(300));
        assert_eq!(CacheTtl::Medium.as_seconds(), Some(3600));
        assert_eq!(CacheTtl::Long.as_seconds(), Some(86400));
        assert_eq!(CacheTtl::Custom(1800).as_seconds(), Some(1800));
        assert_eq!(CacheTtl::Never.as_seconds(), None);
    }

    #[test]
    fn test_cache_ttl_creation() {
        assert_eq!(CacheTtl::from_minutes(30).as_seconds(), Some(1800));
        assert_eq!(CacheTtl::from_hours(2).as_seconds(), Some(7200));
        assert_eq!(CacheTtl::from_days(1).as_seconds(), Some(86400));
    }

    #[test]
    fn test_cache_strategy_validation() {
        let strategy = CacheStrategy::default();

        // 正常值应该通过验证
        assert!(strategy.validate_value("normal value"));

        // 空值在不允许时应该失败
        let no_null_strategy = CacheStrategy::default().with_null_values(false);
        assert!(!no_null_strategy.validate_value(""));

        // 空值在允许时应该通过
        let allow_null_strategy = CacheStrategy::default().with_null_values(true);
        assert!(allow_null_strategy.validate_value(""));

        // 超大值应该失败
        let small_size_strategy = CacheStrategy::default().with_max_value_size(10);
        assert!(!small_size_strategy.validate_value("this is a very long string"));
    }

    #[test]
    fn test_predefined_strategies() {
        let session_strategy = CacheStrategies::user_session();
        assert_eq!(session_strategy.ttl.as_seconds(), Some(1800)); // 30分钟

        let config_strategy = CacheStrategies::config();
        assert_eq!(config_strategy.ttl.as_seconds(), Some(43200)); // 12小时
        assert!(config_strategy.warmup_enabled);
    }
}
