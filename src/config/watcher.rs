//! # 配置文件监控模块
//!
//! 实现配置文件的热重载功能

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

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
        
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    if let Err(e) = Self::handle_file_event(
                        &event,
                        &config_clone,
                        &sender_clone,
                        &path_clone,
                    ) {
                        error!("处理文件变更事件失败: {}", e);
                    }
                }
                Err(e) => {
                    error!("文件监控错误: {}", e);
                }
            }
        }).map_err(|e| crate::error::ProxyError::config_with_source(
            "创建文件监控器失败",
            e
        ))?;

        // 监控配置文件目录
        let config_dir = config_path.parent()
            .ok_or_else(|| crate::error::ProxyError::config("无法获取配置文件目录"))?;
            
        watcher.watch(config_dir, RecursiveMode::NonRecursive)
            .map_err(|e| crate::error::ProxyError::config_with_source(
                "启动文件监控失败",
                e
            ))?;

        info!("配置文件监控器已启动: {:?}", config_path);

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
                info!("配置重载成功");
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("配置重载失败: {}", e);
                let _ = self.event_sender.send(ConfigEvent::ReloadFailed(error_msg.clone()));
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
        let is_our_file = event.paths.iter().any(|path| {
            path.file_name() == config_path.file_name()
        });

        if !is_our_file {
            return Ok(());
        }

        match &event.kind {
            EventKind::Modify(_) | EventKind::Create(_) => {
                debug!("检测到配置文件变更: {:?}", event.paths);
                
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
                        info!("配置文件热重载成功");
                    }
                    Err(e) => {
                        let error_msg = format!("配置文件重载失败: {}", e);
                        warn!("{}", error_msg);
                        let _ = sender.send(ConfigEvent::ReloadFailed(error_msg));
                    }
                }
            }
            EventKind::Remove(_) => {
                warn!("配置文件被删除: {:?}", event.paths);
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
        return Err(crate::error::ProxyError::config(
            format!("配置文件不存在: {:?}", path)
        ));
    }

    let config_content = std::fs::read_to_string(path)
        .map_err(|e| crate::error::ProxyError::config_with_source(
            format!("读取配置文件失败: {:?}", path),
            e
        ))?;

    let config: AppConfig = toml::from_str(&config_content)?;
    
    // 验证配置
    super::validate_config(&config)?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_config_watcher_creation() {
        // 创建临时配置文件
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, r#"
[server]
host = "127.0.0.1"
port = 8080
https_port = 8443
workers = 4

[database]
url = "sqlite:./test.db"
max_connections = 10
connect_timeout = 30
query_timeout = 30

[redis]
url = "redis://127.0.0.1:6379"
pool_size = 10

[tls]
cert_path = "./certs"
acme_email = "test@example.com"
domains = ["localhost"]
        "#).unwrap();

        let watcher = ConfigWatcher::new(temp_file.path()).unwrap();
        let config = watcher.get_config().await;
        
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.workers, 4);
    }

    #[tokio::test]
    async fn test_config_hot_reload() {
        // 创建临时配置文件
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, r#"
[server]
host = "127.0.0.1"
port = 8080
https_port = 8443
workers = 4

[database]
url = "sqlite:./test.db"
max_connections = 10
connect_timeout = 30
query_timeout = 30

[redis]
url = "redis://127.0.0.1:6379"
pool_size = 10

[tls]
cert_path = "./certs"
acme_email = "test@example.com"
domains = ["localhost"]
        "#).unwrap();

        let watcher = ConfigWatcher::new(temp_file.path()).unwrap();
        let mut event_receiver = watcher.subscribe();
        
        // 验证初始配置
        let config = watcher.get_config().await;
        assert_eq!(config.server.workers, 4);

        // 修改配置文件
        write!(temp_file, r#"
[server]
host = "127.0.0.1"
port = 8080
https_port = 8443
workers = 8

[database]
url = "sqlite:./test.db"
max_connections = 10
connect_timeout = 30
query_timeout = 30

[redis]
url = "redis://127.0.0.1:6379"
pool_size = 10

[tls]
cert_path = "./certs"
acme_email = "test@example.com"
domains = ["localhost"]
        "#).unwrap();
        temp_file.flush().unwrap();

        // 等待文件变更事件
        tokio::select! {
            event = event_receiver.recv() => {
                match event.unwrap() {
                    ConfigEvent::Reloaded(_) => {
                        let new_config = watcher.get_config().await;
                        assert_eq!(new_config.server.workers, 8);
                    }
                    _ => panic!("应该收到重载成功事件"),
                }
            }
            _ = sleep(Duration::from_secs(5)) => {
                panic!("热重载测试超时");
            }
        }
    }
}