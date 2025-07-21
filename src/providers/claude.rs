//! # Anthropic Claude API 适配器
//!
//! 实现对 Anthropic Claude API 的适配，包括请求转换、响应处理和流式传输

use crate::providers::{
    types::{
        AdapterRequest, AdapterResponse, ProviderError, ProviderResult, StreamChunk,
        ChatMessage, ChatChoice, ChatCompletionResponse, ChatCompletionRequest,
        Usage, MessageRole,
    },
    traits::ProviderAdapter,
    models::ClaudeModel,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tracing::{debug, warn};

/// Anthropic Claude API 适配器
pub struct ClaudeAdapter {
    /// 配置
    config: ClaudeConfig,
    /// 支持的端点
    supported_endpoints: HashMap<String, EndpointConfig>,
}

/// Claude 适配器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeConfig {
    /// 默认模型
    pub default_model: ClaudeModel,
    /// 支持的模型列表
    pub supported_models: Vec<ClaudeModel>,
    /// API 版本
    pub api_version: String,
    /// 最大上下文长度
    pub max_context_length: u32,
}

/// 端点配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointConfig {
    /// 是否支持流式传输
    pub streaming: bool,
    /// 路径映射
    pub path_mapping: String,
    /// 最大令牌数
    pub max_tokens: Option<u32>,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            default_model: ClaudeModel::default(),
            supported_models: ClaudeModel::all(),
            api_version: "2023-06-01".to_string(),
            max_context_length: 200000,
        }
    }
}

impl ClaudeAdapter {
    /// 创建新的 Claude 适配器
    pub fn new() -> Self {
        let config = ClaudeConfig::default();
        let mut supported_endpoints = HashMap::new();

        // 配置支持的端点
        supported_endpoints.insert(
            "/v1/chat/completions".to_string(),
            EndpointConfig {
                streaming: true,
                path_mapping: "/v1/messages".to_string(),
                max_tokens: Some(4096),
            },
        );

        supported_endpoints.insert(
            "/v1/messages".to_string(),
            EndpointConfig {
                streaming: true,
                path_mapping: "/v1/messages".to_string(),
                max_tokens: Some(4096),
            },
        );

        Self {
            config,
            supported_endpoints,
        }
    }

    /// 使用自定义配置创建适配器
    pub fn with_config(config: ClaudeConfig) -> Self {
        let mut adapter = Self::new();
        adapter.config = config;
        adapter
    }

    /// 验证模型是否支持
    pub fn validate_model(&self, model: &str) -> bool {
        if let Some(model_enum) = ClaudeModel::from_str(model) {
            self.config.supported_models.contains(&model_enum)
        } else {
            false
        }
    }

    /// 转换 OpenAI 格式请求为 Claude 格式
    fn transform_openai_to_claude_chat(&self, request: &ChatCompletionRequest) -> ProviderResult<ClaudeMessageRequest> {
        // 分离系统消息和其他消息
        let mut system_message = None;
        let mut messages = Vec::new();

        for msg in &request.messages {
            match msg.role {
                MessageRole::System => {
                    if system_message.is_none() {
                        system_message = Some(msg.content.clone());
                    } else {
                        // 如果已有系统消息，合并内容
                        if let Some(ref mut existing) = system_message {
                            existing.push_str("\n\n");
                            existing.push_str(&msg.content);
                        }
                    }
                }
                MessageRole::User => {
                    messages.push(ClaudeMessage {
                        role: "user".to_string(),
                        content: msg.content.clone(),
                    });
                }
                MessageRole::Assistant => {
                    messages.push(ClaudeMessage {
                        role: "assistant".to_string(),
                        content: msg.content.clone(),
                    });
                }
                MessageRole::Tool => {
                    // Claude API 暂不直接支持工具消息，转换为用户消息
                    messages.push(ClaudeMessage {
                        role: "user".to_string(),
                        content: format!("Tool result: {}", msg.content),
                    });
                }
            }
        }

        // 构建 Claude 请求
        let mut claude_request = ClaudeMessageRequest {
            model: request.model.clone(),
            max_tokens: request.parameters.max_tokens.unwrap_or(1024),
            messages,
            system: system_message,
            temperature: request.parameters.temperature,
            top_p: request.parameters.top_p,
            stop_sequences: request.parameters.stop.clone(),
            stream: request.parameters.stream,
        };

        // 验证模型
        if !self.validate_model(&claude_request.model) {
            // 如果模型不支持，使用默认模型
            warn!("Model '{}' not supported, using default model '{}'", 
                  claude_request.model, self.config.default_model);
            claude_request.model = self.config.default_model.to_string();
        }

        // 验证最大令牌数
        if claude_request.max_tokens > 4096 {
            claude_request.max_tokens = 4096;
        }

        // 确保消息数组不为空
        if claude_request.messages.is_empty() {
            return Err(ProviderError::InvalidRequest(
                "Messages array cannot be empty".to_string()
            ));
        }

        // 确保最后一条消息是用户消息（Claude API 要求）
        if let Some(last_msg) = claude_request.messages.last() {
            if last_msg.role != "user" {
                claude_request.messages.push(ClaudeMessage {
                    role: "user".to_string(),
                    content: "Please continue.".to_string(),
                });
            }
        }

        Ok(claude_request)
    }

