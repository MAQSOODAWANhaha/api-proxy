//! # 请求转发器
//!
//! 简化的请求转发器，替代复杂的forwarding.rs

use crate::error::{ProxyError, Result};
use crate::proxy::types::{ProviderId, ForwardingContext, ForwardingResult};
use crate::proxy::provider_adapter::ProviderAdapter;
use crate::providers::AdapterRequest;
use pingora_core::upstreams::peer::HttpPeer;
use pingora_http::{RequestHeader, ResponseHeader};
use std::sync::Arc;
use std::time::Duration;

/// 简化的请求转发器
pub struct RequestForwarder {
    /// 提供商适配器
    provider_adapter: Arc<ProviderAdapter>,
}

impl RequestForwarder {
    /// 创建新的请求转发器
    pub fn new(provider_adapter: Arc<ProviderAdapter>) -> Self {
        Self {
            provider_adapter,
        }
    }

    /// 准备转发请求
    pub async fn prepare_request(
        &self,
        provider_id: &ProviderId,
        request_header: &RequestHeader,
        body: Option<&[u8]>,
    ) -> Result<AdapterRequest> {
        // 验证提供商是否支持该端点
        if !self.provider_adapter.supports_endpoint(provider_id, request_header.uri.path()).await {
            return Err(ProxyError::internal(
                format!("Provider {:?} does not support endpoint {}", provider_id, request_header.uri.path())
            ));
        }

        // 转换请求格式
        let adapter_request = self.provider_adapter
            .transform_request(provider_id, request_header, body)
            .await?;

        // 验证请求
        self.provider_adapter
            .validate_request(provider_id, &adapter_request)
            .await?;

        Ok(adapter_request)
    }

    /// 创建上游对等体
    pub fn create_upstream_peer(&self, host: &str, port: u16, use_tls: bool) -> HttpPeer {
        let address = format!("{}:{}", host, port);
        HttpPeer::new(&address, use_tls, host.to_string())
    }

    /// 修改请求头以转发
    pub fn modify_request_headers(
        &self,
        request_header: &mut RequestHeader,
        adapter_request: &AdapterRequest,
        provider_id: &ProviderId,
    ) -> Result<()> {
        // 清除可能影响转发的头
        request_header.remove_header("Host");
        request_header.remove_header("Connection");
        request_header.remove_header("Proxy-Connection");

        // 添加适配器请求中的头
        for (name, value) in &adapter_request.headers {
            request_header
                .insert_header(name.clone(), value.clone())
                .map_err(|e| ProxyError::internal(format!("Failed to set header {}: {}", name, e)))?;
        }

        // 添加代理标识头
        request_header
            .insert_header("X-Forwarded-By", "AI-Proxy")
            .map_err(|e| ProxyError::internal(format!("Failed to set proxy header: {}", e)))?;

        // 添加提供商标识头
        request_header
            .insert_header("X-Provider-ID", &provider_id.to_string())
            .map_err(|e| ProxyError::internal(format!("Failed to set provider header: {}", e)))?;

        Ok(())
    }

    /// 处理上游响应
    pub async fn process_response(
        &self,
        provider_id: &ProviderId,
        response_header: &ResponseHeader,
        response_body: Option<&[u8]>,
        original_request: &AdapterRequest,
        context: &ForwardingContext,
    ) -> Result<ForwardingResult> {
        let start_time = context.start_time;
        let response_time = start_time.elapsed();

        // 构建适配器响应
        let adapter_response = self.build_adapter_response(response_header, response_body)?;

        // 转换响应格式
        let _transformed_response = self.provider_adapter
            .transform_response(provider_id, &adapter_response, original_request)
            .await?;

        // 构建转发结果
        let status_code: u16 = response_header.status.into();
        let success = status_code >= 200 && status_code < 400;
        let bytes_transferred = response_body.map(|b| b.len() as u64).unwrap_or(0);

        Ok(ForwardingResult {
            success,
            status_code,
            response_time,
            provider_id: context.provider_id,
            error_message: if success { None } else { Some(format!("HTTP {}", status_code)) },
            bytes_transferred,
        })
    }

