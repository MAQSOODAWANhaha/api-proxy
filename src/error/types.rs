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