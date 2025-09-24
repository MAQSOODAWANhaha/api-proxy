//! # 代理端统计服务（迁移）
//!
//! 从 `src/proxy/statistics_service.rs` 迁移至此，作为统计模块对外服务。

use anyhow::Result;
use pingora_http::ResponseHeader;
use pingora_proxy::Session;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use url::form_urlencoded;

use crate::auth::AuthUtils;
use crate::error::ProxyError;
use crate::pricing::PricingCalculatorService;
use crate::proxy::ProxyContext;
// 提取器在此处不直接依赖，避免强耦合；如需高级解析可在 providers 层使用。

// 重用request_handler中的类型，避免重复定义
pub use crate::proxy::request_handler::{RequestDetails, ResponseDetails};
use crate::statistics::types::ComputedStats;

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
        usage: &crate::pricing::TokenUsage,
        request_id: &str,
    ) -> Result<(Option<f64>, Option<String>), ProxyError> {
        match self
            .pricing_calculator
            .calculate_cost(model, provider_type_id, usage, request_id)
            .await
        {
            Ok(cost) => Ok((Some(cost.total_cost), Some(cost.currency))),
            Err(e) => {
                tracing::warn!(request_id = %request_id, error = %e, "Failed to calculate cost (direct)");
                Ok((None, None))
            }
        }
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
            "model",              // OpenAI格式
            "modelName",          // 通用格式
            "model_id",           // 通用格式
            "data.0.model",       // 数组访问格式
            "choices.0.model",    // 数组访问格式
            "candidates.0.model", // 数组访问格式
            "response.model",     // 嵌套格式
            "result.model",       // 嵌套格式
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
    #[allow(dead_code)]
    pub async fn extract_detailed_stats_from_response(
        &self,
        ctx: &mut ProxyContext,
    ) -> Result<ComputedStats, ProxyError> {
        let mut stats = ComputedStats::default();

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
                    stats.usage.prompt_tokens =
                        extractor.extract_token_u32(&normalized, "tokens_prompt");
                    stats.usage.completion_tokens =
                        extractor.extract_token_u32(&normalized, "tokens_completion");
                    // total: 优先映射，如果没有，则根据 prompt+completion 回退
                    stats.usage.total_tokens = extractor
                        .extract_token_u32(&normalized, "tokens_total")
                        .or_else(|| {
                            match (stats.usage.prompt_tokens, stats.usage.completion_tokens) {
                                (Some(p), Some(c)) => Some(p + c),
                                (Some(p), None) => Some(p),
                                (None, Some(c)) => Some(c),
                                _ => None,
                            }
                        });
                    // 可选缓存字段
                    stats.usage.cache_create_tokens =
                        extractor.extract_token_u32(&normalized, "cache_create_tokens");
                    stats.usage.cache_read_tokens =
                        extractor.extract_token_u32(&normalized, "cache_read_tokens");
                }
            }
        }

        // 若数据库未配置或未提取到token字段，则设为0值
        if stats.usage.prompt_tokens.is_none() {
            stats.usage.prompt_tokens = Some(0);
        }
        if stats.usage.completion_tokens.is_none() {
            stats.usage.completion_tokens = Some(0);
        }
        if stats.usage.total_tokens.is_none() {
            stats.usage.total_tokens = Some(0);
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
            input_tokens = ?stats.usage.prompt_tokens,
            output_tokens = ?stats.usage.completion_tokens,
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
                prompt_tokens: stats.usage.prompt_tokens,
                completion_tokens: stats.usage.completion_tokens,
                cache_create_tokens: stats.usage.cache_create_tokens,
                cache_read_tokens: stats.usage.cache_read_tokens,
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
    pub fn extract_model_from_request_body_fallback(&self, ctx: &ProxyContext) -> Option<String> {
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
    pub fn parse_request_body_for_model(&self, ctx: &ProxyContext) -> Option<serde_json::Value> {
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
    pub fn extract_query_params_for_model(&self, session: &Session) -> HashMap<String, String> {
        let uri = &session.req_header().uri;
        let query_string = uri.query().unwrap_or("");

        if query_string.is_empty() {
            return HashMap::new();
        }

        form_urlencoded::parse(query_string.as_bytes())
            .into_owned()
            .collect()
    }

    /// 最终统计入口（统一流式与非流式）
    /// 1) 非流式：从原始分块获取字节，如存在 content-encoding 则限量解压，仅用于统计；解析JSON提取统计；完成后清理分块以节省内存
    /// 2) 流式：不还原整段，仅使用 sse_usage_agg（latest-wins）作为主来源；必要时从 tail_window 解析末端事件获取模型兜底
    pub async fn finalize_and_extract_stats(
        &self,
        ctx: &mut ProxyContext,
    ) -> Result<ComputedStats, ProxyError> {
        // 流式路径：不解压，不还原整段
        if ctx.response_details.is_sse_format() {
            let mut stats = ComputedStats::default();
            // 使用增量聚合的 usage（latest-wins）
            if ctx.usage_partial.prompt_tokens.is_some()
                || ctx.usage_partial.completion_tokens.is_some()
                || ctx.usage_partial.total_tokens.is_some()
                || ctx.usage_partial.cache_create_tokens.is_some()
                || ctx.usage_partial.cache_read_tokens.is_some()
            {
                stats.usage.prompt_tokens = ctx.usage_partial.prompt_tokens;
                stats.usage.completion_tokens = ctx.usage_partial.completion_tokens;
                stats.usage.total_tokens = ctx
                    .usage_partial
                    .total_tokens
                    .or_else(|| {
                        match (
                            ctx.usage_partial.prompt_tokens,
                            ctx.usage_partial.completion_tokens,
                        ) {
                            (Some(p), Some(c)) => Some(p + c),
                            (Some(p), None) => Some(p),
                            (None, Some(c)) => Some(c),
                            _ => Some(0),
                        }
                    })
                    .or(Some(0));
                stats.usage.cache_create_tokens = ctx.usage_partial.cache_create_tokens;
                stats.usage.cache_read_tokens = ctx.usage_partial.cache_read_tokens;
            } else {
                stats.usage.prompt_tokens = Some(0);
                stats.usage.completion_tokens = Some(0);
                stats.usage.total_tokens = Some(0);
            }

            // 从尾窗尝试提取模型兜底
            if stats.model_name.is_none() {
                if !ctx.response_details.tail_window.is_empty() {
                    if let Ok(s) = String::from_utf8(ctx.response_details.tail_window.clone()) {
                        if let Some(last_json) = extract_last_json_from_str(&s) {
                            stats.model_name = self.extract_model_from_response_body(&last_json);
                        }
                    }
                }
            }

            // 费用计算（有模型与 provider）
            if let (Some(model), Some(provider)) =
                (stats.model_name.clone(), ctx.provider_type.as_ref())
            {
                let usage_now = crate::pricing::TokenUsage {
                    prompt_tokens: stats.usage.prompt_tokens,
                    completion_tokens: stats.usage.completion_tokens,
                    cache_create_tokens: stats.usage.cache_create_tokens,
                    cache_read_tokens: stats.usage.cache_read_tokens,
                };
                if let Ok((cost, currency)) = self
                    .calculate_cost_direct(&model, provider.id, &usage_now, &ctx.request_id)
                    .await
                {
                    stats.cost = cost;
                    stats.cost_currency = currency;
                }
            }

            // 同步新统一字段
            ctx.usage_final = Some(stats.usage.clone());

            return Ok(stats);
        }

        // 非流式：从原始分块获取字节，按需限量解压，仅用于统计
        // 提前获取encoding以避免借用冲突
        let encoding_owned = ctx
            .response_details
            .content_encoding
            .clone();
        let encoding = encoding_owned.as_deref();
        let raw = &ctx.response_details.body_chunks;

        // ===== 增强的响应体数据验证 =====
        tracing::debug!(
            request_id = %ctx.request_id,
            body_chunks_len = raw.len(),
            total_bytes = raw.len(),
            content_encoding = ?encoding,
            response_content_type = ?ctx.response_details.content_type,
            "Response body data validation for stats"
        );

        // 验证响应体数据完整性
        if raw.is_empty() {
            tracing::info!(
                request_id = %ctx.request_id,
                "Empty response body chunks for stats; using default values"
            );
            ctx.usage_final = Some(crate::statistics::types::TokenUsageMetrics {
                prompt_tokens: Some(0),
                completion_tokens: Some(0),
                total_tokens: Some(0),
                cache_create_tokens: None,
                cache_read_tokens: None,
            });
            return Ok(ComputedStats::default());
        }

        // 计算总数据大小并验证合理性
        let total_bytes: usize = raw.len();
        if total_bytes == 0 {
            tracing::warn!(
                request_id = %ctx.request_id,
                chunks_count = raw.len(),
                "Response body chunks exist but contain no data; using default values"
            );
            ctx.usage_final = Some(crate::statistics::types::TokenUsageMetrics {
                prompt_tokens: Some(0),
                completion_tokens: Some(0),
                total_tokens: Some(0),
                cache_create_tokens: None,
                cache_read_tokens: None,
            });
            return Ok(ComputedStats::default());
        }

        // 检查数据大小是否在合理范围内
        if total_bytes > 10 * 1024 * 1024 { // 10MB 上限
            tracing::warn!(
                request_id = %ctx.request_id,
                total_bytes = total_bytes,
                "Response body too large for stats processing; using default values"
            );
            ctx.usage_final = Some(crate::statistics::types::TokenUsageMetrics {
                prompt_tokens: Some(0),
                completion_tokens: Some(0),
                total_tokens: Some(0),
                cache_create_tokens: None,
                cache_read_tokens: None,
            });
            return Ok(ComputedStats::default());
        }

        let decoded: Cow<[u8]> = decompress_for_stats(encoding, raw, 512 * 1024); // 512KB 上限

        // 验证解压后的数据
        if decoded.is_empty() {
            tracing::warn!(
                request_id = %ctx.request_id,
                original_bytes = total_bytes,
                compression = ?encoding,
                "Decompressed data is empty for stats; using default values"
            );
            ctx.usage_final = Some(crate::statistics::types::TokenUsageMetrics {
                prompt_tokens: Some(0),
                completion_tokens: Some(0),
                total_tokens: Some(0),
                cache_create_tokens: None,
                cache_read_tokens: None,
            });
            return Ok(ComputedStats::default());
        }

        let body_str = match std::str::from_utf8(&decoded) {
            Ok(s) => {
                tracing::debug!(
                    request_id = %ctx.request_id,
                    decoded_bytes = decoded.len(),
                    utf8_string_len = s.len(),
                    "Successfully decoded response body for stats"
                );
                s
            }
            Err(e) => {
                tracing::warn!(
                    request_id = %ctx.request_id,
                    error = %e,
                    compressed = ?encoding,
                    decoded_bytes = decoded.len(),
                    "Failed to parse response body as UTF-8 for stats"
                );
                // 清理原始分块，避免双份占用
                ctx.response_details.clear_body_chunks();
                // 同步默认统计到上下文
                ctx.usage_final = Some(crate::statistics::types::TokenUsageMetrics {
                    prompt_tokens: Some(0),
                    completion_tokens: Some(0),
                    total_tokens: Some(0),
                    cache_create_tokens: None,
                    cache_read_tokens: None,
                });
                return Ok(ComputedStats::default());
            }
        };

        // ===== 增强的JSON解析流程 =====

        // 进一步清理和验证响应体
        let cleaned_body_str = preprocess_json_string(body_str);

        // 若响应体为空或仅包含空白，跳过JSON解析，按默认统计处理（不视为错误）
        if cleaned_body_str.trim().is_empty() {
            tracing::info!(
                request_id = %ctx.request_id,
                compressed = ?encoding,
                original_len = body_str.len(),
                cleaned_len = cleaned_body_str.len(),
                "Empty response body for stats; skipping JSON parsing"
            );
            ctx.response_details.clear_body_chunks();
            // 同步空使用统计到上下文，避免后续为None
            ctx.usage_final = Some(crate::statistics::types::TokenUsageMetrics {
                prompt_tokens: Some(0),
                completion_tokens: Some(0),
                total_tokens: Some(0),
                cache_create_tokens: None,
                cache_read_tokens: None,
            });
            return Ok(ComputedStats::default());
        }

        // ===== JSON格式预检查 =====
        if !is_valid_json_format(&cleaned_body_str) {
            tracing::warn!(
                request_id = %ctx.request_id,
                body_preview = %truncate_string(&cleaned_body_str, 200),
                first_char = %cleaned_body_str.chars().next().map_or('?', |c| c),
                last_char = %cleaned_body_str.chars().last().map_or('?', |c| c),
                body_len = cleaned_body_str.len(),
                "Response body doesn't appear to be valid JSON format"
            );

            // 尝试提取可能的JSON部分
            if let Some(extracted_json) = try_extract_json_from_mixed_content(&cleaned_body_str) {
                tracing::info!(
                    request_id = %ctx.request_id,
                    "Successfully extracted JSON from mixed content"
                );
                return self.process_extracted_json(ctx, extracted_json, encoding).await;
            } else {
                ctx.response_details.clear_body_chunks();
                ctx.usage_final = Some(crate::statistics::types::TokenUsageMetrics {
                    prompt_tokens: Some(0),
                    completion_tokens: Some(0),
                    total_tokens: Some(0),
                    cache_create_tokens: None,
                    cache_read_tokens: None,
                });
                return Ok(ComputedStats::default());
            }
        }

        // 主要JSON解析
        match serde_json::from_str::<serde_json::Value>(&cleaned_body_str) {
            Ok(json) => {
                tracing::debug!(
                    request_id = %ctx.request_id,
                    json_type = %get_json_type(&json),
                    "Successfully parsed JSON for stats"
                );
                let stats = self.extract_stats_from_json(ctx, &json).await?;
                // 清理原始分块，避免双份占用
                ctx.response_details.clear_body_chunks();
                // 同步新统一字段
                ctx.usage_final = Some(stats.usage.clone());
                Ok(stats)
            }
            Err(e) => {
                tracing::warn!(
                    request_id = %ctx.request_id,
                    error = %e,
                    error_kind = ?e.classify(),
                    compressed = ?encoding,
                    body_len = cleaned_body_str.len(),
                    body_preview = %truncate_string(&cleaned_body_str, 300),
                    "Failed to parse JSON from response for stats"
                );

                // 尝试容错解析
                if let Some(fallback_json) = attempt_fallback_json_parsing(&cleaned_body_str) {
                    tracing::info!(
                        request_id = %ctx.request_id,
                        "Successfully parsed JSON using fallback method"
                    );
                    match self.extract_stats_from_json(ctx, &fallback_json).await {
                        Ok(stats) => {
                            ctx.response_details.clear_body_chunks();
                            ctx.usage_final = Some(stats.usage.clone());
                            return Ok(stats);
                        }
                        Err(fallback_error) => {
                            tracing::warn!(
                                request_id = %ctx.request_id,
                                fallback_error = %fallback_error,
                                "Fallback JSON parsing succeeded but stats extraction failed"
                            );
                        }
                    }
                }

                ctx.response_details.clear_body_chunks();
                ctx.usage_final = Some(crate::statistics::types::TokenUsageMetrics {
                    prompt_tokens: Some(0),
                    completion_tokens: Some(0),
                    total_tokens: Some(0),
                    cache_create_tokens: None,
                    cache_read_tokens: None,
                });
                Ok(ComputedStats::default())
            }
        }
    }
}

/// 流式场景：从 SSE/stream 分块中尝试提取 usage 统计并合并到上下文聚合
/// 注意：此函数不依赖实例，可直接在流式响应阶段调用
impl StatisticsService {
    /// 使用 provider 的 token_mappings_json（经 TokenFieldExtractor）从 JSON 提取统计
    pub async fn extract_stats_from_json(
        &self,
        ctx: &mut ProxyContext,
        json: &serde_json::Value,
    ) -> Result<ComputedStats, ProxyError> {
        let mut stats = ComputedStats::default();

        // 模型信息（复用现有响应模型提取）
        stats.model_name = self.extract_model_from_response_body(json);

        // 基于 provider token_mappings_json 的数据驱动提取
        if let Some(provider) = ctx.provider_type.as_ref() {
            if let Some(mapping_json) = provider.token_mappings_json.as_ref() {
                if let Ok(cfg) =
                    crate::providers::field_extractor::TokenMappingConfig::from_json(mapping_json)
                {
                    let extractor =
                        crate::providers::field_extractor::TokenFieldExtractor::new(cfg);
                    stats.usage.prompt_tokens = extractor.extract_token_u32(json, "tokens_prompt");
                    stats.usage.completion_tokens =
                        extractor.extract_token_u32(json, "tokens_completion");
                    stats.usage.total_tokens = extractor
                        .extract_token_u32(json, "tokens_total")
                        .or_else(|| {
                            match (stats.usage.prompt_tokens, stats.usage.completion_tokens) {
                                (Some(p), Some(c)) => Some(p + c),
                                (Some(p), None) => Some(p),
                                (None, Some(c)) => Some(c),
                                _ => None,
                            }
                        });
                    stats.usage.cache_create_tokens =
                        extractor.extract_token_u32(json, "cache_create_tokens");
                    stats.usage.cache_read_tokens =
                        extractor.extract_token_u32(json, "cache_read_tokens");
                }
            }
        }

        // 默认值兜底
        if stats.usage.prompt_tokens.is_none() {
            stats.usage.prompt_tokens = Some(0);
        }
        if stats.usage.completion_tokens.is_none() {
            stats.usage.completion_tokens = Some(0);
        }
        if stats.usage.total_tokens.is_none() {
            stats.usage.total_tokens = Some(0);
        }

        // 费用计算（有模型与 provider 才计算）
        if let (Some(model), Some(provider)) =
            (stats.model_name.clone(), ctx.provider_type.as_ref())
        {
            let usage_now = crate::pricing::TokenUsage {
                prompt_tokens: stats.usage.prompt_tokens,
                completion_tokens: stats.usage.completion_tokens,
                cache_create_tokens: stats.usage.cache_create_tokens,
                cache_read_tokens: stats.usage.cache_read_tokens,
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

/// 仅用于统计侧的限量解压（不影响下游透传）
///
/// 增强版：包含更好的错误处理、空数据检测和透明压缩检测
fn decompress_for_stats<'a>(
    encoding: Option<&'a str>,
    input: &'a [u8],
    max_out: usize,
) -> Cow<'a, [u8]> {
    use flate2::read::{GzDecoder, ZlibDecoder};
    use std::io::Read;

    // 输入验证：空数据处理
    if input.is_empty() {
        tracing::debug!("Empty input for decompression");
        return Cow::Borrowed(input);
    }

    // 输入大小验证
    if input.len() > 50 * 1024 * 1024 { // 50MB 硬编码上限
        tracing::warn!(
            input_size = input.len(),
            "Input too large for decompression, returning as-is"
        );
        return Cow::Borrowed(input);
    }

    // ===== 增强的压缩检测逻辑 =====
    // 不仅依赖 Content-Encoding 头，还尝试自动检测压缩格式
    let effective_encoding = if let Some(enc) = encoding {
        Some(enc.to_ascii_lowercase())
    } else {
        // 尝试通过魔术字自动检测压缩格式
        detect_compression_format(input)
    };

    tracing::debug!(
        provided_encoding = ?encoding,
        detected_encoding = ?effective_encoding,
        input_size = input.len(),
        "Processing decompression for stats"
    );

    match effective_encoding {
        Some(enc) if enc.contains("gzip") => {
            tracing::debug!("Using gzip decompression");
            let mut dec = match GzDecoder::new(input) {
                decoder => decoder,
            };
            let mut out = Vec::with_capacity(std::cmp::min(input.len() * 3, max_out)); // 预估解压后大小
            let mut buf = [0u8; 8192];

            let mut total_read = 0;
            while out.len() < max_out {
                match dec.read(&mut buf) {
                    Ok(0) => {
                        tracing::debug!(total_decompressed = out.len(), "Gzip decompression complete");
                        break;
                    }
                    Ok(n) => {
                        total_read += n;
                        let take = n.min(max_out - out.len());
                        out.extend_from_slice(&buf[..take]);
                        if take < n {
                            tracing::debug!(reason = "max_out_reached", "Gzip decompression truncated");
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            bytes_read = total_read,
                            "Gzip decompression failed, using original data"
                        );
                        return Cow::Borrowed(input); // 解压失败，返回原始数据
                    }
                }
            }
            if out.is_empty() {
                tracing::warn!("Gzip decompression produced empty output, using original data");
                Cow::Borrowed(input)
            } else {
                Cow::Owned(out)
            }
        }
        Some(enc) if enc.contains("deflate") => {
            tracing::debug!("Using deflate decompression");
            let mut dec = match ZlibDecoder::new(input) {
                decoder => decoder,
            };
            let mut out = Vec::with_capacity(std::cmp::min(input.len() * 3, max_out));
            let mut buf = [0u8; 8192];

            let mut total_read = 0;
            while out.len() < max_out {
                match dec.read(&mut buf) {
                    Ok(0) => {
                        tracing::debug!(total_decompressed = out.len(), "Deflate decompression complete");
                        break;
                    }
                    Ok(n) => {
                        total_read += n;
                        let take = n.min(max_out - out.len());
                        out.extend_from_slice(&buf[..take]);
                        if take < n {
                            tracing::debug!(reason = "max_out_reached", "Deflate decompression truncated");
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            bytes_read = total_read,
                            "Deflate decompression failed, using original data"
                        );
                        return Cow::Borrowed(input);
                    }
                }
            }
            if out.is_empty() {
                tracing::warn!("Deflate decompression produced empty output, using original data");
                Cow::Borrowed(input)
            } else {
                Cow::Owned(out)
            }
        }
        Some(enc) if enc.contains("br") || enc.contains("brotli") => {
            tracing::debug!("Using brotli decompression");
            let mut out = Vec::with_capacity(std::cmp::min(input.len() * 5, max_out)); // Brotli 压缩比更高
            let mut reader = match brotli_decompressor::Decompressor::new(input, 4096) {
                reader => reader,
            };
            let mut buf = [0u8; 8192];

            let mut total_read = 0;
            while out.len() < max_out {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        tracing::debug!(total_decompressed = out.len(), "Brotli decompression complete");
                        break;
                    }
                    Ok(n) => {
                        total_read += n;
                        let take = n.min(max_out - out.len());
                        out.extend_from_slice(&buf[..take]);
                        if take < n {
                            tracing::debug!(reason = "max_out_reached", "Brotli decompression truncated");
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            bytes_read = total_read,
                            "Brotli decompression failed, using original data"
                        );
                        return Cow::Borrowed(input);
                    }
                }
            }
            if out.is_empty() {
                tracing::warn!("Brotli decompression produced empty output, using original data");
                Cow::Borrowed(input)
            } else {
                Cow::Owned(out)
            }
        }
        None => {
            tracing::debug!("No compression detected, using original data");
            Cow::Borrowed(input)
        }
        Some(other) => {
            tracing::warn!(encoding = %other, "Unsupported encoding, using original data");
            Cow::Borrowed(input)
        }
    }
}

/// 通过魔术字检测压缩格式
pub fn detect_compression_format(data: &[u8]) -> Option<String> {
    if data.len() < 2 {
        return None;
    }

    // Gzip 魔术字: 0x1f 0x8b
    if data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b {
        tracing::debug!("Detected gzip format by magic bytes");
        return Some("gzip".to_string());
    }

    // Zlib 魔术字: 0x78 0x9c, 0x78 0xda, 0x78 0x5e
    if data.len() >= 2 && data[0] == 0x78 {
        if data[1] == 0x9c || data[1] == 0xda || data[1] == 0x5e {
            tracing::debug!("Detected zlib/deflate format by magic bytes");
            return Some("deflate".to_string());
        }
    }

    // Brotli 魔术字检测较复杂，这里简单处理
    // Brotli 通常以特定位模式开始，但准确检测需要更复杂的逻辑

    None
}

/// 从字符串中提取最后一个JSON对象（简单容错）
fn extract_last_json_from_str(s: &str) -> Option<serde_json::Value> {
    // 先尝试逐行 data: {...}
    for line in s.lines().rev() {
        let t = line.trim_start_matches("data: ").trim();
        if let Some(pos) = t.find('{') {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t[pos..]) {
                return Some(v);
            }
        }
    }
    // 退而求其次：从整体文本中寻找最后一个 '{'
    if let Some(idx) = s.rfind('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s[idx..]) {
            return Some(v);
        }
    }
    None
}

