use crate::auth::types::OAuthProviderConfig;
use entity::oauth_client_sessions;
use std::borrow::Cow;
use std::collections::HashMap;

#[derive(Debug)]
pub struct AuthorizationRequest<'a> {
    params: Vec<(Cow<'a, str>, String)>,
}

impl<'a> AuthorizationRequest<'a> {
    #[must_use]
    pub const fn new(params: Vec<(Cow<'a, str>, String)>) -> Self {
        Self { params }
    }

    #[must_use]
    pub const fn params(&self) -> &Vec<(Cow<'a, str>, String)> {
        &self.params
    }

    #[must_use]
    pub fn into_params(self) -> Vec<(Cow<'a, str>, String)> {
        self.params
    }

    pub fn upsert(&mut self, key: impl Into<Cow<'a, str>>, value: impl Into<String>) {
        let key_cow = key.into();
        if let Some(entry) = self
            .params
            .iter_mut()
            .find(|(existing, _)| existing == &key_cow)
        {
            entry.1 = value.into();
        } else {
            self.params.push((key_cow, value.into()));
        }
    }

    pub fn insert_if_absent(&mut self, key: impl Into<Cow<'a, str>>, value: impl Into<String>) {
        let key_cow = key.into();
        if !self.params.iter().any(|(existing, _)| existing == &key_cow) {
            self.params.push((key_cow, value.into()));
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenRequestPayload {
    pub url: String,
    pub form: HashMap<String, String>,
}

impl TokenRequestPayload {
    #[must_use]
    pub fn new(url: impl Into<String>, form: HashMap<String, String>) -> Self {
        Self {
            url: url.into(),
            form,
        }
    }

    #[must_use]
    pub fn into_parts(self) -> (String, HashMap<String, String>) {
        (self.url, self.form)
    }
}

#[derive(Debug)]
pub struct TokenExchangeContext<'a> {
    pub session: &'a oauth_client_sessions::Model,
    pub config: &'a OAuthProviderConfig,
    pub authorization_code: &'a str,
}

impl TokenExchangeContext<'_> {
    #[must_use]
    pub fn base_form(&self) -> HashMap<String, String> {
        let mut form = HashMap::new();
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
    pub fn base_form(&self) -> HashMap<String, String> {
        let mut form = HashMap::new();
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
    pub fn base_form(&self) -> HashMap<String, String> {
        let mut form = HashMap::new();
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
