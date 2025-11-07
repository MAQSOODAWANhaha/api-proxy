use crate::provider::{
    OauthProvider, ProviderType, TokenExchangeContext, TokenRefreshContext, TokenRequestPayload,
};

#[derive(Debug)]
pub struct AnthropicProvider;

impl OauthProvider for AnthropicProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Anthropic
    }

    fn build_token_request(&self, context: TokenExchangeContext<'_>) -> TokenRequestPayload {
        let mut form = context.base_form();
        form.insert(
            "client_secret".to_string(),
            context.session.code_verifier.clone(),
        );
        TokenRequestPayload::new(context.config.token_url.clone(), form)
    }

    fn build_refresh_request(&self, context: TokenRefreshContext<'_>) -> TokenRequestPayload {
        let mut form = context.base_form();
        form.insert(
            "client_secret".to_string(),
            context.session.code_verifier.clone(),
        );
        TokenRequestPayload::new(context.config.token_url.clone(), form)
    }
}
