use serde::Serialize;

/// 通用的服务层响应包装，可附带提示消息。
#[derive(Debug, Serialize)]
pub struct ServiceResponse<T> {
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl<T> ServiceResponse<T> {
    #[must_use]
    pub const fn new(data: T) -> Self {
        Self {
            data,
            message: None,
        }
    }

    #[must_use]
    pub fn with_message(data: T, message: impl Into<String>) -> Self {
        Self {
            data,
            message: Some(message.into()),
        }
    }
}
