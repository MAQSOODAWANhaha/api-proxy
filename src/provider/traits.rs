use crate::auth::types::OAuthProviderConfig;
use entity::oauth_client_sessions;

use super::request::{
    TokenExchangeContext, TokenRefreshContext, TokenRequestPayload, TokenRevokeContext,
    create_token_request, into_token_request,
};
use super::types::ProviderType;

pub trait OauthProvider: Send + Sync + std::fmt::Debug {
    fn provider_type(&self) -> ProviderType;

    fn build_authorization_url(
        &self,
        params: &mut std::collections::HashMap<String, String>,
        session: &oauth_client_sessions::Model,
        config: &OAuthProviderConfig,
    ) {
        let _ = (params, session, config);
    }

    fn build_token_request(&self, context: TokenExchangeContext<'_>) -> TokenRequestPayload {
        into_token_request(create_token_request(context.config.token_url.clone(), context.base_form()))
    }

    fn build_refresh_request(&self, context: TokenRefreshContext<'_>) -> TokenRequestPayload {
        into_token_request(create_token_request(context.config.token_url.clone(), context.base_form()))
    }

    fn build_revoke_request(&self, context: TokenRevokeContext<'_>) -> Option<TokenRequestPayload> {
        let _ = context;
        None
    }
}
