//! # 应用配置结构定义

use super::dual_port_config::DualPortServerConfig;
use serde::{Deserialize, Serialize};

/// 应用主配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 双端口服务器配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dual_port: Option<DualPortServerConfig>,
    /// 数据库配置
    pub database: super::DatabaseConfig,
    /// 缓存配置
    pub cache: CacheConfig,
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
    /// Redis 缓存配置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redis: Option<RedisConfig>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            cache_type: CacheType::Memory,
            memory_max_entries: 10000,
            default_ttl: 300,
            redis: None,
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
            max_connections: 10,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            dual_port: Some(DualPortServerConfig::default()),
            database: super::DatabaseConfig::default(),
            cache: CacheConfig::default(),
        }
    }
}

impl AppConfig {
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
        self.dual_port
            .as_ref()
            .map_or(9090, |dual_port| dual_port.management.http.port)
    }

    /// 获取代理端口
    pub fn get_proxy_port(&self) -> u16 {
        self.dual_port
            .as_ref()
            .map_or(8080, |dual_port| dual_port.proxy.http.port)
    }

    // 已删除：get_proxy_https_port() 方法
    // 原因：不再支持HTTPS配置

    /// 验证配置的有效性
    pub fn validate(&self) -> Result<(), String> {
        // 验证双端口配置 - 必须提供
        let dual_port = self.dual_port.as_ref().ok_or_else(|| {
            "dual_port configuration must be provided (single-port mode is no longer supported)"
                .to_string()
        })?;

        // 验证双端口配置
        dual_port.validate()?;

        // 验证数据库配置
        if self.database.url.is_empty() {
            return Err("Database URL cannot be empty".to_string());
        }
        if self.database.max_connections == 0 {
            return Err("Database max_connections must be greater than 0".to_string());
        }

        match self.cache.cache_type {
            CacheType::Memory => {
                if self.cache.redis.is_some() {
                    return Err("cache.redis 配置仅在 cache_type = \"redis\" 时可用".to_string());
                }
            }
            CacheType::Redis => {
                let redis = self
                    .cache
                    .redis
                    .as_ref()
                    .ok_or_else(|| "Redis cache configuration must be provided".to_string())?;

                if redis.url.is_empty() {
                    return Err("Redis URL cannot be empty".to_string());
                }
            }
        }

        Ok(())
    }

    /// 获取所有监听地址信息 - 双端口模式
    pub fn get_listener_info(&self) -> Vec<(String, String, String)> {
        self.dual_port.as_ref().map_or_else(Vec::new, |dual_port| {
            dual_port
                .get_all_listeners()
                .into_iter()
                .map(|(name, addr, protocol)| (name, addr.to_string(), protocol))
                .collect()
        })
    }

    /// 是否启用追踪
    pub fn is_trace_enabled(&self) -> bool {
        false
    }
}
