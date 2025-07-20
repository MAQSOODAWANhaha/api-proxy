//! # 错误处理宏

/// 快速创建配置错误的宏
#[macro_export]
macro_rules! config_error {
    ($msg:expr) => {
        crate::error::ProxyError::config($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        crate::error::ProxyError::config(format!($fmt, $($arg)*))
    };
}

/// 快速创建数据库错误的宏
#[macro_export]
macro_rules! database_error {
    ($msg:expr) => {
        crate::error::ProxyError::database($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        crate::error::ProxyError::database(format!($fmt, $($arg)*))
    };
}

/// 快速创建网络错误的宏
#[macro_export]
macro_rules! network_error {
    ($msg:expr) => {
        crate::error::ProxyError::network($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        crate::error::ProxyError::network(format!($fmt, $($arg)*))
    };
}

/// 快速创建认证错误的宏
#[macro_export]
macro_rules! auth_error {
    ($msg:expr) => {
        crate::error::ProxyError::auth($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        crate::error::ProxyError::auth(format!($fmt, $($arg)*))
    };
}

/// 快速创建AI服务商错误的宏
#[macro_export]
macro_rules! ai_provider_error {
    ($provider:expr, $msg:expr) => {
        crate::error::ProxyError::ai_provider($msg, $provider)
    };
    ($provider:expr, $fmt:expr, $($arg:tt)*) => {
        crate::error::ProxyError::ai_provider(format!($fmt, $($arg)*), $provider)
    };
}

/// 快速创建业务错误的宏
#[macro_export]
macro_rules! business_error {
    ($msg:expr) => {
        crate::error::ProxyError::business($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        crate::error::ProxyError::business(format!($fmt, $($arg)*))
    };
}

/// 快速创建内部错误的宏
#[macro_export]
macro_rules! internal_error {
    ($msg:expr) => {
        crate::error::ProxyError::internal($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        crate::error::ProxyError::internal(format!($fmt, $($arg)*))
    };
}

/// 确保条件成立，否则返回配置错误
#[macro_export]
macro_rules! ensure_config {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err(crate::config_error!($msg));
        }
    };
    ($cond:expr, $fmt:expr, $($arg:tt)*) => {
        if !($cond) {
            return Err(crate::config_error!($fmt, $($arg)*));
        }
    };
}

/// 确保条件成立，否则返回业务错误
#[macro_export]
macro_rules! ensure_business {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err(crate::business_error!($msg));
        }
    };
    ($cond:expr, $fmt:expr, $($arg:tt)*) => {
        if !($cond) {
            return Err(crate::business_error!($fmt, $($arg)*));
        }
    };
}