/// 流式分块增量统计（数据驱动，基于 provider 的 token_mappings_json）
pub fn ingest_streaming_chunk(ctx: &mut ProxyContext, data: &[u8]) {
    if data.is_empty() {
        return;
    }
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    let mut last_json: Option<serde_json::Value> = None;
    for line in text.lines() {
        let t = line.trim_start_matches("data: ").trim();
        if let Some(pos) = t.find('{') {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t[pos..]) {
                last_json = Some(v);
            }
        }
    }
    let Some(json) = last_json else {
        return;
    };

    // Token 提取（数据驱动）
    if let Some(provider) = ctx.provider_type.as_ref() {
        if let Some(mapping_json) = provider.token_mappings_json.as_ref() {
            if let Ok(cfg) =
                crate::providers::field_extractor::TokenMappingConfig::from_json(mapping_json)
            {
                let extractor = crate::providers::field_extractor::TokenFieldExtractor::new(cfg);
                let p = extractor.extract_token_u32(&json, "tokens_prompt");
                let c = extractor.extract_token_u32(&json, "tokens_completion");
                let t = extractor
                    .extract_token_u32(&json, "tokens_total")
                    .or_else(|| match (p, c) {
                        (Some(pp), Some(cc)) => Some(pp + cc),
                        (Some(pp), None) => Some(pp),
                        (None, Some(cc)) => Some(cc),
                        _ => None,
                    });
                let cache_create = extractor.extract_token_u32(&json, "cache_create_tokens");
                let cache_read = extractor.extract_token_u32(&json, "cache_read_tokens");

                if p.is_some()
                    || c.is_some()
                    || t.is_some()
                    || cache_create.is_some()
                    || cache_read.is_some()
                {
                    let partial = crate::statistics::types::PartialUsage {
                        prompt_tokens: p,
                        completion_tokens: c,
                        total_tokens: t,
                        cache_create_tokens: cache_create,
                        cache_read_tokens: cache_read,
                    };
                    ctx.usage_partial.merge_latest(&partial);
                }
            }
        }
    }

    // 模型兜底更新
    // 可选：从尾事件中提取模型名（简单路径）
    if ctx.requested_model.is_none() {
        let model = json
            .get("model")
            .and_then(|m| m.as_str())
            .map(|s| s.to_string());
        if model.is_some() {
            ctx.requested_model = model;
        }
    }
}

