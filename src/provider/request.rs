use crate::auth::types::OAuthProviderConfig;
use entity::oauth_client_sessions;
/// Token请求的简化表示，直接使用元组
pub type TokenRequestPayload = (String, std::collections::HashMap<String, String, std::hash::RandomState>);

/// 创建Token请求payload的便利函数
#[must_use]
pub fn create_token_request<S: std::hash::BuildHasher>(
    url: impl Into<String>,
    form: std::collections::HashMap<String, String, S>,
) -> (String, std::collections::HashMap<String, String, S>) {
    (url.into(), form)
}

/// 转换为标准 `TokenRequestPayload` 类型
#[must_use]
pub fn into_token_request<S: std::hash::BuildHasher>(
    payload: (String, std::collections::HashMap<String, String, S>)
) -> TokenRequestPayload {
    // 转换 HashMap 的 hasher 类型为 RandomState
    let (url, form) = payload;
    let random_form: std::collections::HashMap<String, String, std::hash::RandomState> =
        form.into_iter().collect();
    (url, random_form)
}

#[derive(Debug)]
pub struct TokenExchangeContext<'a> {
    pub session: &'a oauth_client_sessions::Model,
    pub config: &'a OAuthProviderConfig,
    pub authorization_code: &'a str,
}

impl TokenExchangeContext<'_> {
    #[must_use]
    pub fn base_form(&self) -> std::collections::HashMap<String, String, std::hash::RandomState> {
        let mut form = std::collections::HashMap::default();
        form.insert("grant_type".to_string(), "authorization_code".to_string());
        form.insert("code".to_string(), self.authorization_code.to_string());
        form.insert("client_id".to_string(), self.config.client_id.clone());

        if let Some(secret) = &self.config.client_secret {
            form.insert("client_secret".to_string(), secret.clone());
        }

        form.insert("redirect_uri".to_string(), self.config.redirect_uri.clone());

        if self.config.pkce_required {
            form.insert(
                "code_verifier".to_string(),
                self.session.code_verifier.clone(),
            );
        }

        form
    }
}

#[derive(Debug)]
pub struct TokenRefreshContext<'a> {
    pub session: &'a oauth_client_sessions::Model,
    pub config: &'a OAuthProviderConfig,
    pub refresh_token: &'a str,
}

impl TokenRefreshContext<'_> {
    #[must_use]
    pub fn base_form(&self) -> std::collections::HashMap<String, String, std::hash::RandomState> {
        let mut form = std::collections::HashMap::default();
        form.insert("grant_type".to_string(), "refresh_token".to_string());
        form.insert("refresh_token".to_string(), self.refresh_token.to_string());
        form.insert("client_id".to_string(), self.config.client_id.clone());

        if let Some(secret) = &self.config.client_secret {
            form.insert("client_secret".to_string(), secret.clone());
        }

        form
    }
}

#[derive(Debug)]
pub struct TokenRevokeContext<'a> {
    pub session: &'a oauth_client_sessions::Model,
    pub config: &'a OAuthProviderConfig,
    pub token: &'a str,
    pub hint: Option<&'a str>,
}

impl TokenRevokeContext<'_> {
    #[must_use]
    pub fn base_form(&self) -> std::collections::HashMap<String, String, std::hash::RandomState> {
        let mut form = std::collections::HashMap::default();
        form.insert("token".to_string(), self.token.to_string());
        form.insert("client_id".to_string(), self.config.client_id.clone());

        if let Some(secret) = &self.config.client_secret {
            form.insert("client_secret".to_string(), secret.clone());
        }

        if let Some(hint) = self.hint {
            form.insert("token_type_hint".to_string(), hint.to_string());
        }

        form
    }
}
