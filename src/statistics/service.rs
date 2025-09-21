//! # 代理端统计服务（迁移）
//!
//! 从 `src/proxy/statistics_service.rs` 迁移至此，作为统计模块对外服务。

use anyhow::Result;
use pingora_http::ResponseHeader;
use pingora_proxy::Session;
use std::sync::Arc;

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
    pub fn normalize_usage_metadata(&self, mut root: serde_json::Value) -> serde_json::Value {
        // 如果顶层已存在，直接返回
        if root.get("usageMetadata").is_some() {
            return root;
        }

        // 深度优先在任意层级查找包含 token 计数字段的对象
        fn dfs_find(obj: &serde_json::Value) -> Option<serde_json::Map<String, serde_json::Value>> {
            match obj {
                serde_json::Value::Object(map) => {
                    let mut acc: serde_json::Map<String, serde_json::Value> =
                        serde_json::Map::new();

                    for key in [
                        "promptTokenCount",
                        "candidatesTokenCount",
                        "totalTokenCount",
                    ] {
                        if let Some(v) = map.get(key) {
                            acc.insert(key.to_string(), v.clone());
                        }
                    }

                    if !acc.is_empty() {
                        return Some(acc);
                    }

                    // 递归遍历子对象/数组
                    for (_k, v) in map.iter() {
                        if let Some(found) = dfs_find(v) {
                            return Some(found);
                        }
                    }
                    None
                }
                serde_json::Value::Array(arr) => {
                    for v in arr {
                        if let Some(found) = dfs_find(v) {
                            return Some(found);
                        }
                    }
                    None
                }
                _ => None,
            }
        }

        if let Some(meta) = dfs_find(&root) {
            if let Some(root_map) = root.as_object_mut() {
                root_map.insert("usageMetadata".to_string(), serde_json::Value::Object(meta));
            }
        }

        root
    }

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
    pub fn extract_usage_from_json(&self, json: &serde_json::Value) -> DetailedRequestStats {
        let mut stats = DetailedRequestStats::default();
        // 默认直接读取 usageMetadata 常见字段，以便无配置也能工作
        if let Some(usage) = json.get("usageMetadata") {
            if let Some(p) = usage.get("promptTokenCount").and_then(|v| v.as_u64()) {
                stats.input_tokens = Some(p as u32);
            }
            if let Some(c) = usage.get("candidatesTokenCount").and_then(|v| v.as_u64()) {
                stats.output_tokens = Some(c as u32);
            }
            if let Some(t) = usage.get("totalTokenCount").and_then(|v| v.as_u64()) {
                stats.total_tokens = Some(t as u32);
            }
        }
        // 模型名（回退）
        stats.model_name = json
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                json.get("data")
                    .and_then(|d| d.get("model"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });

        stats
    }

    /// 从上下文响应体中提取统计信息（统一实现）
    pub async fn extract_stats_from_response_body(
        &self,
        ctx: &mut ProxyContext,
    ) -> Result<DetailedRequestStats, ProxyError> {
        let body = match ctx.response_details.body.as_ref() {
            Some(b) => b,
            None => return Ok(DetailedRequestStats::default()),
        };

        // 尝试解析 JSON
        let parsed: serde_json::Value = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(_) => return Ok(DetailedRequestStats::default()),
        };

        // 归一化 usageMetadata（若在深层）
        let normalized = self.normalize_usage_metadata(parsed);
        let mut stats = DetailedRequestStats::default();

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

        // 若数据库未配置或未提取到，则使用通用回退提取
        if stats.input_tokens.is_none()
            && stats.output_tokens.is_none()
            && stats.total_tokens.is_none()
        {
            let fallback = self.extract_usage_from_json(&normalized);
            stats.input_tokens = fallback.input_tokens;
            stats.output_tokens = fallback.output_tokens;
            stats.total_tokens = fallback.total_tokens;
            stats.model_name = fallback.model_name;
        }

        // 同步模型名（优先使用请求时的模型信息）
        if stats.model_name.is_none() {
            stats.model_name = ctx.requested_model.clone();
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
        if let (Some(model), Some(provider)) =
            (stats.model_name.clone(), ctx.provider_type.as_ref())
        {
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
                    stats.cost = Some(cost.total_cost);
                    stats.cost_currency = Some(cost.currency);
                }
                Err(e) => {
                    tracing::warn!(request_id = %ctx.request_id, error = %e, "Failed to calc cost");
                }
            }
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn normalize_usage_metadata_lifts_nested() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let svc = StatisticsService::new(Arc::new(PricingCalculatorService::new(Arc::new(db))));
        let nested = serde_json::json!({
            "data": {"totalTokenCount": 123, "foo": 1},
            "other": [{"promptTokenCount": 11}]
        });
        let out = svc.normalize_usage_metadata(nested);
        let meta = out
            .get("usageMetadata")
            .and_then(|v| v.as_object())
            .unwrap();
        // 任意一个计数字段被提取到顶层即可
        assert!(meta.contains_key("totalTokenCount") || meta.contains_key("promptTokenCount"));
    }

    #[tokio::test]
    async fn extract_usage_from_json_reads_common_fields() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let svc = StatisticsService::new(Arc::new(PricingCalculatorService::new(Arc::new(db))));
        let json = serde_json::json!({
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 20,
                "totalTokenCount": 30
            },
            "model": "gpt-4o"
        });
        let stats = svc.extract_usage_from_json(&json);
        assert_eq!(stats.input_tokens, Some(10));
        assert_eq!(stats.output_tokens, Some(20));
        assert_eq!(stats.total_tokens, Some(30));
    }
}
