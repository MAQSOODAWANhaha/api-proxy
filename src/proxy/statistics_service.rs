//! # 代理端统计服务
//!
//! 从RequestHandler中提取的统计相关逻辑，专门负责处理代理端的统计需求
//! 包括token使用统计、请求/响应详情收集、成本计算等功能

use anyhow::Result;
use pingora_http::ResponseHeader;
use pingora_proxy::Session;
use std::sync::Arc;

use crate::auth::AuthUtils;
use crate::error::ProxyError;
use crate::pricing::PricingCalculatorService;
use crate::proxy::ProxyContext;
use crate::providers::field_extractor::{ModelExtractor, TokenFieldExtractor};

// 重用request_handler中的类型，避免重复定义
pub use crate::proxy::request_handler::{
    TokenUsage, RequestDetails, ResponseDetails, 
    SerializableResponseDetails, DetailedRequestStats
};

/// 请求统计信息
#[derive(Debug, Clone)]
pub struct RequestStats {
    pub method: String,
    pub path: String,
    pub client_ip: String,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
}

/// 响应统计信息
#[derive(Debug, Clone)]
pub struct ResponseStats {
    pub status_code: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub content_type: Option<String>,
    pub content_length: Option<i64>,
}


/// 代理端统计服务
/// 
/// 专门处理代理请求的统计逻辑，从RequestHandler中分离出来
/// 包含统计数据收集、token使用分析、成本计算等功能
pub struct StatisticsService {
    /// 费用计算服务
    pricing_calculator: Arc<PricingCalculatorService>,
}

impl StatisticsService {
    /// 创建新的统计服务
    pub fn new(pricing_calculator: Arc<PricingCalculatorService>) -> Self {
        Self {
            pricing_calculator,
        }
    }
    
    /// 提取模型名称并初始化token使用信息
    pub async fn initialize_token_usage(&self, ctx: &mut ProxyContext) -> Result<TokenUsage, ProxyError> {
        // 提取模型名称（使用数据驱动的ModelExtractor）
        let model_used = self.extract_model_with_model_extractor(ctx).await;

        // 创建token使用信息
        let token_usage = TokenUsage {
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: 0,
            model_used: model_used.clone(),
        };
        
        Ok(token_usage)
    }

    /// 收集请求统计信息
    pub fn collect_request_stats(&self, session: &Session) -> RequestStats {
        // 将Pingora headers转换为标准HeaderMap以便使用AuthUtils
        let mut headers = axum::http::HeaderMap::new();
        for (name, value) in session.req_header().headers.iter() {
            if let Ok(header_name) = axum::http::HeaderName::from_bytes(name.as_str().as_bytes()) {
                if let Ok(header_value) = axum::http::HeaderValue::from_bytes(value.as_bytes()) {
                    headers.insert(header_name, header_value);
                }
            }
        }

        // 使用AuthUtils提取客户端信息
        let client_ip = AuthUtils::extract_real_client_ip(
            &headers,
            session.client_addr().map(|addr| addr.to_string()),
        );
        let user_agent = AuthUtils::extract_user_agent(&headers);
        let referer = AuthUtils::extract_referer(&headers);

        let req_header = session.req_header();
        RequestStats {
            method: req_header.method.to_string(),
            path: req_header.uri.path().to_string(),
            client_ip,
            user_agent,
            referer,
        }
    }

    /// 收集请求详情
    pub fn collect_request_details(&self, session: &Session, request_stats: &RequestStats) -> RequestDetails {
        let req_header = session.req_header();

        // 收集请求头
        let mut headers = std::collections::HashMap::new();
        for (name, value) in req_header.headers.iter() {
            if let Ok(value_str) = std::str::from_utf8(value.as_bytes()) {
                headers.insert(name.as_str().to_string(), value_str.to_string());
            }
        }

        // 获取Content-Type
        let content_type = req_header
            .headers
            .get("content-type")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .map(|s| s.to_string());

        // 获取Content-Length（请求体大小）
        let content_length = req_header
            .headers
            .get("content-length")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .and_then(|s| s.parse::<i64>().ok());

        // 获取协议版本
        let protocol_version = Some(format!("{:?}", req_header.version));

        // 构建请求详情
        RequestDetails {
            method: request_stats.method.clone(),
            path: request_stats.path.clone(),
            headers,
            body_size: content_length.map(|l| l as u64),
            content_type,
            client_ip: request_stats.client_ip.clone(),
            user_agent: request_stats.user_agent.clone(),
            referer: request_stats.referer.clone(),
            protocol_version,
        }
    }

