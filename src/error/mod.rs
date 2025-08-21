//! # 错误处理模块
//!
//! 统一的错误类型定义和处理

mod macros;
mod types;

#[cfg(test)]
mod tests;

pub use types::*;

/// 应用结果类型
pub type Result<T> = std::result::Result<T, ProxyError>;

/// 错误上下文扩展trait
pub trait ErrorContext<T> {
    /// 添加配置错误上下文
    fn with_config_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;

    /// 添加数据库错误上下文
    fn with_database_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;

    /// 添加网络错误上下文
    fn with_network_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;

    /// 添加认证错误上下文
    fn with_auth_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;

    /// 添加缓存错误上下文
    fn with_cache_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: Into<anyhow::Error>,
{
    fn with_config_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| ProxyError::config_with_source(f(), e.into()))
    }

    fn with_database_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| ProxyError::database_with_source(f(), e.into()))
    }

    fn with_network_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| ProxyError::network_with_source(f(), e.into()))
    }

    fn with_auth_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| ProxyError::authentication_with_source(f(), e.into()))
    }

    fn with_cache_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| ProxyError::cache_with_source(f(), e.into()))
    }
}

impl<T> ErrorContext<T> for Option<T> {
    fn with_config_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| ProxyError::config(f()))
    }

    fn with_database_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| ProxyError::database(f()))
    }

    fn with_network_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| ProxyError::network(f()))
    }

    fn with_auth_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| ProxyError::auth(f()))
    }

    fn with_cache_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.ok_or_else(|| ProxyError::cache(f()))
    }
}
