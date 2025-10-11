//! # `OAuth提供商配置管理`
//!
//! 管理公共OAuth配置，包括Google/Gemini、Claude、OpenAI等服务商的公共客户端凭据
//! 实现动态配置加载、授权URL生成和PKCE参数管理

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::doc_markdown)]

use super::{OAuthError, OAuthProviderConfig, OAuthResult};
use crate::ldebug;
use crate::logging::{LogComponent, LogStage};
// use crate::auth::oauth_client::pkce::PkceChallenge; // 未使用
use entity::{ProviderTypes, provider_types};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
// use serde_json; // 未使用
use std::collections::HashMap;
use url::Url;

/// `OAuth提供商管理器`
#[derive(Debug, Clone)]
pub struct OAuthProviderManager {
    db: std::sync::Arc<DatabaseConnection>,
    cache: std::sync::Arc<std::sync::RwLock<HashMap<String, OAuthProviderConfig>>>,
}

impl OAuthProviderManager {
    /// 创建新的提供商管理器
    #[must_use]
    pub fn new(db: std::sync::Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            cache: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    /// 获取提供商配置
    #[allow(clippy::cognitive_complexity)]
    pub async fn get_config(&self, provider_name: &str) -> OAuthResult<OAuthProviderConfig> {
        ldebug!(
            "system",
            LogStage::Configuration,
            LogComponent::OAuth,
            "get_provider_config",
            &format!("🔍 [OAuth] 获取提供商配置: provider_name={}", provider_name)
        );

        // 先检查缓存
        if let Some(config) = self.get_from_cache(provider_name) {
            ldebug!(
                "system",
                LogStage::Cache,
                LogComponent::OAuth,
                "get_provider_config_cache_hit",
                &format!(
                    "✅ [OAuth] 从缓存获取配置成功: provider_name={}",
                    provider_name
                )
            );
            return Ok(config);
        }

        ldebug!(
            "system",
            LogStage::Db,
            LogComponent::OAuth,
            "load_provider_config_from_db",
            &format!(
                "📡 [OAuth] 从数据库加载配置: provider_name={}",
                provider_name
            )
        );

        // 从数据库加载
        let config = self.load_from_db(provider_name).await?;

        // 更新缓存
        self.update_cache(provider_name.to_string(), config.clone());

        ldebug!(
            "system",
            LogStage::Configuration,
            LogComponent::OAuth,
            "provider_config_cached",
            &format!(
                "💾 [OAuth] 配置加载完成并缓存: provider_name={}, client_id={}, authorize_url={}",
                provider_name, config.client_id, config.authorize_url
            )
        );

        Ok(config)
    }

    /// 获取所有活跃的提供商配置
    pub async fn list_active_configs(&self) -> OAuthResult<Vec<OAuthProviderConfig>> {
        let models = ProviderTypes::find()
            .filter(provider_types::Column::IsActive.eq(true))
            .all(self.db.as_ref())
            .await?;

        let mut configs = Vec::new();
        for model in models {
            // 获取该Provider支持的所有OAuth配置类型
            let oauth_types = model.get_oauth_types();
            for oauth_type in oauth_types {
                if let Ok(Some(oauth_config)) = model.get_oauth_config(&oauth_type) {
                    let config = Self::oauth_model_to_config(&model, &oauth_type, oauth_config);
                    configs.push(config);
                }
            }
        }

        Ok(configs)
    }

    /// `构建授权URL`
    #[allow(clippy::cognitive_complexity)]
    #[allow(clippy::map_unwrap_or)]
    pub fn build_authorize_url(
        &self,
        config: &OAuthProviderConfig,
        session: &entity::oauth_client_sessions::Model,
    ) -> OAuthResult<String> {
        ldebug!(
            "system",
            LogStage::Authentication,
            LogComponent::OAuth,
            "build_auth_url",
            &format!(
                "🔗 [OAuth] 开始构建授权URL: provider_name={}, session_id={}",
                config.provider_name, session.session_id
            )
        );

        let mut url = Url::parse(&config.authorize_url)
            .map_err(|e| OAuthError::NetworkError(format!("Invalid authorize URL: {}", e)))?;

        // 基础参数
        let scope = config.scopes.join(" ");
        let mut params = vec![
            ("client_id", config.client_id.as_str()),
            ("redirect_uri", config.redirect_uri.as_str()),
            ("state", &session.state),
            ("scope", &scope),
        ];

        // 添加response_type，优先使用配置中的值，否则使用默认值
        let response_type = config
            .extra_params
            .get("response_type")
            .map(String::as_str)
            .unwrap_or("code");
        params.push(("response_type", response_type));

        ldebug!(
            "system",
            LogStage::Authentication,
            LogComponent::OAuth,
            "auth_url_base_params",
            &format!(
                "⚙️ [OAuth] 基础参数: client_id={}, redirect_uri={}, response_type={}, scopes={}",
                config.client_id, config.redirect_uri, response_type, scope
            )
        );

        // PKCE参数
        if config.pkce_required {
            params.push(("code_challenge", &session.code_challenge));
            params.push(("code_challenge_method", "S256"));
            ldebug!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "auth_url_pkce_added",
                "🔐 [OAuth] PKCE参数已添加: code_challenge_method=S256"
            );
        }

        // 额外参数（排除已经添加的参数）
        let already_added = params
            .iter()
            .map(|(k, _)| *k)
            .collect::<std::collections::HashSet<_>>();
        let extra_params: Vec<(&str, &str)> = config
            .extra_params
            .iter()
            .filter(|(k, _)| !already_added.contains(k.as_str()))
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        if !extra_params.is_empty() {
            ldebug!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "auth_url_extra_params",
                &format!("📋 [OAuth] 额外参数: {:?}", extra_params)
            );
            params.extend(extra_params);
        }