// ========== JSON解析辅助函数 ==========

/// 预处理JSON字符串，清理常见问题
pub fn preprocess_json_string(input: &str) -> String {
    let mut cleaned = input.trim().to_string();

    // 移除BOM（如果存在）
    if cleaned.starts_with('\u{FEFF}') {
        cleaned.remove(0);
    }

    // 移除可能的UTF-8 BOM序列
    if cleaned.starts_with("\u{EF}\u{BB}\u{BF}") {
        cleaned.drain(..3);
    }

    // 移除开头和结尾的垃圾字符
    cleaned = cleaned.trim_matches(|c: char| {
        c.is_control() || c == '\u{200B}' || c == '\u{FEFF}'
    }).to_string();

    // 尝试修复常见JSON格式问题
    if !cleaned.is_empty() {
        // 确保以合适的开头和结尾
        if !cleaned.starts_with('{') && !cleaned.starts_with('[') {
            // 尝试查找第一个JSON开始标记
            if let Some(start) = cleaned.find('{') {
                cleaned = cleaned[start..].to_string();
            } else if let Some(start) = cleaned.find('[') {
                cleaned = cleaned[start..].to_string();
            }
        }

        // 确保以合适的结尾
        if !cleaned.is_empty() && !cleaned.ends_with('}') && !cleaned.ends_with(']') {
            // 尝试查找最后一个JSON结束标记
            if let Some(end) = cleaned.rfind('}') {
                cleaned = cleaned[..=end].to_string();
            } else if let Some(end) = cleaned.rfind(']') {
                cleaned = cleaned[..=end].to_string();
            }
        }
    }

    cleaned
}