    /// 收集响应统计信息
    pub fn collect_response_stats(&self, upstream_response: &ResponseHeader) -> ResponseStats {
        let status_code = upstream_response.status.as_u16();
        
        // 收集响应头
        let mut headers = std::collections::HashMap::new();
        for (name, value) in upstream_response.headers.iter() {
            if let Ok(value_str) = std::str::from_utf8(value.as_bytes()) {
                headers.insert(name.as_str().to_string(), value_str.to_string());
            }
        }

        // 获取Content-Type
        let content_type = upstream_response
            .headers
            .get("content-type")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .map(|s| s.to_string());

        // 获取Content-Length（响应体大小）
        let content_length = upstream_response
            .headers
            .get("content-length")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .and_then(|s| s.parse::<i64>().ok());

        ResponseStats {
            status_code,
            headers,
            content_type,
            content_length,
        }
    }
    
    /// 收集响应详情（兼容方法）
    pub fn collect_response_details(&self, upstream_response: &ResponseHeader, ctx: &mut ProxyContext) {
        // 收集响应头
        let mut headers = std::collections::HashMap::new();
        for (name, value) in upstream_response.headers.iter() {
            if let Ok(value_str) = std::str::from_utf8(value.as_bytes()) {
                headers.insert(name.as_str().to_string(), value_str.to_string());
            }
        }

        // 获取Content-Type
        let content_type = upstream_response
            .headers
            .get("content-type")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .map(|s| s.to_string());

        // 获取Content-Length（响应体大小）
        let body_size = upstream_response
            .headers
            .get("content-length")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .and_then(|s| s.parse::<u64>().ok());

        // 获取Content-Encoding
        let content_encoding = upstream_response
            .headers
            .get("content-encoding")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .map(|s| s.to_string());

        ctx.response_details = ResponseDetails {
            headers,
            body: None, // 响应体稍后在response body处理时收集
            body_size,
            content_type,
            content_encoding,
            body_chunks: Vec::new(), // 初始化为空的Vec
        };

        tracing::info!(
            request_id = %ctx.request_id,
            response_headers_count = ctx.response_details.headers.len(),
            content_type = ?ctx.response_details.content_type,
            content_encoding = ?ctx.response_details.content_encoding,
            body_size = ?ctx.response_details.body_size,
            "Collected response details"
        );
    }

    /// 提取模型信息（使用数据驱动的ModelExtractor）
    pub async fn extract_model_with_model_extractor(&self, ctx: &ProxyContext) -> Option<String> {
        // 获取provider_type以确定使用哪种模型映射
        let provider_type = ctx.provider_type.as_ref()?;

        // 检查是否配置了模型提取规则
        let model_extraction_json = provider_type.model_extraction_json.as_ref()?;

        // 创建ModelExtractor实例
        let model_extractor = match ModelExtractor::from_json_config(model_extraction_json) {
            Ok(extractor) => extractor,
            Err(e) => {
                tracing::error!(
                    request_id = %ctx.request_id,
                    provider = %provider_type.name,
                    error = %e,
                    "Failed to create ModelExtractor from database configuration"
                );
                return None;
            }
        };

        // 准备查询参数（如果有的话）
        let query_params = std::collections::HashMap::new(); // 简化实现，可以后续扩展
        
        // 准备请求体JSON（如果有的话）
        let request_body_json = None; // 简化实现，可以后续扩展

        // 使用ModelExtractor提取模型名称
        let model = model_extractor.extract_model_name(
            &ctx.request_details.path,
            request_body_json,
            &query_params,
        );

        // 检查是否成功提取到模型（空字符串表示未提取到）
        if !model.is_empty() {
            tracing::info!(
                request_id = %ctx.request_id,
                provider = %provider_type.name,
                extracted_model = %model,
                path = %ctx.request_details.path,
                "Successfully extracted model using ModelExtractor"
            );
            return Some(model);
        }

        tracing::debug!(
            request_id = %ctx.request_id,
            provider = %provider_type.name,
            "No model could be extracted using ModelExtractor"
        );

        None
    }

