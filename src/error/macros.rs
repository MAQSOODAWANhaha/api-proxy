//! Defines the core error handling macros: `error!`, `bail!`, and `ensure!`.

#[macro_export]
macro_rules! error {

    // --- Internal errors ---
    (Internal, $message:expr, $source:expr) => {
        $crate::error::ProxyError::internal_with_source($message, $source)
    };

    (Internal, $message:expr) => {
        $crate::error::ProxyError::internal($message)
    };

    (Internal, $fmt:expr, $($arg:tt)+) => {
        $crate::error::ProxyError::internal(format!($fmt, $($arg)+))
    };

    // --- Config errors ---
    (Config, $variant:ident ( $($arg:tt)* ) $(,)?) => {
        $crate::error::ProxyError::from($crate::error::config::ConfigError::$variant($($arg)*))
    };

    (Config, $message:expr $(,)?) => {
        $crate::error::ProxyError::from($crate::error::config::ConfigError::Load($message.into()))
    };

    (Config, $fmt:expr, $($arg:tt)+) => {
        $crate::error::ProxyError::from($crate::error::config::ConfigError::Load(format!($fmt, $($arg)+)))
    };

    // --- Database errors ---
    (Database, $variant:ident ( $($arg:tt)* ) $(,)?) => {
        $crate::error::ProxyError::from($crate::error::database::DatabaseError::$variant($($arg)*))
    };

    (Database, $message:expr $(,)?) => {
        $crate::error::ProxyError::from($crate::error::database::DatabaseError::Connection($message.into()))
    };

    (Database, $fmt:expr, $($arg:tt)+) => {
        $crate::error::ProxyError::from($crate::error::database::DatabaseError::Connection(format!($fmt, $($arg)+)))
    };

    // --- Conversion errors ---
    (Conversion, $variant:ident ( $($arg:tt)* ) $(,)?) => {
        $crate::error::ProxyError::from($crate::error::conversion::ConversionError::$variant($($arg)*))
    };

    (Conversion, $message:expr $(,)?) => {
        $crate::error::ProxyError::from($crate::error::conversion::ConversionError::Message($message.into()))
    };

    (Conversion, $fmt:expr, $($arg:tt)+) => {
        $crate::error::ProxyError::from(
            $crate::error::conversion::ConversionError::Message(format!($fmt, $($arg)+))
        )
    };

    // --- Authentication errors ---
    (Authentication, $variant:ident ( $($arg:tt)* ) $(,)?) => {
        $crate::error::ProxyError::from($crate::error::auth::AuthError::$variant($($arg)*))
    };

    (Authentication, $message:expr $(,)?) => {
        $crate::error::ProxyError::from($crate::error::auth::AuthError::Message($message.into()))
    };

    (Authentication, $fmt:expr, $($arg:tt)+) => {
        $crate::error::ProxyError::from($crate::error::auth::AuthError::Message(format!($fmt, $($arg)+)))
    };

    // Matches: error!(Auth, VariantInAuth)
    (Auth, $sub_variant:ident) => {
        $crate::error::ProxyError::Authentication(
            $crate::error::auth::AuthError::$sub_variant
        )
    };

    // Matches: error!(Auth, VariantInAuth { .. })
    (Auth, $sub_variant:ident { $($field:tt)* }) => {
        $crate::error::ProxyError::Authentication(
            $crate::error::auth::AuthError::$sub_variant { $($field)* }
        )
    };

    // --- Provider errors ---
    (Provider, message = $message:expr, provider = $provider:expr, source = $source:expr $(,)?) => {
        $crate::error::ProxyError::provider_with_source($message, $provider, $source)
    };

    // Matches: error!(Variant, message="...", provider="...") for AiProvider
    (Provider, message = $message:expr, provider = $provider:expr $(,)?) => {
        $crate::error::ProxyError::provider($message, $provider)
    };

    // --- Network errors ---
    (Network, RateLimitExceeded $(,)?) => {
        $crate::error::ProxyError::from($crate::error::network::NetworkError::RateLimitExceeded)
    };

    (Network, $variant:ident ( $($arg:tt)* ) $(,)?) => {
        $crate::error::ProxyError::from($crate::error::network::NetworkError::$variant($($arg)*))
    };

    (Network, $message:expr $(,)?) => {
        $crate::error::ProxyError::from($crate::error::network::NetworkError::BadGateway($message.into()))
    };

    // Fallback to compile error for unsupported patterns to catch mistakes early.
    ($($t:tt)*) => {
        compile_error!("Unsupported pattern in error! macro")
    };
}

/// Creates and returns an error immediately.
/// Example: `bail!(Business, "Operation not allowed")`
#[macro_export]
macro_rules! bail {
    ($($t:tt)*) => { return Err($crate::error!($($t)*)) };
}

/// Checks a condition and returns an error if it's false.
/// Example: `ensure!(user.is_active, Business, "User is disabled")`
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $($t:tt)*) => {
        if !($cond) {
            $crate::bail!($($t)*);
        }
    };
}