/// 检查字符串是否可能是有效的JSON格式
pub fn is_valid_json_format(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }

    // 检查基本结构
    (trimmed.starts_with('{') && trimmed.ends_with('}')) ||
    (trimmed.starts_with('[') && trimmed.ends_with(']')) ||
    // 某些简单的JSON值
    (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() > 1) ||
    trimmed == "true" || trimmed == "false" || trimmed == "null"
}

/// 尝试从混合内容中提取JSON
pub fn try_extract_json_from_mixed_content(s: &str) -> Option<serde_json::Value> {
    // 尝试找到最外层的JSON结构
    let json_start = s.find('{').or_else(|| s.find('['))?;
    let json_str = &s[json_start..];

    // 尝试找到匹配的结束括号
    let mut brace_count = 0;
    let mut bracket_count = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, c) in json_str.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match c {
            '\\' => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => brace_count += 1,
            '}' if !in_string => {
                brace_count -= 1;
                if brace_count == 0 && bracket_count == 0 {
                    let json_candidate = &json_str[..=i];
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_candidate) {
                        return Some(json);
                    }
                }
            }
            '[' if !in_string => bracket_count += 1,
            ']' if !in_string => {
                bracket_count -= 1;
                if brace_count == 0 && bracket_count == 0 {
                    let json_candidate = &json_str[..=i];
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_candidate) {
                        return Some(json);
                    }
                }
            }
            _ => {}
        }
    }

    None
}

