//! # Google Gemini API 适配器
//!
//! 实现对 Google Gemini API 的适配，包括请求转换、响应处理和流式传输

use crate::providers::{
    types::{
        AdapterRequest, AdapterResponse, ProviderError, ProviderResult, StreamChunk,
        ChatMessage, ChatChoice, ChatCompletionResponse, ChatCompletionRequest,
        Usage, MessageRole,
    },
    traits::ProviderAdapter,
    models::GeminiModel,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tracing::{debug, warn};

/// Google Gemini API 适配器
pub struct GeminiAdapter {
    /// 配置
    config: GeminiConfig,
    /// 支持的端点
    supported_endpoints: HashMap<String, EndpointConfig>,
}

/// Gemini 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiConfig {
    /// API 基础地址
    pub base_url: String,
    /// API 版本
    pub api_version: String,
    /// 支持的模型列表
    pub supported_models: Vec<GeminiModel>,
    /// 默认模型
    pub default_model: GeminiModel,
    /// 请求超时时间（秒）
    pub timeout_seconds: u64,
    /// 重试次数
    pub max_retries: u32,
}

/// 端点配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointConfig {
    /// 路径映射
    pub path_mapping: String,
    /// HTTP 方法
    pub method: String,
    /// 是否支持流式响应
    pub supports_streaming: bool,
    /// 请求转换器
    pub request_transformer: String,
    /// 响应转换器
    pub response_transformer: String,
}

/// Gemini 聊天请求格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiChatRequest {
    /// 内容列表
    pub contents: Vec<GeminiContent>,
    /// 模型参数
    #[serde(flatten)]
    pub parameters: GeminiParameters,
    /// 安全设置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_settings: Option<Vec<GeminiSafetySetting>>,
    /// 生成配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GeminiGenerationConfig>,
}

/// Gemini 内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiContent {
    /// 角色
    pub role: String,
    /// 部分内容
    pub parts: Vec<GeminiPart>,
}

/// Gemini 内容部分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiPart {
    /// 文本内容
    pub text: String,
}

/// Gemini 参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiParameters {
    /// 候选数量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_count: Option<u32>,
    /// 最大输出令牌数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    /// 温度
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Top-k
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    /// 停止序列
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

/// Gemini 安全设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiSafetySetting {
    /// 类别
    pub category: String,
    /// 阈值
    pub threshold: String,
}

/// Gemini 生成配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiGenerationConfig {
    /// 停止序列
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// 最大输出令牌数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    /// 温度
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Top-k
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
}

/// Gemini 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiChatResponse {
    /// 候选响应
    pub candidates: Vec<GeminiCandidate>,
    /// 使用统计
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_metadata: Option<GeminiUsageMetadata>,
    /// 提示反馈
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_feedback: Option<GeminiPromptFeedback>,
}

/// Gemini 候选响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiCandidate {
    /// 内容
    pub content: GeminiContent,
    /// 完成原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    /// 索引
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    /// 安全评级
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_ratings: Option<Vec<GeminiSafetyRating>>,
}

/// Gemini 安全评级
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiSafetyRating {
    /// 类别
    pub category: String,
    /// 概率
    pub probability: String,
}

/// Gemini 使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiUsageMetadata {
    /// 提示令牌数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_token_count: Option<u32>,
    /// 候选令牌数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates_token_count: Option<u32>,
    /// 总令牌数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_token_count: Option<u32>,
}

/// Gemini 提示反馈
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiPromptFeedback {
    /// 阻止原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_reason: Option<String>,
    /// 安全评级
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_ratings: Option<Vec<GeminiSafetyRating>>,
}

impl Default for GeminiConfig {
    fn default() -> Self {
        Self {
            base_url: "https://generativelanguage.googleapis.com".to_string(),
            api_version: "v1beta".to_string(),
            supported_models: GeminiModel::all(),
            default_model: GeminiModel::default(),
            timeout_seconds: 30,
            max_retries: 3,
        }
    }
}