    /// 转换 Claude 响应为 OpenAI 格式
    fn transform_claude_to_openai_chat(&self, response: &ClaudeMessageResponse, model: &str) -> ProviderResult<ChatCompletionResponse> {
        let content = response.content.get(0)
            .ok_or_else(|| ProviderError::InvalidRequest("Empty response content".to_string()))?;

        let choice = ChatChoice {
            index: 0,
            message: ChatMessage {
                role: MessageRole::Assistant,
                content: content.text.clone(),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            finish_reason: match response.stop_reason.as_deref() {
                Some("end_turn") => Some("stop".to_string()),
                Some("max_tokens") => Some("length".to_string()),
                Some("stop_sequence") => Some("stop".to_string()),
                _ => Some("stop".to_string()),
            },
        };

        let usage = response.usage.as_ref().map(|u| Usage {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
            total_tokens: u.input_tokens + u.output_tokens,
        });

        Ok(ChatCompletionResponse {
            id: format!("chatcmpl-claude-{}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()),
            object: "chat.completion".to_string(),
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            model: model.to_string(),
            choices: vec![choice],
            usage,
        })
    }

    /// 处理错误响应
    fn handle_error_response(&self, status_code: u16, body: &str) -> ProviderError {
        // 尝试解析 Claude 错误格式
        if let Ok(error_data) = serde_json::from_str::<serde_json::Value>(body) {
            if let Some(error_obj) = error_data.get("error") {
                let message = error_obj.get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown error");
                
                let error_type = error_obj.get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("api_error");

                return match error_type {
                    "invalid_request_error" => ProviderError::InvalidRequest(message.to_string()),
                    "authentication_error" => ProviderError::AuthenticationFailed(message.to_string()),
                    "rate_limit_error" => ProviderError::RateLimitExceeded(message.to_string()),
                    _ => ProviderError::ApiError {
                        status_code,
                        message: message.to_string(),
                    }
                };
            }
        }

        // 默认错误处理
        match status_code {
            401 => ProviderError::AuthenticationFailed("Invalid API key".to_string()),
            429 => ProviderError::RateLimitExceeded("Rate limit exceeded".to_string()),
            _ => ProviderError::ApiError {
                status_code,
                message: body.to_string(),
            }
        }
    }
}

impl ProviderAdapter for ClaudeAdapter {
    fn name(&self) -> &str {
        "claude"
    }

    fn supports_endpoint(&self, endpoint: &str) -> bool {
        self.supported_endpoints.keys().any(|ep| endpoint.starts_with(ep))
    }

    fn supports_streaming(&self, endpoint: &str) -> bool {
        self.supported_endpoints.get(endpoint)
            .map(|config| config.streaming)
            .unwrap_or(false)
    }

    fn transform_request(&self, request: &AdapterRequest) -> ProviderResult<AdapterRequest> {
        // 验证API密钥
        if let Some(auth_header) = request.get_authorization() {
            if !auth_header.starts_with("Bearer ") {
                return Err(ProviderError::AuthenticationFailed(
                    "Invalid authorization header format".to_string()
                ));
            }
            
            let api_key = &auth_header[7..]; // 跳过 "Bearer "
            if !self.validate_api_key(api_key) {
                return Err(ProviderError::AuthenticationFailed(
                    "Invalid API key format".to_string()
                ));
            }
        } else {
            return Err(ProviderError::AuthenticationFailed(
                "Missing authorization header".to_string()
            ));
        }

        // 根据端点处理请求
        match request.path.as_str() {
            path if path.starts_with("/v1/chat/completions") => {
                self.process_chat_completion(request)
            }
            path if path.starts_with("/v1/messages") => {
                // 直接透传 Claude 原生格式
                Ok(request.clone())
            }
            _ => {
                // 其他端点暂不支持
                Err(ProviderError::UnsupportedOperation(
                    format!("Endpoint {} not supported by Claude adapter", request.path)
                ))
            }
        }
    }

    fn transform_response(&self, response: &AdapterResponse, original_request: &AdapterRequest) -> ProviderResult<AdapterResponse> {
        debug!("Transforming response for endpoint: {}", original_request.path);

        if !response.is_success() {
            return Ok(response.clone());
        }

        match original_request.path.as_str() {
            "/v1/chat/completions" => {
                // 解析 Claude 响应
                let claude_response: ClaudeMessageResponse = serde_json::from_value(response.body.clone())
                    .map_err(|e| ProviderError::SerializationError(format!("Failed to parse Claude response: {}", e)))?;

                // 提取原始请求中的模型名称
                let model = if let Some(ref body) = original_request.body {
                    if let Ok(openai_req) = serde_json::from_value::<ChatCompletionRequest>(body.clone()) {
                        openai_req.model
                    } else {
                        self.config.default_model.to_string()
                    }
                } else {
                    self.config.default_model.to_string()
                };

                // 转换为 OpenAI 格式
                let openai_response = self.transform_claude_to_openai_chat(&claude_response, &model)?;
                
                let response_body = serde_json::to_value(&openai_response)
                    .map_err(|e| ProviderError::SerializationError(format!("Failed to serialize response: {}", e)))?;

                let mut transformed_response = response.clone();
                transformed_response.body = response_body;
                
                Ok(transformed_response)
            },
            _ => {
                warn!("Unsupported endpoint for response transformation: {}", original_request.path);
                Ok(response.clone())
            }
        }
    }

    fn handle_streaming_chunk(&self, chunk: &[u8], _request: &AdapterRequest) -> ProviderResult<Option<StreamChunk>> {
        let chunk_str = std::str::from_utf8(chunk)
            .map_err(|e| ProviderError::SerializationError(format!("Invalid UTF-8: {}", e)))?;

        // Claude 流式响应格式与 OpenAI 类似，使用 SSE
        if chunk_str.trim().is_empty() {
            return Ok(None);
        }

        // 检查结束标记
        if chunk_str.contains("event: message_stop") {
            return Ok(Some(StreamChunk::final_chunk(Vec::new())));
        }

        // 处理数据块
        if chunk_str.starts_with("data: ") {
            let data_part = &chunk_str[6..]; // 跳过 "data: "
            
            if data_part.trim().starts_with("{") {
                // 尝试解析并转换 Claude 流式数据为 OpenAI 格式
                match serde_json::from_str::<serde_json::Value>(data_part.trim()) {
                    Ok(claude_data) => {
                        // 简化转换，实际可能需要更复杂的逻辑
                        if let Some(delta) = claude_data.get("delta") {
                            if let Some(text) = delta.get("text") {
                                let openai_chunk = json!({
                                    "object": "chat.completion.chunk",
                                    "choices": [{
                                        "index": 0,
                                        "delta": {
                                            "content": text
                                        }
                                    }]
                                });
                                let formatted_chunk = format!("data: {}\n\n", serde_json::to_string(&openai_chunk).unwrap());
                                return Ok(Some(StreamChunk::data(formatted_chunk.into_bytes())));
                            }
                        }
                    }
                    Err(_) => {
                        // 如果解析失败，直接传递原始数据
                        return Ok(Some(StreamChunk::data(chunk.to_vec())));
                    }
                }
            }
        }

        Ok(Some(StreamChunk::data(chunk.to_vec())))
    }

    fn validate_request(&self, request: &AdapterRequest) -> ProviderResult<()> {
        // 验证端点支持
        if !self.supports_endpoint(&request.path) {
            return Err(ProviderError::UnsupportedOperation(
                format!("Endpoint {} not supported by Claude adapter", request.path)
            ));
        }

        // 验证API密钥
        if request.get_authorization().is_none() {
            return Err(ProviderError::AuthenticationFailed(
                "Missing authorization header".to_string()
            ));
        }

        Ok(())
    }

    fn get_supported_endpoints(&self) -> Vec<String> {
        self.supported_endpoints.keys().cloned().collect()
    }
}

impl ClaudeAdapter {
    /// 验证API密钥格式
    pub fn validate_api_key(&self, api_key: &str) -> bool {
        // Claude API密钥格式验证
        api_key.starts_with("sk-ant-") && api_key.len() >= 60
    }

    /// 处理聊天完成请求
    fn process_chat_completion(&self, request: &AdapterRequest) -> ProviderResult<AdapterRequest> {
        let body = request.body.as_ref()
            .ok_or_else(|| ProviderError::InvalidRequest("Missing request body".to_string()))?;

        // 解析请求体
        let chat_request: ChatCompletionRequest = serde_json::from_value(body.clone())
            .map_err(|e| ProviderError::InvalidRequest(format!("Invalid chat completion request: {}", e)))?;

        // 转换为 Claude 格式
        let claude_request = self.transform_openai_to_claude_chat(&chat_request)?;

        // 构建 Claude API 请求
        let claude_body = serde_json::to_value(&claude_request)
            .map_err(|e| ProviderError::SerializationError(format!("Failed to serialize Claude request: {}", e)))?;

        let mut claude_request_adapted = request.clone();
        claude_request_adapted.body = Some(claude_body);
        claude_request_adapted.path = "/v1/messages".to_string();

        // 设置 Claude 特定的请求头
        claude_request_adapted.headers.insert(
            "Content-Type".to_string(),
            "application/json".to_string()
        );
        claude_request_adapted.headers.insert(
            "anthropic-version".to_string(),
            self.config.api_version.clone()
        );

        Ok(claude_request_adapted)
    }
}

// Claude API 类型定义

/// Claude 消息请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMessageRequest {
    /// 模型名称
    pub model: String,
    /// 最大令牌数
    pub max_tokens: u32,
    /// 消息列表
    pub messages: Vec<ClaudeMessage>,
    /// 系统消息（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// 温度参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p 参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// 停止序列
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// 是否流式传输
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// Claude 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMessage {
    /// 角色
    pub role: String,
    /// 内容
    pub content: String,
}

/// Claude 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMessageResponse {
    /// 响应 ID
    pub id: String,
    /// 对象类型
    #[serde(rename = "type")]
    pub response_type: String,
    /// 角色
    pub role: String,
    /// 内容
    pub content: Vec<ClaudeContent>,
    /// 模型
    pub model: String,
    /// 停止原因
    pub stop_reason: Option<String>,
    /// 停止序列
    pub stop_sequence: Option<String>,
    /// 使用情况
    pub usage: Option<ClaudeUsage>,
}