/// 尝试容错JSON解析
fn attempt_fallback_json_parsing(s: &str) -> Option<serde_json::Value> {
    // 尝试1: 移除可能的JavaScript注释
    let without_comments = remove_json_comments(s);
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&without_comments) {
        return Some(json);
    }

    // 尝试2: 尝试修复常见的JSON格式问题
    let fixed = try_fix_common_json_issues(s);
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&fixed) {
        return Some(json);
    }

    // 尝试3: 提取并解析JSON-like对象
    if let Some(extracted) = extract_json_like_object(s) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&extracted) {
            return Some(json);
        }
    }

    None
}

/// 移除JavaScript风格的注释
fn remove_json_comments(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_single_line_comment = false;
    let mut in_multi_line_comment = false;
    let mut in_string = false;
    let mut escape_next = false;

    for c in s.chars() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        match c {
            '\\' => {
                result.push(c);
                escape_next = true;
            }
            '"' => {
                result.push(c);
                if !in_single_line_comment && !in_multi_line_comment {
                    in_string = !in_string;
                }
            }
            '/' if !in_string => {
                let next_chars: String = s.chars().skip(result.len()).take(2).collect();
                if next_chars == "//" && !in_multi_line_comment {
                    in_single_line_comment = true;
                    result.push(c);
                } else if next_chars == "/*" && !in_single_line_comment {
                    in_multi_line_comment = true;
                    result.push(c);
                } else if !in_single_line_comment && !in_multi_line_comment {
                    result.push(c);
                }
            }
            '\n' => {
                result.push(c);
                in_single_line_comment = false;
            }
            '*' if in_multi_line_comment && !in_string => {
                let next_char = s.chars().skip(result.len()).next();
                if let Some('/') = next_char {
                    in_multi_line_comment = false;
                    result.push(c);
                    // 添加 closing '/' but don't break out of the loop
                    continue;
                }
            }
            _ if !in_single_line_comment && !in_multi_line_comment => {
                result.push(c);
            }
            _ => {}
        }
    }

    result
}

