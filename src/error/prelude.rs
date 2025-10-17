//! A "prelude" for easily importing the most common error handling items.

pub use super::auth::AuthError;
pub use super::config::ConfigError;
pub use super::conversion::ConversionError;
pub use super::database::DatabaseError;
pub use super::key_pool::KeyPoolError;
pub use super::network::NetworkError;
pub use super::provider::ProviderError;
pub use super::{
    AuthResult, ConfigResult, Context, DatabaseResult, KeyPoolResult, NetworkResult, ProxyError,
    Result,
};

pub use crate::{bail, ensure, error};
