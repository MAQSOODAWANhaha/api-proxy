//! # 适配器管理器
//! 
//! 管理和协调不同AI服务提供商的适配器

use std::collections::HashMap;
use crate::proxy::upstream::UpstreamType;
use super::types::{
    AdapterRequest, AdapterResponse, StreamChunk,
    ProviderError, ProviderResult
};
use super::traits::ProviderAdapter;
use super::{OpenAIAdapter, GeminiAdapter, ClaudeAdapter};

/// 适配器管理器
pub struct AdapterManager {
    adapters: HashMap<UpstreamType, Box<dyn ProviderAdapter>>,
}

impl AdapterManager {
    /// 创建新的适配器管理器
    pub fn new() -> Self {
        let mut manager = Self {
            adapters: HashMap::new(),
        };

        // 注册默认适配器
        manager.register_default_adapters();
        manager
    }

    /// 注册默认适配器
    fn register_default_adapters(&mut self) {
        self.adapters.insert(UpstreamType::OpenAI, Box::new(OpenAIAdapter::new()));
        self.adapters.insert(UpstreamType::GoogleGemini, Box::new(GeminiAdapter::new()));
        self.adapters.insert(UpstreamType::Anthropic, Box::new(ClaudeAdapter::new()));
    }

    /// 注册自定义适配器
    pub fn register_adapter(&mut self, upstream_type: UpstreamType, adapter: Box<dyn ProviderAdapter>) {
        self.adapters.insert(upstream_type, adapter);
    }

    /// 获取适配器
    pub fn get_adapter(&self, upstream_type: &UpstreamType) -> Option<&dyn ProviderAdapter> {
        self.adapters.get(upstream_type).map(|a| a.as_ref())
    }

    /// 处理请求
    pub fn process_request(&self, upstream_type: &UpstreamType, request: AdapterRequest) -> ProviderResult<AdapterRequest> {
        let adapter = self.get_adapter(upstream_type)
            .ok_or_else(|| ProviderError::UnsupportedOperation(
                format!("No adapter found for upstream type: {:?}", upstream_type)
            ))?;

        adapter.transform_request(&request)
    }

    /// 处理响应
    pub fn process_response(&self, upstream_type: &UpstreamType, response: AdapterResponse, original_request: &AdapterRequest) -> ProviderResult<AdapterResponse> {
        let adapter = self.get_adapter(upstream_type)
            .ok_or_else(|| ProviderError::UnsupportedOperation(
                format!("No adapter found for upstream type: {:?}", upstream_type)
            ))?;

        adapter.transform_response(&response, original_request)
    }

    /// 处理流式响应
    pub fn process_streaming_response(&self, upstream_type: &UpstreamType, chunk: &[u8], request: &AdapterRequest) -> ProviderResult<Option<StreamChunk>> {
        let adapter = self.get_adapter(upstream_type)
            .ok_or_else(|| ProviderError::UnsupportedOperation(
                format!("No adapter found for upstream type: {:?}", upstream_type)
            ))?;

        adapter.handle_streaming_chunk(chunk, request)
    }

    /// 验证请求
    pub fn validate_request(&self, upstream_type: &UpstreamType, request: &AdapterRequest) -> ProviderResult<()> {
        let adapter = self.get_adapter(upstream_type)
            .ok_or_else(|| ProviderError::UnsupportedOperation(
                format!("No adapter found for upstream type: {:?}", upstream_type)
            ))?;

        adapter.validate_request(request)
    }

    /// 检查端点是否支持
    pub fn supports_endpoint(&self, upstream_type: &UpstreamType, path: &str) -> bool {
        if let Some(adapter) = self.get_adapter(upstream_type) {
            adapter.supports_endpoint(path)
        } else {
            false
        }
    }

    /// 获取所有支持的上游类型
    pub fn supported_upstream_types(&self) -> Vec<&UpstreamType> {
        self.adapters.keys().collect()
    }