        // 设置查询参数
        url.query_pairs_mut().extend_pairs(params);

        let final_url = url.to_string();
        ldebug!(
            "system",
            LogStage::Authentication,
            LogComponent::OAuth,
            "auth_url_build_complete",
            &format!(
                "🌐 [OAuth] 授权URL构建完成: session_id={}, url_length={}",
                session.session_id,
                final_url.len()
            )
        );

        Ok(final_url)
    }

    /// 刷新缓存
    #[allow(clippy::significant_drop_tightening)]
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

    /// `验证提供商是否支持OAuth`
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
            .one(self.db.as_ref())
            .await?;

        match model {
            Some(model) => {
                // 先尝试指定的OAuth类型
                if let Ok(Some(oauth_config)) = model.get_oauth_config(oauth_type) {
                    return Ok(Self::oauth_model_to_config(
                        &model,
                        oauth_type,
                        oauth_config,
                    ));
                }

                // 如果指定类型不存在，尝试其他OAuth类型
                let oauth_types = model.get_oauth_types();
                for available_type in oauth_types {
                    if let Ok(Some(oauth_config)) = model.get_oauth_config(&available_type) {
                        return Ok(Self::oauth_model_to_config(
                            &model,
                            &available_type,
                            oauth_config,
                        ));
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

    /// `将OAuth配置转换为OAuthProviderConfig`
    fn oauth_model_to_config(
        model: &provider_types::Model,
        oauth_type: &str,
        oauth_config: entity::provider_types::OAuthConfig,
    ) -> OAuthProviderConfig {
        // 解析作用域
        let scopes: Vec<String> = oauth_config
            .scopes
            .split_whitespace()
            .map(str::to_string)
            .collect();

        // 构建额外参数 - 完全数据库驱动
        let mut extra_params = HashMap::new();

        // 直接使用数据库配置的extra_params，包含所有需要的参数
        if let Some(ref config_extra_params) = oauth_config.extra_params {
            extra_params.extend(config_extra_params.clone());
            ldebug!(
                "system",
                LogStage::Db,
                LogComponent::OAuth,
                "load_extra_params",
                &format!(
                    "📊 [OAuth] 从数据库加载了{}个额外参数: {:?}",
                    extra_params.len(),
                    extra_params.keys().collect::<Vec<_>>()
                )
            );
        } else {
            ldebug!(
                "system",
                LogStage::Db,
                LogComponent::OAuth,
                "no_extra_params",
                "📊 [OAuth] 数据库中没有配置extra_params"
            );
        }

        // 创建最终配置对象
        OAuthProviderConfig {
            provider_name: format!("{}:{}", model.name, oauth_type),
            client_id: oauth_config.client_id,
            client_secret: oauth_config.client_secret,
            authorize_url: oauth_config.authorize_url,
            token_url: oauth_config.token_url,
            redirect_uri: oauth_config.redirect_uri.unwrap_or_default(),
            scopes,
            pkce_required: oauth_config.pkce_required,
            extra_params,
        }
    }

    /// 保留原有的build_extra_params方法用于向后兼容
    /// 现在从数据库配置中读取，而不是硬编码
    fn build_extra_params(config: &OAuthProviderConfig) -> HashMap<String, String> {
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
    #[must_use]
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
    #[must_use]
    pub fn client_id(mut self, client_id: &str) -> Self {
        self.config.client_id = client_id.to_string();
        self
    }

    /// 设置客户端密钥
    #[must_use]
    pub fn client_secret(mut self, client_secret: Option<&str>) -> Self {
        self.config.client_secret = client_secret.map(str::to_string);
        self
    }

    /// 设置授权URL
    #[must_use]
    pub fn authorize_url(mut self, authorize_url: &str) -> Self {
        self.config.authorize_url = authorize_url.to_string();
        self
    }

    /// 设置令牌URL
    #[must_use]
    pub fn token_url(mut self, token_url: &str) -> Self {
        self.config.token_url = token_url.to_string();
        self
    }

    /// 设置重定向URI
    #[must_use]
    pub fn redirect_uri(mut self, redirect_uri: &str) -> Self {
        self.config.redirect_uri = redirect_uri.to_string();
        self
    }

    /// 设置作用域
    #[must_use]
    pub fn scopes(mut self, scopes: Vec<&str>) -> Self {
        self.config.scopes = scopes.into_iter().map(str::to_string).collect();
        self
    }

    /// 设置是否需要PKCE
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn pkce_required(mut self, required: bool) -> Self {
        self.config.pkce_required = required;
        self
    }

    /// 添加额外参数
    #[must_use]
    pub fn extra_param(mut self, key: &str, value: &str) -> Self {
        self.config
            .extra_params
            .insert(key.to_string(), value.to_string());
        self
    }

    /// 构建配置
    #[must_use]
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
