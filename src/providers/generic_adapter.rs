//! # 通用适配器
//!
//! 基于数据库配置的通用AI服务适配器，支持任意提供商

use super::traits::ProviderAdapter;
use super::types::{
    AdapterRequest, AdapterResponse, ProviderError, ProviderResult, StreamChunk,
    ChatCompletionRequest, ChatCompletionResponse, Usage, ChatChoice, ChatMessage, MessageRole
};
use crate::proxy::upstream::ProviderId;
use serde_json::{json, Value};

/// 通用适配器配置
#[derive(Debug, Clone)]
pub struct GenericAdapterConfig {
    /// 提供商ID
    pub provider_id: ProviderId,
    /// 提供商名称
    pub provider_name: String,
    /// 显示名称
    pub display_name: String,
    /// API格式（openai-compatible, gemini, claude等）
    pub api_format: String,
    /// 请求转换规则
    pub request_transform_rules: Option<Value>,
    /// 响应转换规则
    pub response_transform_rules: Option<Value>,
    /// 流式响应处理配置
    pub streaming_config: Option<Value>,
}

impl Default for GenericAdapterConfig {
    fn default() -> Self {
        Self {
            provider_id: ProviderId::from_database_id(0),
            provider_name: "generic".to_string(),
            display_name: "Generic Provider".to_string(),
            api_format: "openai-compatible".to_string(),
            request_transform_rules: None,
            response_transform_rules: None,
            streaming_config: None,
        }
    }
}

/// 通用适配器实现
pub struct GenericAdapter {
    config: GenericAdapterConfig,
}

impl GenericAdapter {
    /// 创建新的通用适配器
    pub fn new(config: GenericAdapterConfig) -> Self {
        Self { config }
    }

    /// 从数据库配置创建适配器
    pub fn from_provider_config(
        provider_id: ProviderId,
        provider_name: String,
        display_name: String,
        api_format: String,
        config_json: Option<Value>,
    ) -> Self {
        let config = GenericAdapterConfig {
            provider_id,
            provider_name: provider_name.clone(),
            display_name,
            api_format: api_format.clone(),
            request_transform_rules: config_json.as_ref().and_then(|c| c.get("request_transform").cloned()),
            response_transform_rules: config_json.as_ref().and_then(|c| c.get("response_transform").cloned()),
            streaming_config: config_json.as_ref().and_then(|c| c.get("streaming").cloned()),
        };

        Self::new(config)
    }


    /// 转换OpenAI格式请求到目标格式
    fn transform_request_format(&self, request: &AdapterRequest) -> ProviderResult<AdapterRequest> {
        match self.config.api_format.to_lowercase().as_str() {
            "openai" | "openai-compatible" => {
                // 已经是OpenAI格式，直接返回
                Ok(request.clone())
            }
            "gemini" | "google" => {
                self.transform_to_gemini_format(request)
            }
            "anthropic" | "claude" => {
                self.transform_to_claude_format(request)
            }
            _ => {
                // 未知格式，尝试保持原样
                tracing::warn!("Unknown API format: {}, keeping original request", self.config.api_format);
                Ok(request.clone())
            }
        }
    }

    /// 转换到Gemini格式
    fn transform_to_gemini_format(&self, request: &AdapterRequest) -> ProviderResult<AdapterRequest> {
        if let Some(body) = &request.body {
            // 尝试解析为ChatCompletionRequest
            if let Ok(chat_request) = serde_json::from_value::<ChatCompletionRequest>(body.clone()) {
                let mut parts = Vec::new();
                
                // 转换消息格式
                for message in &chat_request.messages {
                    let part = match message.role {
                        MessageRole::User => json!({"text": message.content}),
                        MessageRole::Assistant => json!({"text": message.content}),
                        MessageRole::System => json!({"text": message.content}), // Gemini将system消息作为第一个user消息
                        _ => json!({"text": message.content}),
                    };
                    parts.push(part);
                }

                let gemini_request = json!({
                    "contents": [{
                        "parts": parts
                    }],
                    "generationConfig": {
                        "maxOutputTokens": chat_request.parameters.max_tokens.unwrap_or(1024),
                        "temperature": chat_request.parameters.temperature.unwrap_or(0.7)
                    }
                });

                let mut new_request = request.clone();
                new_request.body = Some(gemini_request);
                Ok(new_request)
            } else {
                Err(ProviderError::InvalidRequest("Cannot parse request body as ChatCompletionRequest".to_string()))
            }
        } else {
            Err(ProviderError::InvalidRequest("Request body is empty".to_string()))
        }
    }

