use crate::provider::ProviderType;
use crate::provider::traits::OauthProvider;

#[derive(Debug)]
pub struct StandardOauthProvider;

impl OauthProvider for StandardOauthProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Custom("standard".to_string())
    }
}
