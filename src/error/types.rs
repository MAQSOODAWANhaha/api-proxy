//! # 错误类型定义

use thiserror::Error;

/// 应用主要错误类型
#[derive(Debug, Error)]
pub enum ProxyError {
    /// 配置相关错误
    #[error("配置错误: {message}")]
    Config {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 数据库相关错误  
    #[error("数据库错误: {message}")]
    Database {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 网络通信错误
    #[error("网络错误: {message}")]
    Network {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 认证和授权错误
    #[error("认证错误: {message}")]
    Auth {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// AI服务商错误
    #[error("AI服务错误: {message}")]
    AiProvider {
        message: String,
        provider: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// TLS证书错误
    #[error("TLS证书错误: {message}")]
    Tls {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 业务逻辑错误
    #[error("业务错误: {message}")]
    Business {
        message: String,
    },

    /// 系统内部错误
    #[error("内部错误: {message}")]
    Internal {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// IO相关错误
    #[error("IO错误: {message}")]
    Io {
        message: String,
        #[source]
        source: std::io::Error,
    },

    /// 序列化/反序列化错误
    #[error("序列化错误: {message}")]
    Serialization {
        message: String,
        #[source]
        source: anyhow::Error,
    },

    /// 缓存相关错误
    #[error("缓存错误: {message}")]
    Cache {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 服务器初始化错误
    #[error("服务器初始化错误: {message}")]
    ServerInit {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 服务器启动错误
    #[error("服务器启动错误: {message}")]
    ServerStart {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 认证错误
    #[error("认证错误: {message}")]
    Authentication {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 上游服务器未找到
    #[error("上游服务器未找到: {message}")]
    UpstreamNotFound {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 上游服务器不可用
    #[error("上游服务器不可用: {message}")]
    UpstreamNotAvailable {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 速率限制错误
    #[error("速率限制: {message}")]
    RateLimit {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 网关错误
    #[error("网关错误: {message}")]
    BadGateway {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },
}

impl ProxyError {
    /// 创建配置错误
    pub fn config<T: Into<String>>(message: T) -> Self {
        Self::Config {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的配置错误
    pub fn config_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Config {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建数据库错误
    pub fn database<T: Into<String>>(message: T) -> Self {
        Self::Database {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的数据库错误
    pub fn database_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Database {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建网络错误
    pub fn network<T: Into<String>>(message: T) -> Self {
        Self::Network {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的网络错误
    pub fn network_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Network {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建认证错误
    pub fn auth<T: Into<String>>(message: T) -> Self {
        Self::Auth {
            message: message.into(),
            source: None,
        }
    }

    /// 创建AI服务商错误
    pub fn ai_provider<T: Into<String>, P: Into<String>>(message: T, provider: P) -> Self {
        Self::AiProvider {
            message: message.into(),
            provider: provider.into(),
            source: None,
        }
    }

    /// 创建业务错误
    pub fn business<T: Into<String>>(message: T) -> Self {
        Self::Business {
            message: message.into(),
        }
    }

    /// 创建内部错误
    pub fn internal<T: Into<String>>(message: T) -> Self {
        Self::Internal {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的内部错误
    pub fn internal_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Internal {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建缓存错误
    pub fn cache<T: Into<String>>(message: T) -> Self {
        Self::Cache {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的缓存错误
    pub fn cache_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Cache {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建服务器初始化错误
    pub fn server_init<T: Into<String>>(message: T) -> Self {
        Self::ServerInit {
            message: message.into(),
            source: None,
        }
    }

    /// 创建服务器启动错误
    pub fn server_start<T: Into<String>>(message: T) -> Self {
        Self::ServerStart {
            message: message.into(),
            source: None,
        }
    }

    /// 创建认证错误
    pub fn authentication<T: Into<String>>(message: T) -> Self {
        Self::Authentication {
            message: message.into(),
            source: None,
        }
    }

    /// 创建速率限制错误
    pub fn rate_limit<T: Into<String>>(message: T) -> Self {
        Self::RateLimit {
            message: message.into(),
            source: None,
        }
    }

    /// 创建网关错误
    pub fn bad_gateway<T: Into<String>>(message: T) -> Self {
        Self::BadGateway {
            message: message.into(),
            source: None,
        }
    }

    /// 创建上游服务器未找到错误
    pub fn upstream_not_found<T: Into<String>>(message: T) -> Self {
        Self::UpstreamNotFound {
            message: message.into(),
            source: None,
        }
    }

    /// 创建上游服务器不可用错误
    pub fn upstream_not_available<T: Into<String>>(message: T) -> Self {
        Self::UpstreamNotAvailable {
            message: message.into(),
            source: None,
        }
    }
}

// 自动转换常见错误类型
impl From<std::io::Error> for ProxyError {
    fn from(err: std::io::Error) -> Self {
        Self::Io {
            message: "文件操作失败".to_string(),
            source: err,
        }
    }
}

impl From<toml::de::Error> for ProxyError {
    fn from(err: toml::de::Error) -> Self {
        Self::config_with_source("TOML解析失败", err)
    }
}

impl From<serde_json::Error> for ProxyError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization {
            message: "JSON处理失败".to_string(),
            source: err.into(),
        }
    }
}

impl From<sea_orm::error::DbErr> for ProxyError {
    fn from(err: sea_orm::error::DbErr) -> Self {
        Self::database_with_source("数据库操作失败", err)
    }
}

// 认证相关错误转换
impl From<crate::auth::types::AuthError> for ProxyError {
    fn from(err: crate::auth::types::AuthError) -> Self {
        Self::Auth {
            message: err.to_string(),
            source: Some(anyhow::Error::new(err)),
        }
    }
}

impl From<crate::auth::jwt::JwtError> for ProxyError {
    fn from(err: crate::auth::jwt::JwtError) -> Self {
        Self::Auth {
            message: err.to_string(),
            source: Some(anyhow::Error::new(err)),
        }
    }
}

impl From<crate::auth::api_key::ApiKeyError> for ProxyError {
    fn from(err: crate::auth::api_key::ApiKeyError) -> Self {
        Self::Auth {
            message: err.to_string(),
            source: Some(anyhow::Error::new(err)),
        }
    }
}

impl From<crate::auth::service::AuthServiceError> for ProxyError {
    fn from(err: crate::auth::service::AuthServiceError) -> Self {
        Self::Auth {
            message: err.to_string(),
            source: Some(anyhow::Error::new(err)),
        }
    }
}