    /// 转换到Claude格式
    fn transform_to_claude_format(&self, request: &AdapterRequest) -> ProviderResult<AdapterRequest> {
        if let Some(body) = &request.body {
            // 尝试解析为ChatCompletionRequest
            if let Ok(chat_request) = serde_json::from_value::<ChatCompletionRequest>(body.clone()) {
                let claude_request = json!({
                    "model": chat_request.model,
                    "max_tokens": chat_request.parameters.max_tokens.unwrap_or(1024),
                    "temperature": chat_request.parameters.temperature.unwrap_or(0.7),
                    "messages": chat_request.messages
                });

                let mut new_request = request.clone();
                new_request.body = Some(claude_request);
                Ok(new_request)
            } else {
                Err(ProviderError::InvalidRequest("Cannot parse request body as ChatCompletionRequest".to_string()))
            }
        } else {
            Err(ProviderError::InvalidRequest("Request body is empty".to_string()))
        }
    }

    /// 转换响应格式为OpenAI兼容
    fn transform_response_format(&self, response: &AdapterResponse, request: &AdapterRequest) -> ProviderResult<AdapterResponse> {
        match self.config.api_format.to_lowercase().as_str() {
            "openai" | "openai-compatible" => {
                // 已经是OpenAI格式
                Ok(response.clone())
            }
            "gemini" | "google" => {
                self.transform_from_gemini_format(response, request)
            }
            "anthropic" | "claude" => {
                self.transform_from_claude_format(response, request)
            }
            _ => {
                // 未知格式，保持原样
                Ok(response.clone())
            }
        }
    }

    /// 从Gemini响应转换为OpenAI格式
    fn transform_from_gemini_format(&self, response: &AdapterResponse, request: &AdapterRequest) -> ProviderResult<AdapterResponse> {
        // 尝试解析Gemini响应
        let gemini_response: Value = response.body.clone();

            if let Some(candidates) = gemini_response.get("candidates").and_then(|c| c.as_array()) {
                if let Some(first_candidate) = candidates.first() {
                    if let Some(content) = first_candidate.get("content").and_then(|c| c.get("parts")).and_then(|p| p.as_array()) {
                        if let Some(text_part) = content.first().and_then(|p| p.get("text")).and_then(|t| t.as_str()) {
                            let model = request.body.as_ref()
                                .and_then(|body| body.get("model"))
                                .and_then(|m| m.as_str())
                                .unwrap_or("gemini-pro");

                            let openai_response = ChatCompletionResponse {
                                id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                                object: "chat.completion".to_string(),
                                created: chrono::Utc::now().timestamp() as u64,
                                model: model.to_string(),
                                choices: vec![ChatChoice {
                                    index: 0,
                                    message: ChatMessage {
                                        role: MessageRole::Assistant,
                                        content: text_part.to_string(),
                                        name: None,
                                        tool_calls: None,
                                        tool_call_id: None,
                                    },
                                    finish_reason: Some("stop".to_string()),
                                }],
                                usage: Some(Usage {
                                    prompt_tokens: 0, // Gemini不提供token计数，设为0
                                    completion_tokens: 0,
                                    total_tokens: 0,
                                }),
                            };

                            let mut new_response = response.clone();
                            new_response.body = serde_json::to_value(openai_response)
                                .map_err(|e| ProviderError::SerializationError(format!("Failed to serialize OpenAI response: {}", e)))?;
                            return Ok(new_response);
                        }
                    }
                }
            }

        Err(ProviderError::ResponseParseError("Invalid Gemini response format".to_string()))
    }

