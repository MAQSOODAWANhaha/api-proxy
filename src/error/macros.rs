//! Defines the core error handling macros: `bail!` and `ensure!`.
//!
//! Note: The previous `error!` macro has been removed in favor of standard Rust error conversion.
//! Use `ProxyError::from(...)` or simply `.into()` when constructing errors.

/// Creates and returns an error immediately.
///
/// This macro delegates to `ProxyError::from(...)`, allowing you to bail with any type
/// that can be converted into a `ProxyError` (e.g., strings, `anyhow::Error`, specific error types).
///
/// # Examples
///
/// ```ignore
/// bail!("Operation not allowed"); // Returns ProxyError::Internal
/// bail!(AuthError::ApiKeyMissing); // Returns ProxyError::Authentication
/// bail!(anyhow!("Some error")); // Returns ProxyError::Internal
/// ```
#[macro_export]
macro_rules! bail {
    ($msg:literal $(, $args:tt)*) => {
        return Err($crate::error::ProxyError::from(format!($msg $(, $args)*)))
    };
    ($err:expr) => {
        return Err($crate::error::ProxyError::from($err))
    };
}

/// Checks a condition and returns an error if it's false.
///
/// # Examples
///
/// ```ignore
/// ensure!(user.is_active, "User is inactive");
/// ensure!(count > 0, AuthError::InvalidInput);
/// ```
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $($t:tt)*) => {
        if !($cond) {
            $crate::bail!($($t)*);
        }
    };
}

/// Returns an error with extra context immediately.
#[macro_export]
macro_rules! bail_context {
    ($err:expr, $context:expr $(,)?) => {
        return $crate::error::context_error($err, $context);
    };
}

/// Checks a condition, returning an error with context if it fails.
#[macro_export]
macro_rules! ensure_context {
    ($cond:expr, $err:expr, $context:expr $(,)?) => {
        if !($cond) {
            $crate::bail_context!($err, $context);
        }
    };
}
