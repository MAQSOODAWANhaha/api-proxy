//! # OpenAI API适配器实现

use serde_json::{Value, json};
use super::types::{
    AdapterRequest, AdapterResponse, StreamingResponse, StreamChunk,
    ProviderError, ProviderResult, ChatCompletionRequest,
    ModelParameters
};
use super::traits::ProviderAdapter;
use super::models::OpenAIModel;

/// OpenAI API适配器
#[derive(Debug, Clone)]
pub struct OpenAIAdapter {
    /// 默认模型
    pub default_model: OpenAIModel,
    /// 支持的模型列表
    pub supported_models: Vec<OpenAIModel>,
    /// API版本
    pub api_version: String,
}

impl Default for OpenAIAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAIAdapter {
    /// 创建新的OpenAI适配器
    pub fn new() -> Self {
        Self {
            default_model: OpenAIModel::default(),
            supported_models: OpenAIModel::all(),
            api_version: "v1".to_string(),
        }
    }

    /// 使用自定义配置创建适配器
    pub fn with_config(default_model: OpenAIModel, supported_models: Vec<OpenAIModel>) -> Self {
        Self {
            default_model,
            supported_models,
            api_version: "v1".to_string(),
        }
    }

    /// 验证模型是否支持
    pub fn validate_model(&self, model: &str) -> bool {
        if let Some(model_enum) = OpenAIModel::from_str(model) {
            self.supported_models.contains(&model_enum)
        } else {
            false
        }
    }

    /// 处理聊天完成请求
    fn process_chat_completion(&self, request: &AdapterRequest) -> ProviderResult<AdapterRequest> {
        let body = request.body.as_ref()
            .ok_or_else(|| ProviderError::InvalidRequest("Missing request body".to_string()))?;

        // 解析请求体
        let chat_request: ChatCompletionRequest = serde_json::from_value(body.clone())
            .map_err(|e| ProviderError::InvalidRequest(format!("Invalid chat completion request: {}", e)))?;

        // 验证模型
        if !self.validate_model(&chat_request.model) {
            return Err(ProviderError::UnsupportedOperation(
                format!("Model '{}' is not supported", chat_request.model)
            ));
        }

        // 验证消息
        if chat_request.messages.is_empty() {
            return Err(ProviderError::InvalidRequest("Messages cannot be empty".to_string()));
        }

        // 验证参数
        self.validate_parameters(&chat_request.parameters)?;

        // 构建OpenAI格式的请求
        let openai_body = self.build_openai_request(&chat_request)?;

        let mut openai_request = request.clone();
        openai_request.body = Some(openai_body);
        openai_request.path = "/v1/chat/completions".to_string();

        // 确保正确的Content-Type
        openai_request.headers.insert(
            "Content-Type".to_string(),
            "application/json".to_string()
        );

        Ok(openai_request)
    }

    /// 验证请求参数
    fn validate_parameters(&self, params: &ModelParameters) -> ProviderResult<()> {
        if let Some(temp) = params.temperature {
            if !(0.0..=2.0).contains(&temp) {
                return Err(ProviderError::InvalidRequest(
                    "Temperature must be between 0.0 and 2.0".to_string()
                ));
            }
        }

        if let Some(top_p) = params.top_p {
            if !(0.0..=1.0).contains(&top_p) {
                return Err(ProviderError::InvalidRequest(
                    "top_p must be between 0.0 and 1.0".to_string()
                ));
            }
        }

        if let Some(max_tokens) = params.max_tokens {
            if max_tokens == 0 || max_tokens > 32768 {
                return Err(ProviderError::InvalidRequest(
                    "max_tokens must be between 1 and 32768".to_string()
                ));
            }
        }

        Ok(())
    }

