//! # 适配器特征定义
//!
//! 定义所有AI服务提供商适配器需要实现的通用接口

use super::types::{AdapterRequest, AdapterResponse, ProviderResult, StreamChunk};

/// AI服务提供商适配器特征
pub trait ProviderAdapter: Send + Sync {
    /// 获取适配器名称
    fn name(&self) -> &str;

    /// 检查是否支持流式响应（基于配置而非端点路径）
    fn supports_streaming(&self, endpoint: &str) -> bool;

    /// 转换请求格式
    fn transform_request(&self, request: &AdapterRequest) -> ProviderResult<AdapterRequest>;

    /// 转换响应格式
    fn transform_response(
        &self,
        response: &AdapterResponse,
        original_request: &AdapterRequest,
    ) -> ProviderResult<AdapterResponse>;

    /// 处理流式响应块
    fn handle_streaming_chunk(
        &self,
        chunk: &[u8],
        request: &AdapterRequest,
    ) -> ProviderResult<Option<StreamChunk>>;

    /// 验证请求格式
    fn validate_request(&self, request: &AdapterRequest) -> ProviderResult<()>;

    /// 检查请求是否要求流式响应
    fn is_streaming_request(&self, request: &AdapterRequest) -> bool;

    /// 检查响应是否为流式响应
    fn is_streaming_response(&self, response: &AdapterResponse) -> bool;
}
