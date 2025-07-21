//! # Anthropic Claude API适配器实现
//! 
//! 此模块将在后续任务中实现

use super::types::{ProviderAdapter, AdapterRequest, AdapterResponse, StreamingResponse, ProviderResult};

/// Anthropic Claude适配器占位符
pub struct ClaudeAdapter;

impl ProviderAdapter for ClaudeAdapter {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn process_request(&self, _request: AdapterRequest) -> ProviderResult<AdapterRequest> {
        todo!("Claude adapter implementation pending")
    }

    fn process_response(&self, _response: AdapterResponse) -> ProviderResult<AdapterResponse> {
        todo!("Claude adapter implementation pending")
    }

    fn process_streaming_response(&self, _chunk: &[u8]) -> ProviderResult<StreamingResponse> {
        todo!("Claude adapter implementation pending")
    }

    fn validate_api_key(&self, _api_key: &str) -> bool {
        false
    }

    fn supported_endpoints(&self) -> Vec<&'static str> {
        vec![]
    }
}