//! # 通用适配器
//!
//! 基于数据库配置的通用AI服务适配器，支持任意提供商

use super::field_extractor::{ModelExtractor, TokenFieldExtractor};
use super::traits::ProviderAdapter;
use super::types::{
    AdapterRequest, AdapterResponse, ChatChoice, ChatCompletionRequest, ChatCompletionResponse,
    ChatMessage, MessageRole, ProviderError, ProviderResult, StreamChunk, TraceStats, Usage,
};
use crate::proxy::types::ProviderId;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, warn};

/// 通用适配器配置
/// 简化后只包含必要的配置项，保持代理透明性
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
    /// 请求阶段特殊配置（如required_headers）
    pub request_stage_config: Option<Value>,
    /// 响应阶段特殊配置（目前为空，预留扩展）
    pub response_stage_config: Option<Value>,
    /// Token字段提取器（用于Token统计）
    pub token_field_extractor: Option<TokenFieldExtractor>,
    /// 模型提取器（用于模型名称提取）
    pub model_extractor: Option<Arc<ModelExtractor>>,
}

impl Default for GenericAdapterConfig {
    fn default() -> Self {
        Self {
            provider_id: ProviderId::from_database_id(0),
            provider_name: "generic".to_string(),
            display_name: "Generic Provider".to_string(),
            api_format: "openai-compatible".to_string(),
            request_stage_config: None,
            response_stage_config: None,
            token_field_extractor: None,
            model_extractor: None,
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
        Self::from_provider_config_with_token_mappings(
            provider_id,
            provider_name,
            display_name,
            api_format,
            config_json,
            None, // token_mappings_json
            None, // model_extraction_json
        )
    }

    /// 从数据库配置创建适配器（包含Token映射配置和模型提取配置）
    pub fn from_provider_config_with_token_mappings(
        provider_id: ProviderId,
        provider_name: String,
        display_name: String,
        api_format: String,
        config_json: Option<Value>,
        token_mappings_json: Option<String>,
        model_extraction_json: Option<String>,
    ) -> Self {
        // 简化配置：移除复杂的字段提取器

        // 创建Token字段提取器（用于Token统计）
        let token_field_extractor = token_mappings_json.as_ref().and_then(|token_config| {
            match TokenFieldExtractor::from_json_config(token_config) {
                Ok(extractor) => {
                    debug!(
                        provider_name = %provider_name,
                        "Successfully created token field extractor for provider"
                    );
                    Some(extractor)
                }
                Err(e) => {
                    warn!(
                        provider_name = %provider_name,
                        error = %e,
                        "Failed to create token field extractor, will use default behavior"
                    );
                    None
                }
            }
        });

        // 创建模型提取器（用于模型名称提取）
        let model_extractor = model_extraction_json.as_ref().and_then(|model_config| {
            match ModelExtractor::from_json_config(model_config) {
                Ok(extractor) => {
                    debug!(
                        provider_name = %provider_name,
                        "Successfully created model extractor for provider"
                    );
                    Some(Arc::new(extractor))
                }
                Err(e) => {
                    warn!(
                        provider_name = %provider_name,
                        error = %e,
                        "Failed to create model extractor, will use default behavior"
                    );
                    None
                }
            }
        });

        // 解析新的ConfigJson结构
        let (request_stage_config, response_stage_config) = config_json
            .as_ref()
            .map(|config| {
                let request_stage = config.get("request_stage").cloned();
                let response_stage = config.get("response_stage").cloned();
                (request_stage, response_stage)
            })
            .unwrap_or((None, None));

        let config = GenericAdapterConfig {
            provider_id,
            provider_name: provider_name.clone(),
            display_name,
            api_format: api_format.clone(),
            // 从新的ConfigJson结构中提取阶段配置
            request_stage_config,
            response_stage_config,
            token_field_extractor,
            model_extractor,
        };

        Self::new(config)
    }

    /// 使用字段提取器提取Usage信息
    fn extract_usage_info(&self, response: &Value) -> Option<Usage> {
        // 简化后移除了通用field_extractor，优先使用专门的token_field_extractor
        if let Some(token_extractor) = &self.config.token_field_extractor {
            let input_tokens = token_extractor.extract_token_u32(response, "tokens_prompt").unwrap_or(0);
            let output_tokens = token_extractor.extract_token_u32(response, "tokens_completion").unwrap_or(0);
            let total_tokens = token_extractor.extract_token_u32(response, "tokens_total").unwrap_or(input_tokens + output_tokens);

            debug!(
                provider = %self.config.provider_name,
                input_tokens = input_tokens,
                output_tokens = output_tokens,
                total_tokens = total_tokens,
                "Extracted token usage using field extractor"
            );

            Some(Usage {
                prompt_tokens: input_tokens,
                completion_tokens: output_tokens,
                total_tokens,
            })
        } else {
            // 回退到默认行为：假设没有token统计
            debug!(
                provider = %self.config.provider_name,
                "No field extractor configured, returning default usage"
            );
            Some(Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            })
        }
    }