impl GeminiAdapter {
    /// 创建新的 Gemini 适配器
    pub fn new() -> Self {
        let config = GeminiConfig::default();
        
        // 配置支持的端点
        let mut supported_endpoints = HashMap::new();
        
        // Chat Completions API
        supported_endpoints.insert(
            "/v1/chat/completions".to_string(),
            EndpointConfig {
                path_mapping: format!("/{}/models/{{model}}:generateContent", config.api_version),
                method: "POST".to_string(),
                supports_streaming: true,
                request_transformer: "openai_to_gemini_chat".to_string(),
                response_transformer: "gemini_to_openai_chat".to_string(),
            },
        );

        // Models API
        supported_endpoints.insert(
            "/v1/models".to_string(),
            EndpointConfig {
                path_mapping: format!("/{}/models", config.api_version),
                method: "GET".to_string(),
                supports_streaming: false,
                request_transformer: "passthrough".to_string(),
                response_transformer: "gemini_to_openai_models".to_string(),
            },
        );

        Self {
            config,
            supported_endpoints,
        }
    }

    /// 获取配置
    pub fn config(&self) -> &GeminiConfig {
        &self.config
    }

    /// 更新配置
    pub fn with_config(mut self, config: GeminiConfig) -> Self {
        self.config = config;
        self
    }

    /// 转换 OpenAI 聊天请求为 Gemini 格式
    fn transform_openai_to_gemini_chat(&self, request: &ChatCompletionRequest) -> ProviderResult<GeminiChatRequest> {
        // 转换消息
        let contents = self.convert_messages_to_gemini(&request.messages)?;

        // 转换参数
        let parameters = GeminiParameters {
            candidate_count: Some(1), // Gemini 通常只支持单个候选
            max_output_tokens: request.parameters.max_tokens,
            temperature: request.parameters.temperature,
            top_p: request.parameters.top_p,
            top_k: None, // OpenAI 没有对应参数
            stop_sequences: request.parameters.stop.clone(),
        };

        // 生成配置
        let generation_config = Some(GeminiGenerationConfig {
            stop_sequences: request.parameters.stop.clone(),
            max_output_tokens: request.parameters.max_tokens,
            temperature: request.parameters.temperature,
            top_p: request.parameters.top_p,
            top_k: None,
        });

        Ok(GeminiChatRequest {
            contents,
            parameters,
            safety_settings: None, // 使用默认安全设置
            generation_config,
        })
    }

    /// 转换消息格式
    fn convert_messages_to_gemini(&self, messages: &[ChatMessage]) -> ProviderResult<Vec<GeminiContent>> {
        let mut contents = Vec::new();

        for message in messages {
            let role = match message.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "model", // Gemini 使用 "model" 而不是 "assistant"
                MessageRole::System => {
                    // Gemini 不直接支持 system 消息，将其转换为用户消息
                    warn!("Converting system message to user message for Gemini compatibility");
                    "user"
                },
                MessageRole::Tool => {
                    // 工具消息处理
                    "function"
                }
            };

            let parts = vec![GeminiPart {
                text: message.content.clone(),
            }];

            contents.push(GeminiContent {
                role: role.to_string(),
                parts,
            });
        }

