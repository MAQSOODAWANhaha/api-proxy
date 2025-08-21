//! # AI服务提供商适配器模块
//!
//! 基于数据库配置的动态AI服务适配器系统

pub mod dynamic_manager;
pub mod field_extractor;
pub mod generic_adapter;
pub mod models;
pub mod traits;
pub mod types;

pub use dynamic_manager::DynamicAdapterManager;
pub use field_extractor::{FieldExtractor, FieldMappingConfig, TransformRule};
pub use generic_adapter::{GenericAdapter, GenericAdapterConfig};
pub use models::{AIModel, ClaudeModel, GeminiModel, OpenAIModel};
pub use traits::ProviderAdapter;
pub use types::{
    AdapterRequest, AdapterResponse, ChatChoice, ChatCompletionRequest, ChatCompletionResponse,
    ChatMessage, MessageRole, ModelParameters, ProviderError, ProviderResult, StreamChunk,
    StreamingResponse, TraceStats, Usage,
};