    /// 提取内容（简化版）
    fn extract_content(&self, response: &Value) -> String {
        // 简化后使用通用的响应提取逻辑
        response.get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string()
    }

    /// 提取模型名称（简化版）
    fn extract_model_name(&self, response: &Value, fallback: &str) -> String {
        // 简化后的模型提取逻辑：从响应中直接提取
        // ModelExtractor主要用于从请求阶段提取，这里简化为直接提取
        response.get("model")
            .and_then(|m| m.as_str())
            .or_else(|| response.pointer("/choices/0/model").and_then(|m| m.as_str()))  // 兼容某些格式
            .unwrap_or(fallback)
            .to_string()
    }

    /// 提取完成原因（简化版）
    fn extract_finish_reason(&self, response: &Value) -> Option<String> {
        // 简化后使用通用的提取逻辑
        response.get("finish_reason")
            .and_then(|r| r.as_str())
            .map(|s| s.to_string())
            .or_else(|| Some("stop".to_string()))
    }

    /// 公共接口：提取统计信息供trace系统使用
    pub fn extract_trace_stats(&self, response: &Value) -> TraceStats {
        // 优先使用新的TokenFieldExtractor提取Token信息
        let (input_tokens, output_tokens, total_tokens, cache_create_tokens, cache_read_tokens) =
            if let Some(token_extractor) = &self.config.token_field_extractor {
                (
                    token_extractor.extract_token_u32(response, "tokens_prompt"),
                    token_extractor.extract_token_u32(response, "tokens_completion"),
                    token_extractor.extract_token_u32(response, "tokens_total"),
                    token_extractor.extract_token_u32(response, "cache_create_tokens"),
                    token_extractor.extract_token_u32(response, "cache_read_tokens"),
                )
            // 简化后移除了旧的FieldExtractor
            } else {
                (None, None, None, None, None)
            };

        // 简化的非Token字段提取
        let (cost, cost_currency, model_name, error_type, error_message) = (
            response.get("cost").and_then(|v| v.as_f64()),
            response.get("cost_currency").and_then(|v| v.as_str()).map(String::from),
            response.get("model").and_then(|v| v.as_str()).map(String::from),
            response.get("error_type").and_then(|v| v.as_str()).map(String::from),
            response.get("error_message").and_then(|v| v.as_str()).map(String::from),
        );

        TraceStats {
            input_tokens,
            output_tokens,
            total_tokens,
            cache_create_tokens,
            cache_read_tokens,
            cost,
            cost_currency,
            model_name,
            error_type,
            error_message,
        }
    }

    /// 转换OpenAI格式请求到目标格式
    fn transform_request_format(&self, request: &AdapterRequest) -> ProviderResult<AdapterRequest> {
        match self.config.api_format.to_lowercase().as_str() {
            "openai" | "openai-compatible" => {
                // 已经是OpenAI格式，直接返回
                Ok(request.clone())
            }
            "gemini" | "google" => self.transform_to_gemini_format(request),
            "anthropic" | "claude" => self.transform_to_claude_format(request),
            _ => {
                // 未知格式，尝试保持原样
                tracing::warn!(
                    "Unknown API format: {}, keeping original request",
                    self.config.api_format
                );
                Ok(request.clone())
            }
        }
    }

    /// 转换到Gemini格式
    fn transform_to_gemini_format(
        &self,
        request: &AdapterRequest,
    ) -> ProviderResult<AdapterRequest> {
        if let Some(body) = &request.body {
            // 尝试解析为ChatCompletionRequest
            if let Ok(chat_request) = serde_json::from_value::<ChatCompletionRequest>(body.clone())
            {
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
                Err(ProviderError::InvalidRequest(
                    "Cannot parse request body as ChatCompletionRequest".to_string(),
                ))
            }
        } else {
            Err(ProviderError::InvalidRequest(
                "Request body is empty".to_string(),
            ))
        }
    }

