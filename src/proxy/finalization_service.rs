//! # 请求收尾服务
//!
//! 将原先集中在 `ProxyService::logging` 中的统计、计费、追踪与缓存写入逻辑
//! 抽离为独立服务，减少编排器的职责负担。

use std::{convert::TryFrom, sync::Arc};

use crate::auth::rate_limit_dist::DistributedRateLimiter;
use crate::logging::{LogComponent, LogStage};
use crate::lwarn;
use crate::statistics::service::StatisticsService;
use crate::{logging::log_proxy_failure_details, pricing::TokenUsage};
use crate::{proxy::context::ProxyContext, proxy::tracing_service::TracingService};
use entity::user_service_apis;
use pingora_core::Error;

/// 请求收尾服务
pub struct FinalizationService {
    stats_service: Arc<StatisticsService>,
    trace_service: Arc<TracingService>,
    rate_limiter: Arc<DistributedRateLimiter>,
}

impl FinalizationService {
    /// 创建新的收尾服务实例
    #[must_use]
    pub const fn new(
        stats_service: Arc<StatisticsService>,
        trace_service: Arc<TracingService>,
        rate_limiter: Arc<DistributedRateLimiter>,
    ) -> Self {
        Self {
            stats_service,
            trace_service,
            rate_limiter,
        }
    }

    /// 执行请求收尾逻辑
    pub async fn finalize(&self, ctx: &mut ProxyContext, status_code: u16, error: Option<&Error>) {
        if status_code < 400 {
            self.handle_success(ctx, status_code).await;
        } else {
            self.handle_failure(ctx, status_code, error).await;
        }
    }

    async fn handle_success(&self, ctx: &mut ProxyContext, status_code: u16) {
        Self::finalize_usage(ctx);
        self.update_trace_model_info(ctx).await;

        let (cost_value, cost_currency) = self.calculate_cost(ctx).await;

        self.complete_success_trace(ctx, status_code, cost_value, cost_currency)
            .await;

        self.update_usage_caches(ctx, cost_value).await;
    }

    async fn handle_failure(&self, ctx: &ProxyContext, status_code: u16, error: Option<&Error>) {
        log_proxy_failure_details(&ctx.request_id, status_code, error, ctx);

        let (error_type, error_message) = error.map_or_else(
            || {
                (
                    Some(format!("HTTP {status_code}")),
                    Some(String::from_utf8_lossy(&ctx.response_body).to_string()),
                )
            },
            |err| (Some(format!("{:?}", err.etype)), Some(err.to_string())),
        );

        let _ = self
            .trace_service
            .complete_trace_failure(&ctx.request_id, status_code, error_type, error_message)
            .await;
    }

    fn finalize_usage(ctx: &mut ProxyContext) {
        let stats = crate::statistics::usage_model::finalize_eos(ctx);
        ctx.usage_final = Some(stats.usage.clone());
        if let Some(model_name) = stats.model_name {
            ctx.requested_model = Some(model_name);
        }
    }

    async fn update_trace_model_info(&self, ctx: &ProxyContext) {
        let Some(model_used) = ctx.requested_model.clone() else {
            return;
        };

        let provider_type_id = ctx.provider_type.as_ref().map(|p| p.id);
        let user_provider_key_id = ctx.selected_backend.as_ref().map(|k| k.id);
        if let Err(err) = self
            .trace_service
            .update_trace_model_info(
                &ctx.request_id,
                provider_type_id,
                Some(model_used),
                user_provider_key_id,
            )
            .await
        {
            lwarn!(
                &ctx.request_id,
                LogStage::Error,
                LogComponent::Tracing,
                "update_trace_model_info_failed",
                &format!("Failed to update trace model info: {err}")
            );
        }
    }

    async fn calculate_cost(&self, ctx: &ProxyContext) -> (Option<f64>, Option<String>) {
        let (Some(model), Some(usage)) = (ctx.requested_model.as_ref(), ctx.usage_final.as_ref())
        else {
            return (None, None);
        };

        let Some(provider) = ctx.provider_type.as_ref() else {
            return (None, None);
        };

        let pricing_usage = TokenUsage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            cache_create_tokens: usage.cache_create_tokens,
            cache_read_tokens: usage.cache_read_tokens,
        };

        self.stats_service
            .calculate_cost_direct(model, provider.id, &pricing_usage, &ctx.request_id)
            .await
            .unwrap_or_default()
    }

    async fn complete_success_trace(
        &self,
        ctx: &ProxyContext,
        status_code: u16,
        cost_value: Option<f64>,
        cost_currency: Option<String>,
    ) {
        let _ = self
            .trace_service
            .complete_trace_success(
                &ctx.request_id,
                status_code,
                ctx.usage_final.as_ref().and_then(|u| u.prompt_tokens),
                ctx.usage_final.as_ref().and_then(|u| u.completion_tokens),
                ctx.usage_final.as_ref().and_then(|u| u.total_tokens),
                ctx.requested_model.clone(),
                ctx.usage_final.as_ref().and_then(|u| u.cache_create_tokens),
                ctx.usage_final.as_ref().and_then(|u| u.cache_read_tokens),
                cost_value,
                cost_currency,
            )
            .await;
    }

    async fn update_usage_caches(&self, ctx: &ProxyContext, cost_value: Option<f64>) {
        if let Some(user_api) = ctx.user_service_api.as_ref() {
            self.update_request_cache(ctx, user_api).await;
            self.update_token_cache(ctx, user_api).await;
            self.update_cost_cache(ctx, user_api, cost_value).await;
        }
    }

    async fn update_request_cache(&self, ctx: &ProxyContext, user_api: &user_service_apis::Model) {
        let endpoint = format!("service_api:{}", user_api.id);
        if let Err(e) = self
            .rate_limiter
            .increment_daily_request_cache(user_api.user_id, &endpoint, 1)
            .await
        {
            lwarn!(
                &ctx.request_id,
                LogStage::Internal,
                LogComponent::Cache,
                "request_cache_update_failed",
                &format!("Failed to update daily request cache: {e}")
            );
        }
    }

    async fn update_token_cache(&self, ctx: &ProxyContext, user_api: &user_service_apis::Model) {
        let Some(usage) = ctx.usage_final.as_ref() else {
            return;
        };
        let Some(total_tokens) = usage.total_tokens else {
            return;
        };

        match i64::try_from(total_tokens) {
            Ok(delta_tokens) if delta_tokens > 0 => {
                if let Err(e) = self
                    .rate_limiter
                    .increment_daily_token_cache(user_api.id, delta_tokens)
                    .await
                {
                    lwarn!(
                        &ctx.request_id,
                        LogStage::Internal,
                        LogComponent::Cache,
                        "token_cache_update_failed",
                        &format!("Failed to update daily token cache: {e}")
                    );
                }
            }
            Ok(_) => {}
            Err(_) => {
                lwarn!(
                    &ctx.request_id,
                    LogStage::Internal,
                    LogComponent::Cache,
                    "token_cache_overflow",
                    "Token usage exceeded i64::MAX, skipping cache update"
                );
            }
        }
    }

    async fn update_cost_cache(
        &self,
        ctx: &ProxyContext,
        user_api: &user_service_apis::Model,
        cost_value: Option<f64>,
    ) {
        if let Some(cost) = cost_value
            && let Err(e) = self
                .rate_limiter
                .increment_daily_cost_cache(user_api.id, cost)
                .await
        {
            lwarn!(
                &ctx.request_id,
                LogStage::Internal,
                LogComponent::Cache,
                "cost_cache_update_failed",
                &format!("Failed to update daily cost cache: {e}")
            );
        }
    }
}
