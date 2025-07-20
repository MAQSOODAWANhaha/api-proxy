//! # 测试 Mock 对象
//!
//! 提供各种组件的 Mock 实现用于单元测试

use async_trait::async_trait;
use mockall::{mock, predicate::*};
use serde_json::Value;
use std::collections::HashMap;

/// Mock 配置管理器
mock! {
    pub ConfigManager {
        pub async fn new() -> crate::error::Result<Self>;
        pub async fn get_config(&self) -> crate::config::AppConfig;
        pub async fn reload_config(&self) -> crate::error::Result<()>;
        pub fn subscribe_changes(&self) -> Option<tokio::sync::broadcast::Receiver<crate::config::ConfigEvent>>;
    }
}

/// Mock 缓存客户端
mock! {
    pub CacheClient {
        pub async fn new(config: crate::config::RedisConfig) -> crate::error::Result<Self>;
        pub async fn set(&self, key: &str, value: String) -> crate::error::Result<()>;
        pub async fn get(&self, key: &str) -> crate::error::Result<Option<String>>;
        pub async fn delete(&self, key: &str) -> crate::error::Result<bool>;
        pub async fn exists(&self, key: &str) -> crate::error::Result<bool>;
        pub async fn ping(&self) -> crate::error::Result<()>;
    }
}

/// Mock 数据库连接
mock! {
    pub DatabaseConnection {
        pub async fn execute(&self, sql: &str) -> Result<(), sea_orm::DbErr>;
        pub async fn query_one(&self, sql: &str) -> Result<Option<sea_orm::QueryResult>, sea_orm::DbErr>;
        pub async fn query_all(&self, sql: &str) -> Result<Vec<sea_orm::QueryResult>, sea_orm::DbErr>;
    }
}

/// Mock AI 提供商客户端
#[async_trait]
pub trait MockAiProvider: Send + Sync {
    async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse, Box<dyn std::error::Error + Send + Sync>>;
    async fn health_check(&self) -> Result<HealthStatus, Box<dyn std::error::Error + Send + Sync>>;
}

/// 聊天完成请求
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

/// 聊天消息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// 聊天完成响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: Option<Usage>,
    pub created: u64,
}

/// 聊天选择
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatChoice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

/// 使用统计
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 健康状态
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub response_time_ms: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// AI 提供商错误
#[derive(Debug, thiserror::Error)]
pub enum AiProviderError {
    #[error("网络错误: {0}")]
    Network(String),
    #[error("认证错误: {0}")]
    Authentication(String),
    #[error("速率限制: {0}")]
    RateLimit(String),
    #[error("服务器错误: {0}")]
    Server(String),
}

/// Mock OpenAI 提供商
pub struct MockOpenAiProvider {
    pub responses: HashMap<String, ChatCompletionResponse>,
    pub health_status: HealthStatus,
    pub should_fail: bool,
}

impl MockOpenAiProvider {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            health_status: HealthStatus {
                status: "healthy".to_string(),
                response_time_ms: 150,
                timestamp: chrono::Utc::now(),
            },
            should_fail: false,
        }
    }

    pub fn with_response(mut self, model: &str, response: ChatCompletionResponse) -> Self {
        self.responses.insert(model.to_string(), response);
        self
    }

    pub fn with_failure(mut self) -> Self {
        self.should_fail = true;
        self
    }

    pub fn unhealthy(mut self) -> Self {
        self.health_status.status = "unhealthy".to_string();
        self.health_status.response_time_ms = 5000;
        self
    }
}

impl Default for MockOpenAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MockAiProvider for MockOpenAiProvider {
    async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse, Box<dyn std::error::Error + Send + Sync>> {
        if self.should_fail {
            return Err(Box::new(AiProviderError::Server("Mock server error".to_string())));
        }

        if let Some(response) = self.responses.get(&request.model) {
            Ok(response.clone())
        } else {
            // 默认响应
            Ok(ChatCompletionResponse {
                id: "mock-response-id".to_string(),
                model: request.model,
                choices: vec![ChatChoice {
                    index: 0,
                    message: ChatMessage {
                        role: "assistant".to_string(),
                        content: "This is a mock response".to_string(),
                    },
                    finish_reason: Some("stop".to_string()),
                }],
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                }),
                created: chrono::Utc::now().timestamp() as u64,
            })
        }
    }

    async fn health_check(&self) -> Result<HealthStatus, Box<dyn std::error::Error + Send + Sync>> {
        if self.should_fail {
            return Err(Box::new(AiProviderError::Network("Connection timeout".to_string())));
        }
        Ok(self.health_status.clone())
    }
}