    /// 解析SSE(Server-Sent Events)格式的响应体
    /// 
    /// 改进：遍历所有 data: 行，优先返回包含 usageMetadata 的最后一条；
    /// 若未出现 usageMetadata，则回退为最后一条有效 JSON。
    fn parse_sse_response(&self, response_body: &str) -> Result<serde_json::Value, ProxyError> {
        let mut last_json: Option<serde_json::Value> = None;
        let mut last_with_usage: Option<serde_json::Value> = None;

        for line in response_body.lines() {
            let line = line.trim();
            if line.is_empty() || !line.starts_with("data: ") { continue; }

            let json_str = &line[6..];
            if json_str == "[DONE]" { break; }

            match serde_json::from_str::<serde_json::Value>(json_str) {
                Ok(json_value) => {
                    // 记录最后一条 JSON
                    last_json = Some(json_value.clone());

                    // 如包含 usageMetadata（Gemini/Google 常见字段），则记录为候选
                    if json_value.get("usageMetadata").is_some() {
                        last_with_usage = Some(json_value.clone());
                    }

                    tracing::debug!(
                        has_usage = %json_value.get("usageMetadata").is_some(),
                        json_preview = %if json_str.len() > 200 { format!("{}...", &json_str[..200]) } else { json_str.to_string() },
                        "Parsed SSE data line"
                    );
                }
                Err(e) => {
                    tracing::debug!(
                        line = %if json_str.len() > 200 { format!("{}...", &json_str[..200]) } else { json_str.to_string() },
                        error = %e,
                        "Failed to parse SSE data line as JSON, continuing"
                    );
                }
            }
        }

        if let Some(json) = last_with_usage { return Ok(json); }
        if let Some(json) = last_json { return Ok(json); }
        Err(ProxyError::internal("No valid JSON data found in SSE response"))
    }

    /// 检测响应体是否为SSE格式
    fn is_sse_format(&self, response_body: &str, content_type: Option<&str>) -> bool {
        // 检查Content-Type
        if let Some(ct) = content_type {
            if ct.contains("text/event-stream") {
                return true;
            }
        }
        
        // 检查响应体内容格式
        let first_few_lines: Vec<&str> = response_body.lines().take(3).collect();
        let has_data_prefix = first_few_lines.iter().any(|line| line.trim().starts_with("data: "));
        
        // 如果有多个"data: "开头的行，很可能是SSE格式
        let data_line_count = first_few_lines.iter()
            .filter(|line| line.trim().starts_with("data: "))
            .count();
        
        has_data_prefix && (data_line_count > 0 || content_type.map_or(false, |ct| ct.contains("event-stream")))
    }