    /// 转换到Claude格式
    fn transform_to_claude_format(
        &self,
        request: &AdapterRequest,
    ) -> ProviderResult<AdapterRequest> {
        if let Some(body) = &request.body {
            // 尝试解析为ChatCompletionRequest
            if let Ok(chat_request) = serde_json::from_value::<ChatCompletionRequest>(body.clone())
            {
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
                Err(ProviderError::InvalidRequest(
                    "Cannot parse request body as ChatCompletionRequest".to_string(),
                ))
            }
        } else {
            Err(ProviderError::InvalidRequest(
                "Request body is empty".to_string(),
            ))
        }
    }

    /// 转换响应格式为OpenAI兼容
    fn transform_response_format(
        &self,
        response: &AdapterResponse,
        request: &AdapterRequest,
    ) -> ProviderResult<AdapterResponse> {
        match self.config.api_format.to_lowercase().as_str() {
            "openai" | "openai-compatible" => {
                // 已经是OpenAI格式
                Ok(response.clone())
            }
            "gemini" | "google" => self.transform_from_gemini_format(response, request),
            "anthropic" | "claude" => self.transform_from_claude_format(response, request),
            _ => {
                // 未知格式，保持原样
                Ok(response.clone())
            }
        }
    }

    /// 从Gemini响应转换为OpenAI格式
    fn transform_from_gemini_format(
        &self,
        response: &AdapterResponse,
        request: &AdapterRequest,
    ) -> ProviderResult<AdapterResponse> {
        // 尝试解析Gemini响应
        let gemini_response: Value = response.body.clone();

        if let Some(candidates) = gemini_response.get("candidates").and_then(|c| c.as_array()) {
            if let Some(first_candidate) = candidates.first() {
                if let Some(content) = first_candidate
                    .get("content")
                    .and_then(|c| c.get("parts"))
                    .and_then(|p| p.as_array())
                {
                    if let Some(text_part) = content
                        .first()
                        .and_then(|p| p.get("text"))
                        .and_then(|t| t.as_str())
                    {
                        let model = request
                            .body
                            .as_ref()
                            .and_then(|body| body.get("model"))
                            .and_then(|m| m.as_str())
                            .unwrap_or("gemini-pro");

                        let content = self.extract_content(&gemini_response);
                        let model_name = self.extract_model_name(&gemini_response, model);
                        let finish_reason = self.extract_finish_reason(&gemini_response);
                        let usage = self.extract_usage_info(&gemini_response);

                        let openai_response = ChatCompletionResponse {
                            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                            object: "chat.completion".to_string(),
                            created: chrono::Utc::now().timestamp() as u64,
                            model: model_name,
                            choices: vec![ChatChoice {
                                index: 0,
                                message: ChatMessage {
                                    role: MessageRole::Assistant,
                                    content: if content.is_empty() {
                                        text_part.to_string()
                                    } else {
                                        content
                                    },
                                    name: None,
                                    tool_calls: None,
                                    tool_call_id: None,
                                },
                                finish_reason,
                            }],
                            usage,
                        };

                        let mut new_response = response.clone();
                        new_response.body = serde_json::to_value(openai_response).map_err(|e| {
                            ProviderError::SerializationError(format!(
                                "Failed to serialize OpenAI response: {}",
                                e
                            ))
                        })?;
                        return Ok(new_response);
                    }
                }
            }
        }

        Err(ProviderError::ResponseParseError(
            "Invalid Gemini response format".to_string(),
        ))
    }