    /// 构建OpenAI API请求体
    fn build_openai_request(&self, request: &ChatCompletionRequest) -> ProviderResult<Value> {
        let mut openai_request = json!({
            "model": request.model,
            "messages": request.messages
        });

        // 添加可选参数
        if let Some(max_tokens) = request.parameters.max_tokens {
            openai_request["max_tokens"] = json!(max_tokens);
        }

        if let Some(temperature) = request.parameters.temperature {
            openai_request["temperature"] = json!(temperature);
        }

        if let Some(top_p) = request.parameters.top_p {
            openai_request["top_p"] = json!(top_p);
        }

        if let Some(frequency_penalty) = request.parameters.frequency_penalty {
            openai_request["frequency_penalty"] = json!(frequency_penalty);
        }

        if let Some(presence_penalty) = request.parameters.presence_penalty {
            openai_request["presence_penalty"] = json!(presence_penalty);
        }

        if let Some(stream) = request.parameters.stream {
            openai_request["stream"] = json!(stream);
        }

        Ok(openai_request)
    }

    /// 处理流式响应数据
    fn parse_streaming_chunk(&self, chunk: &str) -> ProviderResult<StreamingResponse> {
        // OpenAI SSE格式: "data: {json}\n\n"
        if chunk.trim().is_empty() {
            return Ok(StreamingResponse::data(Vec::new()));
        }

        if chunk.starts_with("data: ") {
            let data_part = &chunk[6..]; // 跳过 "data: "
            
            // 检查是否为结束标记
            if data_part.trim() == "[DONE]" {
                return Ok(StreamingResponse::final_chunk(b"data: [DONE]\n\n".to_vec()));
            }

            // 尝试解析JSON
            match serde_json::from_str::<Value>(data_part.trim()) {
                Ok(json_data) => {
                    // 验证是否为有效的聊天完成流响应
                    if self.is_valid_streaming_response(&json_data) {
                        let formatted_chunk = format!("data: {}\n\n", data_part.trim());
                        Ok(StreamingResponse::data(formatted_chunk.into_bytes()))
                    } else {
                        Err(ProviderError::InvalidRequest(
                            "Invalid streaming response format".to_string()
                        ))
                    }
                }
                Err(e) => Err(ProviderError::SerializationError(
                    format!("Failed to parse streaming JSON: {}", e)
                ))
            }
        } else if chunk.starts_with("event: ") || chunk.starts_with("id: ") || chunk.starts_with("retry: ") {
            // 保留其他SSE字段
            Ok(StreamingResponse::data(chunk.as_bytes().to_vec()))
        } else {
            // 其他数据直接传递
            Ok(StreamingResponse::data(chunk.as_bytes().to_vec()))
        }
    }

    /// 验证流式响应格式
    fn is_valid_streaming_response(&self, data: &Value) -> bool {
        data.get("object").and_then(|o| o.as_str()) == Some("chat.completion.chunk") ||
        data.get("choices").is_some()
    }

