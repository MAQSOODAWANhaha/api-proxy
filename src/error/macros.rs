//! # 错误处理宏

/// 快速创建配置错误的宏
#[macro_export]
macro_rules! config_error {
    ($msg:expr) => {
        $crate::error::ProxyError::config($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::config(format!($fmt, $($arg)*))
    };
}

/// 快速创建数据库错误的宏
#[macro_export]
macro_rules! database_error {
    ($msg:expr) => {
        $crate::error::ProxyError::database($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::database(format!($fmt, $($arg)*))
    };
}

/// 快速创建网络错误的宏
#[macro_export]
macro_rules! network_error {
    ($msg:expr) => {
        $crate::error::ProxyError::network($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::network(format!($fmt, $($arg)*))
    };
}

/// 快速创建认证错误的宏
#[macro_export]
macro_rules! auth_error {
    ($msg:expr) => {
        $crate::error::ProxyError::auth($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::auth(format!($fmt, $($arg)*))
    };
}

/// 快速创建AI服务商错误的宏
#[macro_export]
macro_rules! ai_provider_error {
    ($provider:expr, $msg:expr) => {
        $crate::error::ProxyError::ai_provider($msg, $provider)
    };
    ($provider:expr, $fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::ai_provider(format!($fmt, $($arg)*), $provider)
    };
}

/// 快速创建业务错误的宏
#[macro_export]
macro_rules! business_error {
    ($msg:expr) => {
        $crate::error::ProxyError::business($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::business(format!($fmt, $($arg)*))
    };
}

/// 快速创建内部错误的宏
#[macro_export]
macro_rules! internal_error {
    ($msg:expr) => {
        $crate::error::ProxyError::internal($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::internal(format!($fmt, $($arg)*))
    };
}

/// 快速创建缓存错误的宏
#[macro_export]
macro_rules! cache_error {
    ($msg:expr) => {
        $crate::error::ProxyError::cache($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::cache(format!($fmt, $($arg)*))
    };
}

/// 快速创建服务器初始化错误的宏
#[macro_export]
macro_rules! server_init_error {
    ($msg:expr) => {
        $crate::error::ProxyError::server_init($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::server_init(format!($fmt, $($arg)*))
    };
}

/// 快速创建服务器启动错误的宏
#[macro_export]
macro_rules! server_start_error {
    ($msg:expr) => {
        $crate::error::ProxyError::server_start($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::server_start(format!($fmt, $($arg)*))
    };
}

/// 快速创建认证错误的宏
#[macro_export]
macro_rules! authentication_error {
    ($msg:expr) => {
        $crate::error::ProxyError::authentication($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::authentication(format!($fmt, $($arg)*))
    };
}

/// 快速创建速率限制错误的宏
#[macro_export]
macro_rules! rate_limit_error {
    ($msg:expr) => {
        $crate::error::ProxyError::rate_limit($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::rate_limit(format!($fmt, $($arg)*))
    };
}

/// 快速创建网关错误的宏
#[macro_export]
macro_rules! bad_gateway_error {
    ($msg:expr) => {
        $crate::error::ProxyError::bad_gateway($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::bad_gateway(format!($fmt, $($arg)*))
    };
}

/// 快速创建上游服务器未找到错误的宏
#[macro_export]
macro_rules! upstream_not_found_error {
    ($msg:expr) => {
        $crate::error::ProxyError::upstream_not_found($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::upstream_not_found(format!($fmt, $($arg)*))
    };
}

/// 快速创建上游服务器不可用错误的宏
#[macro_export]
macro_rules! upstream_not_available_error {
    ($msg:expr) => {
        $crate::error::ProxyError::upstream_not_available($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::upstream_not_available(format!($fmt, $($arg)*))
    };
}

/// 快速创建连接超时错误的宏
#[macro_export]
macro_rules! connection_timeout_error {
    ($msg:expr, $timeout:expr) => {
        $crate::error::ProxyError::connection_timeout($msg, $timeout)
    };
    ($fmt:expr, $timeout:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::connection_timeout(format!($fmt, $($arg)*), $timeout)
    };
}

/// 快速创建读取超时错误的宏
#[macro_export]
macro_rules! read_timeout_error {
    ($msg:expr, $timeout:expr) => {
        $crate::error::ProxyError::read_timeout($msg, $timeout)
    };
    ($fmt:expr, $timeout:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::read_timeout(format!($fmt, $($arg)*), $timeout)
    };
}

/// 快速创建写入超时错误的宏
#[macro_export]
macro_rules! write_timeout_error {
    ($msg:expr, $timeout:expr) => {
        $crate::error::ProxyError::write_timeout($msg, $timeout)
    };
    ($fmt:expr, $timeout:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::write_timeout(format!($fmt, $($arg)*), $timeout)
    };
}

/// 快速创建负载均衡错误的宏
#[macro_export]
macro_rules! load_balancer_error {
    ($msg:expr) => {
        $crate::error::ProxyError::load_balancer($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::load_balancer(format!($fmt, $($arg)*))
    };
}

/// 快速创建健康检查错误的宏
#[macro_export]
macro_rules! health_check_error {
    ($msg:expr) => {
        $crate::error::ProxyError::health_check($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::health_check(format!($fmt, $($arg)*))
    };
}

/// 快速创建统计收集错误的宏
#[macro_export]
macro_rules! statistics_error {
    ($msg:expr) => {
        $crate::error::ProxyError::statistics($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::statistics(format!($fmt, $($arg)*))
    };
}

/// 快速创建跟踪系统错误的宏
#[macro_export]
macro_rules! tracing_error {
    ($msg:expr) => {
        $crate::error::ProxyError::tracing($msg)
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::error::ProxyError::tracing(format!($fmt, $($arg)*))
    };
}

/// 确保条件成立，否则返回配置错误
#[macro_export]
macro_rules! ensure_config {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err($crate::config_error!($msg));
        }
    };
    ($cond:expr, $fmt:expr, $($arg:tt)*) => {
        if !($cond) {
            return Err($crate::config_error!($fmt, $($arg)*));
        }
    };
}

/// 确保条件成立，否则返回业务错误
#[macro_export]
macro_rules! ensure_business {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err($crate::business_error!($msg));
        }
    };
    ($cond:expr, $fmt:expr, $($arg:tt)*) => {
        if !($cond) {
            return Err($crate::business_error!($fmt, $($arg)*));
        }
    };
}

/// 确保条件成立，否则返回数据库错误
#[macro_export]
macro_rules! ensure_database {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err($crate::database_error!($msg));
        }
    };
    ($cond:expr, $fmt:expr, $($arg:tt)*) => {
        if !($cond) {
            return Err($crate::database_error!($fmt, $($arg)*));
        }
    };
}

/// 确保条件成立，否则返回网络错误
#[macro_export]
macro_rules! ensure_network {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err($crate::network_error!($msg));
        }
    };
    ($cond:expr, $fmt:expr, $($arg:tt)*) => {
        if !($cond) {
            return Err($crate::network_error!($fmt, $($arg)*));
        }
    };
}

/// 确保条件成立，否则返回认证错误
#[macro_export]
macro_rules! ensure_auth {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err($crate::auth_error!($msg));
        }
    };
    ($cond:expr, $fmt:expr, $($arg:tt)*) => {
        if !($cond) {
            return Err($crate::auth_error!($fmt, $($arg)*));
        }
    };
}

/// 确保条件成立，否则返回缓存错误
#[macro_export]
macro_rules! ensure_cache {
    ($cond:expr, $msg:expr) => {
        if !($cond) {
            return Err($crate::cache_error!($msg));
        }
    };
    ($cond:expr, $fmt:expr, $($arg:tt)*) => {
        if !($cond) {
            return Err($crate::cache_error!($fmt, $($arg)*));
        }
    };
}