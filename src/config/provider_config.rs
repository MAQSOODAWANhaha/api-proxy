//! # 服务商配置管理
//!
//! 从数据库动态加载服务商配置，替代硬编码地址

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use sea_orm::DatabaseConnection;
use sea_orm::{EntityTrait, ColumnTrait, QueryFilter};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn, error};

use entity::provider_types::{self, Entity as ProviderTypes};
use crate::cache::UnifiedCacheManager;
use crate::error::{Result, ProxyError};

/// 服务商配置管理器
pub struct ProviderConfigManager {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 缓存管理器
    cache: Arc<UnifiedCacheManager>,
    /// 配置缓存
    config_cache: Arc<RwLock<HashMap<String, ProviderConfig>>>,
}

/// 解析后的服务商配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// 服务商ID
    pub id: i32,
    /// 服务商名称
    pub name: String,
    /// 显示名称
    pub display_name: String,
    /// 基础URL（不含协议）
    pub base_url: String,
    /// 完整的HTTPS URL
    pub https_url: String,
    /// 带端口的地址（用于Pingora upstream）
    pub upstream_address: String,
    /// API格式
    pub api_format: String,
    /// 默认模型
    pub default_model: Option<String>,
    /// 最大Token数
    pub max_tokens: Option<i32>,
    /// 速率限制
    pub rate_limit: Option<i32>,
    /// 超时时间（秒）
    pub timeout_seconds: Option<i32>,
    /// 健康检查路径
    pub health_check_path: String,
    /// 认证头格式
    pub auth_header_format: String,
    /// 是否启用
    pub is_active: bool,
    /// 额外的JSON配置
    pub config_json: Option<serde_json::Value>,
}

