use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration parse failed: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("Configuration load failed: {0}")]
    Load(String),
}
