//! # AI服务提供商适配器模块
//! 
//! 基于数据库配置的动态AI服务适配器系统

pub mod types;
pub mod traits;
pub mod models;
pub mod generic_adapter;
pub mod dynamic_manager;
pub mod field_extractor;

pub use models::{AIModel, OpenAIModel, GeminiModel, ClaudeModel};
pub use generic_adapter::{GenericAdapter, GenericAdapterConfig};
pub use dynamic_manager::DynamicAdapterManager;
pub use field_extractor::{FieldExtractor, FieldMappingConfig, TransformRule};
pub use types::{
    AdapterRequest, AdapterResponse, StreamingResponse, StreamChunk,
    ProviderError, ProviderResult, ChatCompletionRequest, 
    ChatCompletionResponse, ChatMessage, MessageRole, Usage, 
    ChatChoice, ModelParameters, TraceStats
};
pub use traits::ProviderAdapter;