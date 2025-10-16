//! The unified error handling system for the application.

// 1. Core Types
pub use types::ProxyError;

/// A unified `Result` type for the entire application.
///
/// All functions that can fail should return this type.
pub type Result<T> = std::result::Result<T, ProxyError>;

// 2. Domain-specific Result aliases for better readability.
pub type AuthResult<T> = Result<T>;
pub type ConfigResult<T> = Result<T>;
pub type DatabaseResult<T> = Result<T>;
pub type NetworkResult<T> = Result<T>;
pub type SchedulerResult<T> = Result<T>;

/// Deprecated alias kept temporarily for incremental migration.
#[allow(dead_code)]
#[deprecated(since = "0.1.0", note = "Please migrate to `crate::error::Result`")]
pub type OldAppResult<T> = Result<T>;

// 3. Module declarations
pub mod auth;
pub mod config;
pub mod conversion;
pub mod database;
pub mod macros;
pub mod network;
pub mod prelude;
pub mod provider;
pub mod scheduler;
pub mod types;

// 4. Context Trait for adding context to errors.
pub trait Context<T, E> {
    fn context<C>(self, context: C) -> Result<T>
    where
        C: std::fmt::Display;

    fn with_context<C, F>(self, context: F) -> Result<T>
    where
        F: FnOnce() -> C,
        C: std::fmt::Display;
}

impl<T, E> Context<T, E> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn context<C>(self, context: C) -> Result<T>
    where
        C: std::fmt::Display,
    {
        self.with_context(|| context)
    }

    fn with_context<C, F>(self, context: F) -> Result<T>
    where
        F: FnOnce() -> C,
        C: std::fmt::Display,
    {
        self.map_err(|error| {
            let message = format!("{}: {}", context(), error);
            ProxyError::internal_with_source(message, error)
        })
    }
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
