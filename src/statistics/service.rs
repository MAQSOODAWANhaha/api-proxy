//! # 代理端统计服务（迁移）
//!
//! 从 `src/proxy/statistics_service.rs` 迁移至此，作为统计模块对外服务。

use anyhow::Result;
use pingora_http::ResponseHeader;
use pingora_proxy::Session;
use std::collections::HashMap;
use std::sync::Arc;
use url::form_urlencoded;

// use crate::auth::AuthUtils; // moved to request collector
use crate::error::ProxyError;
use crate::pricing::PricingCalculatorService;
use crate::proxy::ProxyContext;
// 提取器在此处不直接依赖，避免强耦合；如需高级解析可在 providers 层使用。

// 复用统计模块类型定义
pub use crate::statistics::types::{RequestDetails, ResponseDetails};
use crate::statistics::types::{RequestStats, ResponseStats};

// RequestStats/ResponseStats 已迁移至 statistics::types

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
                tracing::warn!(component = "statistics.service", request_id = %request_id, error = %e, "Failed to calculate cost (direct)");
                Ok((None, None))
            }
        }
    }

    /// 收集请求统计信息
    pub fn collect_request_stats(&self, session: &Session) -> RequestStats {
        crate::statistics::request::collect_stats(session)
    }

    /// 收集请求详情
    pub fn collect_request_details(
        &self,
        session: &Session,
        request_stats: &RequestStats,
    ) -> RequestDetails {
        crate::statistics::request::collect_details(session, request_stats)
    }

    /// 收集响应详情
    pub fn collect_response_details(
        &self,
        upstream_response: &ResponseHeader,
        ctx: &mut ProxyContext,
    ) -> ResponseStats {
        crate::statistics::response::collect_details(upstream_response, ctx)
    }

    /// 从 JSON 响应中提取 token 统计（通用提取器）

    /// 从响应体中提取模型信息
    ///
    /// 支持多种AI API响应格式的模型提取，包括数组索引访问
    pub fn extract_model_from_response_body(&self, json: &serde_json::Value) -> Option<String> {
        crate::statistics::usage_model::extract_model_from_json(json)
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
                                    component = "statistics.service",
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
                                component = "statistics.service",
                                path = %path,
                                field_name = %field_name,
                                index = %index_str,
                                "Invalid array index format"
                            );
                            return None;
                        }
                    } else {
                        tracing::debug!(
                            component = "statistics.service",
                            path = %path,
                            field_name = %field_name,
                            "Field is not an array"
                        );
                        return None;
                    }
                } else {
                    tracing::debug!(
                        component = "statistics.service",
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
                                component = "statistics.service",
                                path = %path,
                                index = %part,
                                array_len = array.len(),
                                "Direct array index out of bounds"
                            );
                            return None;
                        }
                    } else {
                        tracing::debug!(
                            component = "statistics.service",
                            path = %path,
                            index = %part,
                            "Invalid direct array index format"
                        );
                        return None;
                    }
                } else {
                    tracing::debug!(
                        component = "statistics.service",
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
                        component = "statistics.service",
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
            component = "statistics.service",
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

    // ========== 请求阶段模型提取方法 ==========

    /// 统一的请求阶段模型提取入口点
    pub fn extract_model_from_request(
        &self,
        session: &Session,
        ctx: &ProxyContext,
    ) -> Option<String> {
        tracing::debug!(
            component = "statistics.service",
            request_id = ctx.request_id,
            "Starting model extraction from request"
        );

        // 尝试使用数据库驱动的 ModelExtractor
        if let Some(model_name) = self.extract_model_from_request_with_db_config(session, ctx) {
            tracing::info!(
                component = "statistics.service",
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
                component = "statistics.service",
                request_id = ctx.request_id,
                model = model_name,
                extraction_method = "request_body_fallback",
                "Model extracted from request body (fallback method)"
            );
            return Some(model_name);
        }

        tracing::debug!(
            component = "statistics.service",
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
            component = "statistics.service",
            request_id = ctx.request_id,
            provider_id = provider.id,
            provider_name = provider.name,
            has_model_extraction_config = provider.model_extraction_json.is_some(),
            "Checking provider model extraction configuration"
        );

        let model_extraction_json = provider.model_extraction_json.as_ref()?;

        tracing::debug!(
            component = "statistics.service",
            request_id = ctx.request_id,
            provider_id = provider.id,
            "Creating ModelExtractor from database configuration"
        );

        let extractor = crate::statistics::field_extractor::ModelExtractor::from_json_config(
            model_extraction_json,
        )
        .map_err(|e| {
            tracing::warn!(
                component = "statistics.service",
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
            component = "statistics.service",
            request_id = ctx.request_id,
            url_path = url_path,
            has_body_json = body_json.is_some(),
            query_params_count = query_params.len(),
            "Extracting model with configured extractor"
        );

        let model_name = extractor.extract_model_name(url_path, body_json.as_ref(), &query_params);

        tracing::info!(
            component = "statistics.service",
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
            component = "statistics.service",
            request_id = ctx.request_id,
            body_size = ctx.request_body.len(),
            "Attempting to extract model from request body"
        );

        if ctx.request_body.is_empty() {
            tracing::debug!(
                request_id = ctx.request_id,
                "Request body is empty, skipping body extraction"
            );
            return None;
        }

        let body_str = std::str::from_utf8(&ctx.request_body)
            .map_err(|e| {
                tracing::debug!(
                    component = "statistics.service",
                    request_id = ctx.request_id,
                    error = %e,
                    "Request body is not valid UTF-8"
                );
                e
            })
            .ok()?;

        tracing::debug!(
            component = "statistics.service",
            request_id = ctx.request_id,
            body_length = body_str.len(),
            "Request body parsed as UTF-8 successfully"
        );

        let json_value = serde_json::from_str::<serde_json::Value>(body_str)
            .map_err(|e| {
                tracing::debug!(
                    component = "statistics.service",
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
                    component = "statistics.service",
                    request_id = ctx.request_id,
                    model = model,
                    extraction_method = "fallback_direct",
                    "Extracted model from request body (fallback)"
                );
            }
            None => {
                tracing::debug!(
                    component = "statistics.service",
                    request_id = ctx.request_id,
                    "No model field found in request body JSON"
                );
            }
        }

        model_name
    }

    /// 解析请求体用于模型提取
    pub fn parse_request_body_for_model(&self, ctx: &ProxyContext) -> Option<serde_json::Value> {
        if ctx.response_body.is_empty() {
            return None;
        }

        let body_str = std::str::from_utf8(&ctx.response_body)
            .map_err(|e| {
                tracing::debug!(
                    component = "statistics.service",
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
                    component = "statistics.service",
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

    // 统计收口逻辑已前移到 proxy 分块阶段，避免在此重复解析与解压
}

// 流式与非流式统计均已在 proxy/service.rs 分块阶段完成
// 解压逻辑已统一到 statistics::util::decompress_for_stats