    /// 从Claude响应转换为OpenAI格式
    fn transform_from_claude_format(
        &self,
        response: &AdapterResponse,
        _request: &AdapterRequest,
    ) -> ProviderResult<AdapterResponse> {
        let claude_response: Value = response.body.clone();

        if let Some(content) = claude_response.get("content").and_then(|c| c.as_array()) {
            if let Some(text_block) = content
                .first()
                .and_then(|b| b.get("text"))
                .and_then(|t| t.as_str())
            {
                let content = self.extract_content(&claude_response);
                let model_name = self.extract_model_name(&claude_response, "claude-3");
                let finish_reason = self.extract_finish_reason(&claude_response);
                let usage = self.extract_usage_info(&claude_response);

                let openai_response = ChatCompletionResponse {
                    id: claude_response
                        .get("id")
                        .and_then(|id| id.as_str())
                        .unwrap_or(&format!("chatcmpl-{}", uuid::Uuid::new_v4()))
                        .to_string(),
                    object: "chat.completion".to_string(),
                    created: chrono::Utc::now().timestamp() as u64,
                    model: model_name,
                    choices: vec![ChatChoice {
                        index: 0,
                        message: ChatMessage {
                            role: MessageRole::Assistant,
                            content: if content.is_empty() {
                                text_block.to_string()
                            } else {
                                content
                            },
                            name: None,
                            tool_calls: None,
                            tool_call_id: None,
                        },
                        finish_reason,
                    }],
                    usage,
                };

                let mut new_response = response.clone();
                new_response.body = serde_json::to_value(openai_response).map_err(|e| {
                    ProviderError::SerializationError(format!(
                        "Failed to serialize OpenAI response: {}",
                        e
                    ))
                })?;
                return Ok(new_response);
            }
        }

        Err(ProviderError::ResponseParseError(
            "Invalid Claude response format".to_string(),
        ))
    }
}

impl ProviderAdapter for GenericAdapter {
    fn name(&self) -> &str {
        &self.config.display_name
    }

    fn transform_request(&self, request: &AdapterRequest) -> ProviderResult<AdapterRequest> {
        // 简化后直接使用格式转换，保持代理透明性
        // 可以根据 request_stage_config 添加特殊头部处理
        self.transform_request_format(request)
    }

    fn transform_response(
        &self,
        response: &AdapterResponse,
        request: &AdapterRequest,
    ) -> ProviderResult<AdapterResponse> {
        // 简化后直接使用格式转换，保持代理透明性
        self.transform_response_format(response, request)
    }

    fn validate_request(&self, request: &AdapterRequest) -> ProviderResult<()> {
        // 基本验证
        if request.body.is_none() {
            return Err(ProviderError::InvalidRequest(
                "Request body cannot be empty".to_string(),
            ));
        }

        // 如果请求体存在，尝试解析为ChatCompletionRequest进行进一步验证
        if let Some(body) = &request.body {
            // 尝试解析为ChatCompletionRequest
            if let Ok(chat_request) = serde_json::from_value::<ChatCompletionRequest>(body.clone())
            {
                if chat_request.messages.is_empty() {
                    return Err(ProviderError::InvalidRequest(
                        "Chat completion must have at least one message".to_string(),
                    ));
                }

                if chat_request.model.is_empty() {
                    return Err(ProviderError::InvalidRequest(
                        "Model name cannot be empty".to_string(),
                    ));
                }
            }
            // 如果不是ChatCompletionRequest格式，也允许通过（可能是其他格式的请求）
        }

        Ok(())
    }

    fn supports_streaming(&self, _endpoint: &str) -> bool {
        // 简化后默认都支持流式响应，保持代理透明性
        true
    }

    /// 检查请求是否要求流式响应
    fn is_streaming_request(&self, request: &AdapterRequest) -> bool {
        // 检查请求参数中是否有stream=true
        if request
            .query
            .get("stream")
            .map(|s| s == "true")
            .unwrap_or(false)
        {
            return true;
        }

        // 检查请求体中是否有stream字段
        if let Some(body) = &request.body {
            if let Some(stream) = body.get("stream") {
                return stream.as_bool().unwrap_or(false);
            }
        }

        // 检查请求头中是否有Accept: text/event-stream
        request
            .headers
            .get("accept")
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

    fn handle_streaming_chunk(
        &self,
        chunk: &[u8],
        _request: &AdapterRequest,
    ) -> ProviderResult<Option<StreamChunk>> {
        // 基本的流式处理实现
        let chunk_str = std::str::from_utf8(chunk).map_err(|e| {
            ProviderError::StreamParseError(format!("Invalid UTF-8 in chunk: {}", e))
        })?;

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
                        "openai" | "openai-compatible" => Ok(Some(StreamChunk::Data(json_data))),
                        "gemini" | "google" => {
                            // 转换Gemini流式响应为OpenAI格式
                            if let Some(candidates) =
                                json_data.get("candidates").and_then(|c| c.as_array())
                            {
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
                            if let Some(content_block) =
                                json_data.get("delta").and_then(|d| d.get("text"))
                            {
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
                        _ => Ok(Some(StreamChunk::Data(json_data))),
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse streaming chunk as JSON: {}, chunk: {}",
                        e,
                        data
                    );
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
