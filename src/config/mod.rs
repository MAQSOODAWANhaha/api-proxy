//! # 配置管理模块
//!
//! 处理应用配置加载、验证和管理

mod app_config;
mod database;
mod dual_port_config;
mod manager;

pub use app_config::{AppConfig, CacheConfig, CacheType, RedisConfig};
pub use database::DatabaseConfig;
pub use dual_port_config::{DualPortServerConfig, ManagementPortConfig, ProxyPortConfig};
pub use manager::ConfigManager;

use crate::error::Context;
use std::env;
use std::path::Path;

/// 加载配置文件
pub fn load_config() -> crate::error::Result<AppConfig> {
    let env = env::var("RUST_ENV").unwrap_or_else(|_| "dev".to_string());
    let config_file = format!("config/config.{env}.toml");

    if !Path::new(&config_file).exists() {
        return Err(crate::error::config::ConfigError::Load(format!(
            "配置文件不存在: {config_file}"
        ))
        .into());
    }

    let config_content = std::fs::read_to_string(&config_file)
        .with_context(|| format!("读取配置文件失败: {config_file}"))?;

    let config: AppConfig =
        toml::from_str(&config_content).with_context(|| format!("TOML解析失败: {config_file}"))?;

    // 验证配置的有效性
    validate_config(&config)?;

    Ok(config)
}

/// 验证配置有效性
fn validate_config(config: &AppConfig) -> crate::error::Result<()> {
    // 验证双端口配置 - 必须提供
    let dual_port = config.dual_port.as_ref().ok_or_else(|| {
        crate::error::config::ConfigError::Load(
            "dual_port configuration must be provided (single-port mode is no longer supported)"
                .to_string(),
        )
    })?;

    // 验证双端口配置
    dual_port.validate().context("双端口配置校验失败")?;

    // 验证数据库配置
    if config.database.url.is_empty() {
        return Err(
            crate::error::config::ConfigError::Load("数据库URL不能为空".to_string()).into(),
        );
    }

    if config.database.max_connections == 0 {
        return Err(crate::error::config::ConfigError::Load(
            "数据库最大连接数必须大于0".to_string(),
        )
        .into());
    }

    if matches!(config.cache.cache_type, CacheType::Memory) && config.cache.redis.is_some() {
        return Err(crate::error::config::ConfigError::Load(
            "cache.redis 仅在 cache_type 为 redis 时允许".to_string(),
        )
        .into());
    }

    if matches!(config.cache.cache_type, CacheType::Redis) {
        let redis_config = config.cache.redis.as_ref().ok_or_else(|| {
            crate::error::config::ConfigError::Load("需要提供 Redis 缓存配置".to_string())
        })?;

        if redis_config.url.is_empty() {
            return Err(
                crate::error::config::ConfigError::Load("Redis URL不能为空".to_string()).into(),
            );
        }
    }

    if config.auth.jwt_expires_in <= 0 {
        return Err(crate::error::config::ConfigError::Load(
            "auth.jwt_expires_in 必须为正数".to_string(),
        )
        .into());
    }
    if config.auth.refresh_expires_in <= config.auth.jwt_expires_in {
        return Err(crate::error::config::ConfigError::Load(
            "auth.refresh_expires_in 必须大于 jwt_expires_in".to_string(),
        )
        .into());
    }

    Ok(())
}
