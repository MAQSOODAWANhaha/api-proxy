//! # 代理端统计服务（迁移）
//!
//! 从 `src/proxy/statistics_service.rs` 迁移至此，作为统计模块对外服务。

use anyhow::Result;
use pingora_http::ResponseHeader;
use pingora_proxy::Session;
use std::collections::HashMap;
use std::sync::Arc;
use url::form_urlencoded;

use crate::auth::AuthUtils;
use crate::error::ProxyError;
use crate::pricing::PricingCalculatorService;
use crate::proxy::ProxyContext;
// 提取器在此处不直接依赖，避免强耦合；如需高级解析可在 providers 层使用。

// 重用request_handler中的类型，避免重复定义
pub use crate::proxy::request_handler::{
    DetailedRequestStats, RequestDetails, ResponseDetails, SerializableResponseDetails, TokenUsage,
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
pub struct StatisticsService {
    /// 费用计算服务
    pricing_calculator: Arc<PricingCalculatorService>,
}

impl StatisticsService {
    /// 在任意层级查找并归一化 usageMetadata 到顶层，便于通用提取器工作

    /// 创建新的统计服务
    pub fn new(pricing_calculator: Arc<PricingCalculatorService>) -> Self {
        Self { pricing_calculator }
    }

    /// 直接根据给定的 TokenUsage 计算成本（用于 SSE 聚合覆盖后重算）
    pub async fn calculate_cost_direct(
        &self,
        model: &str,
        provider_type_id: i32,
        usage: &TokenUsage,
        request_id: &str,
    ) -> Result<(Option<f64>, Option<String>), ProxyError> {
        let pricing_usage = crate::pricing::TokenUsage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            cache_create_tokens: None,
            cache_read_tokens: None,
        };
        match self
            .pricing_calculator
            .calculate_cost(model, provider_type_id, &pricing_usage, request_id)
            .await
        {
            Ok(cost) => Ok((Some(cost.total_cost), Some(cost.currency))),
            Err(e) => {
                tracing::warn!(request_id = %request_id, error = %e, "Failed to calculate cost (direct)");
                Ok((None, None))
            }
        }
    }

    /// 提取模型名称并初始化token使用信息
    pub async fn initialize_token_usage(
        &self,
        ctx: &mut ProxyContext,
    ) -> Result<TokenUsage, ProxyError> {
        // 直接使用请求时提取的模型信息
        let model_used = ctx.requested_model.clone();

        // 记录使用的模型信息用于调试
        if let Some(model) = &model_used {
            tracing::debug!(
                request_id = ctx.request_id,
                model = model,
                "Using requested model for token usage initialization"
            );
        } else {
            tracing::debug!(
                request_id = ctx.request_id,
                "No model information available for token usage initialization"
            );
        }

        // 创建token使用信息
        let token_usage = TokenUsage {
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: 0,
            model_used,
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
    pub fn collect_request_details(
        &self,
        session: &Session,
        request_stats: &RequestStats,
    ) -> RequestDetails {
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

        // 获取Content-Length
        let body_size = req_header
            .headers
            .get("content-length")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .and_then(|s| s.parse::<u64>().ok());

        RequestDetails {
            headers,
            body_size,
            content_type,
            client_ip: request_stats.client_ip.clone(),
            user_agent: request_stats.user_agent.clone(),
            referer: request_stats.referer.clone(),
            method: request_stats.method.clone(),
            path: request_stats.path.clone(),
            protocol_version: Some("HTTP/1.1".to_string()),
        }
    }

    /// 收集响应详情
    pub fn collect_response_details(
        &self,
        upstream_response: &ResponseHeader,
        ctx: &mut ProxyContext,
    ) -> ResponseStats {
        // 收集响应头
        let mut headers = std::collections::HashMap::new();
        for (name, value) in upstream_response.headers.iter() {
            if let Ok(value_str) = std::str::from_utf8(value.as_bytes()) {
                headers.insert(name.as_str().to_string(), value_str.to_string());
            }
        }

        // 获取Content-Type和Content-Length
        let content_type = upstream_response
            .headers
            .get("content-type")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .map(|s| s.to_string());

        let content_length = upstream_response
            .headers
            .get("content-length")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .and_then(|s| s.parse::<i64>().ok());

        // 更新上下文中的响应详情元数据
        if let Some(ct) = &content_type {
            ctx.response_details.content_type = Some(ct.clone());
        }

        ResponseStats {
            status_code: upstream_response.status.as_u16(),
            headers,
            content_type,
            content_length,
        }
    }

    /// 从 JSON 响应中提取 token 统计（通用提取器）

    /// 从响应体中提取模型信息
    ///
    /// 支持多种AI API响应格式的模型提取，包括数组索引访问
    pub fn extract_model_from_response_body(&self, json: &serde_json::Value) -> Option<String> {
        // 定义支持的模型字段路径，按优先级排序
        let model_paths = [
            "model",           // OpenAI格式
            "modelName",       // 通用格式
            "model_id",        // 通用格式
            "data.0.model",    // 数组访问格式
            "choices.0.model", // 数组访问格式
            "candidates.0.model", // 数组访问格式
            "response.model",  // 嵌套格式
            "result.model",    // 嵌套格式
        ];

        // 按优先级尝试提取模型信息
        for path in &model_paths {
            if let Some(model) = self.extract_model_by_path(json, path) {
                tracing::debug!(
                    model = %model,
                    path = %path,
                    "Model extracted from response"
                );
                return Some(model);
            }
        }

        tracing::debug!("No model found in response using any supported path");
        None
    }

    /// 根据路径提取模型信息
    fn extract_model_by_path(&self, json: &serde_json::Value, path: &str) -> Option<String> {
        if path.is_empty() {
            return None;
        }

        // 分割路径并遍历JSON结构
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = json;

        for part in parts {
            // 检查是否是数组索引访问，如 choices[0]
            if let Some((field_name, index_str)) = self.parse_array_access(part) {
                // 处理 field_name[index] 格式
                if let Some(array_field) = current.get(field_name) {
                    if let Some(array) = array_field.as_array() {
                        if let Ok(index) = index_str.parse::<usize>() {
                            if let Some(element) = array.get(index) {
                                current = element;
                                continue;
                            } else {
                                tracing::debug!(
                                    path = %path,
                                    field_name = %field_name,
                                    index = %index_str,
                                    array_len = array.len(),
                                    "Array index out of bounds"
                                );
                                return None;
                            }
                        } else {
                            tracing::debug!(
                                path = %path,
                                field_name = %field_name,
                                index = %index_str,
                                "Invalid array index format"
                            );
                            return None;
                        }
                    } else {
                        tracing::debug!(
                            path = %path,
                            field_name = %field_name,
                            "Field is not an array"
                        );
                        return None;
                    }
                } else {
                    tracing::debug!(
                        path = %path,
                        field_name = %field_name,
                        "Field not found"
                    );
                    return None;
                }
            } else if part.chars().all(|c| c.is_ascii_digit()) {
                // 处理直接的数字索引，如 data.0.model 中的 "0"
                if let Some(array) = current.as_array() {
                    if let Ok(index) = part.parse::<usize>() {
                        if let Some(element) = array.get(index) {
                            current = element;
                            continue;
                        } else {
                            tracing::debug!(
                                path = %path,
                                index = %part,
                                array_len = array.len(),
                                "Direct array index out of bounds"
                            );
                            return None;
                        }
                    } else {
                        tracing::debug!(
                            path = %path,
                            index = %part,
                            "Invalid direct array index format"
                        );
                        return None;
                    }
                } else {
                    tracing::debug!(
                        path = %path,
                        index = %part,
                        "Current value is not an array for direct indexing"
                    );
                    return None;
                }
            } else {
                // 处理普通字段
                if let Some(next) = current.get(part) {
                    current = next;
                } else {
                    tracing::debug!(
                        path = %path,
                        part = %part,
                        "Path segment not found"
                    );
                    return None;
                }
            }
        }

        // 提取字符串值
        if let Some(model_str) = current.as_str() {
            let model = model_str.trim();
            if !model.is_empty() {
                return Some(model.to_string());
            }
        }

        tracing::debug!(
            path = %path,
            final_value = %current,
            "Model value is not a valid string"
        );
        None
    }

    /// 解析数组索引访问，如 choices[0] -> (choices, 0) 或 0 -> None
    fn parse_array_access<'a>(&self, part: &'a str) -> Option<(&'a str, &'a str)> {
        // 首先检查是否是直接的数字索引，如 "0"
        if part.chars().all(|c| c.is_ascii_digit()) {
            return None; // 直接数字索引，由调用者处理
        }

        // 检查是否是 field[index] 格式
        if let Some(bracket_pos) = part.find('[') {
            if let Some(close_bracket_pos) = part.find(']') {
                if bracket_pos < close_bracket_pos {
                    let field_name = &part[..bracket_pos];
                    let index_str = &part[bracket_pos + 1..close_bracket_pos];
                    return Some((field_name, index_str));
                }
            }
        }
        None
    }

    /// 从上下文响应体中提取统计信息（统一实现）
    pub async fn extract_detailed_stats_from_response(
        &self,
        ctx: &mut ProxyContext,
    ) -> Result<DetailedRequestStats, ProxyError> {
        let mut stats = DetailedRequestStats::default();

        // 如果没有响应体或JSON解析失败，直接使用请求时的模型信息
        let body = match ctx.response_details.body.as_ref() {
            Some(b) => b,
            None => {
                // 没有响应体，直接使用请求时的模型信息
                if let Some(requested_model) = &ctx.requested_model {
                    stats.model_name = Some(requested_model.clone());
                    tracing::debug!(
                        request_id = %ctx.request_id,
                        requested_model = %requested_model,
                        "No response body, using requested model info"
                    );
                }
                return Ok(stats);
            }
        };

        // 尝试解析 JSON
        let parsed: serde_json::Value = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(_) => {
                // JSON解析失败，直接使用请求时的模型信息
                if let Some(requested_model) = &ctx.requested_model {
                    stats.model_name = Some(requested_model.clone());
                    tracing::debug!(
                        request_id = %ctx.request_id,
                        requested_model = %requested_model,
                        "Invalid JSON response, using requested model info"
                    );
                }
                return Ok(stats);
            }
        };

        // 直接使用原始解析的JSON，不再进行格式转换
        let normalized = parsed;

        // === 响应时模型信息提取逻辑 ===
        // 在响应处理阶段，尝试从响应体中提取更准确的模型信息
        let response_model = self.extract_model_from_response_body(&normalized);

        // 如果从响应中提取到模型信息，并且与请求时的模型信息不同，则更新追踪记录
        if let Some(ref response_model_name) = response_model {
            // 检查是否与请求时的模型信息不同或请求时没有模型信息
            let should_update = ctx.requested_model.as_ref().map_or(true, |requested| {
                requested != response_model_name || requested.is_empty()
            });

            if should_update {
                tracing::info!(
                    request_id = %ctx.request_id,
                    requested_model = ?ctx.requested_model,
                    response_model = %response_model_name,
                    "响应时检测到模型信息变化，准备更新追踪记录"
                );

                // 注意：这里我们不能直接调用 tracing_service.update_trace_model_info，
                // 因为 statistics service 不应该直接依赖 tracing service。
                // 我们将模型信息存储在 stats 中，由调用者决定是否更新追踪记录。
                stats.model_name = Some(response_model_name.clone());

                // 同时更新上下文中的请求模型信息，确保后续处理使用最新的模型信息
                ctx.requested_model = Some(response_model_name.clone());

                tracing::debug!(
                    request_id = %ctx.request_id,
                    updated_model = %response_model_name,
                    "模型信息已更新到上下文和统计信息中"
                );
            }
        }

        // 若 provider 配置了 token_mappings_json，则用数据库映射优先提取
        if let Some(provider) = ctx.provider_type.as_ref() {
            if let Some(mapping_json) = provider.token_mappings_json.as_ref() {
                if let Ok(cfg) =
                    crate::providers::field_extractor::TokenMappingConfig::from_json(mapping_json)
                {
                    let extractor =
                        crate::providers::field_extractor::TokenFieldExtractor::new(cfg);
                    stats.input_tokens = extractor.extract_token_u32(&normalized, "tokens_prompt");
                    stats.output_tokens =
                        extractor.extract_token_u32(&normalized, "tokens_completion");
                    // total: 优先映射，如果没有，则根据 prompt+completion 回退
                    stats.total_tokens = extractor
                        .extract_token_u32(&normalized, "tokens_total")
                        .or_else(|| match (stats.input_tokens, stats.output_tokens) {
                            (Some(p), Some(c)) => Some(p + c),
                            (Some(p), None) => Some(p),
                            (None, Some(c)) => Some(c),
                            _ => None,
                        });
                    // 可选缓存字段
                    stats.cache_create_tokens =
                        extractor.extract_token_u32(&normalized, "cache_create_tokens");
                    stats.cache_read_tokens =
                        extractor.extract_token_u32(&normalized, "cache_read_tokens");
                }
            }
        }

        // 若数据库未配置或未提取到token字段，则设为0值
        if stats.input_tokens.is_none() {
            stats.input_tokens = Some(0);
        }
        if stats.output_tokens.is_none() {
            stats.output_tokens = Some(0);
        }
        if stats.total_tokens.is_none() {
            stats.total_tokens = Some(0);
        }

        // 同步模型名（优先使用请求时已提取的模型信息）
        // 请求时的模型信息通常是最准确的，但如果响应中提取到了更准确的模型信息，则保持响应提取的结果
        if let Some(requested_model) = &ctx.requested_model {
            // 只有在stats中没有模型信息或者模型信息为空时，才使用请求时的模型信息
            if stats.model_name.is_none()
                || stats
                    .model_name
                    .as_ref()
                    .map(|m| m.is_empty())
                    .unwrap_or(false)
            {
                stats.model_name = Some(requested_model.clone());
                tracing::debug!(
                    request_id = %ctx.request_id,
                    requested_model = %requested_model,
                    previous_model = ?stats.model_name,
                    "模型信息同步：使用请求时提取的模型信息（覆盖响应提取）"
                );
            } else {
                // 如果stats中已经有模型信息（来自响应提取），验证是否与请求时一致
                if let Some(ref response_model) = stats.model_name {
                    if requested_model != response_model {
                        tracing::info!(
                            request_id = %ctx.request_id,
                            requested_model = %requested_model,
                            response_model = %response_model,
                            "模型信息不一致：请求时的模型与响应中的模型不同，保持响应提取的结果"
                        );
                    }
                }
            }
        }

        // 记录模型提取结果用于调试
        if let Some(requested) = &ctx.requested_model {
            tracing::debug!(
                request_id = ctx.request_id,
                requested_model = requested,
                response_model = stats.model_name.as_deref().unwrap_or("unknown"),
                "Model extraction: requested vs response"
            );
        }

        // 若存在模型与provider，计算费用
        tracing::debug!(
            request_id = %ctx.request_id,
            model_name = ?stats.model_name,
            provider_id = ?ctx.provider_type.as_ref().map(|p| p.id),
            input_tokens = ?stats.input_tokens,
            output_tokens = ?stats.output_tokens,
            "费用计算条件检查"
        );

        if let (Some(model), Some(provider)) =
            (stats.model_name.clone(), ctx.provider_type.as_ref())
        {
            tracing::info!(
                request_id = %ctx.request_id,
                model = %model,
                provider_id = provider.id,
                "开始计算费用"
            );

            let pricing_usage = crate::pricing::TokenUsage {
                prompt_tokens: stats.input_tokens,
                completion_tokens: stats.output_tokens,
                cache_create_tokens: None,
                cache_read_tokens: None,
            };
            match self
                .pricing_calculator
                .calculate_cost(&model, provider.id, &pricing_usage, &ctx.request_id)
                .await
            {
                Ok(cost) => {
                    let total_cost = cost.total_cost;
                    let currency = cost.currency.clone();
                    stats.cost = Some(total_cost);
                    stats.cost_currency = Some(currency.clone());
                    tracing::info!(
                        request_id = %ctx.request_id,
                        total_cost = total_cost,
                        currency = %currency,
                        "费用计算成功"
                    );
                }
                Err(e) => {
                    tracing::warn!(request_id = %ctx.request_id, error = %e, "Failed to calc cost");
                }
            }
        } else {
            tracing::warn!(
                request_id = %ctx.request_id,
                model_name = ?stats.model_name,
                provider_available = ?ctx.provider_type.as_ref().is_some(),
                "费用计算条件不满足：缺少模型信息或提供商信息"
            );
        }

        Ok(stats)
    }

    // ========== 请求阶段模型提取方法 ==========

    /// 统一的请求阶段模型提取入口点
    pub fn extract_model_from_request(
        &self,
        session: &Session,
        ctx: &ProxyContext,
    ) -> Option<String> {
        tracing::debug!(
            request_id = ctx.request_id,
            "Starting model extraction from request"
        );

        // 尝试使用数据库驱动的 ModelExtractor
        if let Some(model_name) = self.extract_model_from_request_with_db_config(session, ctx) {
            tracing::info!(
                request_id = ctx.request_id,
                model = model_name,
                extraction_method = "database_extractor",
                "Model extracted successfully using database configuration"
            );
            return Some(model_name);
        }

        // 回退：直接从请求体提取
        if let Some(model_name) = self.extract_model_from_request_body_fallback(ctx) {
            tracing::info!(
                request_id = ctx.request_id,
                model = model_name,
                extraction_method = "request_body_fallback",
                "Model extracted from request body (fallback method)"
            );
            return Some(model_name);
        }

        tracing::debug!(
            request_id = ctx.request_id,
            "No model found in request using any method"
        );
        None
    }

    /// 尝试使用数据库驱动的 ModelExtractor 提取模型
    pub fn extract_model_from_request_with_db_config(
        &self,
        session: &Session,
        ctx: &ProxyContext,
    ) -> Option<String> {
        let provider = ctx.provider_type.as_ref()?;
        tracing::debug!(
            request_id = ctx.request_id,
            provider_id = provider.id,
            provider_name = provider.name,
            has_model_extraction_config = provider.model_extraction_json.is_some(),
            "Checking provider model extraction configuration"
        );

        let model_extraction_json = provider.model_extraction_json.as_ref()?;

        tracing::debug!(
            request_id = ctx.request_id,
            provider_id = provider.id,
            "Creating ModelExtractor from database configuration"
        );

        let extractor = crate::providers::field_extractor::ModelExtractor::from_json_config(
            model_extraction_json,
        )
        .map_err(|e| {
            tracing::warn!(
                request_id = ctx.request_id,
                provider_id = provider.id,
                error = %e,
                "Failed to create ModelExtractor from database config"
            );
            e
        })
        .ok()?;

        let body_json = self.parse_request_body_for_model(ctx);
        let query_params = self.extract_query_params_for_model(session);
        let url_path = session.req_header().uri.path();

        tracing::debug!(
            request_id = ctx.request_id,
            url_path = url_path,
            has_body_json = body_json.is_some(),
            query_params_count = query_params.len(),
            "Extracting model with configured extractor"
        );

        let model_name = extractor.extract_model_name(url_path, body_json.as_ref(), &query_params);

        tracing::info!(
            request_id = ctx.request_id,
            model = model_name,
            extraction_method = "database_driven",
            provider_id = provider.id,
            "Extracted model using database-configured ModelExtractor"
        );

        Some(model_name)
    }

    /// 尝试从请求体直接提取模型（fallback方法）
    pub fn extract_model_from_request_body_fallback(
        &self,
        ctx: &ProxyContext,
    ) -> Option<String> {
        tracing::debug!(
            request_id = ctx.request_id,
            body_size = ctx.body.len(),
            "Attempting to extract model from request body"
        );

        if ctx.body.is_empty() {
            tracing::debug!(
                request_id = ctx.request_id,
                "Request body is empty, skipping body extraction"
            );
            return None;
        }

        let body_str = std::str::from_utf8(&ctx.body)
            .map_err(|e| {
                tracing::debug!(
                    request_id = ctx.request_id,
                    error = %e,
                    "Request body is not valid UTF-8"
                );
                e
            })
            .ok()?;

        tracing::debug!(
            request_id = ctx.request_id,
            body_length = body_str.len(),
            "Request body parsed as UTF-8 successfully"
        );

        let json_value = serde_json::from_str::<serde_json::Value>(body_str)
            .map_err(|e| {
                tracing::debug!(
                    request_id = ctx.request_id,
                    error = %e,
                    "Failed to parse request body as JSON"
                );
                e
            })
            .ok()?;

        let model_name = json_value
            .get("model")
            .and_then(|m| m.as_str())
            .map(|s| s.to_string());

        match model_name {
            Some(ref model) => {
                tracing::info!(
                    request_id = ctx.request_id,
                    model = model,
                    extraction_method = "fallback_direct",
                    "Extracted model from request body (fallback)"
                );
            }
            None => {
                tracing::debug!(
                    request_id = ctx.request_id,
                    "No model field found in request body JSON"
                );
            }
        }

        model_name
    }

    /// 解析请求体用于模型提取
    pub fn parse_request_body_for_model(
        &self,
        ctx: &ProxyContext,
    ) -> Option<serde_json::Value> {
        if ctx.body.is_empty() {
            return None;
        }

        let body_str = std::str::from_utf8(&ctx.body)
            .map_err(|e| {
                tracing::debug!(
                    request_id = ctx.request_id,
                    error = %e,
                    "Request body is not valid UTF-8"
                );
                e
            })
            .ok()?;

        serde_json::from_str::<serde_json::Value>(body_str)
            .map_err(|e| {
                tracing::debug!(
                    request_id = ctx.request_id,
                    error = %e,
                    "Failed to parse request body as JSON"
                );
                e
            })
            .ok()
    }

    /// 提取查询参数用于模型提取
    pub fn extract_query_params_for_model(
        &self,
        session: &Session,
    ) -> HashMap<String, String> {
        let uri = &session.req_header().uri;
        let query_string = uri.query().unwrap_or("");

        if query_string.is_empty() {
            return HashMap::new();
        }

        form_urlencoded::parse(query_string.as_bytes())
            .into_owned()
            .collect()
    }

    /// 最终统计入口（统一流式与非流式）：
    /// 1) 若未完成收集，先执行 finalize_body
    /// 2) 流式：先标准化为等价的非流式 JSON，再提取
    /// 3) 非流式：直接从 body JSON 提取
    /// 4) 若存在 sse_usage_agg，作为兜底覆盖（latest-wins）并在具备模型/Provider时重算费用
    pub async fn finalize_and_extract_stats(
        &self,
        ctx: &mut ProxyContext,
    ) -> Result<DetailedRequestStats, ProxyError> {
        if ctx.response_details.body.is_none() {
            ctx.response_details.finalize_body();
        }

        // 尝试将流式响应规范化为等价非流式 JSON
        if ctx.response_details.is_sse_format() {
            if let Some(json) = self.normalize_streaming_json(ctx) {
                let mut stats = self.extract_stats_from_json(ctx, &json).await?;
                // 兜底覆盖：如先前增量聚合到 ctx.sse_usage_agg，则以 latest-wins 更新
                if let Some(agg) = ctx.sse_usage_agg.clone() {
                    stats.input_tokens = agg.prompt_tokens.or(stats.input_tokens);
                    stats.output_tokens = agg.completion_tokens.or(stats.output_tokens);
                    stats.total_tokens = Some(
                        agg.total_tokens
                            .or_else(|| match (agg.prompt_tokens, agg.completion_tokens) {
                                (Some(p), Some(c)) => Some(p + c),
                                (Some(p), None) => Some(p),
                                (None, Some(c)) => Some(c),
                                (None, None) => stats.total_tokens,
                            })
                            .unwrap_or(0),
                    );
                }
                return Ok(stats);
            }
        }

        // 非流式或无法标准化：直接从响应体解析
        self.extract_detailed_stats_from_response(ctx).await
    }
}

