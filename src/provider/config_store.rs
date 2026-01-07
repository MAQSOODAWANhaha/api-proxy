use crate::auth::types::{OAuthAuthorizeConfig, OAuthProviderConfig, OAuthTokenConfig};
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
        let base_provider = provider_name.split(':').next().unwrap_or(provider_name);

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
        let authorize = OAuthAuthorizeConfig {
            url: oauth_config.authorize.url,
            method: oauth_config.authorize.method,
            headers: oauth_config.authorize.headers,
            query: oauth_config.authorize.query,
        };
        let exchange = OAuthTokenConfig {
            url: oauth_config.exchange.url,
            method: oauth_config.exchange.method,
            headers: oauth_config.exchange.headers,
            body: oauth_config.exchange.body,
        };
        let refresh = OAuthTokenConfig {
            url: oauth_config.refresh.url,
            method: oauth_config.refresh.method,
            headers: oauth_config.refresh.headers,
            body: oauth_config.refresh.body,
        };

        ldebug!(
            "system",
            LogStage::Db,
            LogComponent::OAuth,
            "load_oauth_config",
            &format!(
                "📊 [OAuth] 加载 provider 配置: name={}, auth_type={}",
                model.name, oauth_type
            )
        );

        OAuthProviderConfig {
            provider_name: format!("{}:{}", model.name, oauth_type),
            client_id: oauth_config.client_id,
            client_secret: oauth_config.client_secret,
            redirect_uri: oauth_config.redirect_uri.unwrap_or_default(),
            pkce_required: oauth_config.pkce_required,
            scopes: oauth_config.scopes,
            authorize,
            exchange,
            refresh,
            extra: oauth_config.extra,
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
        let mut authorize_query = HashMap::new();
        // 默认填充基础参数，便于测试直接生成可用的授权 URL。
        authorize_query.insert(
            "client_id".to_string(),
            serde_json::Value::String("{{client_id}}".to_string()),
        );
        authorize_query.insert(
            "redirect_uri".to_string(),
            serde_json::Value::String("{{redirect_uri}}".to_string()),
        );
        authorize_query.insert(
            "state".to_string(),
            serde_json::Value::String("{{session.state}}".to_string()),
        );
        authorize_query.insert(
            "scope".to_string(),
            serde_json::Value::String("{{scopes}}".to_string()),
        );
        authorize_query.insert(
            "response_type".to_string(),
            serde_json::Value::String("code".to_string()),
        );
        // 默认开启 PKCE 并填充占位符
        authorize_query.insert(
            "code_challenge".to_string(),
            serde_json::Value::String("{{session.code_challenge}}".to_string()),
        );
        authorize_query.insert(
            "code_challenge_method".to_string(),
            serde_json::Value::String("S256".to_string()),
        );

        Self {
            config: OAuthProviderConfig {
                provider_name: provider_name.to_string(),
                client_id: String::new(),
                client_secret: None,
                redirect_uri: String::new(),
                scopes: String::new(),
                pkce_required: true,
                authorize: OAuthAuthorizeConfig {
                    url: String::new(),
                    method: "GET".to_string(),
                    headers: HashMap::new(),
                    query: authorize_query,
                },
                exchange: OAuthTokenConfig {
                    url: String::new(),
                    method: "POST".to_string(),
                    headers: HashMap::new(),
                    body: HashMap::new(),
                },
                refresh: OAuthTokenConfig {
                    url: String::new(),
                    method: "POST".to_string(),
                    headers: HashMap::new(),
                    body: HashMap::new(),
                },
                extra: HashMap::new(),
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
        self.config.authorize.url = authorize_url.to_string();
        self
    }

    #[must_use]
    pub fn token_url(mut self, token_url: &str) -> Self {
        self.config.exchange.url = token_url.to_string();
        self.config.refresh.url = token_url.to_string();
        self
    }

    #[must_use]
    pub fn redirect_uri(mut self, redirect_uri: &str) -> Self {
        self.config.redirect_uri = redirect_uri.to_string();
        self
    }

    #[must_use]
    pub fn scopes(mut self, scopes: &[&str]) -> Self {
        self.config.scopes = scopes.join(" ");
        self
    }

    #[must_use]
    pub fn pkce_required(mut self, required: bool) -> Self {
        self.config.pkce_required = required;
        if required {
            self.config
                .authorize
                .query
                .entry("code_challenge".to_string())
                .or_insert_with(|| {
                    serde_json::Value::String("{{session.code_challenge}}".to_string())
                });
            self.config
                .authorize
                .query
                .entry("code_challenge_method".to_string())
                .or_insert_with(|| serde_json::Value::String("S256".to_string()));
        } else {
            self.config.authorize.query.remove("code_challenge");
            self.config.authorize.query.remove("code_challenge_method");
        }
        self
    }

    #[must_use]
    pub fn authorize_query_value(mut self, key: &str, value: serde_json::Value) -> Self {
        self.config.authorize.query.insert(key.to_string(), value);
        self
    }

    #[must_use]
    pub fn authorize_query_string(self, key: &str, value: &str) -> Self {
        self.authorize_query_value(key, serde_json::Value::String(value.to_string()))
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
            .scopes(&["read", "write"])
            .pkce_required(true)
            .authorize_query_string("custom", "value")
            .build();

        assert_eq!(config.provider_name, "test");
        assert_eq!(config.client_id, "test_client_id");
        assert_eq!(config.client_secret, Some("test_secret".to_string()));
        assert_eq!(config.scopes, "read write");
        assert_eq!(
            config.authorize.query.get("custom"),
            Some(&serde_json::Value::String("value".to_string()))
        );
    }
}
