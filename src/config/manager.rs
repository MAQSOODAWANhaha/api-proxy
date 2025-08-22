//! # 配置管理器
//!
//! 统一的配置管理接口，支持热重载、加密和环境变量覆盖

use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tracing::{debug, info, warn};

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
        let config_file = if let Ok(path) = env::var("API_PROXY_CONFIG_PATH") {
            path
        } else {
            let env = env::var("RUST_ENV").unwrap_or_else(|_| "dev".to_string());
            format!("config/config.{env}.toml")
        };

        Self::from_file(&config_file).await
    }

    /// 从指定文件创建配置管理器
    pub async fn from_file(config_path: impl AsRef<Path>) -> crate::error::Result<Self> {
        let config_path = config_path.as_ref();

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
        let watcher = if env::var("PROXY_DISABLE_CONFIG_WATCH").unwrap_or_default() != "true" {
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
                                        warn!("应用环境变量覆盖失败: {}", e);
                                    } else {
                                        *config_clone.write().await = final_config;
                                        info!("配置热重载并应用环境变量覆盖完成");
                                    }
                                }
                                ConfigEvent::ReloadFailed(error) => {
                                    warn!("配置重载失败: {}", error);
                                }
                                ConfigEvent::FileDeleted => {
                                    warn!("配置文件被删除");
                                }
                            }
                        }
                    });

                    Some(watcher)
                }
                Err(e) => {
                    warn!("无法启动配置文件监控: {}, 将禁用热重载功能", e);
                    None
                }
            }
        } else {
            None
        };

        info!("配置管理器初始化完成");
        info!(
            "- 热重载: {}",
            if watcher.is_some() {
                "启用"
            } else {
                "禁用"
            }
        );
        info!("- 加密: {}", if crypto.is_some() { "启用" } else { "禁用" });
        info!("- 环境变量覆盖: {} 个", env_overrides.len());

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
    pub fn subscribe_changes(&self) -> Option<broadcast::Receiver<ConfigEvent>> {
        self.watcher.as_ref().map(|w| w.subscribe())
    }

    /// 手动重载配置
    pub async fn reload(&self) -> crate::error::Result<()> {
        if let Some(watcher) = &self.watcher {
            watcher.reload().await?;
            info!("手动重载配置成功");
        } else {
            return Err(crate::error::ProxyError::config("配置热重载功能未启用"));
        }
        Ok(())
    }

    /// 获取敏感字段定义
    pub fn get_sensitive_fields(&self) -> &SensitiveFields {
        &self.sensitive_fields
    }

    /// 加密敏感配置值
    pub fn encrypt_value(&self, value: &str) -> crate::error::Result<String> {
        if let Some(crypto) = &self.crypto {
            let encrypted = crypto.encrypt(value)?;
            Ok(serde_json::to_string(&encrypted).map_err(|e| {
                crate::error::ProxyError::config_with_source("序列化加密数据失败", e)
            })?)
        } else {
            Err(crate::error::ProxyError::config("配置加密功能未启用"))
        }
    }

    /// 解密敏感配置值
    pub fn decrypt_value(&self, encrypted_json: &str) -> crate::error::Result<String> {
        if let Some(crypto) = &self.crypto {
            let encrypted = serde_json::from_str(encrypted_json).map_err(|e| {
                crate::error::ProxyError::config_with_source("反序列化加密数据失败", e)
            })?;
            crypto.decrypt(&encrypted)
        } else {
            Err(crate::error::ProxyError::config("配置加密功能未启用"))
        }
    }

    /// 加载配置文件
    fn load_config_file(path: &Path) -> crate::error::Result<AppConfig> {
        if !path.exists() {
            return Err(crate::error::ProxyError::config(format!(
                "配置文件不存在: {:?}",
                path
            )));
        }

        let config_content = std::fs::read_to_string(path).map_err(|e| {
            crate::error::ProxyError::config_with_source(format!("读取配置文件失败: {:?}", path), e)
        })?;

        let config: AppConfig = toml::from_str(&config_content).map_err(|e| {
            crate::error::ProxyError::config_with_source(
                format!("TOML解析失败 - 配置文件: {:?}, 详细错误: {}", path, e),
                e,
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

        debug!("发现 {} 个环境变量覆盖", overrides.len());
        overrides
    }

    /// 应用环境变量覆盖
    fn apply_env_overrides(
        config: &mut AppConfig,
        overrides: &HashMap<String, String>,
    ) -> crate::error::Result<()> {
        for (path, value) in overrides {
            debug!(
                "应用环境变量覆盖: {} = {}",
                path,
                if path.contains("password") || path.contains("key") || path.contains("secret") {
                    "***"
                } else {
                    value
                }
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
            ["server", "host"] => {
                if let Some(ref mut server) = config.server {
                    server.host = value.to_string();
                }
            }
            ["server", "port"] => {
                if let Some(ref mut server) = config.server {
                    server.port = value.parse().map_err(|e| {
                        crate::error::ProxyError::config_with_source(
                            format!("无效的端口号: {}", value),
                            e,
                        )
                    })?;
                }
            }
            ["server", "https", "port"] | ["server", "httpsport"] => {
                if let Some(ref mut server) = config.server {
                    server.https_port = value.parse().map_err(|e| {
                        crate::error::ProxyError::config_with_source(
                            format!("无效的HTTPS端口号: {}", value),
                            e,
                        )
                    })?;
                }
            }
            ["server", "workers"] => {
                if let Some(ref mut server) = config.server {
                    server.workers = value.parse().map_err(|e| {
                        crate::error::ProxyError::config_with_source(
                            format!("无效的工作线程数: {}", value),
                            e,
                        )
                    })?;
                }
            }
            ["database", "url"] => config.database.url = value.to_string(),
            ["database", "max", "connections"] | ["database", "maxconnections"] => {
                config.database.max_connections = value.parse().map_err(|e| {
                    crate::error::ProxyError::config_with_source(
                        format!("无效的最大连接数: {}", value),
                        e,
                    )
                })?;
            }
            ["redis", "url"] => config.redis.url = value.to_string(),
            ["redis", "pool", "size"] | ["redis", "poolsize"] => {
                config.redis.pool_size = value.parse().map_err(|e| {
                    crate::error::ProxyError::config_with_source(
                        format!("无效的Redis连接池大小: {}", value),
                        e,
                    )
                })?;
            }
            // TLS 配置已移除，忽略相关环境变量覆盖
            ["tls", "cert", "path"] | ["tls", "certpath"] => {
                warn!(
                    "TLS configuration has been removed, ignoring environment variable override for tls.cert_path"
                );
            }
            ["tls", "acme", "email"] | ["tls", "acmeemail"] => {
                warn!(
                    "TLS configuration has been removed, ignoring environment variable override for tls.acme_email"
                );
            }
            _ => {
                warn!("未知的配置路径，忽略环境变量覆盖: {}", path);
            }
        }

        Ok(())
    }
}
