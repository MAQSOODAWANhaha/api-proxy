use std::collections::HashMap;

use crate::provider::traits::OauthProvider;
use crate::provider::{
    ProviderType, TokenExchangeContext, TokenRefreshContext, TokenRequestPayload,
    TokenRevokeContext, create_token_request, into_token_request,
};

#[derive(Debug)]
pub struct GeminiProvider;

impl OauthProvider for GeminiProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Gemini
    }

    fn build_authorization_url(
        &self,
        params: &mut std::collections::HashMap<String, String>,
        _session: &entity::oauth_client_sessions::Model,
        _config: &crate::auth::types::OAuthProviderConfig,
    ) {
        params
            .entry("access_type".to_string())
            .or_insert_with(|| "offline".to_string());
        params
            .entry("include_granted_scopes".to_string())
            .or_insert_with(|| "true".to_string());
        params
            .entry("prompt".to_string())
            .or_insert_with(|| "consent".to_string());
    }

    fn build_token_request(&self, context: TokenExchangeContext<'_>) -> TokenRequestPayload {
        let mut form = context.base_form();
        apply_google_oauth_params(&mut form);
        into_token_request(create_token_request(context.config.token_url.clone(), form))
    }

    fn build_refresh_request(&self, context: TokenRefreshContext<'_>) -> TokenRequestPayload {
        let mut form = context.base_form();
        apply_google_oauth_params(&mut form);
        into_token_request(create_token_request(context.config.token_url.clone(), form))
    }

    fn build_revoke_request(&self, context: TokenRevokeContext<'_>) -> Option<TokenRequestPayload> {
        let form = context.base_form();
        Some(into_token_request(create_token_request(
            "https://oauth2.googleapis.com/revoke",
            form,
        )))
    }
}

fn apply_google_oauth_params(form: &mut HashMap<String, String>) {
    form.entry("access_type".to_string())
        .or_insert_with(|| "offline".to_string());
    form.entry("include_granted_scopes".to_string())
        .or_insert_with(|| "true".to_string());
    form.entry("prompt".to_string())
        .or_insert_with(|| "consent".to_string());
}