/// Claude 内容块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeContent {
    /// 内容类型
    #[serde(rename = "type")]
    pub content_type: String,
    /// 文本内容
    pub text: String,
}

/// Claude 使用情况
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeUsage {
    /// 输入令牌数
    pub input_tokens: u32,
    /// 输出令牌数
    pub output_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_claude_adapter_creation() {
        let adapter = ClaudeAdapter::new();
        assert_eq!(adapter.name(), "claude");
        assert_eq!(adapter.config.default_model, ClaudeModel::Claude3Sonnet20240229);
        assert!(adapter.config.supported_models.contains(&ClaudeModel::Claude3Opus20240229));
    }

    #[test]
    fn test_api_key_validation() {
        let adapter = ClaudeAdapter::new();
        
        assert!(adapter.validate_api_key("sk-ant-api03-1234567890abcdef1234567890abcdef1234567890abcdef12345678"));
        assert!(!adapter.validate_api_key("invalid-key"));
        assert!(!adapter.validate_api_key("sk-ant-short"));
    }

    #[test]
    fn test_supported_endpoints() {
        let adapter = ClaudeAdapter::new();
        
        assert!(adapter.supports_endpoint("/v1/chat/completions"));
        assert!(adapter.supports_endpoint("/v1/messages"));
        assert!(!adapter.supports_endpoint("/v2/unknown"));
    }

    #[test]
    fn test_model_validation() {
        let adapter = ClaudeAdapter::new();
        
        assert!(adapter.validate_model("claude-3-opus-20240229"));
        assert!(adapter.validate_model("claude-3-sonnet-20240229"));
        assert!(!adapter.validate_model("unknown-model"));
    }

    #[test]
    fn test_openai_to_claude_conversion() {
        let adapter = ClaudeAdapter::new();
        
        let openai_request = ChatCompletionRequest {
            model: "claude-3-sonnet-20240229".to_string(),
            messages: vec![
                ChatMessage::system("You are a helpful assistant"),
                ChatMessage::user("Hello"),
            ],
            parameters: super::super::types::ModelParameters {
                model: "claude-3-sonnet-20240229".to_string(),
                max_tokens: Some(1000),
                temperature: Some(0.7),
                ..Default::default()
            },
        };

        let result = adapter.transform_openai_to_claude_chat(&openai_request);
        assert!(result.is_ok());
        
        let claude_request = result.unwrap();
        assert_eq!(claude_request.model, "claude-3-sonnet-20240229");
        assert_eq!(claude_request.max_tokens, 1000);
        assert_eq!(claude_request.system, Some("You are a helpful assistant".to_string()));
        assert_eq!(claude_request.messages.len(), 1);
        assert_eq!(claude_request.messages[0].role, "user");
        assert_eq!(claude_request.messages[0].content, "Hello");
    }

    #[test]
    fn test_claude_to_openai_conversion() {
        let adapter = ClaudeAdapter::new();
        
        let claude_response = ClaudeMessageResponse {
            id: "msg_123".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![ClaudeContent {
                content_type: "text".to_string(),
                text: "Hello! How can I help you today?".to_string(),
            }],
            model: "claude-3-sonnet-20240229".to_string(),
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            usage: Some(ClaudeUsage {
                input_tokens: 10,
                output_tokens: 15,
            }),
        };

        let result = adapter.transform_claude_to_openai_chat(&claude_response, "claude-3-sonnet-20240229");
        assert!(result.is_ok());
        
        let openai_response = result.unwrap();
        assert_eq!(openai_response.model, "claude-3-sonnet-20240229");
        assert_eq!(openai_response.choices.len(), 1);
        assert_eq!(openai_response.choices[0].message.content, "Hello! How can I help you today?");
        assert_eq!(openai_response.choices[0].finish_reason, Some("stop".to_string()));
        assert!(openai_response.usage.is_some());
        assert_eq!(openai_response.usage.unwrap().total_tokens, 25);
    }

    #[test]
    fn test_streaming_chunk_handling() {
        let adapter = ClaudeAdapter::new();
        let request = AdapterRequest::new("POST", "/v1/chat/completions");
        
        let chunk = b"data: {\"type\": \"content_block_delta\", \"delta\": {\"text\": \"Hello\"}}\n\n";
        let result = adapter.handle_streaming_chunk(chunk, &request);
        
        assert!(result.is_ok());
        let response_opt = result.unwrap();
        assert!(response_opt.is_some());
        let response = response_opt.unwrap();
        assert!(!response.is_final);
        assert!(response.error.is_none());
    }

    #[test]
    fn test_streaming_stop_handling() {
        let adapter = ClaudeAdapter::new();
        let request = AdapterRequest::new("POST", "/v1/chat/completions");
        
        let chunk = b"event: message_stop\n\n";
        let result = adapter.handle_streaming_chunk(chunk, &request);
        
        assert!(result.is_ok());
        let response_opt = result.unwrap();
        assert!(response_opt.is_some());
        let response = response_opt.unwrap();
        assert!(response.is_final);
    }
}