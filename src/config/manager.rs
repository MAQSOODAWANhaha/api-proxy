//! # 配置管理器
//!
//! 提供统一的配置加载入口：从配置文件加载结构化配置，从环境变量加载敏感信息。

use crate::error::Context;
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, linfo};
use std::env;
use std::path::Path;
use std::sync::Arc;

use super::AppConfig;

/// 配置管理器
pub struct ConfigManager {
    /// 应用配置（来自配置文件）
    config: Arc<AppConfig>,
}

impl ConfigManager {
    /// 创建配置管理器
    pub fn new() -> crate::error::Result<Self> {
        let config_file = env::var("API_PROXY_CONFIG_PATH")
            .or_else(|_| {
                env::var("CONFIG_FILE").map(|file| {
                    if Path::new(&file).is_absolute() {
                        file
                    } else {
                        format!("config/{file}")
                    }
                })
            })
            .unwrap_or_else(|_| "config/config.toml".to_string());

        Self::from_file(Path::new(&config_file))
    }

    /// 从指定文件创建配置管理器
    pub fn from_file(config_path: &Path) -> crate::error::Result<Self> {
        let mut config = Self::load_config_file(config_path)?;
        config.auth.load_jwt_secret_from_env()?;

        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::Config,
            "config_loaded",
            &format!("配置文件加载成功: {}", config_path.display())
        );
        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::Config,
            "runtime_secrets_loaded",
            &format!(
                "安全配置已从环境变量加载: JWT_SECRET={}",
                mask_secret(config.auth.jwt_secret.len())
            )
        );
        Ok(Self {
            config: Arc::new(config),
        })
    }

    /// 获取应用配置（克隆 `Arc`，开销可忽略）
    #[must_use]
    pub fn config(&self) -> Arc<AppConfig> {
        Arc::clone(&self.config)
    }

    /// 加载配置文件
    fn load_config_file(path: &Path) -> crate::error::Result<AppConfig> {
        if !path.exists() {
            return Err(crate::error::ProxyError::from(
                crate::error::config::ConfigError::Load(format!(
                    "配置文件不存在: {}",
                    path.display()
                )),
            ));
        }

        let config_content = std::fs::read_to_string(path)
            .with_context(|| format!("读取配置文件失败: {}", path.display()))?;

        let config: AppConfig = toml::from_str(&config_content)
            .with_context(|| format!("TOML解析失败 - 配置文件: {}", path.display()))?;

        super::validate_config(&config)?;

        ldebug!(
            "system",
            LogStage::Configuration,
            LogComponent::Config,
            "config_parsed",
            "配置文件解析并验证成功"
        );

        Ok(config)
    }
}

fn mask_secret(len: usize) -> String {
    format!("*** (len {len})")
}
