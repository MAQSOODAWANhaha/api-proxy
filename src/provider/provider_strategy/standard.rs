use crate::provider::{OauthProvider, ProviderType};

#[derive(Debug)]
pub struct StandardOauthProvider;

impl OauthProvider for StandardOauthProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Custom("standard".to_string())
    }
}