/// 尝试修复常见的JSON格式问题
fn try_fix_common_json_issues(s: &str) -> String {
    let mut fixed = s.to_string();

    // 移除尾随逗号
    fixed = regex::Regex::new(r#",\s*([}\]])"#)
        .unwrap_or_else(|_| regex::Regex::new(r"#").unwrap())
        .replace_all(&fixed, |caps: &regex::Captures| {
            caps.get(1).unwrap().as_str().to_string()
        }).to_string();

    // 确保属性名有引号
    fixed = regex::Regex::new(r#"([a-zA-Z_$][a-zA-Z0-9_$]*)\s*:"#)
        .unwrap_or_else(|_| regex::Regex::new(r"#").unwrap())
        .replace_all(&fixed, r#""$1":"#)
        .to_string();

    fixed
}

/// 提取JSON-like对象
fn extract_json_like_object(s: &str) -> Option<String> {
    let trimmed = s.trim();
    let start = trimmed.find('{')?;
    let mut brace_count = 1;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, c) in trimmed[start + 1..].char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match c {
            '\\' => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => brace_count += 1,
            '}' if !in_string => {
                brace_count -= 1;
                if brace_count == 0 {
                    return Some(trimmed[start..=start + 1 + i].to_string());
                }
            }
            _ => {}
        }
    }

    None
}

/// 获取JSON值的类型描述
fn get_json_type(json: &serde_json::Value) -> &'static str {
    match json {
        serde_json::Value::Object(_) => "object",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Null => "null",
    }
}

/// 截断字符串用于日志
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

/// StatisticsService的辅助方法：处理提取的JSON
impl StatisticsService {
    async fn process_extracted_json(
        &self,
        ctx: &mut ProxyContext,
        json: serde_json::Value,
        _encoding: Option<&str>,
    ) -> Result<ComputedStats, ProxyError> {
        match self.extract_stats_from_json(ctx, &json).await {
            Ok(stats) => {
                ctx.response_details.clear_body_chunks();
                ctx.usage_final = Some(stats.usage.clone());
                Ok(stats)
            }
            Err(e) => {
                tracing::warn!(
                    request_id = %ctx.request_id,
                    error = %e,
                    "Failed to extract stats from extracted JSON"
                );
                ctx.response_details.clear_body_chunks();
                ctx.usage_final = Some(crate::statistics::types::TokenUsageMetrics {
                    prompt_tokens: Some(0),
                    completion_tokens: Some(0),
                    total_tokens: Some(0),
                    cache_create_tokens: None,
                    cache_read_tokens: None,
                });
                Ok(ComputedStats::default())
            }
        }
    }
}

// 移除了已删除功能的测试函数