    /// 从响应体JSON提取token使用信息（数据驱动版本，支持SSE格式）
    /// 
    /// 这个方法应该在响应体收集完成后调用，使用数据库配置的TokenFieldExtractor获取准确的token数据
    pub async fn extract_token_usage_from_response_body(
        &self,
        ctx: &mut ProxyContext,
    ) -> Result<TokenUsage, ProxyError> {
        // 确保响应体已经被收集和处理
        if ctx.response_details.body.is_none() {
            tracing::debug!(
                request_id = %ctx.request_id,
                "No response body available for token extraction"
            );
            return Ok(TokenUsage::default());
        }

        let response_body = ctx.response_details.body.as_ref().unwrap();

        // 获取provider_type以确定使用哪种token映射
        let provider_type = match ctx.provider_type.as_ref() {
            Some(provider) => provider,
            None => {
                tracing::warn!(
                    request_id = %ctx.request_id,
                    "No provider type available for token extraction"
                );
                return Ok(TokenUsage::default());
            }
        };

        // 检查是否配置了token映射
        let token_mappings_json = match &provider_type.token_mappings_json {
            Some(mappings) => mappings,
            None => {
                tracing::debug!(
                    request_id = %ctx.request_id,
                    provider = %provider_type.name,
                    "No token_mappings_json configured for this provider"
                );
                return Ok(TokenUsage::default());
            }
        };

        // 获取响应的Content-Type用于格式检测
        let content_type = ctx.response_details.content_type.as_deref();
        
        // 解析响应体为JSON - 支持SSE和传统流式响应
        let response_json: serde_json::Value = {
            // 首先检测是否为SSE格式
            if self.is_sse_format(response_body, content_type) {
                tracing::info!(
                    request_id = %ctx.request_id,
                    provider = %provider_type.name,
                    content_type = ?content_type,
                    "Detected SSE format response, using SSE parser"
                );
                
                match self.parse_sse_response(response_body) {
                    Ok(json) => json,
                    Err(e) => {
                        tracing::warn!(
                            request_id = %ctx.request_id,
                            provider = %provider_type.name,
                            error = %e,
                            body_preview = %if response_body.len() > 500 {
                                format!("{}...", &response_body[..500])
                            } else {
                                response_body.clone()
                            },
                            "Failed to parse SSE response body for token extraction"
                        );
                        return Ok(TokenUsage::default());
                    }
                }
            } else {
                // 传统流式响应处理：响应体可能是多个JSON对象的拼接
                let mut last_json = None;
                let stream =
                    serde_json::Deserializer::from_str(response_body).into_iter::<serde_json::Value>();

                for value_result in stream {
                    if let Ok(value) = value_result {
                        last_json = Some(value);
                    }
                }

                match last_json {
                    Some(json) => {
                        tracing::debug!(
                            request_id = %ctx.request_id,
                            provider = %provider_type.name,
                            "Successfully parsed traditional streaming response"
                        );
                        json
                    }
                    None => {
                        // 如果流式解析失败，尝试作为单个JSON解析（兼容非流式响应）
                        match serde_json::from_str(response_body) {
                            Ok(json) => {
                                tracing::debug!(
                                    request_id = %ctx.request_id,
                                    provider = %provider_type.name,
                                    "Successfully parsed single JSON response"
                                );
                                json
                            }
                            Err(e) => {
                                tracing::warn!(
                                    request_id = %ctx.request_id,
                                    provider = %provider_type.name,
                                    error = %e,
                                    body_preview = %if response_body.len() > 200 {
                                        format!("{}...", &response_body[..200])
                                    } else {
                                        response_body.clone()
                                    },
                                    "Failed to parse response body as JSON for token extraction"
                                );
                                return Ok(TokenUsage::default());
                            }
                        }
                    }
                }
            }
        };

        // 创建TokenFieldExtractor实例
        let token_extractor = match TokenFieldExtractor::from_json_config(token_mappings_json) {
            Ok(extractor) => extractor,
            Err(e) => {
                tracing::error!(
                    request_id = %ctx.request_id,
                    provider = %provider_type.name,
                    error = %e,
                    "Failed to create TokenFieldExtractor from database configuration"
                );
                return Ok(TokenUsage::default());
            }
        };

        // 使用TokenFieldExtractor从响应体JSON中提取token信息
        let mut prompt_tokens = token_extractor.extract_token_u32(&response_json, "tokens_prompt");
        let mut completion_tokens =
            token_extractor.extract_token_u32(&response_json, "tokens_completion");
        let mut total_tokens = token_extractor.extract_token_u32(&response_json, "tokens_total");

        // 额外容错：Gemini SSE 可能只提供 usageMetadata.totalTokenCount
        if total_tokens.is_none() {
            if let Some(usage) = response_json.get("usageMetadata") {
                if let Some(tt) = usage.get("totalTokenCount").and_then(|v| v.as_u64()) {
                    total_tokens = Some(tt as u32);
                }
                // 若缺失 completion，则用 total - prompt 估算（有 prompt 时）
                if completion_tokens.is_none() && prompt_tokens.is_some() {
                    if let Some(tt) = total_tokens {
                        let p = prompt_tokens.unwrap_or(0);
                        if tt >= p { completion_tokens = Some(tt - p); }
                    }
                }
                // 若缺失 prompt，则用 total - completion 估算（有 completion 时）
                if prompt_tokens.is_none() && completion_tokens.is_some() {
                    if let Some(tt) = total_tokens {
                        let c = completion_tokens.unwrap_or(0);
                        if tt >= c { prompt_tokens = Some(tt - c); }
                    }
                }
            }
        }

        let total_tokens = total_tokens.unwrap_or_else(|| match (prompt_tokens, completion_tokens) {
            (Some(p), Some(c)) => p + c,
            (Some(p), None) => p,
            (None, Some(c)) => c,
            (None, None) => 0,
        });

        // 提取模型信息（使用数据驱动的ModelExtractor）
        let model_used = self
            .extract_model_with_model_extractor(ctx)
            .await
            .or_else(|| ctx.token_usage.model_used.clone());

        let new_token_usage = TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            model_used: model_used.clone(),
        };