    /// 处理错误响应
    fn handle_error_response(&self, status_code: u16, body: &str) -> ProviderError {
        // 尝试解析OpenAI错误格式
        if let Ok(error_data) = serde_json::from_str::<Value>(body) {
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

impl ProviderAdapter for OpenAIAdapter {
    fn name(&self) -> &str {
        "openai"
    }

    fn supports_endpoint(&self, endpoint: &str) -> bool {
        let supported = [
            "/v1/chat/completions",
            "/v1/completions",
            "/v1/models",
            "/v1/embeddings",
            "/v1/audio/transcriptions",
            "/v1/audio/translations",
            "/v1/images/generations",
            "/v1/images/edits",
            "/v1/images/variations",
            "/v1/moderations",
        ];
        supported.iter().any(|&ep| endpoint.starts_with(ep))
    }

    fn supports_streaming(&self, endpoint: &str) -> bool {
        endpoint.starts_with("/v1/chat/completions") || endpoint.starts_with("/v1/completions")
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
            _ => {
                // 其他端点直接透传
                Ok(request.clone())
            }
        }
    }

    fn transform_response(&self, response: &AdapterResponse, _original_request: &AdapterRequest) -> ProviderResult<AdapterResponse> {
        if response.status_code >= 400 {
            let error_msg = response.body.as_str()
                .unwrap_or("Unknown error")
                .to_string();
            return Err(self.handle_error_response(response.status_code, &error_msg));
        }

        // 对于成功响应，直接返回
        Ok(response.clone())
    }

    fn handle_streaming_chunk(&self, chunk: &[u8], _request: &AdapterRequest) -> ProviderResult<Option<StreamChunk>> {
        let chunk_str = std::str::from_utf8(chunk)
            .map_err(|e| ProviderError::SerializationError(format!("Invalid UTF-8: {}", e)))?;

        // 简化实现，直接返回数据块
        if chunk_str.trim().is_empty() {
            return Ok(None);
        }

        if chunk_str.contains("[DONE]") {
            return Ok(Some(StreamChunk::final_chunk(Vec::new())));
        }

        Ok(Some(StreamChunk::data(chunk.to_vec())))
    }

    fn validate_request(&self, request: &AdapterRequest) -> ProviderResult<()> {
        // 验证端点支持
        if !self.supports_endpoint(&request.path) {
            return Err(ProviderError::UnsupportedOperation(
                format!("Endpoint {} not supported by OpenAI adapter", request.path)
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
        vec![
            "/v1/chat/completions".to_string(),
            "/v1/completions".to_string(),
            "/v1/models".to_string(),
            "/v1/embeddings".to_string(),
            "/v1/audio/transcriptions".to_string(),
            "/v1/audio/translations".to_string(),
            "/v1/images/generations".to_string(),
            "/v1/images/edits".to_string(),
            "/v1/images/variations".to_string(),
            "/v1/moderations".to_string(),
        ]
    }
}

impl OpenAIAdapter {
    /// 验证API密钥格式
    pub fn validate_api_key(&self, api_key: &str) -> bool {
        // OpenAI API密钥格式验证
        api_key.starts_with("sk-") && api_key.len() >= 40
    }
}

/// OpenAI流式响应解析器
pub struct OpenAIStreamParser {
    buffer: String,
}

impl OpenAIStreamParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// 处理传入的数据块
    pub fn process_chunk(&mut self, chunk: &[u8]) -> ProviderResult<Vec<StreamingResponse>> {
        let chunk_str = std::str::from_utf8(chunk)
            .map_err(|e| ProviderError::SerializationError(format!("Invalid UTF-8: {}", e)))?;

        self.buffer.push_str(chunk_str);

        let mut responses = Vec::new();
        
        // 按SSE事件分割
        while let Some(event_end) = self.buffer.find("\n\n") {
            let event = self.buffer[..event_end].to_string();
            self.buffer.drain(..event_end + 2);

            if !event.trim().is_empty() {
                let adapter = OpenAIAdapter::new();
                let response = adapter.parse_streaming_chunk(&event)?;
                responses.push(response);
            }
        }

        Ok(responses)
    }

    /// 完成解析，返回缓冲区中剩余的数据
    pub fn finish(&mut self) -> ProviderResult<Option<StreamingResponse>> {
        if !self.buffer.trim().is_empty() {
            let adapter = OpenAIAdapter::new();
            let response = adapter.parse_streaming_chunk(&self.buffer)?;
            self.buffer.clear();
            Ok(Some(response))
        } else {
            Ok(None)
        }
    }
}

impl Default for OpenAIStreamParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_openai_adapter_creation() {
        let adapter = OpenAIAdapter::new();
        assert_eq!(adapter.name(), "openai");
        assert_eq!(adapter.default_model, OpenAIModel::Gpt35Turbo);
        assert!(adapter.supported_models.contains(&OpenAIModel::Gpt4));
    }

    #[test]
    fn test_api_key_validation() {
        let adapter = OpenAIAdapter::new();
        
        assert!(adapter.validate_api_key("sk-1234567890abcdef1234567890abcdef12345678"));
        assert!(!adapter.validate_api_key("invalid-key"));
        assert!(!adapter.validate_api_key("sk-short"));
    }

    #[test]
    fn test_supported_endpoints() {
        let adapter = OpenAIAdapter::new();
        
        assert!(adapter.supports_endpoint("/v1/chat/completions"));
        assert!(adapter.supports_endpoint("/v1/models"));
        assert!(!adapter.supports_endpoint("/v2/unknown"));
    }

    #[test]
    fn test_model_validation() {
        let adapter = OpenAIAdapter::new();
        
        assert!(adapter.validate_model("gpt-4"));
        assert!(adapter.validate_model("gpt-3.5-turbo"));
        assert!(!adapter.validate_model("unknown-model"));
    }

    #[test]
    fn test_chat_completion_request_processing() {
        let adapter = OpenAIAdapter::new();
        
        let request = AdapterRequest::new("POST", "/v1/chat/completions")
            .with_header("Authorization", "Bearer sk-1234567890abcdef1234567890abcdef12345678")
            .with_body(json!({
                "model": "gpt-3.5-turbo",
                "messages": [
                    {"role": "user", "content": "Hello"}
                ],
                "max_tokens": 100,
                "temperature": 0.7
            }));

        let result = adapter.transform_request(&request);
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        assert_eq!(processed.path, "/v1/chat/completions");
        assert!(processed.body.is_some());
    }

    #[test]
    fn test_invalid_request_handling() {
        let adapter = OpenAIAdapter::new();
        
        // 测试缺少认证头
        let request = AdapterRequest::new("POST", "/v1/chat/completions")
            .with_body(json!({"model": "gpt-3.5-turbo", "messages": []}));
        
        let result = adapter.transform_request(&request);
        assert!(matches!(result, Err(ProviderError::AuthenticationFailed(_))));

        // 测试无效模型
        let request = AdapterRequest::new("POST", "/v1/chat/completions")
            .with_header("Authorization", "Bearer sk-1234567890abcdef1234567890abcdef12345678")
            .with_body(json!({
                "model": "invalid-model",
                "messages": [{"role": "user", "content": "test"}]
            }));

        let result = adapter.transform_request(&request);
        assert!(matches!(result, Err(ProviderError::UnsupportedOperation(_))));
    }

    #[test]
    fn test_streaming_response_parsing() {
        let adapter = OpenAIAdapter::new();
        let request = AdapterRequest::new("POST", "/v1/chat/completions");
        
        let chunk = b"data: {\"object\": \"chat.completion.chunk\", \"choices\": [{\"delta\": {\"content\": \"Hello\"}}]}\n\n";
        let result = adapter.handle_streaming_chunk(chunk, &request);
        
        assert!(result.is_ok());
        let response_opt = result.unwrap();
        assert!(response_opt.is_some());
        let response = response_opt.unwrap();
        assert!(!response.is_final);
        assert!(response.error.is_none());
    }

    #[test]
    fn test_streaming_done_parsing() {
        let adapter = OpenAIAdapter::new();
        let request = AdapterRequest::new("POST", "/v1/chat/completions");
        
        let chunk = b"data: [DONE]\n\n";
        let result = adapter.handle_streaming_chunk(chunk, &request);
        
        assert!(result.is_ok());
        let response_opt = result.unwrap();
        assert!(response_opt.is_some());
        let response = response_opt.unwrap();
        assert!(response.is_final);
    }

    #[test]
    fn test_stream_parser() {
        let mut parser = OpenAIStreamParser::new();
        
        let chunk1 = b"data: {\"object\": \"chat.completion.chunk\"}\n\n";
        let chunk2 = b"data: [DONE]\n\n";
        
        let responses1 = parser.process_chunk(chunk1).unwrap();
        assert_eq!(responses1.len(), 1);
        
        let responses2 = parser.process_chunk(chunk2).unwrap();
        assert_eq!(responses2.len(), 1);
        assert!(responses2[0].is_final);
    }

    #[test]
    fn test_parameter_validation() {
        let adapter = OpenAIAdapter::new();
        
        // 有效参数
        let valid_params = ModelParameters {
            model: "gpt-3.5-turbo".to_string(),
            temperature: Some(0.7),
            top_p: Some(0.9),
            max_tokens: Some(1000),
            ..Default::default()
        };
        assert!(adapter.validate_parameters(&valid_params).is_ok());

        // 无效温度
        let invalid_temp = ModelParameters {
            temperature: Some(3.0), // 超出范围
            ..Default::default()
        };
        assert!(adapter.validate_parameters(&invalid_temp).is_err());

        // 无效top_p
        let invalid_top_p = ModelParameters {
            top_p: Some(1.5), // 超出范围
            ..Default::default()
        };
        assert!(adapter.validate_parameters(&invalid_top_p).is_err());
    }
}