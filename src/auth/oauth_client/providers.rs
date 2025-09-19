//! # OAuth提供商配置管理
//!
//! 管理公共OAuth配置，包括Google/Gemini、Claude、OpenAI等服务商的公共客户端凭据
//! 实现动态配置加载、授权URL生成和PKCE参数管理

use super::{OAuthError, OAuthProviderConfig, OAuthResult};
// use crate::auth::oauth_client::pkce::PkceChallenge; // 未使用
use entity::{ProviderTypes, provider_types};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
// use serde_json; // 未使用
use std::collections::HashMap;
use url::Url;

/// OAuth提供商管理器
#[derive(Debug, Clone)]
pub struct OAuthProviderManager {
    db: DatabaseConnection,
    cache: std::sync::Arc<std::sync::RwLock<HashMap<String, OAuthProviderConfig>>>,
}

impl OAuthProviderManager {
    /// 创建新的提供商管理器
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            cache: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    /// 获取提供商配置
    pub async fn get_config(&self, provider_name: &str) -> OAuthResult<OAuthProviderConfig> {
        // 先检查缓存
        if let Some(config) = self.get_from_cache(provider_name) {
            return Ok(config);
        }

        // 从数据库加载
        let config = self.load_from_db(provider_name).await?;

        // 更新缓存
        self.update_cache(provider_name.to_string(), config.clone());

        Ok(config)
    }

    /// 获取所有活跃的提供商配置
    pub async fn list_active_configs(&self) -> OAuthResult<Vec<OAuthProviderConfig>> {
        let models = ProviderTypes::find()
            .filter(provider_types::Column::IsActive.eq(true))
            .all(&self.db)
            .await?;

        let mut configs = Vec::new();
        for model in models {
            // 获取该Provider支持的所有OAuth配置类型
            let oauth_types = model.get_oauth_types();
            for oauth_type in oauth_types {
                if let Ok(Some(oauth_config)) = model.get_oauth_config(&oauth_type) {
                    let config = self.oauth_model_to_config(&model, &oauth_type, oauth_config)?;
                    configs.push(config);
                }
            }
        }

        Ok(configs)
    }

    /// 构建授权URL
    pub fn build_authorize_url(
        &self,
        config: &OAuthProviderConfig,
        session: &entity::oauth_client_sessions::Model,
    ) -> OAuthResult<String> {
        let mut url = Url::parse(&config.authorize_url)
            .map_err(|e| OAuthError::NetworkError(format!("Invalid authorize URL: {}", e)))?;

        // 基础参数
        let scope = config.scopes.join(" ");
        let mut params = vec![
            ("client_id", config.client_id.as_str()),
            ("redirect_uri", config.redirect_uri.as_str()),
            ("response_type", "code"),
            ("state", &session.state),
            ("scope", &scope),
        ];

        // PKCE参数
        if config.pkce_required {
            params.push(("code_challenge", &session.code_challenge));
            params.push(("code_challenge_method", "S256"));
        }

        // 额外参数
        let extra_params: Vec<(&str, &str)> = config
            .extra_params
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        params.extend(extra_params);

        // 设置查询参数
        url.query_pairs_mut().extend_pairs(params);

        Ok(url.to_string())
    }

    /// 刷新缓存
    pub async fn refresh_cache(&self) -> OAuthResult<()> {
        let configs = self.list_active_configs().await?;
        let mut cache = self
            .cache
            .write()
            .map_err(|_| OAuthError::DatabaseError("Cache lock error".to_string()))?;

        cache.clear();
        for config in configs {
            cache.insert(config.provider_name.clone(), config);
        }

        Ok(())
    }

