use crate::auth::types::OAuthProviderConfig;
use entity::oauth_client_sessions;

use super::request::{
    AuthorizationRequest, TokenExchangeContext, TokenRefreshContext, TokenRequestPayload,
    TokenRevokeContext,
};
use super::types::ProviderType;

pub trait OauthProvider: Send + Sync + std::fmt::Debug {
    fn provider_type(&self) -> ProviderType;

    fn build_authorization_request(
        &self,
        request: &mut AuthorizationRequest<'_>,
        session: &oauth_client_sessions::Model,
        config: &OAuthProviderConfig,
    ) {
        let _ = (request, session, config);
    }

    fn build_token_request(&self, context: TokenExchangeContext<'_>) -> TokenRequestPayload {
        TokenRequestPayload::new(context.config.token_url.clone(), context.base_form())
    }

    fn build_refresh_request(&self, context: TokenRefreshContext<'_>) -> TokenRequestPayload {
        TokenRequestPayload::new(context.config.token_url.clone(), context.base_form())
    }

    fn build_revoke_request(&self, context: TokenRevokeContext<'_>) -> Option<TokenRequestPayload> {
        let _ = context;
        None
    }
}