        tracing::info!(
            request_id = %ctx.request_id,
            provider = %provider_type.name,
            extracted_prompt_tokens = ?prompt_tokens,
            extracted_completion_tokens = ?completion_tokens,
            extracted_total_tokens = total_tokens,
            extracted_model = ?model_used,
            "Successfully extracted token usage using data-driven TokenFieldExtractor"
        );

        Ok(new_token_usage)
    }

    /// 从响应体JSON提取完整的统计信息（包括token、cost等）- 数据驱动版本
    ///
    /// 这个方法在响应体收集完成后调用，提取所有可用的统计数据用于追踪和记录
    pub async fn extract_stats_from_response_body(
        &self,
        ctx: &mut ProxyContext,
    ) -> Result<DetailedRequestStats, ProxyError> {
        // 确保响应体已经被收集和处理
        if ctx.response_details.body.is_none() {
            tracing::debug!(
                request_id = %ctx.request_id,
                "No response body available for stats extraction"
            );
            return Ok(DetailedRequestStats {
                input_tokens: None,
                output_tokens: None,
                total_tokens: Some(0),
                model_name: None,
                cache_create_tokens: None,
                cache_read_tokens: None,
                cost: None,
                cost_currency: None,
            });
        }

        let response_body = ctx.response_details.body.as_ref().unwrap();

        // 获取provider_type以确定使用哪种映射
        let provider_type = match ctx.provider_type.as_ref() {
            Some(provider) => provider,
            None => {
                tracing::warn!(
                    request_id = %ctx.request_id,
                    "No provider type available for stats extraction"
                );
                return Ok(DetailedRequestStats::default());
            }
        };

        // 检查是否配置了token映射
        let token_mappings_json = match &provider_type.token_mappings_json {
            Some(mappings) => mappings,
            None => {
                tracing::debug!(
                    request_id = %ctx.request_id,
                    provider = %provider_type.name,
                    "No token_mappings_json configured for this provider"
                );
                return Ok(DetailedRequestStats::default());
            }
        };

        // 获取响应的Content-Type用于格式检测
        let content_type = ctx.response_details.content_type.as_deref();
        
        // 解析响应体为JSON - 支持SSE和传统流式响应
        let response_json: serde_json::Value = {
            // 首先检测是否为SSE格式
            if self.is_sse_format(response_body, content_type) {
                tracing::info!(
                    request_id = %ctx.request_id,
                    provider = %provider_type.name,
                    content_type = ?content_type,
                    "Detected SSE format response for stats extraction, using SSE parser"
                );
                
                match self.parse_sse_response(response_body) {
                    Ok(json) => json,
                    Err(e) => {
                        tracing::warn!(
                            request_id = %ctx.request_id,
                            provider = %provider_type.name,
                            error = %e,
                            "Failed to parse SSE response body for stats extraction"
                        );
                        return Ok(DetailedRequestStats::default());
                    }
                }
            } else {
                // 传统流式响应处理
                let mut last_json = None;
                let stream =
                    serde_json::Deserializer::from_str(response_body).into_iter::<serde_json::Value>();

                for value_result in stream {
                    if let Ok(value) = value_result {
                        last_json = Some(value);
                    }
                }

                match last_json {
                    Some(json) => json,
                    None => {
                        match serde_json::from_str(response_body) {
                            Ok(json) => json,
                            Err(e) => {
                                tracing::warn!(
                                    request_id = %ctx.request_id,
                                    provider = %provider_type.name,
                                    error = %e,
                                    "Failed to parse response body as JSON for stats extraction"
                                );
                                return Ok(DetailedRequestStats::default());
                            }
                        }
                    }
                }
            }
        };

        // 创建TokenFieldExtractor实例
        let token_extractor = match TokenFieldExtractor::from_json_config(token_mappings_json) {
            Ok(extractor) => extractor,
            Err(e) => {
                tracing::error!(
                    request_id = %ctx.request_id,
                    provider = %provider_type.name,
                    error = %e,
                    "Failed to create TokenFieldExtractor for stats extraction"
                );
                return Ok(DetailedRequestStats::default());
            }
        };

        // 提取基础的token信息
        let mut input_tokens = token_extractor.extract_token_u32(&response_json, "tokens_prompt");
        let mut output_tokens = token_extractor.extract_token_u32(&response_json, "tokens_completion");
        let mut total_tokens = token_extractor.extract_token_u32(&response_json, "tokens_total");

        // 额外容错：Gemini SSE 仅有 totalTokenCount 的情况
        if total_tokens.is_none() {
            if let Some(usage) = response_json.get("usageMetadata") {
                if let Some(tt) = usage.get("totalTokenCount").and_then(|v| v.as_u64()) {
                    total_tokens = Some(tt as u32);
                }
                if output_tokens.is_none() && input_tokens.is_some() {
                    if let Some(tt) = total_tokens { let p = input_tokens.unwrap_or(0); if tt >= p { output_tokens = Some(tt - p); } }
                }
                if input_tokens.is_none() && output_tokens.is_some() {
                    if let Some(tt) = total_tokens { let c = output_tokens.unwrap_or(0); if tt >= c { input_tokens = Some(tt - c); } }
                }
            }
        }

        // 提取缓存相关的token信息（如果配置了的话）
        let cache_create_tokens = token_extractor.extract_token_u32(&response_json, "cache_creation_input_tokens");
        let cache_read_tokens = token_extractor.extract_token_u32(&response_json, "cache_read_input_tokens");

        // 提取模型信息
        let model_name = self.extract_model_with_model_extractor(ctx).await;

        // 计算成本（如果有token信息的话）
        let (cost, cost_currency) = if let Some(model) = model_name.as_ref() {
            // 创建pricing模块需要的TokenUsage结构
            let pricing_token_usage = crate::pricing::TokenUsage {
                prompt_tokens: input_tokens,
                completion_tokens: output_tokens,
                cache_create_tokens,
                cache_read_tokens,
            };

            match self.pricing_calculator.calculate_cost(
                model,
                provider_type.id,
                &pricing_token_usage,
                &ctx.request_id
            ).await {
                Ok(cost_info) => (Some(cost_info.total_cost), Some(cost_info.currency)),
                Err(e) => {
                    tracing::warn!(
                        request_id = %ctx.request_id,
                        provider = %provider_type.name,
                        model = %model,
                        error = %e,
                        "Failed to calculate cost for request"
                    );
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        let stats = DetailedRequestStats {
            input_tokens,
            output_tokens,
            total_tokens,
            model_name: model_name.clone(),
            cache_create_tokens,
            cache_read_tokens,
            cost,
            cost_currency,
        };

        tracing::info!(
            request_id = %ctx.request_id,
            provider = %provider_type.name,
            input_tokens = ?stats.input_tokens,
            output_tokens = ?stats.output_tokens,
            total_tokens = ?stats.total_tokens,
            model_name = ?stats.model_name,
            cache_create_tokens = ?stats.cache_create_tokens,
            cache_read_tokens = ?stats.cache_read_tokens,
            cost = ?stats.cost,
            "Successfully extracted detailed request statistics"
        );

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_usage_default() {
        let usage = TokenUsage::default();
        assert!(usage.prompt_tokens.is_none());
        assert!(usage.completion_tokens.is_none());
        assert_eq!(usage.total_tokens, 0);
        assert!(usage.model_used.is_none());
    }

    #[test]
    fn test_response_details_add_chunk() {
        let mut details = ResponseDetails::default();
        details.add_body_chunk(b"test");
        assert_eq!(details.body_chunks, b"test");
    }

    #[test]
    fn test_response_details_finalize_body() {
        let mut details = ResponseDetails::default();
        details.add_body_chunk(b"hello world");
        details.finalize_body();
        
        assert_eq!(details.body.as_ref().unwrap(), "hello world");
        assert_eq!(details.body_size, Some(11));
    }

    #[test]
    fn test_serializable_response_details_conversion() {
        let details = ResponseDetails {
            headers: [("content-type".to_string(), "application/json".to_string())].into(),
            body: Some("test".to_string()),
            body_size: Some(4),
            content_type: Some("application/json".to_string()),
            content_encoding: None,
            body_chunks: vec![116, 101, 115, 116], // "test" as bytes
        };

        let serializable: SerializableResponseDetails = (&details).into();
        assert_eq!(serializable.body, Some("test".to_string()));
        assert_eq!(serializable.body_size, Some(4));
        assert_eq!(serializable.content_type, Some("application/json".to_string()));
    }
}
