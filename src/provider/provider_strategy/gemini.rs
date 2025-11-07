use std::collections::HashMap;

use crate::provider::{
    AuthorizationRequest, OauthProvider, ProviderType, TokenExchangeContext, TokenRefreshContext,
    TokenRequestPayload, TokenRevokeContext,
};

#[derive(Debug)]
pub struct GeminiProvider;

impl OauthProvider for GeminiProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Gemini
    }

    fn build_authorization_request(
        &self,
        request: &mut AuthorizationRequest<'_>,
        _session: &entity::oauth_client_sessions::Model,
        _config: &crate::auth::types::OAuthProviderConfig,
    ) {
        request.insert_if_absent("access_type", "offline");
        request.insert_if_absent("include_granted_scopes", "true");
        request.insert_if_absent("prompt", "consent");
    }

    fn build_token_request(&self, context: TokenExchangeContext<'_>) -> TokenRequestPayload {
        let mut form = context.base_form();
        apply_google_oauth_params(&mut form);
        TokenRequestPayload::new(context.config.token_url.clone(), form)
    }

    fn build_refresh_request(&self, context: TokenRefreshContext<'_>) -> TokenRequestPayload {
        let mut form = context.base_form();
        apply_google_oauth_params(&mut form);
        TokenRequestPayload::new(context.config.token_url.clone(), form)
    }

    fn build_revoke_request(&self, context: TokenRevokeContext<'_>) -> Option<TokenRequestPayload> {
        let form = context.base_form();
        Some(TokenRequestPayload::new(
            "https://oauth2.googleapis.com/revoke",
            form,
        ))
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
