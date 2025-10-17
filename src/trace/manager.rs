//! Trace 模块统一入口

use std::convert::TryFrom;
use std::sync::Arc;

use crate::auth::rate_limit_dist::DistributedRateLimiter;
use crate::collect::types::CollectedMetrics;
use crate::logging::{LogComponent, LogStage, log_proxy_failure_details};
use crate::proxy::ProxyContext;
use crate::trace::immediate::{CompleteTraceParams, ImmediateProxyTracer, StartTraceParams};
use crate::{error::ProxyError, error::Result, linfo, lwarn};
use pingora_core::Error as PingoraError;

/// 统一的请求追踪管理器
pub struct TraceManager {
    tracer: Option<Arc<ImmediateProxyTracer>>,
    rate_limiter: Arc<DistributedRateLimiter>,
}

impl TraceManager {
    #[must_use]
    pub const fn new(
        tracer: Option<Arc<ImmediateProxyTracer>>,
        rate_limiter: Arc<DistributedRateLimiter>,
    ) -> Self {
        Self {
            tracer,
            rate_limiter,
        }
    }

    /// 开始追踪（认证成功后调用）
    #[allow(clippy::too_many_arguments)]
    pub async fn start_trace(
        &self,
        request_id: &str,
        user_service_api_id: i32,
        user_id: Option<i32>,
        provider_type_id: Option<i32>,
        user_provider_key_id: Option<i32>,
        method: &str,
        path: Option<String>,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<()> {
        let Some(tracer) = &self.tracer else {
            return Ok(());
        };

        let params = StartTraceParams {
            request_id: request_id.to_string(),
            user_service_api_id,
            user_id,
            provider_type_id,
            user_provider_key_id,
            method: method.to_string(),
            path,
            client_ip,
            user_agent,
        };

        tracer.start_trace(params).await.map_err(|e| {
            lwarn!(
                request_id,
                LogStage::Error,
                LogComponent::Tracing,
                "trace_start_failed",
                "即时追踪启动失败",
                error = format!("{:?}", e)
            );
            ProxyError::internal_with_source("Failed to start trace", e)
        })
    }

    /// 更新模型信息（在解析完成后调用）
    pub async fn update_model(
        &self,
        request_id: &str,
        provider_type_id: Option<i32>,
        model_used: Option<String>,
        user_provider_key_id: Option<i32>,
    ) {
        let Some(tracer) = &self.tracer else {
            return;
        };

        if let Err(err) = tracer
            .update_trace_model_info(
                request_id,
                provider_type_id,
                model_used.clone(),
                user_provider_key_id,
            )
            .await
        {
            lwarn!(
                request_id,
                LogStage::Error,
                LogComponent::Tracing,
                "model_info_update_failed",
                "模型信息更新失败",
                error = format!("{:?}", err)
            );
        } else {
            linfo!(
                request_id,
                LogStage::RequestModify,
                LogComponent::Tracing,
                "model_info_updated",
                "模型信息更新成功",
                provider_type_id = provider_type_id,
                model_used = model_used,
                user_provider_key_id = user_provider_key_id
            );
        }
    }

    /// 记录成功请求
    pub async fn record_success(
        &self,
        metrics: &CollectedMetrics,
        ctx: &ProxyContext,
    ) -> Result<()> {
        self.write_success_trace(metrics).await?;
        self.update_rate_limits(metrics, ctx).await;
        Ok(())
    }

    /// 记录失败请求
    pub async fn record_failure(
        &self,
        metrics: Option<&CollectedMetrics>,
        status_code: u16,
        error: Option<&PingoraError>,
        ctx: &ProxyContext,
    ) {
        log_proxy_failure_details(&ctx.request_id, status_code, error, ctx);

        if let Some(tracer) = &self.tracer {
            let (error_type, error_message) = error.map_or_else(
                || (Some(format!("HTTP {status_code}")), Some(String::new())),
                |err| (Some(format!("{:?}", err.etype)), Some(err.to_string())),
            );

            let params = CompleteTraceParams {
                status_code,
                is_success: false,
                tokens_prompt: metrics.and_then(|m| m.usage.prompt_tokens),
                tokens_completion: metrics.and_then(|m| m.usage.completion_tokens),
                error_type,
                error_message,
                cache_create_tokens: metrics.and_then(|m| m.usage.cache_create_tokens),
                cache_read_tokens: metrics.and_then(|m| m.usage.cache_read_tokens),
                cost: metrics.and_then(|m| m.cost.value),
                cost_currency: metrics.and_then(|m| m.cost.currency.clone()),
            };

            if let Err(e) = tracer
                .complete_trace_with_stats(&ctx.request_id, params)
                .await
            {
                lwarn!(
                    &ctx.request_id,
                    LogStage::Error,
                    LogComponent::Tracing,
                    "failure_trace_complete_failed",
                    "失败请求追踪完成失败",
                    error = format!("{:?}", e)
                );
            }
        }
    }

    async fn write_success_trace(&self, metrics: &CollectedMetrics) -> Result<()> {
        let Some(tracer) = &self.tracer else {
            return Ok(());
        };

        let params = CompleteTraceParams {
            status_code: metrics.status_code,
            is_success: true,
            tokens_prompt: metrics.usage.prompt_tokens,
            tokens_completion: metrics.usage.completion_tokens,
            error_type: None,
            error_message: None,
            cache_create_tokens: metrics.usage.cache_create_tokens,
            cache_read_tokens: metrics.usage.cache_read_tokens,
            cost: metrics.cost.value,
            cost_currency: metrics.cost.currency.clone(),
        };

        tracer
            .complete_trace_with_stats(&metrics.request_id, params)
            .await
            .map_err(|e| {
                lwarn!(
                    &metrics.request_id,
                    LogStage::Error,
                    LogComponent::Tracing,
                    "success_trace_complete_failed",
                    "成功请求追踪完成失败",
                    error = format!("{:?}", e)
                );
                ProxyError::internal_with_source("Failed to complete trace", e)
            })
    }

    async fn update_rate_limits(&self, metrics: &CollectedMetrics, ctx: &ProxyContext) {
        let Some(user_api) = ctx.user_service_api.as_ref() else {
            return;
        };

        self.update_request_cache(metrics, user_api).await;
        self.update_token_cache(metrics, user_api).await;
        self.update_cost_cache(metrics, user_api).await;
    }

    async fn update_request_cache(
        &self,
        metrics: &CollectedMetrics,
        user_api: &entity::user_service_apis::Model,
    ) {
        let endpoint = format!("service_api:{}", user_api.id);
        if let Err(e) = self
            .rate_limiter
            .increment_daily_request_cache(user_api.user_id, &endpoint, 1)
            .await
        {
            lwarn!(
                &metrics.request_id,
                LogStage::Internal,
                LogComponent::Cache,
                "request_cache_update_failed",
                &format!("Failed to update daily request cache: {e}")
            );
        }
    }

    async fn update_token_cache(
        &self,
        metrics: &CollectedMetrics,
        user_api: &entity::user_service_apis::Model,
    ) {
        let Some(total_tokens) = metrics.usage.total_tokens.filter(|&tokens| tokens > 0) else {
            return;
        };

        let Ok(total_tokens_i64) = i64::try_from(total_tokens) else {
            lwarn!(
                &metrics.request_id,
                LogStage::Internal,
                LogComponent::Cache,
                "token_cache_overflow",
                &format!("Token count {total_tokens} exceeds i64 range")
            );
            return;
        };

        if let Err(e) = self
            .rate_limiter
            .increment_daily_token_cache(user_api.id, total_tokens_i64)
            .await
        {
            lwarn!(
                &metrics.request_id,
                LogStage::Internal,
                LogComponent::Cache,
                "token_cache_update_failed",
                &format!("Failed to update daily token cache: {e}")
            );
        }
    }

    async fn update_cost_cache(
        &self,
        metrics: &CollectedMetrics,
        user_api: &entity::user_service_apis::Model,
    ) {
        let Some(cost) = metrics.cost.value else {
            return;
        };

        if let Err(e) = self
            .rate_limiter
            .increment_daily_cost_cache(user_api.id, cost)
            .await
        {
            lwarn!(
                &metrics.request_id,
                LogStage::Internal,
                LogComponent::Cache,
                "cost_cache_update_failed",
                &format!("Failed to update daily cost cache: {e}")
            );
        }
    }
}