        Ok(contents)
    }

    /// 转换 Gemini 响应为 OpenAI 格式
    fn transform_gemini_to_openai_chat(&self, gemini_response: &GeminiChatResponse, request_model: &str) -> ProviderResult<ChatCompletionResponse> {
        if gemini_response.candidates.is_empty() {
            return Err(ProviderError::ApiError {
                status_code: 500,
                message: "No candidates in Gemini response".to_string(),
            });
        }

        let mut choices = Vec::new();

        for (index, candidate) in gemini_response.candidates.iter().enumerate() {
            if candidate.content.parts.is_empty() {
                continue;
            }

            // 提取文本内容
            let content = candidate.content.parts
                .iter()
                .map(|part| part.text.clone())
                .collect::<Vec<_>>()
                .join("");

            let choice = ChatChoice {
                index: index as u32,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content,
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                finish_reason: candidate.finish_reason.clone(),
            };

            choices.push(choice);
        }

        // 转换使用统计
        let usage = gemini_response.usage_metadata.as_ref().map(|usage_meta| {
            Usage {
                prompt_tokens: usage_meta.prompt_token_count.unwrap_or(0),
                completion_tokens: usage_meta.candidates_token_count.unwrap_or(0),
                total_tokens: usage_meta.total_token_count.unwrap_or(0),
            }
        });

        Ok(ChatCompletionResponse {
            id: format!("chatcmpl-{}", fastrand::u64(..)),
            object: "chat.completion".to_string(),
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            model: request_model.to_string(),
            choices,
            usage,
        })
    }

    /// 解析流式响应块
    fn parse_streaming_chunk(&self, chunk: &[u8]) -> ProviderResult<Option<StreamChunk>> {
        let chunk_str = std::str::from_utf8(chunk)
            .map_err(|e| ProviderError::SerializationError(format!("Invalid UTF-8: {}", e)))?;

        // Gemini 流式响应格式处理
        if chunk_str.trim().is_empty() {
            return Ok(None);
        }

        // 解析 JSON 响应
        let response: GeminiChatResponse = serde_json::from_str(chunk_str)
            .map_err(|e| ProviderError::SerializationError(format!("Failed to parse Gemini response: {}", e)))?;

        // 转换为 OpenAI 格式
        if !response.candidates.is_empty() {
            let candidate = &response.candidates[0];
            if !candidate.content.parts.is_empty() {
                let text = &candidate.content.parts[0].text;
                let chunk_data = json!({
                    "id": format!("chatcmpl-{}", fastrand::u64(..)),
                    "object": "chat.completion.chunk",
                    "created": std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    "model": "gemini-1.5-pro",
                    "choices": [{
                        "index": 0,
                        "delta": {
                            "content": text
                        },
                        "finish_reason": candidate.finish_reason
                    }]
                });

                return Ok(Some(StreamChunk::data(
                    format!("data: {}\n\n", chunk_data).into_bytes()
                )));
            }
        }

        Ok(None)
    }

    /// 获取模型路径
    fn get_model_path(&self, model: &str) -> String {
        // 确保模型名称有效
        let model_name = if let Some(model_enum) = GeminiModel::from_str(model) {
            if self.config.supported_models.contains(&model_enum) {
                model
            } else {
                &self.config.default_model.to_string()
            }
        } else {
            &self.config.default_model.to_string()
        };

        format!("{}/{}/models/{}:generateContent", 
            self.config.base_url, 
            self.config.api_version, 
            model_name
        )
    }

    /// 处理错误响应
    fn handle_error_response(&self, status_code: u16, body: &str) -> ProviderError {
        match status_code {
            400 => ProviderError::InvalidRequest(format!("Bad request: {}", body)),
            401 => ProviderError::AuthenticationFailed("Invalid API key".to_string()),
            403 => ProviderError::AuthenticationFailed("Forbidden".to_string()),
            429 => ProviderError::RateLimitExceeded("Rate limit exceeded".to_string()),
            500..=599 => ProviderError::ApiError {
                status_code,
                message: format!("Server error: {}", body),
            },
            _ => ProviderError::ApiError {
                status_code,
                message: format!("Unexpected error: {}", body),
            },
        }
    }
}

impl ProviderAdapter for GeminiAdapter {
    fn name(&self) -> &str {
        "google-gemini"
    }

    fn supports_endpoint(&self, endpoint: &str) -> bool {
        self.supported_endpoints.contains_key(endpoint)
    }

    fn supports_streaming(&self, endpoint: &str) -> bool {
        self.supported_endpoints
            .get(endpoint)
            .map(|config| config.supports_streaming)
            .unwrap_or(false)
    }

