//! # 配置管理器
//!
//! 统一的配置管理接口，支持热重载、加密和环境变量覆盖

use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, linfo, lwarn};
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

use super::{AppConfig, ConfigCrypto, ConfigEvent, ConfigWatcher, SensitiveFields};

/// 配置管理器
pub struct ConfigManager {
    /// 配置监控器
    watcher: Option<ConfigWatcher>,
    /// 当前配置
    config: Arc<RwLock<AppConfig>>,
    /// 配置加密器
    crypto: Option<ConfigCrypto>,
    /// 敏感字段定义
    sensitive_fields: SensitiveFields,
    /// 环境变量覆盖映射
    #[allow(dead_code)]
    env_overrides: HashMap<String, String>,
}

impl ConfigManager {
    /// 创建配置管理器
    pub async fn new() -> crate::error::Result<Self> {
        // 优先使用环境变量指定的配置文件路径
        let config_file = env::var("API_PROXY_CONFIG_PATH").unwrap_or_else(|_| {
            let env = env::var("RUST_ENV").unwrap_or_else(|_| "dev".to_string());
            format!("config/config.{env}.toml")
        });

        Self::from_file(Path::new(&config_file)).await
    }

    /// 从指定文件创建配置管理器
    #[allow(
        clippy::unused_async,
        clippy::cognitive_complexity,
        clippy::too_many_lines
    )]
    pub async fn from_file(config_path: &Path) -> crate::error::Result<Self> {
        // 加载初始配置
        let mut config = Self::load_config_file(config_path)?;

        // 创建配置加密器（如果需要）
        let crypto = if env::var("PROXY_ENABLE_CONFIG_ENCRYPTION").unwrap_or_default() == "true" {
            Some(ConfigCrypto::from_env()?)
        } else {
            None
        };

        // 初始化敏感字段
        let sensitive_fields = SensitiveFields::new();

        // 初始化环境变量覆盖
        let env_overrides = Self::build_env_overrides();

        // 应用环境变量覆盖
        Self::apply_env_overrides(&mut config, &env_overrides)?;

        let config = Arc::new(RwLock::new(config));

        // 创建文件监控器（如果启用）
        let watcher = if env::var("PROXY_DISABLE_CONFIG_WATCH").unwrap_or_default() == "true" {
            None
        } else {
            match ConfigWatcher::new(config_path) {
                Ok(watcher) => {
                    let config_clone = Arc::clone(&config);
                    let env_overrides_clone = env_overrides.clone();

                    // 监听配置变更事件
                    let mut event_receiver = watcher.subscribe();
                    tokio::spawn(async move {
                        while let Ok(event) = event_receiver.recv().await {
                            match event {
                                ConfigEvent::Reloaded(new_config) => {
                                    let mut final_config = (*new_config).clone();
                                    if let Err(e) = Self::apply_env_overrides(
                                        &mut final_config,
                                        &env_overrides_clone,
                                    ) {
                                        lwarn!(
                                            "system",
                                            LogStage::Configuration,
                                            LogComponent::Config,
                                            "env_override_failed",
                                            &format!("应用环境变量覆盖失败: {e}")
                                        );
                                    } else {
                                        *config_clone.write().await = final_config;
                                        linfo!(
                                            "system",
                                            LogStage::Configuration,
                                            LogComponent::Config,
                                            "reload_complete",
                                            "配置热重载并应用环境变量覆盖完成"
                                        );
                                    }
                                }
                                ConfigEvent::ReloadFailed(error) => {
                                    lwarn!(
                                        "system",
                                        LogStage::Configuration,
                                        LogComponent::Config,
                                        "reload_failed",
                                        &format!("配置重载失败: {error}")
                                    );
                                }
                                ConfigEvent::FileDeleted => {
                                    lwarn!(
                                        "system",
                                        LogStage::Configuration,
                                        LogComponent::Config,
                                        "file_deleted",
                                        "配置文件被删除"
                                    );
                                }
                            }
                        }
                    });

                    Some(watcher)
                }
                Err(e) => {
                    lwarn!(
                        "system",
                        LogStage::Startup,
                        LogComponent::Config,
                        "watcher_start_failed",
                        &format!("无法启动配置文件监控: {e}, 将禁用热重载功能")
                    );
                    None
                }
            }
        };

        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::Config,
            "init_complete",
            "配置管理器初始化完成"
        );
        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::Config,
            "init_status_reload",
            &format!(
                "- 热重载: {}",
                if watcher.is_some() {
                    "启用"
                } else {
                    "禁用"
                }
            )
        );
        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::Config,
            "init_status_crypto",
            &format!("- 加密: {}", if crypto.is_some() { "启用" } else { "禁用" })
        );
        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::Config,
            "init_status_env",
            &format!("- 环境变量覆盖: {} 个", env_overrides.len())
        );

        Ok(Self {
            watcher,
            config,
            crypto,
            sensitive_fields,
            env_overrides,
        })
    }

    /// 获取当前配置
    pub async fn get_config(&self) -> AppConfig {
        (*self.config.read().await).clone()
    }

    /// 订阅配置变更事件
    #[must_use]
    pub fn subscribe_changes(&self) -> Option<broadcast::Receiver<ConfigEvent>> {
        self.watcher
            .as_ref()
            .map(super::watcher::ConfigWatcher::subscribe)
    }

    /// 手动重载配置
    pub async fn reload(&self) -> crate::error::Result<()> {
        if let Some(watcher) = &self.watcher {
            watcher.reload().await?;
            linfo!(
                "system",
                LogStage::Configuration,
                LogComponent::Config,
                "manual_reload_ok",
                "手动重载配置成功"
            );
        } else {
            return Err(crate::error!(Config, "配置热重载功能未启用"));
        }
        Ok(())
    }

    /// 获取敏感字段定义
    #[must_use]
    pub const fn get_sensitive_fields(&self) -> &SensitiveFields {
        &self.sensitive_fields
    }

    /// 加密敏感配置值
    pub fn encrypt_value(&self, value: &str) -> crate::error::Result<String> {
        if let Some(crypto) = &self.crypto {
            let encrypted = crypto.encrypt(value)?;
            Ok(serde_json::to_string(&encrypted)
                .map_err(|e| crate::error!(Config, format!("序列化加密数据失败: {e}")))?)
        } else {
            Err(crate::error!(Config, "配置加密功能未启用"))
        }
    }

    /// 解密敏感配置值
    pub fn decrypt_value(&self, encrypted_json: &str) -> crate::error::Result<String> {
        if let Some(crypto) = &self.crypto {
            let encrypted = serde_json::from_str(encrypted_json)
                .map_err(|e| crate::error!(Config, format!("反序列化加密数据失败: {e}")))?;
            crypto.decrypt(&encrypted)
        } else {
            Err(crate::error!(Config, "配置加密功能未启用"))
        }
    }

    /// 加载配置文件
    fn load_config_file(path: &Path) -> crate::error::Result<AppConfig> {
        if !path.exists() {
            return Err(crate::error!(Config, format!("配置文件不存在: {}", path.display())));
        }

        let config_content = std::fs::read_to_string(path).map_err(|e| {
            crate::error!(
                Config,
                format!("读取配置文件失败: {}: {}", path.display(), e)
            )
        })?;

        let config: AppConfig = toml::from_str(&config_content).map_err(|e| {
            crate::error!(
                Config,
                format!("TOML解析失败 - 配置文件: {}, 详细错误: {}", path.display(), e)
            )
        })?;

        // 验证配置
        super::validate_config(&config)?;

        Ok(config)
    }

    /// 构建环境变量覆盖映射
    fn build_env_overrides() -> HashMap<String, String> {
        let mut overrides = HashMap::new();

        // 扫描所有环境变量，查找以 PROXY_ 开头的
        for (key, value) in env::vars() {
            if let Some(config_key) = key.strip_prefix("PROXY_") {
                // 转换环境变量名为配置路径
                // 例如: PROXY_SERVER_PORT -> server.port
                let config_path = config_key.to_lowercase().replace('_', ".");
                overrides.insert(config_path, value);
            }
        }

        ldebug!(
            "system",
            LogStage::Configuration,
            LogComponent::Config,
            "env_override_scan",
            &format!("发现 {} 个环境变量覆盖", overrides.len())
        );
        overrides
    }

    /// 应用环境变量覆盖
    fn apply_env_overrides(
        config: &mut AppConfig,
        overrides: &HashMap<String, String>,
    ) -> crate::error::Result<()> {
        for (path, value) in overrides {
            ldebug!(
                "system",
                LogStage::Configuration,
                LogComponent::Config,
                "apply_env_override",
                &format!(
                    "应用环境变量覆盖: {} = {}",
                    path,
                    if path.contains("password") || path.contains("key") || path.contains("secret")
                    {
                        "***"
                    } else {
                        value
                    }
                )
            );

            Self::apply_override_to_config(config, path, value)?;
        }
        Ok(())
    }

    /// 将环境变量覆盖应用到配置对象
    fn apply_override_to_config(
        config: &mut AppConfig,
        path: &str,
        value: &str,
    ) -> crate::error::Result<()> {
        let parts: Vec<&str> = path.split('.').collect();

        match parts.as_slice() {
            ["dual_port", "workers"] => {
                if let Some(ref mut dual_port) = config.dual_port {
                    dual_port.workers = value
                        .parse()
                        .map_err(|e| crate::error!(Config, format!("无效的工作线程数: {value}: {e}")))?;
                }
            }
            ["dual_port", "management", "http", "host"] => {
                if let Some(ref mut dual_port) = config.dual_port {
                    dual_port.management.http.host = value.to_string();
                }
            }
            ["dual_port", "management", "http", "port"] => {
                if let Some(ref mut dual_port) = config.dual_port {
                    dual_port.management.http.port = value
                        .parse()
                        .map_err(|e| crate::error!(Config, format!("无效的管理端口: {value}: {e}")))?;
                }
            }
            ["dual_port", "proxy", "http", "host"] => {
                if let Some(ref mut dual_port) = config.dual_port {
                    dual_port.proxy.http.host = value.to_string();
                }
            }
            ["dual_port", "proxy", "http", "port"] => {
                if let Some(ref mut dual_port) = config.dual_port {
                    dual_port.proxy.http.port = value
                        .parse()
                        .map_err(|e| crate::error!(Config, format!("无效的代理端口: {value}: {e}")))?;
                }
            }
            ["database", "url"] => config.database.url = value.to_string(),
            ["database", "max", "connections"] | ["database", "maxconnections"] => {
                config.database.max_connections = value
                    .parse()
                    .map_err(|e| crate::error!(Config, format!("无效的最大连接数: {value}: {e}")))?;
            }
            ["cache", "redis", "url"] | ["redis", "url"] => {
                let redis = config
                    .cache
                    .redis
                    .get_or_insert_with(super::RedisConfig::default);
                redis.url = value.to_string();
            }
            ["cache", "redis", "pool", "size"]
            | ["cache", "redis", "poolsize"]
            | ["redis", "pool", "size"]
            | ["redis", "poolsize"] => {
                let redis = config
                    .cache
                    .redis
                    .get_or_insert_with(super::RedisConfig::default);
                redis.pool_size = value
                    .parse()
                    .map_err(|e| crate::error!(Config, format!("无效的Redis连接池大小: {value}: {e}")))?;
            }

            _ => {
                lwarn!(
                    "system",
                    LogStage::Configuration,
                    LogComponent::Config,
                    "unknown_env_override",
                    &format!("未知的配置路径，忽略环境变量覆盖: {path}")
                );
            }
        }

        Ok(())
    }
}