/// Mock HTTP 服务器
pub struct MockHttpServer {
    server: wiremock::MockServer,
}

impl MockHttpServer {
    /// 启动 Mock 服务器
    pub async fn start() -> Self {
        let server = wiremock::MockServer::start().await;
        Self { server }
    }

    /// 获取服务器 URI
    pub fn uri(&self) -> String {
        self.server.uri()
    }

    /// 添加 Mock 响应
    pub async fn mock_response(&self, path: &str, method: &str, status: u16, body: Value) {
        use wiremock::{Mock, ResponseTemplate};

        let method = match method.to_uppercase().as_str() {
            "GET" => wiremock::matchers::method("GET"),
            "POST" => wiremock::matchers::method("POST"),
            "PUT" => wiremock::matchers::method("PUT"),
            "DELETE" => wiremock::matchers::method("DELETE"),
            _ => wiremock::matchers::method("GET"),
        };

        Mock::given(method)
            .and(wiremock::matchers::path(path))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .mount(&self.server)
            .await;
    }

    /// 添加 OpenAI Chat Completion Mock
    pub async fn mock_openai_chat(&self, model: &str, response_content: &str) {
        let response = serde_json::json!({
            "id": "chatcmpl-mock",
            "object": "chat.completion",
            "created": chrono::Utc::now().timestamp(),
            "model": model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": response_content
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });

        self.mock_response("/v1/chat/completions", "POST", 200, response).await;
    }

    /// 添加错误响应 Mock
    pub async fn mock_error(&self, path: &str, status: u16, error_message: &str) {
        let error_response = serde_json::json!({
            "error": {
                "message": error_message,
                "type": "invalid_request_error",
                "code": "invalid_request"
            }
        });

        self.mock_response(path, "POST", status, error_response).await;
    }
}

/// 测试时间辅助
pub struct MockTime {
    current_time: std::sync::Arc<std::sync::Mutex<chrono::DateTime<chrono::Utc>>>,
}

impl MockTime {
    pub fn new(initial_time: chrono::DateTime<chrono::Utc>) -> Self {
        Self {
            current_time: std::sync::Arc::new(std::sync::Mutex::new(initial_time)),
        }
    }

    pub fn now(&self) -> chrono::DateTime<chrono::Utc> {
        *self.current_time.lock().unwrap()
    }

    pub fn advance(&self, duration: chrono::Duration) {
        let mut time = self.current_time.lock().unwrap();
        *time = *time + duration;
    }

    pub fn set(&self, new_time: chrono::DateTime<chrono::Utc>) {
        let mut time = self.current_time.lock().unwrap();
        *time = new_time;
    }
}

impl Default for MockTime {
    fn default() -> Self {
        Self::new(chrono::Utc::now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_openai_provider() {
        let provider = MockOpenAiProvider::new();
        
        let request = ChatCompletionRequest {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            temperature: Some(0.7),
            max_tokens: Some(100),
        };

        let response = provider.chat_completion(request).await.unwrap();
        assert_eq!(response.model, "gpt-3.5-turbo");
        assert!(!response.choices.is_empty());
    }

    #[tokio::test]
    async fn test_mock_openai_provider_failure() {
        let provider = MockOpenAiProvider::new().with_failure();
        
        let request = ChatCompletionRequest {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
        };

        let result = provider.chat_completion(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_http_server() {
        let server = MockHttpServer::start().await;
        
        server.mock_openai_chat("gpt-3.5-turbo", "Hello from mock!").await;
        
        let client = reqwest::Client::new();
        let response = client
            .post(&format!("{}/v1/chat/completions", server.uri()))
            .json(&serde_json::json!({
                "model": "gpt-3.5-turbo",
                "messages": [{"role": "user", "content": "Hi"}]
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
        
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["model"], "gpt-3.5-turbo");
    }

    #[test]
    fn test_mock_time() {
        let start_time = chrono::Utc::now();
        let mock_time = MockTime::new(start_time);
        
        assert_eq!(mock_time.now(), start_time);
        
        mock_time.advance(chrono::Duration::hours(1));
        assert_eq!(mock_time.now(), start_time + chrono::Duration::hours(1));
        
        let new_time = start_time + chrono::Duration::days(1);
        mock_time.set(new_time);
        assert_eq!(mock_time.now(), new_time);
    }
}