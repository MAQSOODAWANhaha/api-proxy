//! # 应用配置结构定义

use super::dual_port_config::DualPortServerConfig;
use serde::{Deserialize, Serialize};

/// 应用主配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 服务器配置（传统单端口模式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerConfig>,
    /// 双端口服务器配置（推荐模式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dual_port: Option<DualPortServerConfig>,
    /// 数据库配置
    pub database: super::DatabaseConfig,
    /// Redis配置
    pub redis: RedisConfig,
    /// 缓存配置
    pub cache: CacheConfig,
}

/// 传统服务器配置 - 简化版（仅保留兼容性，推荐使用dual_port）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// HTTP监听地址
    pub host: String,
    /// HTTP监听端口
    pub port: u16,
    /// 工作线程数
    #[serde(default = "default_workers")]
    pub workers: usize,
}

fn default_workers() -> usize {
    num_cpus::get()
}

// PingoraConfig 已删除，超时配置现在从数据库 user_service_apis.timeout_seconds 获取

/// 缓存类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheType {
    /// 内存缓存
    Memory,
    /// Redis缓存
    Redis,
}

impl Default for CacheType {
    fn default() -> Self {
        Self::Memory
    }
}

/// 缓存配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// 缓存类型
    pub cache_type: CacheType,
    /// 内存缓存最大条目数
    pub memory_max_entries: usize,
    /// 默认过期时间（秒）
    pub default_ttl: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            cache_type: CacheType::Memory,
            memory_max_entries: 10000,
            default_ttl: 300,
        }
    }
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

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            workers: default_workers(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: None, // 优先使用双端口配置
            dual_port: Some(DualPortServerConfig::default()),
            database: super::DatabaseConfig::default(),
            redis: RedisConfig::default(),
            cache: CacheConfig::default(),
        }
    }
}

impl AppConfig {
    /// 获取有效的服务器配置（优先使用双端口配置）- 简化版
    pub fn get_server_config(&self) -> ServerConfig {
        if let Some(dual_port) = &self.dual_port {
            // 从双端口配置转换为传统配置（兼容性）
            ServerConfig {
                host: dual_port.management.http.host.clone(),
                port: dual_port.management.http.port,
                workers: dual_port.workers,
            }
        } else if let Some(server) = &self.server {
            server.clone()
        } else {
            ServerConfig::default()
        }
    }

    /// 获取双端口配置
    pub fn get_dual_port_config(&self) -> Option<&DualPortServerConfig> {
        self.dual_port.as_ref()
    }

    /// 是否启用双端口模式
    pub fn is_dual_port_mode(&self) -> bool {
        self.dual_port.is_some()
    }

    /// 获取管理端口
    pub fn get_management_port(&self) -> u16 {
        if let Some(dual_port) = &self.dual_port {
            dual_port.management.http.port
        } else if let Some(server) = &self.server {
            server.port
        } else {
            8080
        }
    }

    /// 获取代理端口
    pub fn get_proxy_port(&self) -> u16 {
        if let Some(dual_port) = &self.dual_port {
            dual_port.proxy.http.port
        } else if let Some(server) = &self.server {
            server.port // 单端口模式下，代理和管理共用端口
        } else {
            8081
        }
    }

    // 已删除：get_proxy_https_port() 方法
    // 原因：不再支持HTTPS配置

    /// 验证配置的有效性
    pub fn validate(&self) -> Result<(), String> {
        // 检查配置模式
        if self.server.is_none() && self.dual_port.is_none() {
            return Err("Either server or dual_port configuration must be provided".to_string());
        }

        if self.server.is_some() && self.dual_port.is_some() {
            return Err(
                "Cannot use both server and dual_port configurations simultaneously".to_string(),
            );
        }

        // 验证双端口配置
        if let Some(dual_port) = &self.dual_port {
            dual_port.validate()?;
        }

        // 验证传统配置 - 简化版
        if let Some(server) = &self.server {
            if server.port == 0 {
                return Err(format!("Invalid server port: {}", server.port));
            }
            if server.workers == 0 {
                return Err("Worker count must be greater than 0".to_string());
            }
        }

        // 验证数据库配置
        if self.database.url.is_empty() {
            return Err("Database URL cannot be empty".to_string());
        }
        if self.database.max_connections == 0 {
            return Err("Database max_connections must be greater than 0".to_string());
        }

        // 验证Redis配置
        if self.redis.url.is_empty() {
            return Err("Redis URL cannot be empty".to_string());
        }

        Ok(())
    }

    /// 获取所有监听地址信息 - 简化版（仅HTTP）
    pub fn get_listener_info(&self) -> Vec<(String, String, String)> {
        if let Some(dual_port) = &self.dual_port {
            dual_port
                .get_all_listeners()
                .into_iter()
                .map(|(name, addr, protocol)| (name, addr.to_string(), protocol))
                .collect()
        } else if let Some(server) = &self.server {
            vec![(
                "server-http".to_string(),
                format!("{}:{}", server.host, server.port),
                "HTTP".to_string(),
            )]
        } else {
            Vec::new()
        }
    }

    /// 是否启用追踪
    pub fn is_trace_enabled(&self) -> bool {
        false
    }
}
