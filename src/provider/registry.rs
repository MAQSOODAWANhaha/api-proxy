use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use crate::error::{Result, auth::OAuthError};

use super::provider_strategy::{
    AnthropicProvider, GeminiProvider, OpenAIProvider, StandardOauthProvider,
};
use super::traits::OauthProvider;
use super::types::ProviderType;

static OAUTH_PROVIDERS: LazyLock<HashMap<ProviderType, Arc<dyn OauthProvider>>> =
    LazyLock::new(|| {
        let mut map: HashMap<ProviderType, Arc<dyn OauthProvider>> = HashMap::new();
        map.insert(
            ProviderType::OpenAI,
            Arc::new(OpenAIProvider) as Arc<dyn OauthProvider>,
        );
        map.insert(
            ProviderType::Gemini,
            Arc::new(GeminiProvider) as Arc<dyn OauthProvider>,
        );
        map.insert(
            ProviderType::Anthropic,
            Arc::new(AnthropicProvider) as Arc<dyn OauthProvider>,
        );
        map
    });

static STANDARD_PROVIDER: LazyLock<Arc<dyn OauthProvider>> =
    LazyLock::new(|| Arc::new(StandardOauthProvider) as Arc<dyn OauthProvider>);

pub fn resolve_oauth_provider(pt: &ProviderType) -> Result<Arc<dyn OauthProvider>> {
    if matches!(pt, ProviderType::Custom(_)) {
        return Ok(STANDARD_PROVIDER.clone());
    }

    OAUTH_PROVIDERS
        .get(pt)
        .cloned()
        .ok_or_else(|| OAuthError::ProviderNotFound(pt.as_str().to_string()).into())
}

pub fn get_provider_by_name(provider_name: &str) -> Result<Arc<dyn OauthProvider>> {
    let provider_type = ProviderType::parse(provider_name)?;
    resolve_oauth_provider(&provider_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_providers_resolve() {
        let openai = get_provider_by_name("openai").unwrap();
        assert_eq!(openai.provider_type(), ProviderType::OpenAI);

        let gemini = get_provider_by_name("gemini").unwrap();
        assert_eq!(gemini.provider_type(), ProviderType::Gemini);

        let anthropic = get_provider_by_name("anthropic").unwrap();
        assert_eq!(anthropic.provider_type(), ProviderType::Anthropic);
    }

    #[test]
    fn custom_provider_falls_back_to_standard() {
        let custom = get_provider_by_name("new_vendor:oauth").unwrap();
        assert!(matches!(custom.provider_type(), ProviderType::Custom(_)));
    }
}
