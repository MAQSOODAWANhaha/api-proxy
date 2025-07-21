//! # AI服务提供商适配器模块
//! 
//! 为不同的AI服务提供商提供统一的接口适配

pub mod openai;
pub mod gemini;
pub mod claude;
pub mod types;
pub mod manager;

pub use openai::{OpenAIAdapter, OpenAIStreamParser};
pub use gemini::GeminiAdapter;
pub use claude::ClaudeAdapter;
pub use manager::{AdapterManager, AdapterStats};
pub use types::{
    ProviderAdapter, AdapterRequest, AdapterResponse, 
    StreamingResponse, ProviderError, ProviderResult,
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage,
    MessageRole, Usage, ChatChoice, ModelParameters
};