//! # 缓存键命名规范
//!
//! 定义统一的缓存键生成和管理策略

use serde::{Deserialize, Serialize};
use std::fmt;

/// 缓存键类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheKey {
    /// 用户会话缓存 - user:session:{user_id}:{session_id}
    UserSession { user_id: i32, session_id: String },
    
    /// 用户 API 密钥缓存 - user:apikey:{user_id}:{key_id}
    UserApiKey { user_id: i32, key_id: i32 },
    
    /// API 健康状态缓存 - health:api:{provider}:{api_name}
    ApiHealth { provider: String, api_name: String },
    
    /// 请求统计缓存 - stats:request:{date}:{hour}
    RequestStats { date: String, hour: u8 },
    
    /// 用户每日统计缓存 - stats:daily:{user_id}:{date}
    DailyStats { user_id: i32, date: String },
    
    /// 配置缓存 - config:{section}
    Config { section: String },
    
    /// 速率限制缓存 - ratelimit:{user_id}:{endpoint}
    RateLimit { user_id: i32, endpoint: String },
    
    /// 提供商配置缓存 - provider:config:{provider}
    ProviderConfig { provider: String },
    
    /// 认证令牌缓存 - auth:token:{token_hash}
    AuthToken { token_hash: String },
    
    /// 自定义键 - custom:{prefix}:{key}
    Custom { prefix: String, key: String },
}

impl CacheKey {
    /// 生成缓存键字符串
    pub fn build(&self) -> String {
        match self {
            CacheKey::UserSession { user_id, session_id } => {
                format!("user:session:{}:{}", user_id, session_id)
            }
            CacheKey::UserApiKey { user_id, key_id } => {
                format!("user:apikey:{}:{}", user_id, key_id)
            }
            CacheKey::ApiHealth { provider, api_name } => {
                format!("health:api:{}:{}", provider, api_name)
            }
            CacheKey::RequestStats { date, hour } => {
                format!("stats:request:{}:{:02}", date, hour)
            }
            CacheKey::DailyStats { user_id, date } => {
                format!("stats:daily:{}:{}", user_id, date)
            }
            CacheKey::Config { section } => {
                format!("config:{}", section)
            }
            CacheKey::RateLimit { user_id, endpoint } => {
                format!("ratelimit:{}:{}", user_id, sanitize_endpoint(endpoint))
            }
            CacheKey::ProviderConfig { provider } => {
                format!("provider:config:{}", provider)
            }
            CacheKey::AuthToken { token_hash } => {
                format!("auth:token:{}", token_hash)
            }
            CacheKey::Custom { prefix, key } => {
                format!("custom:{}:{}", prefix, key)
            }
        }
    }
    
    /// 获取缓存键的模式（用于批量操作）
    pub fn pattern(&self) -> String {
        match self {
            CacheKey::UserSession { user_id, .. } => {
                format!("user:session:{}:*", user_id)
            }
            CacheKey::UserApiKey { user_id, .. } => {
                format!("user:apikey:{}:*", user_id)
            }
            CacheKey::ApiHealth { provider, .. } => {
                format!("health:api:{}:*", provider)
            }
            CacheKey::RequestStats { date, .. } => {
                format!("stats:request:{}:*", date)
            }
            CacheKey::DailyStats { user_id, .. } => {
                format!("stats:daily:{}:*", user_id)
            }
            CacheKey::Config { .. } => {
                "config:*".to_string()
            }
            CacheKey::RateLimit { user_id, .. } => {
                format!("ratelimit:{}:*", user_id)
            }
            CacheKey::ProviderConfig { .. } => {
                "provider:config:*".to_string()
            }
            CacheKey::AuthToken { .. } => {
                "auth:token:*".to_string()
            }
            CacheKey::Custom { prefix, .. } => {
                format!("custom:{}:*", prefix)
            }
        }
    }
    
