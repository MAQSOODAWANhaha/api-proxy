//! # 配置管理模块
//!
//! 处理应用配置加载、验证和管理

mod app_config;
mod dual_port_config;
mod database;
mod crypto;
mod watcher;
mod manager;

pub use app_config::{AppConfig, CacheConfig, CacheType, RedisConfig, ServerConfig, TlsConfig, TraceConfig};
pub use dual_port_config::{DualPortServerConfig, EnabledServices, ManagementPortConfig, ProxyPortConfig};
pub use database::DatabaseConfig;
pub use crypto::{ConfigCrypto, EncryptedValue, SensitiveFields};
pub use watcher::{ConfigEvent, ConfigWatcher};
pub use manager::ConfigManager;

use std::env;
use std::path::Path;

/// 加载配置文件
pub fn load_config() -> crate::error::Result<AppConfig> {
    let env = env::var("RUST_ENV").unwrap_or_else(|_| "dev".to_string());
    let config_file = format!("config/config.{env}.toml");
    
    if !Path::new(&config_file).exists() {
        return Err(crate::error::ProxyError::config(
            format!("配置文件不存在: {config_file}")
        ));
    }
    
    let config_content = std::fs::read_to_string(&config_file)
        .map_err(|e| crate::error::ProxyError::config_with_source(
            format!("读取配置文件失败: {config_file}"),
            e
        ))?;
    
    let config: AppConfig = toml::from_str(&config_content)?;
    
    // 验证配置的有效性
    validate_config(&config)?;
    
    Ok(config)
}

/// 验证配置有效性
fn validate_config(config: &AppConfig) -> crate::error::Result<()> {
    // 验证服务器配置（传统单端口模式）
    if let Some(server) = &config.server {
        if server.port == 0 || server.port > 65535 {
            return Err(crate::error::ProxyError::config(
                format!("无效的服务器端口: {}", server.port)
            ));
        }
        
        if server.https_port == 0 || server.https_port > 65535 {
            return Err(crate::error::ProxyError::config(
                format!("无效的HTTPS端口: {}", server.https_port)
            ));
        }
        
        if server.workers == 0 {
            return Err(crate::error::ProxyError::config(
                "工作线程数必须大于0"
            ));
        }
    }
    
    // 验证双端口配置
    if let Some(dual_port) = &config.dual_port {
        dual_port.validate()
            .map_err(|e| crate::error::ProxyError::config(e))?;
    }
    
    // 验证数据库配置
    if config.database.url.is_empty() {
        return Err(crate::error::ProxyError::config(
            "数据库URL不能为空"
        ));
    }
    
    if config.database.max_connections == 0 {
        return Err(crate::error::ProxyError::config(
            "数据库最大连接数必须大于0"
        ));
    }
    
    // 验证Redis配置
    if config.redis.url.is_empty() {
        return Err(crate::error::ProxyError::config(
            "Redis URL不能为空"
        ));
    }
    
    // 验证TLS配置（传统模式）
    if let Some(tls) = &config.tls {
        if tls.domains.is_empty() {
            return Err(crate::error::ProxyError::config(
                "必须配置至少一个域名"
            ));
        }
        
        if tls.acme_email.is_empty() {
            return Err(crate::error::ProxyError::config(
                "ACME邮箱不能为空"
            ));
        }
    }
    
    Ok(())
}