/// 流式场景：从 SSE/stream 分块中尝试提取 usage 统计并合并到上下文聚合
/// 注意：此函数不依赖实例，可直接在流式响应阶段调用
impl StatisticsService {
    /// 将流式响应标准化为一个等价的非流式 JSON（取最后一个有效 JSON 对象）
    pub fn normalize_streaming_json(&self, ctx: &mut ProxyContext) -> Option<serde_json::Value> {
        // 确保已经完成收集
        if ctx.response_details.body.is_none() {
            ctx.response_details.finalize_body();
        }
        let body = ctx.response_details.body.as_ref()?;
        // 尝试从按行的 SSE 中提取最后一个 data: {json}
        let mut last_json: Option<serde_json::Value> = None;
        for line in body.lines() {
            let s = line.trim_start_matches("data: ").trim();
            if let Some(start) = s.find('{') {
                // 贪婪到行末（容错：简单匹配）
                let candidate = &s[start..];
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(candidate) {
                    last_json = Some(v);
                }
            } else if s.starts_with('{') {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
                    last_json = Some(v);
                }
            }
        }
        last_json
    }

    /// 使用 provider 的 token_mappings_json（经 TokenFieldExtractor）从 JSON 提取统计
    pub async fn extract_stats_from_json(
        &self,
        ctx: &mut ProxyContext,
        json: &serde_json::Value,
    ) -> Result<DetailedRequestStats, ProxyError> {
        let mut stats = DetailedRequestStats::default();

        // 模型信息（复用现有响应模型提取）
        stats.model_name = self.extract_model_from_response_body(json);

        // 基于 provider token_mappings_json 的数据驱动提取
        if let Some(provider) = ctx.provider_type.as_ref() {
            if let Some(mapping_json) = provider.token_mappings_json.as_ref() {
                if let Ok(cfg) = crate::providers::field_extractor::TokenMappingConfig::from_json(mapping_json) {
                    let extractor = crate::providers::field_extractor::TokenFieldExtractor::new(cfg);
                    stats.input_tokens = extractor.extract_token_u32(json, "tokens_prompt");
                    stats.output_tokens = extractor.extract_token_u32(json, "tokens_completion");
                    stats.total_tokens = extractor.extract_token_u32(json, "tokens_total").or_else(|| match (stats.input_tokens, stats.output_tokens) {
                        (Some(p), Some(c)) => Some(p + c),
                        (Some(p), None) => Some(p),
                        (None, Some(c)) => Some(c),
                        _ => None,
                    });
                    stats.cache_create_tokens = extractor.extract_token_u32(json, "cache_create_tokens");
                    stats.cache_read_tokens = extractor.extract_token_u32(json, "cache_read_tokens");
                }
            }
        }

        // 默认值兜底
        if stats.input_tokens.is_none() { stats.input_tokens = Some(0); }
        if stats.output_tokens.is_none() { stats.output_tokens = Some(0); }
        if stats.total_tokens.is_none() { stats.total_tokens = Some(0); }

        // 费用计算（有模型与 provider 才计算）
        if let (Some(model), Some(provider)) = (stats.model_name.clone(), ctx.provider_type.as_ref()) {
            let usage_now = TokenUsage {
                prompt_tokens: stats.input_tokens,
                completion_tokens: stats.output_tokens,
                total_tokens: stats.total_tokens.unwrap_or(0),
                model_used: Some(model.clone()),
            };
            if let Ok((cost, currency)) = self
                .calculate_cost_direct(&model, provider.id, &usage_now, &ctx.request_id)
                .await
            {
                stats.cost = cost;
                stats.cost_currency = currency;
            }
        }

        Ok(stats)
    }
}

// 移除了已删除功能的测试函数
