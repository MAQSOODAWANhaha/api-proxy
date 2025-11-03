//! 数据采集入口
//!
//! 负责从 `ProxyContext` 中提取请求/响应信息，并计算模型用量、成本等指标。

use std::sync::Arc;

use crate::collect::{
    request, response,
    types::{CollectedCost, CollectedMetrics, RequestDetails, RequestStats, ResponseStats},
    usage_model,
};
use crate::pricing::{PricingCalculatorService, TokenUsage};
use crate::proxy::ProxyContext;
use crate::{
    linfo,
    logging::{LogComponent, LogStage},
    lwarn,
};
use pingora_http::ResponseHeader;
use pingora_proxy::Session;

/// 采集服务，实现“Collect”阶段的全部逻辑
pub struct CollectService {
    pricing: Arc<PricingCalculatorService>,
}

impl CollectService {
    #[must_use]
    pub const fn new(pricing: Arc<PricingCalculatorService>) -> Self {
        Self { pricing }
    }

    /// 收集请求摘要（供认证阶段启动追踪时使用）
    #[must_use]
    pub fn collect_request_stats(&self, session: &Session) -> RequestStats {
        request::collect_stats(session)
    }

    /// 构造请求详情，保存在 `ProxyContext` 中
    #[must_use]
    pub fn collect_request_details(
        &self,
        session: &Session,
        stats: &RequestStats,
    ) -> RequestDetails {
        request::collect_details(session, stats)
    }

    /// 收集响应详情，保存在 `ProxyContext` 中
    #[must_use]
    pub fn collect_response_details(
        &self,
        upstream_response: &ResponseHeader,
        ctx: &mut ProxyContext,
    ) -> ResponseStats {
        response::collect_details(upstream_response, ctx)
    }

    /// 在请求结束时生成指标快照
    pub async fn finalize_metrics(
        &self,
        ctx: &mut ProxyContext,
        status_code: u16,
    ) -> CollectedMetrics {
        let computed = usage_model::finalize_eos(ctx);
        let usage = computed.usage.clone();
        ctx.usage_final = Some(usage.clone());

        if let Some(last_json) = computed.last_sse_json.as_ref()
            && let Ok(last_json_str) = serde_json::to_string(last_json)
        {
            linfo!(
                ctx.request_id,
                LogStage::Response,
                LogComponent::Statistics,
                "sse_last_chunk",
                &last_json_str
            );
        }

        // 尝试更新最终模型名称
        ctx.requested_model.clone_from(&computed.model_name);

        let (cost_value, cost_currency) = self
            .calculate_cost(
                ctx.provider_type.as_ref(),
                ctx.requested_model.as_deref(),
                &usage,
                &ctx.request_id,
            )
            .await;

        CollectedMetrics {
            request_id: ctx.request_id.clone(),
            user_id: ctx.user_service_api.as_ref().map(|u| u.user_id),
            user_service_api_id: ctx.user_service_api.as_ref().map(|u| u.id),
            provider_type_id: ctx.provider_type.as_ref().map(|p| p.id),
            model: ctx.requested_model.clone(),
            usage,
            cost: CollectedCost {
                value: cost_value,
                currency: cost_currency,
            },
            duration_ms: ctx.start_time.elapsed().as_millis(),
            status_code,
        }
    }

    async fn calculate_cost(
        &self,
        provider: Option<&entity::provider_types::Model>,
        model_used: Option<&str>,
        usage: &crate::collect::types::TokenUsageMetrics,
        request_id: &str,
    ) -> (Option<f64>, Option<String>) {
        let Some(provider_model) = provider else {
            return (None, None);
        };
        let Some(model) = model_used else {
            return (None, None);
        };

        let token_usage = TokenUsage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            cache_create_tokens: usage.cache_create_tokens,
            cache_read_tokens: usage.cache_read_tokens,
        };

        match self
            .pricing
            .calculate_cost(model, provider_model.id, &token_usage, request_id)
            .await
        {
            Ok(cost) => (Some(cost.total_cost), Some(cost.currency)),
            Err(err) => {
                lwarn!(
                    request_id,
                    LogStage::Internal,
                    LogComponent::Statistics,
                    "cost_calculation_failed",
                    &format!("Failed to calculate cost: {err}")
                );
                (None, None)
            }
        }
    }
}
