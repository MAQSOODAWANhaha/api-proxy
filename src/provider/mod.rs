//! Provider capability module。
//!
//! - `config_store`：读取数据库+缓存中的 OAuth 配置
//! - `authorize`：根据配置与会话构建授权 URL
//! - `request`：根据配置构建 token 请求
//! - `template`：用于渲染配置中的 `{{...}}` 占位符

mod authorize;
mod config_store;
mod request;
mod template;

pub use authorize::build_authorize_url;
pub use config_store::{ApiKeyProviderConfig, ProviderConfigBuilder};
pub use request::{TokenRequestPayload, build_exchange_request, build_refresh_request};
