//! # 服务商配置管理
//!
//! 从数据库动态加载服务商配置，替代硬编码地址

use crate::auth::types::{AuthType, MultiAuthConfig};
use crate::cache::CacheManager;
use crate::error::{ProxyError, Result};
use crate::{
    ldebug, lerror,
    logging::{LogComponent, LogStage},
    lwarn,
};
use entity::provider_types::{self, Entity as ProviderTypes};
use sea_orm::DatabaseConnection;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// 服务商配置管理器
pub struct ProviderConfigManager {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 缓存管理器
    cache: Arc<CacheManager>,
    /// 配置缓存
    #[allow(dead_code)]
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
    /// 是否启用
    pub is_active: bool,
    /// 额外的JSON配置
    pub config_json: Option<serde_json::Value>,
    /// Token字段映射配置JSON
    pub token_mappings_json: Option<String>,
    /// 模型提取规则配置JSON
    pub model_extraction_json: Option<String>,
    /// 支持的认证类型列表
    pub supported_auth_types: Vec<AuthType>,
    /// 认证配置详情
    pub auth_configs: Option<Vec<MultiAuthConfig>>,
}

impl ProviderConfigManager {
    /// 创建新的配置管理器
    pub fn new(db: Arc<DatabaseConnection>, cache: Arc<CacheManager>) -> Self {
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
        if let Ok(Some(cached_configs)) = self
            .cache
            .provider()
            .get::<Vec<ProviderConfig>>(cache_key)
            .await
        {
            ldebug!(
                "system",
                LogStage::Cache,
                LogComponent::Config,
                "cache_hit",
                &format!(
                    "Retrieved {} active providers from cache",
                    cached_configs.len()
                )
            );
            return Ok(cached_configs);
        }

        // 从数据库获取
        let providers = ProviderTypes::find()
            .filter(provider_types::Column::IsActive.eq(true))
            .all(self.db.as_ref())
            .await
            .map_err(|e| {
                ProxyError::database(&format!("Failed to fetch active providers: {}", e))
            })?;

        let mut configs = Vec::new();
        for provider in providers {
            let provider_name = provider.name.clone(); // 克隆名称用于错误日志
            match self.parse_provider_config(provider) {
                Ok(config) => configs.push(config),
                Err(e) => {
                    lwarn!(
                        "system",
                        LogStage::Configuration,
                        LogComponent::Config,
                        "parse_provider_config_fail",
                        &format!(
                            "Failed to parse provider config for {}: {}",
                            provider_name, e
                        )
                    );
                }
            }
        }

        // 缓存结果（缓存5分钟）
        if let Err(e) = self
            .cache
            .provider()
            .set(cache_key, &configs, Some(Duration::from_secs(300)))
            .await
        {
            lwarn!(
                "system",
                LogStage::Cache,
                LogComponent::Config,
                "cache_fail",
                &format!("Failed to cache active providers: {}", e)
            );
        }

        ldebug!(
            "system",
            LogStage::Db,
            LogComponent::Config,
            "load_from_db",
            &format!("Loaded {} active providers from database", configs.len())
        );
        Ok(configs)
    }