    /// 获取缓存键的命名空间
    pub fn namespace(&self) -> &'static str {
        match self {
            CacheKey::UserSession { .. } => "user",
            CacheKey::UserApiKey { .. } => "user", 
            CacheKey::ApiHealth { .. } => "health",
            CacheKey::RequestStats { .. } => "stats",
            CacheKey::DailyStats { .. } => "stats",
            CacheKey::Config { .. } => "config",
            CacheKey::RateLimit { .. } => "ratelimit",
            CacheKey::ProviderConfig { .. } => "provider",
            CacheKey::AuthToken { .. } => "auth",
            CacheKey::Custom { .. } => "custom",
        }
    }
    
    /// 判断是否是临时缓存（需要较短的 TTL）
    pub fn is_temporary(&self) -> bool {
        matches!(self, 
            CacheKey::UserSession { .. } |
            CacheKey::AuthToken { .. } |
            CacheKey::RateLimit { .. }
        )
    }
    
    /// 判断是否是配置缓存（需要较长的 TTL）
    pub fn is_config(&self) -> bool {
        matches!(self,
            CacheKey::Config { .. } |
            CacheKey::ProviderConfig { .. }
        )
    }
    
    /// 判断是否是统计缓存（需要中等的 TTL）
    pub fn is_stats(&self) -> bool {
        matches!(self,
            CacheKey::RequestStats { .. } |
            CacheKey::DailyStats { .. } |
            CacheKey::ApiHealth { .. }
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
        .replace('/', "_")
        .replace(':', "_")
        .replace('?', "_")
        .replace('&', "_")
        .replace('=', "_")
        .to_lowercase()
}

/// 缓存键构建器
pub struct CacheKeyBuilder;

impl CacheKeyBuilder {
    /// 构建用户会话缓存键
    pub fn user_session(user_id: i32, session_id: &str) -> CacheKey {
        CacheKey::UserSession {
            user_id,
            session_id: session_id.to_string(),
        }
    }
    
    /// 构建用户 API 密钥缓存键
    pub fn user_api_key(user_id: i32, key_id: i32) -> CacheKey {
        CacheKey::UserApiKey { user_id, key_id }
    }
    
    /// 构建 API 健康状态缓存键
    pub fn api_health(provider: &str, api_name: &str) -> CacheKey {
        CacheKey::ApiHealth {
            provider: provider.to_string(),
            api_name: api_name.to_string(),
        }
    }
    
    /// 构建请求统计缓存键
    pub fn request_stats(date: &str, hour: u8) -> CacheKey {
        CacheKey::RequestStats {
            date: date.to_string(),
            hour,
        }
    }
    
    /// 构建每日统计缓存键
    pub fn daily_stats(user_id: i32, date: &str) -> CacheKey {
        CacheKey::DailyStats {
            user_id,
            date: date.to_string(),
        }
    }
    
    /// 构建配置缓存键
    pub fn config(section: &str) -> CacheKey {
        CacheKey::Config {
            section: section.to_string(),
        }
    }
    
    /// 构建速率限制缓存键
    pub fn rate_limit(user_id: i32, endpoint: &str) -> CacheKey {
        CacheKey::RateLimit {
            user_id,
            endpoint: endpoint.to_string(),
        }
    }
    
    /// 构建提供商配置缓存键
    pub fn provider_config(provider: &str) -> CacheKey {
        CacheKey::ProviderConfig {
            provider: provider.to_string(),
        }
    }
    
    /// 构建认证令牌缓存键
    pub fn auth_token(token_hash: &str) -> CacheKey {
        CacheKey::AuthToken {
            token_hash: token_hash.to_string(),
        }
    }
    
    /// 构建自定义缓存键
    pub fn custom(prefix: &str, key: &str) -> CacheKey {
        CacheKey::Custom {
            prefix: prefix.to_string(),
            key: key.to_string(),
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
        assert_eq!(sanitize_endpoint("openai:completion?model=gpt4"), "openai_completion_model_gpt4");
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