    /// 根据路径自动检测上游类型
    pub fn detect_upstream_type(&self, path: &str) -> Option<UpstreamType> {
        for (upstream_type, adapter) in &self.adapters {
            if adapter.supports_endpoint(path) {
                return Some(upstream_type.clone());
            }
        }
        None
    }

    /// 获取适配器统计信息
    pub fn get_adapter_stats(&self) -> HashMap<String, AdapterStats> {
        let mut stats = HashMap::new();
        
        tracing::info!("AdapterManager has {} adapters", self.adapters.len());
        
        for (upstream_type, adapter) in &self.adapters {
            let endpoints = adapter.get_supported_endpoints();
            let adapter_name = adapter.name().to_string();
            
            tracing::info!("Processing adapter: {} (type: {:?}) with {} endpoints", 
                         adapter_name, upstream_type, endpoints.len());
            
            let stat = AdapterStats {
                name: adapter_name.clone(),
                upstream_type: format!("{:?}", upstream_type),
                supported_endpoints: endpoints.len(),
                endpoints,
            };
            stats.insert(adapter_name, stat);
        }
        
        tracing::info!("Final adapter stats keys: {:?}", stats.keys().collect::<Vec<_>>());
        stats
    }
}

impl Default for AdapterManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 适配器统计信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdapterStats {
    pub name: String,
    pub upstream_type: String,
    pub supported_endpoints: usize,
    pub endpoints: Vec<String>,
}

impl std::fmt::Debug for AdapterManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdapterManager")
            .field("adapter_count", &self.adapters.len())
            .field("upstream_types", &self.adapters.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_adapter_manager_creation() {
        let manager = AdapterManager::new();
        
        assert!(manager.get_adapter(&UpstreamType::OpenAI).is_some());
        assert!(manager.get_adapter(&UpstreamType::GoogleGemini).is_some());
        assert!(manager.get_adapter(&UpstreamType::Anthropic).is_some());
    }

    #[test]
    fn test_endpoint_support_detection() {
        let manager = AdapterManager::new();
        
        assert!(manager.supports_endpoint(&UpstreamType::OpenAI, "/v1/chat/completions"));
        assert!(!manager.supports_endpoint(&UpstreamType::OpenAI, "/unknown/endpoint"));
    }

    #[test]
    fn test_upstream_type_detection() {
        let manager = AdapterManager::new();
        
        assert_eq!(manager.detect_upstream_type("/v1/chat/completions"), Some(UpstreamType::OpenAI));
        assert_eq!(manager.detect_upstream_type("/unknown/endpoint"), None);
    }

    #[test]
    fn test_request_validation() {
        let manager = AdapterManager::new();
        
        let valid_request = AdapterRequest::new("POST", "/v1/chat/completions")
            .with_header("Authorization", "Bearer sk-1234567890abcdef1234567890abcdef12345678");
        
        let result = manager.validate_request(&UpstreamType::OpenAI, &valid_request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_request_processing() {
        let manager = AdapterManager::new();
        
        let request = AdapterRequest::new("POST", "/v1/chat/completions")
            .with_header("Authorization", "Bearer sk-1234567890abcdef1234567890abcdef12345678")
            .with_body(json!({
                "model": "gpt-3.5-turbo",
                "messages": [{"role": "user", "content": "Hello"}]
            }));

        let result = manager.process_request(&UpstreamType::OpenAI, request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_adapter_stats() {
        let manager = AdapterManager::new();
        let stats = manager.get_adapter_stats();
        
        assert!(stats.contains_key("openai"));
        assert!(stats.get("openai").unwrap().supported_endpoints > 0);
    }

    #[test]
    fn test_custom_adapter_registration() {
        let mut manager = AdapterManager::new();
        let custom_adapter = Box::new(OpenAIAdapter::new());
        
        manager.register_adapter(UpstreamType::Custom("test".to_string()), custom_adapter);
        
        assert!(manager.get_adapter(&UpstreamType::Custom("test".to_string())).is_some());
    }
}