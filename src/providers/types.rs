//! # 适配器通用类型定义

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::error::{ProxyError, Result};

/// 适配器错误类型
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Invalid request format: {0}")]
    InvalidRequest(String),
    
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),
    
    #[error("Provider API error: {status_code} - {message}")]
    ApiError { status_code: u16, message: String },
    
    #[error("Request timeout: {0}")]
    Timeout(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Response parse error: {0}")]
    ResponseParseError(String),
    
    #[error("Stream parse error: {0}")]
    StreamParseError(String),
    
    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),
}

pub type ProviderResult<T> = std::result::Result<T, ProviderError>;

/// 适配器请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterRequest {
    /// HTTP方法
    pub method: String,
    /// 请求路径
    pub path: String,
    /// 请求头
    pub headers: HashMap<String, String>,
    /// 请求体
    pub body: Option<Value>,
    /// 查询参数
    pub query: HashMap<String, String>,
}

impl AdapterRequest {
    pub fn new(method: &str, path: &str) -> Self {
        Self {
            method: method.to_string(),
            path: path.to_string(),
            headers: HashMap::new(),
            body: None,
            query: HashMap::new(),
        }
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_body(mut self, body: Value) -> Self {
        self.body = Some(body);
        self
    }

    pub fn with_query(mut self, key: &str, value: &str) -> Self {
        self.query.insert(key.to_string(), value.to_string());
        self
    }

    pub fn get_header(&self, key: &str) -> Option<&String> {
        self.headers.get(key)
    }

    pub fn get_authorization(&self) -> Option<&String> {
        self.get_header("authorization")
            .or_else(|| self.get_header("Authorization"))
    }
}

/// 适配器响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterResponse {
    /// HTTP状态码
    pub status_code: u16,
    /// 响应头
    pub headers: HashMap<String, String>,
    /// 响应体
    pub body: Value,
    /// 是否为流式响应
    pub is_streaming: bool,
}

impl AdapterResponse {
    pub fn new(status_code: u16) -> Self {
        Self {
            status_code,
            headers: HashMap::new(),
            body: Value::Null,
            is_streaming: false,
        }
    }

    pub fn success(body: Value) -> Self {
        Self {
            status_code: 200,
            headers: HashMap::new(),
            body,
            is_streaming: false,
        }
    }

    pub fn error(status_code: u16, message: &str) -> Self {
        Self {
            status_code,
            headers: HashMap::new(),
            body: serde_json::json!({
                "error": {
                    "message": message,
                    "type": "api_error",
                    "code": status_code
                }
            }),
            is_streaming: false,
        }
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn streaming(mut self) -> Self {
        self.is_streaming = true;
        self.headers.insert("Content-Type".to_string(), "text/event-stream".to_string());
        self.headers.insert("Cache-Control".to_string(), "no-cache".to_string());
        self.headers.insert("Connection".to_string(), "keep-alive".to_string());
        self
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }
}

/// 流式响应数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingResponse {
    /// 数据块
    pub data: Vec<u8>,
    /// 是否为最后一个数据块
    pub is_final: bool,
    /// 错误信息（如果有）
    pub error: Option<String>,
}

/// 流式响应块
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// 数据块
    Data(Value),
    /// 原始数据块
    Raw(Vec<u8>),
    /// 流结束标记
    Done,
    /// 错误信息
    Error(String),
}

/// 追踪统计信息
#[derive(Debug, Clone, Default)]
pub struct TraceStats {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub cache_create_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    pub cost: Option<f64>,
    pub cost_currency: Option<String>,
    pub model_name: Option<String>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
}

impl StreamChunk {
    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        match self {
            StreamChunk::Data(_) => false,
            StreamChunk::Raw(data) => data.is_empty(),
            StreamChunk::Done => true,
            StreamChunk::Error(_) => false,
        }
    }

    /// 转换为字符串（用于Raw类型）
    pub fn as_str(&self) -> Result<&str> {
        match self {
            StreamChunk::Raw(data) => std::str::from_utf8(data)
                .map_err(|e| ProxyError::internal(format!("Invalid UTF-8 in stream data: {}", e))),
            _ => Err(ProxyError::internal("Cannot convert non-Raw chunk to string".to_string())),
        }
    }
    
    /// 检查是否为最终块
    pub fn is_final(&self) -> bool {
        matches!(self, StreamChunk::Done | StreamChunk::Error(_))
    }
}

