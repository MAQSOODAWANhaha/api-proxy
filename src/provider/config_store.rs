use super::types::ProviderType;
use crate::auth::types::OAuthProviderConfig;
use crate::error::{ProxyError, Result, auth::OAuthError};
use crate::ldebug;
use crate::logging::{LogComponent, LogStage};
use entity::{ProviderTypes, provider_types};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// `OAuth提供商配置管理器`
#[derive(Clone)]
pub struct ApiKeyProviderConfig {
    db: Arc<DatabaseConnection>,
}

impl ApiKeyProviderConfig {
    #[must_use]
    pub const fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn get_config(&self, provider_name: &str) -> Result<OAuthProviderConfig> {
        self.load_config_from_db(provider_name).await
    }

    pub async fn list_active_configs(&self) -> Result<Vec<OAuthProviderConfig>> {
        let models = ProviderTypes::find()
            .filter(provider_types::Column::IsActive.eq(true))
            .all(self.db.as_ref())
            .await?;

        let mut configs = Vec::new();
        for model in models {
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

    pub async fn supports_oauth(&self, provider_name: &str) -> Result<bool> {
        match self.get_config(provider_name).await {
            Ok(_) => Ok(true),
            Err(err) => match err {
                ProxyError::Authentication(crate::error::auth::AuthError::OAuth(
                    OAuthError::ProviderNotFound(_),
                )) => Ok(false),
                other => Err(other),
            },
        }
    }

    pub async fn fetch_redirect_uri(&self, provider_name: &str) -> Result<String> {
        let config = self.get_config(provider_name).await?;
        Ok(config.redirect_uri)
    }

    async fn load_config_from_db(&self, provider_name: &str) -> Result<OAuthProviderConfig> {
        let oauth_type = provider_name.split(':').nth(1).unwrap_or("oauth");
        let provider_type = ProviderType::parse(provider_name)?;
        let base_provider = provider_type.db_name();

        let model = ProviderTypes::find()
            .filter(provider_types::Column::Name.eq(base_provider))
            .filter(provider_types::Column::AuthType.eq(oauth_type))
            .filter(provider_types::Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await?;

        match model {
            Some(model) => {
                if let Ok(Some(oauth_config)) = model.get_oauth_config(oauth_type) {
                    return Ok(Self::oauth_model_to_config(
                        &model,
                        oauth_type,
                        oauth_config,
                    ));
                }

                Err(OAuthError::ProviderNotFound(format!(
                    "No OAuth config found for provider: {provider_name}"
                ))
                .into())
            }
            None => Err(OAuthError::ProviderNotFound(provider_name.to_string()).into()),
        }
    }

    fn oauth_model_to_config(
        model: &provider_types::Model,
        oauth_type: &str,
        oauth_config: entity::provider_types::OAuthConfig,
    ) -> OAuthProviderConfig {
        let scopes: Vec<String> = oauth_config
            .scopes
            .split_whitespace()
            .map(str::to_string)
            .collect();

        let mut extra_params: HashMap<String, serde_json::Value> = HashMap::new();

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
}

impl ApiKeyProviderConfig {
    // 历史上曾缓存 OAuth 配置；目前为避免陈旧配置导致鉴权失败，已禁用该缓存。
}

impl fmt::Debug for ApiKeyProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiKeyProviderConfig").finish()
    }
}

/// 提供商特定的配置构建器
pub struct ProviderConfigBuilder {
    config: OAuthProviderConfig,
}

impl ProviderConfigBuilder {
    #[must_use]
    pub fn new(provider_name: &str) -> Self {
        Self {
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

    #[must_use]
    pub fn client_id(mut self, client_id: &str) -> Self {
        self.config.client_id = client_id.to_string();
        self
    }

    #[must_use]
    pub fn client_secret(mut self, client_secret: Option<&str>) -> Self {
        self.config.client_secret = client_secret.map(str::to_string);
        self
    }

    #[must_use]
    pub fn authorize_url(mut self, authorize_url: &str) -> Self {
        self.config.authorize_url = authorize_url.to_string();
        self
    }

    #[must_use]
    pub fn token_url(mut self, token_url: &str) -> Self {
        self.config.token_url = token_url.to_string();
        self
    }

    #[must_use]
    pub fn redirect_uri(mut self, redirect_uri: &str) -> Self {
        self.config.redirect_uri = redirect_uri.to_string();
        self
    }

    #[must_use]
    pub fn scopes(mut self, scopes: Vec<&str>) -> Self {
        self.config.scopes = scopes.into_iter().map(str::to_string).collect();
        self
    }

    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn pkce_required(mut self, required: bool) -> Self {
        self.config.pkce_required = required;
        self
    }

    #[must_use]
    pub fn extra_param(mut self, key: &str, value: &str) -> Self {
        self.config.extra_params.insert(
            key.to_string(),
            serde_json::Value::String(value.to_string()),
        );
        self
    }

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
            Some(&serde_json::Value::String("value".to_string()))
        );
    }
}