impl ProviderConfigManager {
    /// 创建新的配置管理器
    pub fn new(db: Arc<DatabaseConnection>, cache: Arc<UnifiedCacheManager>) -> Self {
        Self {
            db,
            cache,
            config_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取所有活跃的服务商配置
    pub async fn get_active_providers(&self) -> Result<Vec<ProviderConfig>> {
        let cache_key = "active_providers_config";
        
        // 尝试从缓存获取
        if let Ok(Some(cached_configs)) = self.cache.provider().get::<Vec<ProviderConfig>>(cache_key).await {
            debug!("Retrieved {} active providers from cache", cached_configs.len());
            return Ok(cached_configs);
        }

        // 从数据库获取
        let providers = ProviderTypes::find()
            .filter(provider_types::Column::IsActive.eq(true))
            .all(self.db.as_ref())
            .await
            .map_err(|e| ProxyError::database(&format!("Failed to fetch active providers: {}", e)))?;

        let mut configs = Vec::new();
        for provider in providers {
            let provider_name = provider.name.clone(); // 克隆名称用于错误日志
            match self.parse_provider_config(provider) {
                Ok(config) => configs.push(config),
                Err(e) => {
                    warn!("Failed to parse provider config for {}: {}", provider_name, e);
                    continue;
                }
            }
        }

        // 缓存结果（缓存5分钟）
        if let Err(e) = self.cache.provider().set(cache_key, &configs, Some(Duration::from_secs(300))).await {
            warn!("Failed to cache active providers: {}", e);
        }

        debug!("Loaded {} active providers from database", configs.len());
        Ok(configs)
    }

    /// 根据名称获取服务商配置
    pub async fn get_provider_by_name(&self, name: &str) -> Result<Option<ProviderConfig>> {
        let cache_key = format!("provider_config:{}", name);
        
        // 尝试从缓存获取
        if let Ok(Some(cached_config)) = self.cache.provider().get::<ProviderConfig>(&cache_key).await {
            return Ok(Some(cached_config));
        }

        // 从数据库获取
        let provider = ProviderTypes::find()
            .filter(provider_types::Column::Name.eq(name))
            .filter(provider_types::Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await
            .map_err(|e| ProxyError::database(&format!("Failed to fetch provider {}: {}", name, e)))?;

        if let Some(provider) = provider {
            match self.parse_provider_config(provider) {
                Ok(config) => {
                    // 缓存结果（缓存10分钟）
                    if let Err(e) = self.cache.provider().set(&cache_key, &config, Some(Duration::from_secs(600))).await {
                        warn!("Failed to cache provider config for {}: {}", name, e);
                    }
                    Ok(Some(config))
                }
                Err(e) => {
                    error!("Failed to parse provider config for {}: {}", name, e);
                    Err(e)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// 根据ID获取服务商配置
    pub async fn get_provider_by_id(&self, id: i32) -> Result<Option<ProviderConfig>> {
        let cache_key = format!("provider_config_by_id:{}", id);
        
        // 尝试从缓存获取
        if let Ok(Some(cached_config)) = self.cache.provider().get::<ProviderConfig>(&cache_key).await {
            return Ok(Some(cached_config));
        }

        // 从数据库获取
        let provider = ProviderTypes::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| ProxyError::database(&format!("Failed to fetch provider by id {}: {}", id, e)))?;

        if let Some(provider) = provider {
            if !provider.is_active {
                return Ok(None);
            }

            match self.parse_provider_config(provider) {
                Ok(config) => {
                    // 缓存结果（缓存10分钟）
                    if let Err(e) = self.cache.provider().set(&cache_key, &config, Some(Duration::from_secs(600))).await {
                        warn!("Failed to cache provider config for id {}: {}", id, e);
                    }
                    Ok(Some(config))
                }
                Err(e) => {
                    error!("Failed to parse provider config for id {}: {}", id, e);
                    Err(e)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// 刷新配置缓存
    pub async fn refresh_cache(&self) -> Result<()> {
        debug!("Refreshing provider configuration cache");
        
        // 清除相关缓存
        let _ = self.cache.provider().delete("active_providers_config").await;
        
        // 重新加载配置
        let _configs = self.get_active_providers().await?;
        
        debug!("Provider configuration cache refreshed successfully");
        Ok(())
    }

    /// 解析服务商配置
    fn parse_provider_config(&self, provider: provider_types::Model) -> Result<ProviderConfig> {
        // 解析基础URL，确保正确的格式
        let base_url = self.normalize_base_url(&provider.base_url);
        let https_url = if base_url.starts_with("http") {
            base_url.clone()
        } else {
            format!("https://{}", base_url)
        };

        // 生成upstream地址（hostname:port格式）
        let upstream_address = self.generate_upstream_address(&base_url)?;

        // 解析JSON配置
        let config_json = if let Some(ref json_str) = provider.config_json {
            match serde_json::from_str(json_str) {
                Ok(json) => Some(json),
                Err(e) => {
                    warn!("Failed to parse config_json for provider {}: {}", provider.name, e);
                    None
                }
            }
        } else {
            None
        };

        Ok(ProviderConfig {
            id: provider.id,
            name: provider.name,
            display_name: provider.display_name,
            base_url,
            https_url,
            upstream_address,
            api_format: provider.api_format,
            default_model: provider.default_model,
            max_tokens: provider.max_tokens,
            rate_limit: provider.rate_limit,
            timeout_seconds: provider.timeout_seconds,
            health_check_path: provider.health_check_path.unwrap_or_else(|| "/models".to_string()),
            auth_header_format: provider.auth_header_format.unwrap_or_else(|| "Bearer {key}".to_string()),
            is_active: provider.is_active,
            config_json,
        })
    }

    /// 标准化base_url格式
    fn normalize_base_url(&self, url: &str) -> String {
        let url = url.trim();
        
        // 移除协议前缀
        if url.starts_with("https://") {
            url.strip_prefix("https://").unwrap().to_string()
        } else if url.starts_with("http://") {
            url.strip_prefix("http://").unwrap().to_string()
        } else {
            url.to_string()
        }
    }

    /// 生成upstream地址（hostname:port格式）
    fn generate_upstream_address(&self, base_url: &str) -> Result<String> {
        let normalized = self.normalize_base_url(base_url);
        
        // 如果已经包含端口，直接返回
        if normalized.contains(':') {
            return Ok(normalized);
        }

        // 默认使用443端口（HTTPS）
        Ok(format!("{}:443", normalized))
    }

    /// 检查服务商是否使用Google API Key认证
    pub fn uses_google_api_key_auth(&self, config: &ProviderConfig) -> bool {
        // 检查服务商名称
        let provider_name = config.name.to_lowercase();
        if provider_name.contains("gemini") || provider_name.contains("google") {
            return true;
        }
        
        // 检查认证头格式配置
        let auth_format_lower = config.auth_header_format.to_lowercase();
        if auth_format_lower.contains("x-goog-api-key") {
            return true;
        }
        
        // 检查base_url
        if config.base_url.contains("googleapis.com") || 
           config.base_url.contains("generativelanguage.googleapis.com") {
            return true;
        }
        
        false
    }

    /// 获取服务商的完整API端点URL
    pub fn get_api_endpoint(&self, config: &ProviderConfig, path: &str) -> String {
        let path = if path.starts_with('/') { path } else { &format!("/{}", path) };
        format!("{}{}", config.https_url, path)
    }
}

/// 默认的provider配置工厂（用于向后兼容）
impl ProviderConfig {
    /// 创建默认的OpenAI配置（用于fallback）
    pub fn default_openai() -> Self {
        Self {
            id: 0,
            name: "openai".to_string(),
            display_name: "OpenAI ChatGPT".to_string(),
            base_url: "api.openai.com".to_string(),
            https_url: "https://api.openai.com".to_string(),
            upstream_address: "api.openai.com:443".to_string(),
            api_format: "openai".to_string(),
            default_model: Some("gpt-3.5-turbo".to_string()),
            max_tokens: Some(4096),
            rate_limit: Some(100),
            timeout_seconds: Some(30),
            health_check_path: "/models".to_string(),
            auth_header_format: "Bearer {key}".to_string(),
            is_active: true,
            config_json: None,
        }
    }

    /// 创建默认的Gemini配置（用于fallback）
    pub fn default_gemini() -> Self {
        Self {
            id: 0,
            name: "gemini".to_string(),
            display_name: "Google Gemini".to_string(),
            base_url: "generativelanguage.googleapis.com".to_string(),
            https_url: "https://generativelanguage.googleapis.com".to_string(),
            upstream_address: "generativelanguage.googleapis.com:443".to_string(),
            api_format: "gemini_rest".to_string(),
            default_model: Some("gemini-pro".to_string()),
            max_tokens: Some(4096),
            rate_limit: Some(100),
            timeout_seconds: Some(30),
            health_check_path: "/v1beta/models".to_string(),
            auth_header_format: "X-goog-api-key: {key}".to_string(),
            is_active: true,
            config_json: None,
        }
    }

    /// 创建默认的Claude配置（用于fallback）
    pub fn default_claude() -> Self {
        Self {
            id: 0,
            name: "claude".to_string(),
            display_name: "Anthropic Claude".to_string(),
            base_url: "api.anthropic.com".to_string(),
            https_url: "https://api.anthropic.com".to_string(),
            upstream_address: "api.anthropic.com:443".to_string(),
            api_format: "anthropic".to_string(),
            default_model: Some("claude-3-sonnet".to_string()),
            max_tokens: Some(4096),
            rate_limit: Some(100),
            timeout_seconds: Some(30),
            health_check_path: "/v1/models".to_string(),
            auth_header_format: "Bearer {key}".to_string(),
            is_active: true,
            config_json: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_base_url() {
        use crate::config::CacheConfig;
        
        let cache_config = CacheConfig::default();
        let cache = UnifiedCacheManager::new(&cache_config, "test").unwrap();
        let manager = ProviderConfigManager::new(
            Arc::new(DatabaseConnection::default()),
            Arc::new(cache),
        );

        assert_eq!(manager.normalize_base_url("https://api.example.com"), "api.example.com");
        assert_eq!(manager.normalize_base_url("http://api.example.com"), "api.example.com");
        assert_eq!(manager.normalize_base_url("api.example.com"), "api.example.com");
        assert_eq!(manager.normalize_base_url("api.example.com:8080"), "api.example.com:8080");
    }

    #[test]
    fn test_generate_upstream_address() {
        use crate::config::CacheConfig;
        
        let cache_config = CacheConfig::default();
        let cache = UnifiedCacheManager::new(&cache_config, "test").unwrap();
        let manager = ProviderConfigManager::new(
            Arc::new(DatabaseConnection::default()),
            Arc::new(cache),
        );

        assert_eq!(manager.generate_upstream_address("api.example.com").unwrap(), "api.example.com:443");
        assert_eq!(manager.generate_upstream_address("api.example.com:8080").unwrap(), "api.example.com:8080");
        assert_eq!(manager.generate_upstream_address("https://api.example.com").unwrap(), "api.example.com:443");
    }

    #[test]
    fn test_google_api_key_detection() {
        use crate::config::CacheConfig;
        
        let cache_config = CacheConfig::default();
        let cache = UnifiedCacheManager::new(&cache_config, "test").unwrap();
        let manager = ProviderConfigManager::new(
            Arc::new(DatabaseConnection::default()),
            Arc::new(cache),
        );

        let gemini_config = ProviderConfig::default_gemini();
        assert!(manager.uses_google_api_key_auth(&gemini_config));

        let openai_config = ProviderConfig::default_openai();
        assert!(!manager.uses_google_api_key_auth(&openai_config));
    }

    #[test]
    fn test_api_endpoint_generation() {
        use crate::config::CacheConfig;
        
        let cache_config = CacheConfig::default();
        let cache = UnifiedCacheManager::new(&cache_config, "test").unwrap();
        let manager = ProviderConfigManager::new(
            Arc::new(DatabaseConnection::default()),
            Arc::new(cache),
        );

        let config = ProviderConfig::default_openai();
        assert_eq!(manager.get_api_endpoint(&config, "/v1/chat/completions"), "https://api.openai.com/v1/chat/completions");
        assert_eq!(manager.get_api_endpoint(&config, "v1/models"), "https://api.openai.com/v1/models");
    }
}