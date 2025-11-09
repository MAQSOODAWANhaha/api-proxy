//! Provider capability module。
//!
//! - `config_store`：读取数据库+缓存中的 OAuth 配置
//! - `authorize`：根据配置与会话构建授权 URL
//! - `request`/`traits`：定义 provider trait 及 token 请求上下文
//! - `registry`：集中管理已注册的 provider 策略
//! - `types`：Provider 标识及通用枚举

mod authorize;
mod config_store;
mod provider_strategy;
mod registry;
mod request;
mod traits;
mod types;

pub use authorize::build_authorize_url;
pub use config_store::{ApiKeyProviderConfig, ProviderConfigBuilder};
pub use registry::get_provider_by_name;
pub use request::{
    TokenExchangeContext, TokenRefreshContext, TokenRequestPayload, TokenRevokeContext,
    create_token_request, into_token_request,
};
pub use types::{ProviderType, provider_type_from_name};
