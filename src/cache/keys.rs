//! # 缓存键命名规范
//!
//! 定义统一的缓存键生成和管理策略

use serde::{Deserialize, Serialize};
use std::fmt;

/// 缓存键类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheKey {
    /// 用户会话缓存 - `user:session:{user_id}:{session_id}`
    UserSession { user_id: i32, session_id: String },

    /// 用户 API 密钥缓存 - `user:apikey:{user_id}:{key_id}`
    UserApiKey { user_id: i32, key_id: i32 },

    /// 用户 API 配置缓存 - `user:apiconfig:{user_id}:{api_id}`
    UserApiConfig { user_id: i32, api_id: i32 },

    /// API 健康状态缓存 - `health:api:{provider}:{api_name}`
    ApiHealth { provider: String, api_name: String },

    /// 请求统计缓存 - `stats:request:{date}:{hour}`
    RequestStats { date: String, hour: u8 },

    /// 用户每日统计缓存 - `stats:daily:{user_id}:{date}`
    DailyStats { user_id: i32, date: String },

    /// 配置缓存 - `config:{section}`
    Config { section: String },

    /// 速率限制缓存 - `ratelimit:{user_id}:{endpoint}`
    RateLimit { user_id: i32, endpoint: String },

    /// 提供商配置缓存 - `provider:config:{provider}`
    ProviderConfig { provider: String },

    /// 认证令牌缓存 - `auth:token:{token_hash}`
    AuthToken { token_hash: String },

    /// 自定义键 - `custom:{prefix}:{key}`
    Custom { prefix: String, key: String },
}

impl CacheKey {
    /// 生成缓存键字符串
    #[must_use]
    pub fn build(&self) -> String {
        match self {
            Self::UserSession {
                user_id,
                session_id,
            } => {
                format!("user:session:{user_id}:{session_id}")
            }
            Self::UserApiKey { user_id, key_id } => {
                format!("user:apikey:{user_id}:{key_id}")
            }
            Self::UserApiConfig { user_id, api_id } => {
                format!("user:apiconfig:{user_id}:{api_id}")
            }
            Self::ApiHealth { provider, api_name } => {
                format!("health:api:{provider}:{api_name}")
            }
            Self::RequestStats { date, hour } => {
                format!("stats:request:{date}:{hour:02}")
            }
            Self::DailyStats { user_id, date } => {
                format!("stats:daily:{user_id}:{date}")
            }
            Self::Config { section } => {
                format!("config:{section}")
            }
            Self::RateLimit { user_id, endpoint } => {
                format!("ratelimit:{user_id}:{}", sanitize_endpoint(endpoint))
            }
            Self::ProviderConfig { provider } => {
                format!("provider:config:{provider}")
            }
            Self::AuthToken { token_hash } => {
                format!("auth:token:{token_hash}")
            }
            Self::Custom { prefix, key } => {
                format!("custom:{prefix}:{key}")
            }
        }
    }

    /// 获取缓存键的模式（用于批量操作）
    #[must_use]
    pub fn pattern(&self) -> String {
        match self {
            Self::UserSession { user_id, .. } => format!("user:session:{user_id}:*"),
            Self::UserApiKey { user_id, .. } => format!("user:apikey:{user_id}:*"),
            Self::UserApiConfig { user_id, .. } => format!("user:apiconfig:{user_id}:*"),
            Self::ApiHealth { provider, .. } => format!("health:api:{provider}:*"),
            Self::RequestStats { date, .. } => {
                format!("stats:request:{date}:*")
            }
            Self::DailyStats { user_id, .. } => {
                format!("stats:daily:{user_id}:*")
            }
            Self::Config { .. } => "config:*".to_string(),
            Self::RateLimit { user_id, .. } => format!("ratelimit:{user_id}:*"),
            Self::ProviderConfig { .. } => "provider:config:*".to_string(),
            Self::AuthToken { .. } => "auth:token:*".to_string(),
            Self::Custom { prefix, .. } => format!("custom:{prefix}:*"),
        }
    }

    /// 获取缓存键的命名空间
    #[must_use]
    pub const fn namespace(&self) -> &'static str {
        match self {
            Self::UserSession { .. } | Self::UserApiKey { .. } | Self::UserApiConfig { .. } => {
                "user"
            }
            Self::ApiHealth { .. } => "health",
            Self::RequestStats { .. } | Self::DailyStats { .. } => "stats",
            Self::Config { .. } => "config",
            Self::RateLimit { .. } => "ratelimit",
            Self::ProviderConfig { .. } => "provider",
            Self::AuthToken { .. } => "auth",
            Self::Custom { .. } => "custom",
        }
    }

    /// 判断是否是临时缓存（需要较短的 TTL）
    #[must_use]
    pub const fn is_temporary(&self) -> bool {
        matches!(
            self,
            Self::UserSession { .. } | Self::AuthToken { .. } | Self::RateLimit { .. }
        )
    }

    /// 判断是否是配置缓存（需要较长的 TTL）
    #[must_use]
    pub const fn is_config(&self) -> bool {
        matches!(self, Self::Config { .. } | Self::ProviderConfig { .. })
    }

    /// 判断是否是统计缓存（需要中等的 TTL）
    #[must_use]
    pub const fn is_stats(&self) -> bool {
        matches!(
            self,
            Self::RequestStats { .. } | Self::DailyStats { .. } | Self::ApiHealth { .. }
        )
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.build())
    }
}

