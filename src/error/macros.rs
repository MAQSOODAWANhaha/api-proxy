//! 错误处理宏（精简版）

/// 通用错误构造宏
/// 用法：
/// - proxy_err!(auth, "未认证")
/// - proxy_err!(ai_provider, "OpenAI", "bad request")
/// - proxy_err!(connection_timeout, "connect timeout", 30)
#[macro_export]
macro_rules! proxy_err {
    (config, $($t:tt)*) => { $crate::error::ProxyError::config(format!($($t)*)) };
    (database, $($t:tt)*) => { $crate::error::ProxyError::database(format!($($t)*)) };
    (db, $($t:tt)*) => { $crate::proxy_err!(database, $($t)*) };
    (network, $($t:tt)*) => { $crate::error::ProxyError::network(format!($($t)*)) };
    (net, $($t:tt)*) => { $crate::proxy_err!(network, $($t)*) };
    (auth, $($t:tt)*) => { $crate::error::ProxyError::authentication(format!($($t)*)) };
    (authentication, $($t:tt)*) => { $crate::proxy_err!(auth, $($t)*) };
    (ai_provider, $provider:expr, $($t:tt)*) => { $crate::error::ProxyError::ai_provider(format!($($t)*), $provider) };
    (tls, $($t:tt)*) => { $crate::error::ProxyError::tls(format!($($t)*)) };
    (business, $($t:tt)*) => { $crate::error::ProxyError::business(format!($($t)*)) };
    (internal, $($t:tt)*) => { $crate::error::ProxyError::internal(format!($($t)*)) };
    (cache, $($t:tt)*) => { $crate::error::ProxyError::cache(format!($($t)*)) };
    (server_init, $($t:tt)*) => { $crate::error::ProxyError::server_init(format!($($t)*)) };
    (server_start, $($t:tt)*) => { $crate::error::ProxyError::server_start(format!($($t)*)) };
    (upstream_not_found, $($t:tt)*) => { $crate::error::ProxyError::upstream_not_found(format!($($t)*)) };
    (upstream_not_available, $($t:tt)*) => { $crate::error::ProxyError::upstream_not_available(format!($($t)*)) };
    (rate_limit, $($t:tt)*) => { $crate::error::ProxyError::rate_limit(format!($($t)*)) };
    (bad_gateway, $($t:tt)*) => { $crate::error::ProxyError::bad_gateway(format!($($t)*)) };
    (connection_timeout, $msg:expr, $timeout:expr) => { $crate::error::ProxyError::connection_timeout($msg, $timeout) };
    (read_timeout, $msg:expr, $timeout:expr) => { $crate::error::ProxyError::read_timeout($msg, $timeout) };
    (write_timeout, $msg:expr, $timeout:expr) => { $crate::error::ProxyError::write_timeout($msg, $timeout) };
    (load_balancer, $($t:tt)*) => { $crate::error::ProxyError::load_balancer(format!($($t)*)) };
    (health_check, $($t:tt)*) => { $crate::error::ProxyError::health_check(format!($($t)*)) };
    (statistics, $($t:tt)*) => { $crate::error::ProxyError::statistics(format!($($t)*)) };
    (tracing, $($t:tt)*) => { $crate::error::ProxyError::tracing(format!($($t)*)) };
    // 管理端
    (mgmt_auth, $($t:tt)*) => { $crate::error::ProxyError::management_auth(format!($($t)*)) };
    (mgmt_permission, $($t:tt)*) => { $crate::error::ProxyError::management_permission(format!($($t)*)) };
    (mgmt_validation, $msg:expr) => { $crate::error::ProxyError::management_validation($msg, None) };
    (mgmt_validation, $msg:expr, field = $field:expr) => { $crate::error::ProxyError::management_validation($msg, Some($field.to_string())) };
    (mgmt_business, $($t:tt)*) => { $crate::error::ProxyError::management_business(format!($($t)*)) };
    (mgmt_not_found, $r:expr, $id:expr) => { $crate::error::ProxyError::management_not_found($r, $id) };
    (mgmt_conflict, $r:expr, $id:expr) => { $crate::error::ProxyError::management_conflict($r, $id) };
    (mgmt_rate_limit, $($t:tt)*) => { $crate::error::ProxyError::management_rate_limit(format!($($t)*)) };
}

/// 直接返回错误：proxy_bail!(auth, "msg")
#[macro_export]
macro_rules! proxy_bail {
    ($($t:tt)*) => { return Err($crate::proxy_err!($($t)*)) };
}

/// 通用 ensure：proxy_ensure!(cond, auth, "msg")
#[macro_export]
macro_rules! proxy_ensure {
    ($cond:expr, $($t:tt)*) => {
        if !($cond) { $crate::proxy_bail!($($t)*); }
    };
}

/// 将 ProxyError 转为 (StatusCode, JSON 字符串)
#[macro_export]
macro_rules! http_error_body {
    ($err:expr) => {{ $err.to_http_status_and_body() }};
}

/// 将 ProxyError 直接转换为 Pingora 错误（ErrorType::HTTPStatus + JSON body）
/// 用法：return Err(pingora_error!(err));
#[macro_export]
macro_rules! pingora_error {
    ($err:expr) => {{
        let (status, body) = $err.to_http_status_and_body();
        pingora_core::Error::explain(pingora_core::ErrorType::HTTPStatus(status.as_u16()), body)
    }};
}

/// 构造任意 HTTP 状态的 Pingora 错误（用于成功短路或特殊响应）
#[macro_export]
macro_rules! pingora_http {
    ($status:expr, $msg:expr) => {{ pingora_core::Error::explain(pingora_core::ErrorType::HTTPStatus(($status) as u16), $msg) }};
}

/// 将 Result<T, ProxyError> 一步转换为 pingora_core::Result<T>
/// 用法：let val = pingora_try!(expr_returning_proxy_result);
#[macro_export]
macro_rules! pingora_try {
    ($expr:expr) => {{
        match $expr {
            Ok(v) => v,
            Err(e) => return Err($crate::pingora_error!(e)),
        }
    }};
}

/// 返回 Ok(false)（继续代理）
#[macro_export]
macro_rules! pingora_continue {
    () => {
        Ok(false)
    };
}

/// 返回 Ok(true)（已响应，下游结束）
#[macro_export]
macro_rules! pingora_respond {
    () => {
        Ok(true)
    };
}

/// 管理端错误快速返回：将 ProxyError 转为标准管理端响应包裹
#[macro_export]
macro_rules! manage_error {
    ($err:expr) => {{ $crate::management::response::app_error($err) }};
}

/// 管理端临时代码路径：用现有 code/status 直接返回标准错误包裹
/// 便于逐步从手写 code 过渡到 ProxyError 语义映射
#[macro_export]
macro_rules! manage_error_code {
    ($status:expr, $code:expr, $msg:expr) => {{ $crate::management::response::error($status, $code, $msg) }};
}