    /// 根据提供商名称解析为ProviderId（整合ProviderResolver功能）
    pub async fn resolve_provider(
        &self,
        provider_name: &str,
    ) -> Result<crate::proxy::types::ProviderId> {
        use crate::proxy::types::ProviderId;
        use entity::provider_types::{self, Entity as ProviderTypes};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let normalized_name = provider_name.to_lowercase();

        // 尝试从数据库查询活跃的提供商
        if let Some(provider) = ProviderTypes::find()
            .filter(provider_types::Column::Name.eq(&normalized_name))
            .filter(provider_types::Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await
            .map_err(|e| {
                crate::error::ProxyError::internal(format!("Database query error: {}", e))
            })?
        {
            return Ok(ProviderId::from_database_id(provider.id));
        }

        // 获取所有活跃提供商进行灵活匹配
        let providers = ProviderTypes::find()
            .filter(provider_types::Column::IsActive.eq(true))
            .all(self.db.as_ref())
            .await
            .map_err(|e| {
                crate::error::ProxyError::internal(format!("Database query error: {}", e))
            })?;

        // 尝试多种匹配策略
        for provider in &providers {
            // 1. display_name精确匹配（不区分大小写）
            if provider.display_name.to_lowercase() == normalized_name {
                return Ok(ProviderId::from_database_id(provider.id));
            }

            // 2. display_name中包含查询词
            let display_lower = provider.display_name.to_lowercase();
            if display_lower.contains(&normalized_name) && normalized_name.len() >= 3 {
                return Ok(ProviderId::from_database_id(provider.id));
            }
        }

        // 获取所有活跃提供商的名称用于错误消息
        let available_providers = providers
            .iter()
            .map(|p| format!("{} ({})", p.display_name, p.name))
            .collect::<Vec<_>>()
            .join(", ");

        Err(crate::error::ProxyError::config(format!(
            "Unknown or inactive provider: '{}'. Available providers: {}",
            normalized_name, available_providers
        )))
    }

    /// 根据名称获取服务商配置
    pub async fn get_provider_by_name(&self, name: &str) -> Result<Option<ProviderConfig>> {
        let cache_key = format!("provider_config:{}", name);

        // 尝试从缓存获取
        if let Ok(Some(cached_config)) = self
            .cache
            .provider()
            .get::<ProviderConfig>(&cache_key)
            .await
        {
            return Ok(Some(cached_config));
        }

        // 从数据库获取
        let provider = ProviderTypes::find()
            .filter(provider_types::Column::Name.eq(name))
            .filter(provider_types::Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await
            .map_err(|e| {
                ProxyError::database(&format!("Failed to fetch provider {}: {}", name, e))
            })?;

        if let Some(provider) = provider {
            match self.parse_provider_config(provider) {
                Ok(config) => {
                    // 缓存结果（缓存10分钟）
                    if let Err(e) = self
                        .cache
                        .provider()
                        .set(&cache_key, &config, Some(Duration::from_secs(600)))
                        .await
                    {
                        lwarn!(
                            "system",
                            LogStage::Cache,
                            LogComponent::Config,
                            "cache_fail",
                            &format!("Failed to cache provider config for {}: {}", name, e)
                        );
                    }
                    Ok(Some(config))
                }
                Err(e) => {
                    lerror!(
                        "system",
                        LogStage::Configuration,
                        LogComponent::Config,
                        "parse_provider_config_fail",
                        &format!("Failed to parse provider config for {}: {}", name, e)
                    );
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
        if let Ok(Some(cached_config)) = self
            .cache
            .provider()
            .get::<ProviderConfig>(&cache_key)
            .await
        {
            return Ok(Some(cached_config));
        }

        // 从数据库获取
        let provider = ProviderTypes::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| {
                ProxyError::database(&format!("Failed to fetch provider by id {}: {}", id, e))
            })?;

        if let Some(provider) = provider {
            if !provider.is_active {
                return Ok(None);
            }

            match self.parse_provider_config(provider) {
                Ok(config) => {
                    // 缓存结果（缓存10分钟）
                    if let Err(e) = self
                        .cache
                        .provider()
                        .set(&cache_key, &config, Some(Duration::from_secs(600)))
                        .await
                    {
                        lwarn!(
                            "system",
                            LogStage::Cache,
                            LogComponent::Config,
                            "cache_fail",
                            &format!("Failed to cache provider config for id {}: {}", id, e)
                        );
                    }
                    Ok(Some(config))
                }
                Err(e) => {
                    lerror!(
                        "system",
                        LogStage::Configuration,
                        LogComponent::Config,
                        "parse_provider_config_fail",
                        &format!("Failed to parse provider config for id {}: {}", id, e)
                    );
                    Err(e)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// 刷新配置缓存
    pub async fn refresh_cache(&self) -> Result<()> {
        ldebug!(
            "system",
            LogStage::Configuration,
            LogComponent::Config,
            "refresh_cache",
            "Refreshing provider configuration cache"
        );

        // 清除相关缓存
        let _ = self
            .cache
            .provider()
            .delete("active_providers_config")
            .await;

        // 重新加载配置
        let _configs = self.get_active_providers().await?;

        ldebug!(
            "system",
            LogStage::Configuration,
            LogComponent::Config,
            "refresh_cache_ok",
            "Provider configuration cache refreshed successfully"
        );
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
                    lwarn!(
                        "system",
                        LogStage::Configuration,
                        LogComponent::Config,
                        "parse_config_json_fail",
                        &format!(
                            "Failed to parse config_json for provider {}: {}",
                            provider.name, e
                        )
                    );
                    None
                }
            }
        } else {
            None
        };

        // 解析支持的认证类型
        let supported_auth_types =
            self.parse_supported_auth_types(&provider.supported_auth_types)?;

        // 解析认证配置 - 支持对象映射格式
        let auth_configs = if let Some(ref json_str) = provider.auth_configs_json {
            match self.parse_auth_configs_from_map(json_str) {
                Ok(configs) => Some(configs),
                Err(e) => {
                    lwarn!(
                        "system",
                        LogStage::Configuration,
                        LogComponent::Config,
                        "parse_auth_configs_fail",
                        &format!(
                            "Failed to parse auth_configs_json for provider {}: {}. Raw JSON: '{}'",
                            provider.name, e, json_str
                        )
                    );
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
            health_check_path: provider
                .health_check_path
                .unwrap_or_else(|| "/models".to_string()),
            is_active: provider.is_active,
            config_json,
            token_mappings_json: provider.token_mappings_json,
            model_extraction_json: provider.model_extraction_json,
            supported_auth_types,
            auth_configs,
        })
    }

    /// 解析对象映射格式的认证配置
    fn parse_auth_configs_from_map(&self, map_str: &str) -> Result<Vec<MultiAuthConfig>> {
        let config_map: std::collections::HashMap<String, serde_json::Value> =
            serde_json::from_str(map_str).map_err(|e| {
                ProxyError::config(&format!("Failed to parse auth_configs_map: {}", e))
            })?;

        let mut auth_configs = Vec::new();

        for (auth_type_str, config_value) in config_map {
            // 使用安全的解析方法
            if let Some(auth_type) = AuthType::from(&auth_type_str) {
                let extra_config =
                    if config_value.is_object() && !config_value.as_object().unwrap().is_empty() {
                        Some(config_value)
                    } else {
                        None
                    };

                auth_configs.push(MultiAuthConfig {
                    auth_type,
                    extra_config,
                });
            } else {
                lwarn!(
                    "system",
                    LogStage::Configuration,
                    LogComponent::Config,
                    "unknown_auth_type",
                    &format!("Unknown auth type in provider config: {}", auth_type_str)
                );
            }
        }

        Ok(auth_configs)
    }

    /// 解析支持的认证类型字符串
    fn parse_supported_auth_types(&self, json_str: &str) -> Result<Vec<AuthType>> {
        let type_strings: Vec<String> = serde_json::from_str(json_str).map_err(|e| {
            ProxyError::config(&format!("Failed to parse supported_auth_types: {}", e))
        })?;

        let mut auth_types = Vec::new();
        for type_str in type_strings {
            if let Some(auth_type) = AuthType::from(&type_str) {
                auth_types.push(auth_type);
            } else {
                lwarn!(
                    "system",
                    LogStage::Configuration,
                    LogComponent::Config,
                    "unknown_auth_type",
                    &format!("Unknown auth type in configuration: {}", type_str)
                );
            }
        }

        Ok(auth_types)
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

    /// 获取服务商的完整API端点URL
    pub fn get_api_endpoint(&self, config: &ProviderConfig, path: &str) -> String {
        let path = if path.starts_with('/') {
            path
        } else {
            &format!("/{}", path)
        };
        format!("{}{}", config.https_url, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_base_url() {
        use crate::config::CacheConfig;

        let cache_config = CacheConfig::default();
        let cache = CacheManager::new(&cache_config).unwrap();
        let manager =
            ProviderConfigManager::new(Arc::new(DatabaseConnection::default()), Arc::new(cache));

        assert_eq!(
            manager.normalize_base_url("https://api.example.com"),
            "api.example.com"
        );
        assert_eq!(
            manager.normalize_base_url("http://api.example.com"),
            "api.example.com"
        );
        assert_eq!(
            manager.normalize_base_url("api.example.com"),
            "api.example.com"
        );
        assert_eq!(
            manager.normalize_base_url("api.example.com:8080"),
            "api.example.com:8080"
        );
    }

    #[test]
    fn test_generate_upstream_address() {
        use crate::config::CacheConfig;

        let cache_config = CacheConfig::default();
        let cache = CacheManager::new(&cache_config).unwrap();
        let manager =
            ProviderConfigManager::new(Arc::new(DatabaseConnection::default()), Arc::new(cache));

        assert_eq!(
            manager
                .generate_upstream_address("api.example.com")
                .unwrap(),
            "api.example.com:443"
        );
        assert_eq!(
            manager
                .generate_upstream_address("api.example.com:8080")
                .unwrap(),
            "api.example.com:8080"
        );
        assert_eq!(
            manager
                .generate_upstream_address("https://api.example.com")
                .unwrap(),
            "api.example.com:443"
        );
    }

    #[test]
    fn test_api_endpoint_generation() {
        use crate::config::CacheConfig;

        let cache_config = CacheConfig::default();
        let cache = CacheManager::new(&cache_config).unwrap();
        let manager =
            ProviderConfigManager::new(Arc::new(DatabaseConnection::default()), Arc::new(cache));

        // 使用测试数据而不是默认配置
        let test_config = ProviderConfig {
            id: 1,
            name: "test_openai".to_string(),
            display_name: "Test OpenAI".to_string(),
            base_url: "api.openai.com".to_string(),
            https_url: "https://api.openai.com".to_string(),
            upstream_address: "api.openai.com:443".to_string(),
            api_format: "openai".to_string(),
            default_model: Some("gpt-3.5-turbo".to_string()),
            max_tokens: Some(4096),
            rate_limit: Some(100),
            timeout_seconds: Some(30),
            health_check_path: "/models".to_string(),
            is_active: true,
            config_json: None,
            token_mappings_json: None,
            model_extraction_json: None,
            supported_auth_types: vec![AuthType::ApiKey],
            auth_configs: None,
        };

        assert_eq!(
            manager.get_api_endpoint(&test_config, "/v1/chat/completions"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            manager.get_api_endpoint(&test_config, "v1/models"),
            "https://api.openai.com/v1/models"
        );
    }

    #[test]
    fn test_parse_auth_configs_from_map() {
        use crate::config::CacheConfig;

        let cache_config = CacheConfig::default();
        let cache = CacheManager::new(&cache_config).unwrap();
        let manager =
            ProviderConfigManager::new(Arc::new(DatabaseConnection::default()), Arc::new(cache));

        // 测试gemini格式的配置
        let gemini_config = r#"{
            "api_key": {},
            "oauth": {
                "client_id": "test-client-id",
                "client_secret": "test-secret"
            }
        }"#;

        let result = manager.parse_auth_configs_from_map(gemini_config).unwrap();
        assert_eq!(result.len(), 2);

        // 检查包含的认证类型
        let auth_types: Vec<AuthType> = result.iter().map(|c| c.auth_type.clone()).collect();
        assert!(auth_types.contains(&AuthType::ApiKey));
        assert!(auth_types.contains(&AuthType::OAuth));

        // 验证 extra_config 正确设置
        for config in &result {
            match config.auth_type {
                AuthType::ApiKey => assert!(config.extra_config.is_none()), // 空对象
                AuthType::OAuth => assert!(config.extra_config.is_some()),  // 有配置
            }
        }

        // 测试包含未知类型的配置
        let mixed_config = r#"{
            "api_key": {},
            "oauth": {"client_id": "test"},
            "unknown_type": {}
        }"#;

        let result = manager.parse_auth_configs_from_map(mixed_config).unwrap();
        assert_eq!(result.len(), 2); // unknown_type 被跳过

        // 检查包含的认证类型，不依赖顺序
        let auth_types: Vec<AuthType> = result.iter().map(|c| c.auth_type.clone()).collect();
        assert!(auth_types.contains(&AuthType::ApiKey));
        assert!(auth_types.contains(&AuthType::OAuth));
    }
}