    /// 验证提供商是否支持OAuth
    pub async fn is_oauth_supported(&self, provider_name: &str) -> OAuthResult<bool> {
        match self.get_config(provider_name).await {
            Ok(_) => Ok(true),
            Err(OAuthError::ProviderNotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// 获取提供商的重定向URI
    pub async fn get_redirect_uri(&self, provider_name: &str) -> OAuthResult<String> {
        let config = self.get_config(provider_name).await?;
        Ok(config.redirect_uri)
    }

    // 私有方法

    /// 从缓存获取配置
    fn get_from_cache(&self, provider_name: &str) -> Option<OAuthProviderConfig> {
        let cache = self.cache.read().ok()?;
        cache.get(provider_name).cloned()
    }

    /// 从数据库加载配置
    async fn load_from_db(&self, provider_name: &str) -> OAuthResult<OAuthProviderConfig> {
        // 解析provider_name，格式可能是 "gemini" 或 "gemini:oauth"
        let (base_provider, oauth_type) = if provider_name.contains(':') {
            let parts: Vec<&str> = provider_name.split(':').collect();
            (parts[0], *parts.get(1).unwrap_or(&"oauth"))
        } else {
            // 默认查找OAuth配置
            (provider_name, "oauth")
        };

        let model = ProviderTypes::find()
            .filter(provider_types::Column::Name.eq(base_provider))
            .filter(provider_types::Column::IsActive.eq(true))
            .one(&self.db)
            .await?;

        match model {
            Some(model) => {
                // 先尝试指定的OAuth类型
                if let Ok(Some(oauth_config)) = model.get_oauth_config(oauth_type) {
                    return self.oauth_model_to_config(&model, oauth_type, oauth_config);
                }

                // 如果指定类型不存在，尝试其他OAuth类型
                let oauth_types = model.get_oauth_types();
                for available_type in oauth_types {
                    if let Ok(Some(oauth_config)) = model.get_oauth_config(&available_type) {
                        return self.oauth_model_to_config(&model, &available_type, oauth_config);
                    }
                }

                Err(OAuthError::ProviderNotFound(format!(
                    "No OAuth config found for provider: {}",
                    provider_name
                )))
            }
            None => Err(OAuthError::ProviderNotFound(provider_name.to_string())),
        }
    }

    /// 更新缓存
    fn update_cache(&self, provider_name: String, config: OAuthProviderConfig) {
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(provider_name, config);
        }
    }

    /// 将OAuth配置转换为OAuthProviderConfig
    fn oauth_model_to_config(
        &self,
        model: &provider_types::Model,
        oauth_type: &str,
        oauth_config: entity::provider_types::OAuthConfig,
    ) -> OAuthResult<OAuthProviderConfig> {
        // 解析作用域
        let scopes: Vec<String> = oauth_config
            .scopes
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        // 构建额外参数
        let mut extra_params = HashMap::new();

        // 添加传统的特定参数（向后兼容）
        if let Some(access_type) = oauth_config.access_type {
            extra_params.insert("access_type".to_string(), access_type);
        }
        if let Some(prompt) = oauth_config.prompt {
            extra_params.insert("prompt".to_string(), prompt);
        }
        if let Some(response_type) = oauth_config.response_type {
            extra_params.insert("response_type".to_string(), response_type);
        }
        if let Some(grant_type) = oauth_config.grant_type {
            extra_params.insert("grant_type".to_string(), grant_type);
        }

        // 添加通用extra_params（新增支持）
        if let Some(ref config_extra_params) = oauth_config.extra_params {
            extra_params.extend(config_extra_params.clone());
        }

        // 创建基础配置对象
        let base_config = OAuthProviderConfig {
            provider_name: format!("{}:{}", model.name, oauth_type),
            client_id: oauth_config.client_id,
            client_secret: oauth_config.client_secret,
            authorize_url: oauth_config.authorize_url,
            token_url: oauth_config.token_url,
            redirect_uri: oauth_config.redirect_uri.unwrap_or_default(),
            scopes: scopes.clone(),
            pkce_required: oauth_config.pkce_required,
            extra_params: extra_params.clone(),
        };

        // 添加提供商特定的额外参数
        let provider_params = self.build_extra_params(&base_config);
        extra_params.extend(provider_params);

        Ok(OAuthProviderConfig {
            scopes,
            extra_params,
            ..base_config
        })
    }

    /// 构建特定提供商的额外参数
    /// 现在从数据库配置中读取，而不是硬编码
    fn build_extra_params(&self, config: &OAuthProviderConfig) -> HashMap<String, String> {
        // 直接从配置中返回 extra_params，实现数据库驱动
        config.extra_params.clone()
    }
}

/// 提供商特定的配置构建器
pub struct ProviderConfigBuilder {
    provider_name: String,
    config: OAuthProviderConfig,
}

impl ProviderConfigBuilder {
    /// 创建新的配置构建器
    pub fn new(provider_name: &str) -> Self {
        Self {
            provider_name: provider_name.to_string(),
            config: OAuthProviderConfig {
                provider_name: provider_name.to_string(),
                client_id: String::new(),
                client_secret: None,
                authorize_url: String::new(),
                token_url: String::new(),
                redirect_uri: String::new(),
                scopes: Vec::new(),
                pkce_required: true,
                extra_params: HashMap::new(),
            },
        }
    }

    /// 设置客户端ID
    pub fn client_id(mut self, client_id: &str) -> Self {
        self.config.client_id = client_id.to_string();
        self
    }

    /// 设置客户端密钥
    pub fn client_secret(mut self, client_secret: Option<&str>) -> Self {
        self.config.client_secret = client_secret.map(|s| s.to_string());
        self
    }

    /// 设置授权URL
    pub fn authorize_url(mut self, authorize_url: &str) -> Self {
        self.config.authorize_url = authorize_url.to_string();
        self
    }

    /// 设置令牌URL
    pub fn token_url(mut self, token_url: &str) -> Self {
        self.config.token_url = token_url.to_string();
        self
    }

    /// 设置重定向URI
    pub fn redirect_uri(mut self, redirect_uri: &str) -> Self {
        self.config.redirect_uri = redirect_uri.to_string();
        self
    }

    /// 设置作用域
    pub fn scopes(mut self, scopes: Vec<&str>) -> Self {
        self.config.scopes = scopes.into_iter().map(|s| s.to_string()).collect();
        self
    }

    /// 设置是否需要PKCE
    pub fn pkce_required(mut self, required: bool) -> Self {
        self.config.pkce_required = required;
        self
    }

    /// 添加额外参数
    pub fn extra_param(mut self, key: &str, value: &str) -> Self {
        self.config
            .extra_params
            .insert(key.to_string(), value.to_string());
        self
    }

    /// 构建配置
    pub fn build(self) -> OAuthProviderConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_config_builder() {
        let config = ProviderConfigBuilder::new("test")
            .client_id("test_client_id")
            .client_secret(Some("test_secret"))
            .authorize_url("https://example.com/auth")
            .token_url("https://example.com/token")
            .redirect_uri("https://example.com/callback")
            .scopes(vec!["read", "write"])
            .pkce_required(true)
            .extra_param("custom", "value")
            .build();

        assert_eq!(config.provider_name, "test");
        assert_eq!(config.client_id, "test_client_id");
        assert_eq!(config.client_secret, Some("test_secret".to_string()));
        assert_eq!(config.scopes, vec!["read", "write"]);
        assert_eq!(
            config.extra_params.get("custom"),
            Some(&"value".to_string())
        );
    }
}