    /// 从Claude响应转换为OpenAI格式
    fn transform_from_claude_format(&self, response: &AdapterResponse, request: &AdapterRequest) -> ProviderResult<AdapterResponse> {
        let claude_response: Value = response.body.clone();

        if let Some(content) = claude_response.get("content").and_then(|c| c.as_array()) {
            if let Some(text_block) = content.first().and_then(|b| b.get("text")).and_then(|t| t.as_str()) {
                    let openai_response = ChatCompletionResponse {
                        id: claude_response.get("id")
                            .and_then(|id| id.as_str())
                            .unwrap_or(&format!("chatcmpl-{}", uuid::Uuid::new_v4()))
                            .to_string(),
                        object: "chat.completion".to_string(),
                        created: chrono::Utc::now().timestamp() as u64,
                        model: claude_response.get("model")
                            .and_then(|m| m.as_str())
                            .unwrap_or("claude-3")
                            .to_string(),
                        choices: vec![ChatChoice {
                            index: 0,
                            message: ChatMessage {
                                role: MessageRole::Assistant,
                                content: text_block.to_string(),
                                name: None,
                                tool_calls: None,
                                tool_call_id: None,
                            },
                            finish_reason: claude_response.get("stop_reason")
                                .and_then(|sr| sr.as_str())
                                .map(|sr| sr.to_string())
                                .or_else(|| Some("stop".to_string())),
                        }],
                        usage: claude_response.get("usage").and_then(|u| {
                            Some(Usage {
                                prompt_tokens: u.get("input_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
                                completion_tokens: u.get("output_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
                                total_tokens: (
                                    u.get("input_tokens").and_then(|t| t.as_u64()).unwrap_or(0) +
                                    u.get("output_tokens").and_then(|t| t.as_u64()).unwrap_or(0)
                                ) as u32,
                            })
                        }),
                    };

                    let mut new_response = response.clone();
                    new_response.body = serde_json::to_value(openai_response)
                        .map_err(|e| ProviderError::SerializationError(format!("Failed to serialize OpenAI response: {}", e)))?;
                    return Ok(new_response);
                }
        }

        Err(ProviderError::ResponseParseError("Invalid Claude response format".to_string()))
    }
}

impl ProviderAdapter for GenericAdapter {
    fn name(&self) -> &str {
        &self.config.display_name
    }

    fn transform_request(&self, request: &AdapterRequest) -> ProviderResult<AdapterRequest> {
        // 应用自定义转换规则（如果有）
        if let Some(_transform_rules) = &self.config.request_transform_rules {
            // TODO: 实现基于JSON配置的请求转换
            tracing::debug!("Custom request transform rules not implemented yet");
        }

        // 应用格式转换
        self.transform_request_format(request)
    }

    fn transform_response(&self, response: &AdapterResponse, request: &AdapterRequest) -> ProviderResult<AdapterResponse> {
        // 应用格式转换
        let transformed_response = self.transform_response_format(response, request)?;

        // 应用自定义转换规则（如果有）
        if let Some(_transform_rules) = &self.config.response_transform_rules {
            // TODO: 实现基于JSON配置的响应转换
            tracing::debug!("Custom response transform rules not implemented yet");
        }

        Ok(transformed_response)
    }

    fn validate_request(&self, request: &AdapterRequest) -> ProviderResult<()> {
        // 基本验证
        if request.body.is_none() {
            return Err(ProviderError::InvalidRequest("Request body cannot be empty".to_string()));
        }

        // 如果请求体存在，尝试解析为ChatCompletionRequest进行进一步验证
        if let Some(body) = &request.body {
            // 尝试解析为ChatCompletionRequest
            if let Ok(chat_request) = serde_json::from_value::<ChatCompletionRequest>(body.clone()) {
                if chat_request.messages.is_empty() {
                    return Err(ProviderError::InvalidRequest("Chat completion must have at least one message".to_string()));
                }

                if chat_request.model.is_empty() {
                    return Err(ProviderError::InvalidRequest("Model name cannot be empty".to_string()));
                }
            }
            // 如果不是ChatCompletionRequest格式，也允许通过（可能是其他格式的请求）
        }

        Ok(())
    }


    fn supports_streaming(&self, _endpoint: &str) -> bool {
        // 检查配置中是否启用流式支持
        if let Some(streaming_config) = &self.config.streaming_config {
            // 如果有流式配置，检查是否启用
            streaming_config.get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true) // 默认启用，如果有streaming_config
        } else {
            // 如果没有特定配置，根据API格式判断通用支持情况
            match self.config.api_format.to_lowercase().as_str() {
                "openai" | "openai-compatible" => true,
                "gemini" | "google" => true,
                "anthropic" | "claude" => true,
                _ => false, // 未知格式默认不支持
            }
        }
    }

    /// 检查请求是否要求流式响应
    fn is_streaming_request(&self, request: &AdapterRequest) -> bool {
        // 检查请求参数中是否有stream=true
        if request.query.get("stream").map(|s| s == "true").unwrap_or(false) {
            return true;
        }

        // 检查请求体中是否有stream字段
        if let Some(body) = &request.body {
            if let Some(stream) = body.get("stream") {
                return stream.as_bool().unwrap_or(false);
            }
        }

        // 检查请求头中是否有Accept: text/event-stream
        request.headers.get("accept")
            .or_else(|| request.headers.get("Accept"))
            .map(|accept| accept.contains("text/event-stream"))
            .unwrap_or(false)
    }

    /// 检查响应是否为流式响应
    fn is_streaming_response(&self, response: &AdapterResponse) -> bool {
        // 检查Content-Type响应头
        response.headers.get("content-type")
            .or_else(|| response.headers.get("Content-Type"))
            .map(|ct| ct.contains("text/event-stream") || ct.contains("application/stream"))
            .unwrap_or(false) ||
        // 检查Transfer-Encoding
        response.headers.get("transfer-encoding")
            .or_else(|| response.headers.get("Transfer-Encoding"))
            .map(|te| te.contains("chunked"))
            .unwrap_or(false) ||
        // 检查响应的is_streaming标志
        response.is_streaming
    }

    fn handle_streaming_chunk(&self, chunk: &[u8], request: &AdapterRequest) -> ProviderResult<Option<StreamChunk>> {
        // 基本的流式处理实现
        let chunk_str = std::str::from_utf8(chunk)
            .map_err(|e| ProviderError::StreamParseError(format!("Invalid UTF-8 in chunk: {}", e)))?;

        // SSE格式解析
        if chunk_str.starts_with("data: ") {
            let data = &chunk_str[6..]; // 去掉 "data: " 前缀
            
            if data.trim() == "[DONE]" {
                return Ok(Some(StreamChunk::Done));
            }

            match serde_json::from_str::<Value>(data) {
                Ok(json_data) => {
                    // 根据API格式转换流式数据
                    match self.config.api_format.to_lowercase().as_str() {
                        "openai" | "openai-compatible" => {
                            Ok(Some(StreamChunk::Data(json_data)))
                        }
                        "gemini" | "google" => {
                            // 转换Gemini流式响应为OpenAI格式
                            if let Some(candidates) = json_data.get("candidates").and_then(|c| c.as_array()) {
                                if let Some(first_candidate) = candidates.first() {
                                    if let Some(content) = first_candidate.get("content") {
                                        let openai_chunk = json!({
                                            "choices": [{
                                                "index": 0,
                                                "delta": {"content": content},
                                                "finish_reason": null
                                            }]
                                        });
                                        return Ok(Some(StreamChunk::Data(openai_chunk)));
                                    }
                                }
                            }
                            Ok(Some(StreamChunk::Data(json_data)))
                        }
                        "anthropic" | "claude" => {
                            // 转换Claude流式响应为OpenAI格式  
                            if let Some(content_block) = json_data.get("delta").and_then(|d| d.get("text")) {
                                let openai_chunk = json!({
                                    "choices": [{
                                        "index": 0,
                                        "delta": {"content": content_block},
                                        "finish_reason": null
                                    }]
                                });
                                return Ok(Some(StreamChunk::Data(openai_chunk)));
                            }
                            Ok(Some(StreamChunk::Data(json_data)))
                        }
                        _ => Ok(Some(StreamChunk::Data(json_data)))
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to parse streaming chunk as JSON: {}, chunk: {}", e, data);
                    Ok(Some(StreamChunk::Raw(chunk.to_vec())))
                }
            }
        } else {
            // 非SSE格式的原始数据
            Ok(Some(StreamChunk::Raw(chunk.to_vec())))
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generic_adapter_creation() {
        let config = GenericAdapterConfig::default();
        let adapter = GenericAdapter::new(config.clone());
        assert_eq!(adapter.name(), &config.display_name);
    }

}