//! Provider capability module. 提供 OAuth/PKCE 提供商相关的通用工具。

mod config_store;
pub mod provider_strategy;
mod registry;
mod request;
mod traits;
mod types;

pub use config_store::{ApiKeyProviderConfig, ProviderConfigBuilder};
pub use registry::{get_provider_by_name, resolve_oauth_provider};
pub use request::{
    AuthorizationRequest, TokenExchangeContext, TokenRefreshContext, TokenRequestPayload,
    TokenRevokeContext,
};
pub use traits::OauthProvider;
pub use types::{ProviderType, provider_type_from_name};
