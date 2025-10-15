use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("转换错误: {0}")]
    Message(String),

    #[error("JSON转换失败: {0}")]
    Json(#[from] serde_json::Error),
}

impl ConversionError {
    #[must_use]
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}