/// 清理端点名称，确保可以安全用作缓存键
fn sanitize_endpoint(endpoint: &str) -> String {
    endpoint
        .replace(['/', ':', '?', '&', '='], "_")
        .to_lowercase()
}

/// 缓存键构建器
pub struct CacheKeyBuilder;

impl CacheKeyBuilder {
    /// 构建用户会话缓存键
    #[must_use]
    pub fn user_session(user_id: i32, session_id: &str) -> CacheKey {
        CacheKey::UserSession {
            user_id,
            session_id: session_id.to_string(),
        }
    }

    /// 构建用户 API 密钥缓存键
    #[must_use]
    pub const fn user_api_key(user_id: i32, key_id: i32) -> CacheKey {
        CacheKey::UserApiKey { user_id, key_id }
    }

    /// 构建用户 API 配置缓存键
    #[must_use]
    pub const fn user_api_config(user_id: i32, api_id: i32) -> CacheKey {
        CacheKey::UserApiConfig { user_id, api_id }
    }

    /// 构建 API 健康状态缓存键
    #[must_use]
    pub fn api_health(provider: &str, api_name: &str) -> CacheKey {
        CacheKey::ApiHealth {
            provider: provider.to_string(),
            api_name: api_name.to_string(),
        }
    }

    /// 构建请求统计缓存键
    #[must_use]
    pub fn request_stats(date: &str, hour: u8) -> CacheKey {
        CacheKey::RequestStats {
            date: date.to_string(),
            hour,
        }
    }

    /// 构建每日统计缓存键
    #[must_use]
    pub fn daily_stats(user_id: i32, date: &str) -> CacheKey {
        CacheKey::DailyStats {
            user_id,
            date: date.to_string(),
        }
    }

    /// 构建配置缓存键
    #[must_use]
    pub fn config(section: &str) -> CacheKey {
        CacheKey::Config {
            section: section.to_string(),
        }
    }

    /// 构建速率限制缓存键
    #[must_use]
    pub fn rate_limit(user_id: i32, endpoint: &str) -> CacheKey {
        CacheKey::RateLimit {
            user_id,
            endpoint: endpoint.to_string(),
        }
    }

    /// 构建提供商配置缓存键
    #[must_use]
    pub fn provider_config(provider: &str) -> CacheKey {
        CacheKey::ProviderConfig {
            provider: provider.to_string(),
        }
    }

    /// 构建认证令牌缓存键
    #[must_use]
    pub fn auth_token(token_hash: &str) -> CacheKey {
        CacheKey::AuthToken {
            token_hash: token_hash.to_string(),
        }
    }

    /// 构建自定义缓存键
    #[must_use]
    pub fn custom(prefix: &str, key: &str) -> CacheKey {
        CacheKey::Custom {
            prefix: prefix.to_string(),
            key: key.to_string(),
        }
    }

    /// 构建简单的速率限制缓存键（用于管理接口）
    #[must_use]
    pub fn rate_limit_simple(client_id: &str, endpoint: &str) -> CacheKey {
        CacheKey::Custom {
            prefix: "rate_limit".to_string(),
            key: format!(
                "{}:{}",
                sanitize_endpoint(client_id),
                sanitize_endpoint(endpoint)
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_build() {
        let session_key = CacheKeyBuilder::user_session(123, "abc123");
        assert_eq!(session_key.build(), "user:session:123:abc123");

        let api_key = CacheKeyBuilder::user_api_key(456, 789);
        assert_eq!(api_key.build(), "user:apikey:456:789");

        let health_key = CacheKeyBuilder::api_health("openai", "chat");
        assert_eq!(health_key.build(), "health:api:openai:chat");
    }

    #[test]
    fn test_cache_key_pattern() {
        let session_key = CacheKeyBuilder::user_session(123, "abc123");
        assert_eq!(session_key.pattern(), "user:session:123:*");

        let config_key = CacheKeyBuilder::config("database");
        assert_eq!(config_key.pattern(), "config:*");
    }

    #[test]
    fn test_endpoint_sanitization() {
        assert_eq!(sanitize_endpoint("/api/v1/chat"), "_api_v1_chat");
        assert_eq!(
            sanitize_endpoint("openai:completion?model=gpt4"),
            "openai_completion_model_gpt4"
        );
    }

    #[test]
    fn test_cache_key_properties() {
        let session_key = CacheKeyBuilder::user_session(123, "abc123");
        assert!(session_key.is_temporary());
        assert!(!session_key.is_config());
        assert!(!session_key.is_stats());

        let config_key = CacheKeyBuilder::config("database");
        assert!(!config_key.is_temporary());
        assert!(config_key.is_config());
        assert!(!config_key.is_stats());

        let stats_key = CacheKeyBuilder::request_stats("2024-01-01", 12);
        assert!(!stats_key.is_temporary());
        assert!(!stats_key.is_config());
        assert!(stats_key.is_stats());
    }
}
