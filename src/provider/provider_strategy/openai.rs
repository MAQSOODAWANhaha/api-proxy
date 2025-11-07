use std::collections::HashMap;

use crate::provider::{OauthProvider, ProviderType, TokenRequestPayload, TokenRevokeContext};

#[derive(Debug)]
pub struct OpenAIProvider;

impl OauthProvider for OpenAIProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::OpenAI
    }

    fn build_revoke_request(&self, context: TokenRevokeContext<'_>) -> Option<TokenRequestPayload> {
        let mut form = HashMap::new();
        form.insert("token".to_string(), context.token.to_string());
        form.insert("client_id".to_string(), context.config.client_id.clone());

        if let Some(secret) = &context.config.client_secret {
            form.insert("client_secret".to_string(), secret.clone());
        }

        if let Some(hint) = context.hint {
            form.insert("token_type_hint".to_string(), hint.to_string());
        }

        Some(TokenRequestPayload::new(
            "https://auth.openai.com/oauth/revoke",
            form,
        ))
    }
}
