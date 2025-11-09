use crate::provider::traits::OauthProvider;
use crate::provider::{
    ProviderType, TokenExchangeContext, TokenRefreshContext, TokenRequestPayload,
    create_token_request, into_token_request,
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
        into_token_request(create_token_request(context.config.token_url.clone(), form))
    }

    fn build_refresh_request(&self, context: TokenRefreshContext<'_>) -> TokenRequestPayload {
        let mut form = context.base_form();
        form.insert(
            "client_secret".to_string(),
            context.session.code_verifier.clone(),
        );
        into_token_request(create_token_request(context.config.token_url.clone(), form))
    }
}
