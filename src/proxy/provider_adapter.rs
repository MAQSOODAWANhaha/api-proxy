//! # 提供商适配器
//!
//! 处理不同AI服务提供商的特定逻辑和适配

use crate::error::{ProxyError, Result};
use crate::providers::{DynamicAdapterManager, AdapterRequest, AdapterResponse};
use crate::proxy::types::ProviderId;
use pingora_http::RequestHeader;
use sea_orm::{DatabaseConnection, EntityTrait, ColumnTrait, QueryFilter};
use std::sync::Arc;
use serde_json::Value;

/// 提供商适配器
///
/// 负责处理不同提供商的请求/响应格式转换
pub struct ProviderAdapter {
    /// 适配器管理器
    adapter_manager: Arc<DynamicAdapterManager>,
    /// 数据库连接
    db: Arc<DatabaseConnection>,
}

impl ProviderAdapter {
    /// 创建新的提供商适配器
    pub fn new(
        adapter_manager: Arc<DynamicAdapterManager>,
        db: Arc<DatabaseConnection>,
    ) -> Self {
        Self {
            adapter_manager,
            db,
        }
    }

    /// 转换请求格式
    pub async fn transform_request(
        &self,
        provider_id: &ProviderId,
        request_header: &RequestHeader,
        body: Option<&[u8]>,
    ) -> Result<AdapterRequest> {
        // 构建适配器请求
        let mut adapter_request = AdapterRequest::new(
            request_header.method.as_str(),
            request_header.uri.path(),
        );

        // 复制请求头
        for (name, value) in request_header.headers.iter() {
            if let Ok(value_str) = value.to_str() {
                adapter_request = adapter_request.with_header(name.as_str(), value_str);
            }
        }

        // 处理请求体
        if let Some(body_bytes) = body {
            if let Ok(body_str) = std::str::from_utf8(body_bytes) {
                if !body_str.is_empty() {
                    if let Ok(json_body) = serde_json::from_str::<Value>(body_str) {
                        adapter_request = adapter_request.with_body(json_body);
                    }
                }
            }
        }

        // 使用适配器管理器处理请求
        self.adapter_manager
            .process_request(provider_id, &adapter_request)
            .await
            .map_err(|e| ProxyError::internal(format!("Request transformation failed: {}", e)))
    }

    /// 转换响应格式
    pub async fn transform_response(
        &self,
        provider_id: &ProviderId,
        response: &AdapterResponse,
        original_request: &AdapterRequest,
    ) -> Result<AdapterResponse> {
        self.adapter_manager
            .process_response(provider_id, response, original_request)
            .await
            .map_err(|e| ProxyError::internal(format!("Response transformation failed: {}", e)))
    }

    /// 处理流式响应块
    pub async fn transform_streaming_chunk(
        &self,
        provider_id: &ProviderId,
        chunk: &[u8],
        request: &AdapterRequest,
    ) -> Result<Option<Vec<u8>>> {
        match self.adapter_manager
            .process_streaming_response(provider_id, chunk, request)
            .await
        {
            Ok(Some(stream_chunk)) => {
                match stream_chunk {
                    crate::providers::StreamChunk::Data(json_data) => {
                        // 将JSON数据转换回字节
                        serde_json::to_vec(&json_data)
                            .map(Some)
                            .map_err(|e| ProxyError::internal(format!("Failed to serialize chunk: {}", e)))
                    }
                    crate::providers::StreamChunk::Raw(raw_data) => Ok(Some(raw_data)),
                    crate::providers::StreamChunk::Done => Ok(Some(b"data: [DONE]\n\n".to_vec())),
                    crate::providers::StreamChunk::Error(err) => {
                        Err(ProxyError::internal(format!("Stream chunk error: {}", err)))
                    }
                }
            }
            Ok(None) => Ok(None),
            Err(e) => Err(ProxyError::internal(format!("Streaming chunk processing failed: {}", e))),
        }
    }

    /// 验证请求是否有效
    pub async fn validate_request(
        &self,
        provider_id: &ProviderId,
        request: &AdapterRequest,
    ) -> Result<()> {
        self.adapter_manager
            .validate_request(provider_id, request)
            .await
            .map_err(|e| ProxyError::internal(format!("Request validation failed: {}", e)))
    }

    /// 检查提供商是否支持指定的端点
    pub async fn supports_endpoint(&self, provider_id: &ProviderId, path: &str) -> bool {
        // 检查适配器管理器中是否有对应的适配器
        if !self.adapter_manager.has_adapter(provider_id).await {
            return false;
        }

        // 对于大多数AI提供商，支持常见的聊天端点
        match path {
            "/v1/chat/completions" => true,
            "/v1/completions" => true,
            "/v1/models" => true,
            path if path.starts_with("/v1/") => true,
            path if path.contains("chat") => true,
            path if path.contains("completion") => true,
            _ => false,
        }
    }

    /// 提取模型名称
    pub async fn extract_model_name(
        &self,
        _provider_id: &ProviderId,
        request: &AdapterRequest,
    ) -> Option<String> {
        // 从请求体中提取模型名称
        if let Some(body) = &request.body {
            if let Some(model) = body.get("model").and_then(|m| m.as_str()) {
                return Some(model.to_string());
            }
        }

        // 从查询参数中提取
        if let Some(model) = request.query.get("model") {
            return Some(model.clone());
        }

        // 从路径中提取（如Gemini格式）
        if request.path.contains("/models/") {
            if let Some(model_part) = request.path.split("/models/").nth(1) {
                if let Some(model_name) = model_part.split(':').next() {
                    return Some(model_name.to_string());
                }
            }
        }

        None
    }

    /// 获取默认模型名称
    pub async fn get_default_model(&self, provider_id: &ProviderId) -> Result<String> {
        // 从数据库获取提供商的默认模型
        let provider = entity::provider_types::Entity::find()
            .filter(entity::provider_types::Column::Id.eq(provider_id.id()))
            .one(&*self.db)
            .await
            .map_err(|e| ProxyError::database(format!("Failed to query provider: {}", e)))?
            .ok_or_else(|| ProxyError::internal("Provider not found".to_string()))?;

        Ok(provider.default_model.unwrap_or_else(|| {
            match provider.name.as_str() {
                "openai" => "gpt-3.5-turbo".to_string(),
                "anthropic" => "claude-3-sonnet".to_string(),
                "gemini" => "gemini-pro".to_string(),
                _ => "unknown".to_string(),
            }
        }))
    }
}

