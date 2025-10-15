use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("配置文件解析失败: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("配置文件加载失败: {0}")]
    Load(String),
}
