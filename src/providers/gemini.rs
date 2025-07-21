//! # Google Gemini API适配器实现
//! 
//! 此模块将在后续任务中实现

use super::types::{ProviderAdapter, AdapterRequest, AdapterResponse, StreamingResponse, ProviderResult};

/// Google Gemini适配器占位符
pub struct GeminiAdapter;

impl ProviderAdapter for GeminiAdapter {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn process_request(&self, _request: AdapterRequest) -> ProviderResult<AdapterRequest> {
        todo!("Gemini adapter implementation pending")
    }

    fn process_response(&self, _response: AdapterResponse) -> ProviderResult<AdapterResponse> {
        todo!("Gemini adapter implementation pending")
    }

    fn process_streaming_response(&self, _chunk: &[u8]) -> ProviderResult<StreamingResponse> {
        todo!("Gemini adapter implementation pending")
    }

    fn validate_api_key(&self, _api_key: &str) -> bool {
        false
    }

    fn supported_endpoints(&self) -> Vec<&'static str> {
        vec![]
    }
}