    fn transform_request(&self, request: &AdapterRequest) -> ProviderResult<AdapterRequest> {
        debug!("Transforming request for endpoint: {}", request.path);

        match request.path.as_str() {
            "/v1/chat/completions" => {
                // 解析 OpenAI 格式请求
                let openai_request: ChatCompletionRequest = if let Some(ref body) = request.body {
                    serde_json::from_value(body.clone())
                        .map_err(|e| ProviderError::InvalidRequest(format!("Invalid request body: {}", e)))?
                } else {
                    return Err(ProviderError::InvalidRequest("Missing request body".to_string()));
                };

                // 转换为 Gemini 格式
                let gemini_request = self.transform_openai_to_gemini_chat(&openai_request)?;
                
                // 获取目标路径
                let model = &openai_request.model;
                let target_path = self.get_model_path(model);

                // 构建新的请求
                let mut transformed_request = request.clone();
                transformed_request.path = target_path;
                transformed_request.body = Some(serde_json::to_value(gemini_request)
                    .map_err(|e| ProviderError::SerializationError(format!("Failed to serialize request: {}", e)))?);

                // 更新头部
                transformed_request.headers.remove("authorization");
                if let Some(api_key) = transformed_request.headers.get("x-api-key") {
                    transformed_request.headers.insert("authorization".to_string(), format!("Bearer {}", api_key));
                }

                Ok(transformed_request)
            },
            _ => {
                warn!("Unsupported endpoint: {}", request.path);
                Err(ProviderError::UnsupportedOperation(format!("Endpoint not supported: {}", request.path)))
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
                // 解析 Gemini 响应
                let gemini_response: GeminiChatResponse = serde_json::from_value(response.body.clone())
                    .map_err(|e| ProviderError::SerializationError(format!("Failed to parse Gemini response: {}", e)))?;

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
                let openai_response = self.transform_gemini_to_openai_chat(&gemini_response, &model)?;
                
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
        self.parse_streaming_chunk(chunk)
    }

    fn validate_request(&self, request: &AdapterRequest) -> ProviderResult<()> {
        // 验证端点支持
        if !self.supports_endpoint(&request.path) {
            return Err(ProviderError::UnsupportedOperation(
                format!("Endpoint {} not supported by Gemini adapter", request.path)
            ));
        }

        // 验证API密钥
        if request.get_header("x-api-key").is_none() && request.get_authorization().is_none() {
            return Err(ProviderError::AuthenticationFailed(
                "Missing API key for Gemini".to_string()
            ));
        }

        // 验证请求体（对于需要的端点）
        match request.path.as_str() {
            "/v1/chat/completions" => {
                if request.body.is_none() {
                    return Err(ProviderError::InvalidRequest(
                        "Missing request body for chat completions".to_string()
                    ));
                }
            },
            _ => {}
        }

        Ok(())
    }

    fn get_supported_endpoints(&self) -> Vec<String> {
        self.supported_endpoints.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::types::ModelParameters;

    #[test]
    fn test_gemini_adapter_creation() {
        let adapter = GeminiAdapter::new();
        assert_eq!(adapter.name(), "google-gemini");
        assert!(adapter.supports_endpoint("/v1/chat/completions"));
        assert!(!adapter.supports_endpoint("/unknown/endpoint"));
    }

    #[test]
    fn test_message_conversion() {
        let adapter = GeminiAdapter::new();
        let messages = vec![
            ChatMessage::system("You are a helpful assistant"),
            ChatMessage::user("Hello"),
            ChatMessage::assistant("Hi there!"),
        ];

        let contents = adapter.convert_messages_to_gemini(&messages).unwrap();
        assert_eq!(contents.len(), 3);
        assert_eq!(contents[0].role, "user"); // system -> user
        assert_eq!(contents[1].role, "user");
        assert_eq!(contents[2].role, "model"); // assistant -> model
    }

    #[test]
    fn test_openai_to_gemini_transformation() {
        let adapter = GeminiAdapter::new();
        let request = ChatCompletionRequest {
            model: "gemini-1.5-pro".to_string(),
            messages: vec![
                ChatMessage::user("Hello world"),
            ],
            parameters: ModelParameters {
                temperature: Some(0.7),
                max_tokens: Some(100),
                top_p: Some(0.9),
                ..Default::default()
            },
        };

        let gemini_request = adapter.transform_openai_to_gemini_chat(&request).unwrap();
        assert_eq!(gemini_request.contents.len(), 1);
        assert_eq!(gemini_request.parameters.temperature, Some(0.7));
        assert_eq!(gemini_request.parameters.max_output_tokens, Some(100));
    }

    #[test]
    fn test_gemini_config_default() {
        let config = GeminiConfig::default();
        assert_eq!(config.base_url, "https://generativelanguage.googleapis.com");
        assert_eq!(config.api_version, "v1beta");
        assert!(!config.supported_models.is_empty());
    }

    #[test]
    fn test_model_path_generation() {
        let adapter = GeminiAdapter::new();
        let path = adapter.get_model_path("gemini-1.5-pro-latest");
        assert!(path.contains("gemini-1.5-pro-latest"));
        assert!(path.contains("generateContent"));
    }
}