    /// 构建适配器响应
    fn build_adapter_response(
        &self,
        response_header: &ResponseHeader,
        response_body: Option<&[u8]>,
    ) -> Result<crate::providers::AdapterResponse> {
        use crate::providers::AdapterResponse;
        use std::collections::HashMap;

        // 构建响应头映射
        let mut headers = HashMap::new();
        for (name, value) in response_header.headers.iter() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(name.as_str().to_string(), value_str.to_string());
            }
        }

        // 解析响应体
        let body = if let Some(body_bytes) = response_body {
            if let Ok(body_str) = std::str::from_utf8(body_bytes) {
                if !body_str.is_empty() {
                    serde_json::from_str(body_str)
                        .unwrap_or_else(|_| serde_json::json!({"raw_response": body_str}))
                } else {
                    serde_json::Value::Null
                }
            } else {
                serde_json::json!({"binary_data": true})
            }
        } else {
            serde_json::Value::Null
        };

        Ok(AdapterResponse {
            status_code: response_header.status.into(),
            headers,
            body,
            is_streaming: self.is_streaming_response(response_header),
        })
    }

    /// 检查响应是否为流式
    fn is_streaming_response(&self, response_header: &ResponseHeader) -> bool {
        response_header.headers
            .get("content-type")
            .or_else(|| response_header.headers.get("Content-Type"))
            .map(|ct| ct.to_str().unwrap_or(""))
            .map(|ct| ct.contains("text/event-stream") || ct.contains("application/stream"))
            .unwrap_or(false)
    }

    /// 处理流式响应块
    pub async fn process_streaming_chunk(
        &self,
        provider_id: &ProviderId,
        chunk: &[u8],
        adapter_request: &AdapterRequest,
    ) -> Result<Option<Vec<u8>>> {
        self.provider_adapter
            .transform_streaming_chunk(provider_id, chunk, adapter_request)
            .await
    }

    /// 添加CORS头
    pub fn add_cors_headers(&self, response_header: &mut ResponseHeader) -> Result<()> {
        response_header
            .insert_header("Access-Control-Allow-Origin", "*")
            .map_err(|e| ProxyError::internal(format!("Failed to set CORS origin header: {}", e)))?;

        response_header
            .insert_header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
            .map_err(|e| ProxyError::internal(format!("Failed to set CORS methods header: {}", e)))?;

        response_header
            .insert_header("Access-Control-Allow-Headers", "Content-Type, Authorization, X-API-Key")
            .map_err(|e| ProxyError::internal(format!("Failed to set CORS headers header: {}", e)))?;

        Ok(())
    }

    /// 处理OPTIONS预检请求
    pub fn handle_preflight_request(&self) -> Result<ResponseHeader> {
        let mut response_header = ResponseHeader::build(200, None)
            .map_err(|e| ProxyError::internal(format!("Failed to build preflight response: {}", e)))?;

        self.add_cors_headers(&mut response_header)?;

        response_header
            .insert_header("Access-Control-Max-Age", "86400")
            .map_err(|e| ProxyError::internal(format!("Failed to set CORS max-age header: {}", e)))?;

        Ok(response_header)
    }

    /// 计算重试延迟
    pub fn calculate_retry_delay(&self, retry_count: u32) -> Duration {
        let base_delay = Duration::from_millis(100);
        let max_delay = Duration::from_secs(5);
        
        // 指数退避策略
        let multiplier = 2_u32.pow(retry_count.min(10));
        let delay = base_delay * multiplier;
        delay.min(max_delay)
    }

    /// 检查是否应该重试
    pub fn should_retry(&self, status_code: u16, retry_count: u32) -> bool {
        const MAX_RETRIES: u32 = 3;
        
        if retry_count >= MAX_RETRIES {
            return false;
        }

        // 只对某些状态码进行重试
        matches!(status_code, 
            429 | // Too Many Requests
            500 | // Internal Server Error
            502 | // Bad Gateway
            503 | // Service Unavailable
            504   // Gateway Timeout
        )
    }
}