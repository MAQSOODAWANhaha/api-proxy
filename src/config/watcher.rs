//! # 配置文件监控模块
//!
//! 实现配置文件的热重载功能

use crate::{ldebug, lerror, linfo, lwarn, logging::{LogComponent, LogStage}};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use super::AppConfig;

/// 配置变更事件
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    /// 配置重载成功
    Reloaded(Arc<AppConfig>),
    /// 配置重载失败
    ReloadFailed(String),
    /// 配置文件被删除
    FileDeleted,
}

/// 配置监控器
pub struct ConfigWatcher {
    /// 当前配置
    config: Arc<RwLock<AppConfig>>,
    /// 配置文件路径
    config_path: PathBuf,
    /// 事件发送器
    event_sender: broadcast::Sender<ConfigEvent>,
    /// 文件监控器
    _watcher: RecommendedWatcher,
}

impl ConfigWatcher {
    /// 创建新的配置监控器
    pub fn new(config_path: impl AsRef<Path>) -> crate::error::Result<Self> {
        let config_path = config_path.as_ref().to_path_buf();

        // 加载初始配置
        let initial_config = load_config_from_file(&config_path)?;
        let config = Arc::new(RwLock::new(initial_config));

        // 创建事件通道
        let (event_sender, _) = broadcast::channel(64);

        // 创建文件监控器
        let config_clone = Arc::clone(&config);
        let sender_clone = event_sender.clone();
        let path_clone = config_path.clone();

        let mut watcher =
            notify::recommended_watcher(move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    if let Err(e) =
                        Self::handle_file_event(&event, &config_clone, &sender_clone, &path_clone)
                    {
                        lerror!("system", LogStage::Configuration, LogComponent::Config, "handle_file_event_fail", &format!("处理文件变更事件失败: {}", e));
                    }
                }
                Err(e) => {
                    lerror!("system", LogStage::Configuration, LogComponent::Config, "watcher_error", &format!("文件监控错误: {}", e));
                }
            })
            .map_err(|e| crate::error::ProxyError::config_with_source("创建文件监控器失败", e))?;

        // 监控配置文件目录
        let config_dir = config_path
            .parent()
            .ok_or_else(|| crate::error::ProxyError::config("无法获取配置文件目录"))?;

        watcher
            .watch(config_dir, RecursiveMode::NonRecursive)
            .map_err(|e| crate::error::ProxyError::config_with_source("启动文件监控失败", e))?;

        linfo!(
            "system",
            LogStage::Configuration,
            LogComponent::Config,
            "config_watcher_start",
            &format!("开始监控配置文件: {:?}", config_path)
        );

        Ok(Self {
            config,
            config_path,
            event_sender,
            _watcher: watcher,
        })
    }

    /// 获取当前配置
    pub async fn get_config(&self) -> AppConfig {
        (*self.config.read().await).clone()
    }

    /// 订阅配置变更事件
    pub fn subscribe(&self) -> broadcast::Receiver<ConfigEvent> {
        self.event_sender.subscribe()
    }

    /// 手动重载配置
    pub async fn reload(&self) -> crate::error::Result<()> {
        match load_config_from_file(&self.config_path) {
            Ok(new_config) => {
                let new_config = Arc::new(new_config);
                *self.config.write().await = (*new_config).clone();

                let _ = self.event_sender.send(ConfigEvent::Reloaded(new_config));
                linfo!(
                                    "system",
                                    LogStage::Configuration,
                                    LogComponent::Config,
                                    "config_reloaded",
                                    "配置文件已重新加载"
                                );
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("配置重载失败: {}", e);
                let _ = self
                    .event_sender
                    .send(ConfigEvent::ReloadFailed(error_msg.clone()));
                Err(crate::error::ProxyError::config(error_msg))
            }
        }
    }

    /// 处理文件变更事件
    fn handle_file_event(
        event: &Event,
        config: &Arc<RwLock<AppConfig>>,
        sender: &broadcast::Sender<ConfigEvent>,
        config_path: &Path,
    ) -> crate::error::Result<()> {
        // 只处理我们关心的配置文件
        let is_our_file = event
            .paths
            .iter()
            .any(|path| path.file_name() == config_path.file_name());

        if !is_our_file {
            return Ok(());
        }

        match &event.kind {
            EventKind::Modify(_) | EventKind::Create(_) => {
ldebug!(
                                "system",
                                LogStage::Configuration,
                                LogComponent::Config,
                                "config_event",
                                &format!("配置文件事件: {:?}", event.paths)
                            );

                // 等待一小段时间，确保文件写入完成
                std::thread::sleep(std::time::Duration::from_millis(100));

                match load_config_from_file(config_path) {
                    Ok(new_config) => {
                        let new_config = Arc::new(new_config);

                        // 异步更新配置
                        let config_clone = Arc::clone(config);
                        let new_config_clone = Arc::clone(&new_config);
                        tokio::spawn(async move {
                            *config_clone.write().await = (*new_config_clone).clone();
                        });

                        let _ = sender.send(ConfigEvent::Reloaded(new_config));
                        linfo!("system", LogStage::Configuration, LogComponent::Config, "config_reloaded", "配置文件热重载成功");
                    }
                    Err(e) => {
                        let error_msg = format!("配置文件重载失败: {}", e);
                        lwarn!("system", LogStage::Configuration, LogComponent::Config, "config_reload_fail", &error_msg);
                        let _ = sender.send(ConfigEvent::ReloadFailed(error_msg));
                    }
                }
            }
            EventKind::Remove(_) => {
                lwarn!(
                                    "system",
                                    LogStage::Configuration,
                                    LogComponent::Config,
                                    "config_deleted",
                                    "配置文件被删除"
                                );
                let _ = sender.send(ConfigEvent::FileDeleted);
            }
            _ => {
                // 忽略其他事件类型
            }
        }

        Ok(())
    }
}

/// 加载配置文件
fn load_config_from_file(path: &Path) -> crate::error::Result<AppConfig> {
    if !path.exists() {
        return Err(crate::error::ProxyError::config(format!(
            "配置文件不存在: {:?}",
            path
        )));
    }

    let config_content = std::fs::read_to_string(path).map_err(|e| {
        crate::error::ProxyError::config_with_source(format!("读取配置文件失败: {:?}", path), e)
    })?;

    let config: AppConfig = toml::from_str(&config_content)?;

    // 验证配置
    super::validate_config(&config)?;

    Ok(config)
}