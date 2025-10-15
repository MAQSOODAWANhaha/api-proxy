//! # 配置管理模块
//!
//! 处理应用配置加载、验证和管理

mod app_config;
mod crypto;
mod database;
mod dual_port_config;
mod manager;
mod provider_config;
mod watcher;

pub use app_config::{AppConfig, CacheConfig, CacheType, RedisConfig};
pub use crypto::{ConfigCrypto, EncryptedValue, SensitiveFields};
pub use database::DatabaseConfig;
pub use dual_port_config::{DualPortServerConfig, ManagementPortConfig, ProxyPortConfig};
pub use manager::ConfigManager;
pub use provider_config::{ProviderConfig, ProviderConfigManager};
pub use watcher::{ConfigEvent, ConfigWatcher};

use std::env;
use std::path::Path;

/// 加载配置文件
pub fn load_config() -> crate::error::Result<AppConfig> {
    let env = env::var("RUST_ENV").unwrap_or_else(|_| "dev".to_string());
    let config_file = format!("config/config.{env}.toml");

    if !Path::new(&config_file).exists() {
        return Err(crate::error!(Config, format!("配置文件不存在: {config_file}")));
    }

    let config_content = std::fs::read_to_string(&config_file)
        .map_err(|e| crate::error!(Config, format!("读取配置文件失败: {config_file}: {e}")))?;

    let config: AppConfig = toml::from_str(&config_content)?;

    // 验证配置的有效性
    validate_config(&config)?;

    Ok(config)
}

/// 验证配置有效性
fn validate_config(config: &AppConfig) -> crate::error::Result<()> {
    // 验证双端口配置 - 必须提供
    let dual_port = config.dual_port.as_ref().ok_or_else(|| {
        crate::error!(
            Config,
            "dual_port configuration must be provided (single-port mode is no longer supported)",
        )
    })?;

    // 验证双端口配置
    dual_port
        .validate()
        .map_err(|e| crate::error!(Config, e))?;

    // 验证数据库配置
    if config.database.url.is_empty() {
        return Err(crate::error!(Config, "数据库URL不能为空"));
    }

    if config.database.max_connections == 0 {
        return Err(crate::error!(
            Config,
            "数据库最大连接数必须大于0",
        ));
    }

    if matches!(config.cache.cache_type, CacheType::Memory) && config.cache.redis.is_some() {
        return Err(crate::error!(
            Config,
            "cache.redis 仅在 cache_type 为 redis 时允许",
        ));
    }

    if matches!(config.cache.cache_type, CacheType::Redis) {
        let redis_config = config
            .cache
            .redis
            .as_ref()
            .ok_or_else(|| crate::error!(Config, "需要提供 Redis 缓存配置"))?;

        if redis_config.url.is_empty() {
            return Err(crate::error!(Config, "Redis URL不能为空"));
        }
    }

    Ok(())
}
