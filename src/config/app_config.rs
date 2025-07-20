//! # 应用配置结构定义

use serde::{Deserialize, Serialize};

/// 应用主配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 服务器配置
    pub server: ServerConfig,
    /// 数据库配置
    pub database: super::DatabaseConfig,
    /// Redis配置
    pub redis: RedisConfig,
    /// TLS配置
    pub tls: TlsConfig,
}

/// 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// HTTP监听地址
    pub host: String,
    /// HTTP监听端口
    pub port: u16,
    /// HTTPS监听端口
    pub https_port: u16,
    /// 工作线程数
    pub workers: usize,
}

/// Redis配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis连接URL
    pub url: String,
    /// 连接池大小
    pub pool_size: u32,
    /// 服务器地址
    pub host: String,
    /// 服务器端口
    pub port: u16,
    /// 数据库编号
    pub database: u8,
    /// 连接密码（可选）
    pub password: Option<String>,
    /// 连接超时时间（秒）
    pub connection_timeout: u64,
    /// 默认 TTL（秒）
    pub default_ttl: u64,
    /// 最大连接数
    pub max_connections: u32,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379/0".to_string(),
            pool_size: 10,
            host: "127.0.0.1".to_string(),
            port: 6379,
            database: 0,
            password: None,
            connection_timeout: 10,
            default_ttl: 3600,
            max_connections: 10,
        }
    }
}

/// TLS配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// 证书存储路径
    pub cert_path: String,
    /// ACME邮箱
    pub acme_email: String,
    /// 支持的域名
    pub domains: Vec<String>,
}