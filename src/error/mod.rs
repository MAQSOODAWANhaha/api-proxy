//! The unified error handling system for the application.

use std::fmt::Display;

// 1. Core Types
pub use types::ProxyError;

/// A unified `Result` type for the entire application.
///
/// All functions that can fail should return this type.
pub type Result<T> = std::result::Result<T, ProxyError>;

// 3. Module declarations
pub mod auth;
pub mod cache;
pub mod config;
pub mod conversion;
pub mod database;
pub mod key_pool;
pub mod macros;
pub mod management;
pub mod network;
pub mod prelude;
pub mod provider;
pub mod types;

// 4. Context Trait for adding context to errors.
pub trait Context<T, E> {
    #[track_caller]
    fn context<C>(self, context: C) -> Result<T>
    where
        C: std::fmt::Display;

    #[track_caller]
    fn with_context<C, F>(self, context: F) -> Result<T>
    where
        F: FnOnce() -> C,
        C: std::fmt::Display;
}

impl<T, E> Context<T, E> for std::result::Result<T, E>
where
    E: Into<ProxyError>,
{
    #[track_caller]
    fn context<C>(self, context: C) -> Result<T>
    where
        C: std::fmt::Display,
    {
        self.with_context(|| context)
    }

    #[track_caller]
    fn with_context<C, F>(self, context: F) -> Result<T>
    where
        F: FnOnce() -> C,
        C: std::fmt::Display,
    {
        match self {
            Ok(value) => Ok(value),
            Err(error) => {
                let context_message = context().to_string();
                Err(ProxyError::Context {
                    context: context_message,
                    source: Box::new(error.into()),
                })
            }
        }
    }
}

/// Helper to attach context to an error without intermediate boilerplate.
#[track_caller]
pub fn context_error<T>(err: impl Into<ProxyError>, context: impl Display) -> Result<T> {
    Err(err.into()).context(context)
}

// 5. Error Category for monitoring and alerting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    /// Errors caused by the client (e.g., bad input, invalid credentials).
    /// Corresponds to 4xx HTTP status codes.
    Client,
    /// Errors caused by the server or its dependencies.
    /// Corresponds to 5xx HTTP status codes.
    Server,
}

#[cfg(test)]
mod tests;