impl StreamingResponse {
    pub fn data(data: Vec<u8>) -> Self {
        Self {
            data,
            is_final: false,
            error: None,
        }
    }

    pub fn final_chunk(data: Vec<u8>) -> Self {
        Self {
            data,
            is_final: true,
            error: None,
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            data: Vec::new(),
            is_final: true,
            error: Some(error),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn as_str(&self) -> Result<&str> {
        std::str::from_utf8(&self.data)
            .map_err(|e| ProxyError::internal(format!("Invalid UTF-8 in stream data: {}", e)))
    }
}


/// 通用模型参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelParameters {
    /// 模型名称
    pub model: String,
    /// 最大令牌数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// 温度参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// 频率惩罚
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    /// 出现惩罚
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// 是否流式输出
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// 停止序列
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

impl Default for ModelParameters {
    fn default() -> Self {
        Self {
            model: "gpt-3.5-turbo".to_string(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stream: None,
            stop: None,
        }
    }
}

/// 消息角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

/// 聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn system(content: &str) -> Self {
        Self {
            role: MessageRole::System,
            content: content.to_string(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: &str) -> Self {
        Self {
            role: MessageRole::User,
            content: content.to_string(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: &str) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.to_string(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }
}

/// 聊天完成请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(flatten)]
    pub parameters: ModelParameters,
}

/// 使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 聊天完成选择
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

/// 聊天完成响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_request_creation() {
        let request = AdapterRequest::new("POST", "/v1/chat/completions")
            .with_header("Authorization", "Bearer test-key")
            .with_body(serde_json::json!({"model": "gpt-3.5-turbo"}))
            .with_query("stream", "true");

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/chat/completions");
        assert_eq!(request.get_header("Authorization"), Some(&"Bearer test-key".to_string()));
        assert!(request.body.is_some());
        assert_eq!(request.query.get("stream"), Some(&"true".to_string()));
    }

    #[test]
    fn test_adapter_response_creation() {
        let response = AdapterResponse::success(serde_json::json!({"test": "data"}))
            .with_header("Content-Type", "application/json");

        assert_eq!(response.status_code, 200);
        assert_eq!(response.headers.get("Content-Type"), Some(&"application/json".to_string()));
        assert!(!response.is_streaming);
    }

    #[test]
    fn test_streaming_response() {
        let chunk = StreamingResponse::data(b"test data".to_vec());
        assert!(!chunk.is_final);
        assert!(chunk.error.is_none());
        assert_eq!(chunk.as_str().unwrap(), "test data");

        let final_chunk = StreamingResponse::final_chunk(b"final".to_vec());
        assert!(final_chunk.is_final);

        let error_chunk = StreamingResponse::error("Test error".to_string());
        assert!(error_chunk.is_final);
        assert!(error_chunk.error.is_some());
    }

    #[test]
    fn test_chat_message_creation() {
        let system_msg = ChatMessage::system("You are a helpful assistant");
        assert_eq!(system_msg.role, MessageRole::System);
        assert_eq!(system_msg.content, "You are a helpful assistant");

        let user_msg = ChatMessage::user("Hello");
        assert_eq!(user_msg.role, MessageRole::User);

        let assistant_msg = ChatMessage::assistant("Hi there!");
        assert_eq!(assistant_msg.role, MessageRole::Assistant);
    }

    #[test]
    fn test_model_parameters() {
        let params = ModelParameters {
            model: "gpt-4".to_string(),
            max_tokens: Some(1000),
            temperature: Some(0.7),
            stream: Some(true),
            ..Default::default()
        };

        assert_eq!(params.model, "gpt-4");
        assert_eq!(params.max_tokens, Some(1000));
        assert_eq!(params.temperature, Some(0.7));
        assert_eq!(params.stream, Some(true));
    }
}