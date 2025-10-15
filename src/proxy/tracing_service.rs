//! # 代理端追踪服务
//!
//! 从RequestHandler中提取的追踪相关逻辑，专门负责处理代理端的请求追踪需求
//! 包括请求追踪开始、完成、错误处理和扩展信息更新等功能

use crate::error::network::NetworkError;
use crate::error::ProxyError;
use crate::error::Result;
use crate::logging::{LogComponent, LogStage};
use crate::proxy::ProxyContext;
use crate::trace::immediate::ImmediateProxyTracer;
use crate::types::{ProviderTypeId, TokenCount};
use crate::{ldebug, linfo, lwarn};
use std::sync::Arc;

/// 代理端追踪服务
///
/// 专门处理代理请求的追踪逻辑，从RequestHandler中分离出来
/// 包含请求追踪的完整生命周期管理
pub struct TracingService {
    /// 即时写入追踪器
    tracer: Option<Arc<ImmediateProxyTracer>>,
}

impl TracingService {
    /// 创建新的追踪服务
    #[must_use]
    pub const fn new(tracer: Option<Arc<ImmediateProxyTracer>>) -> Self {
        Self { tracer }
    }

    /// 开始请求追踪
    ///
    /// 在认证成功后调用，记录请求开始信息
    #[allow(clippy::too_many_arguments)]
    pub async fn start_trace(
        &self,
        request_id: &str,
        user_service_api_id: i32,
        user_id: Option<i32>,
        provider_type_id: Option<ProviderTypeId>,
        user_provider_key_id: Option<i32>,
        method: &str,
        path: Option<String>,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<()> {
        if let Some(tracer) = &self.tracer {
            let start_params = crate::trace::immediate::StartTraceParams {
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
            tracer.start_trace(start_params).await.map_err(|e| {
                lwarn!(
                    request_id,
                    LogStage::Error,
                    LogComponent::Tracing,
                    "trace_start_failed",
                    "即时追踪启动失败",
                    error = format!("{:?}", e)
                );
                ProxyError::internal_with_source("Failed to start trace", e)
            })?;

            ldebug!(
                request_id,
                LogStage::RequestStart,
                LogComponent::Tracing,
                "trace_started",
                "请求追踪启动成功",
                user_service_api_id = user_service_api_id,
                provider_type_id = provider_type_id,
                user_provider_key_id = user_provider_key_id
            );
        }

        Ok(())
    }

    /// 更新模型信息（第一层：立即更新核心模型信息）
    ///
    /// 在获取到模型和后端信息时立即更新，确保核心追踪数据实时性
    pub async fn update_trace_model_info(
        &self,
        request_id: &str,
        provider_type_id: Option<ProviderTypeId>,
        model_used: Option<String>,
        user_provider_key_id: Option<i32>,
    ) -> Result<()> {
        if let Some(tracer) = &self.tracer {
            tracer
                .update_trace_model_info(
                    request_id,
                    provider_type_id,
                    model_used.clone(),
                    user_provider_key_id,
                )
                .await
                .map_err(|e| {
                    lwarn!(
                        request_id,
                        LogStage::Error,
                        LogComponent::Tracing,
                        "model_info_update_failed",
                        "模型信息更新失败（第一层）",
                        error = format!("{:?}", e)
                    );
                    ProxyError::internal_with_source("Failed to update model info", e)
                })?;

            linfo!(
                request_id,
                LogStage::RequestModify,
                LogComponent::Tracing,
                "model_info_updated",
                "模型信息更新成功（第一层：立即更新）",
                provider_type_id = provider_type_id,
                model_used = model_used,
                user_provider_key_id = user_provider_key_id
            );
        }

        Ok(())
    }

    /// 完成请求追踪（成功情况）（第二层：批量更新统计信息）
    ///
    /// 在请求成功处理完成时调用，一次性更新所有统计字段
    #[allow(clippy::too_many_arguments)]
    pub async fn complete_trace_success(
        &self,
        request_id: &str,
        status_code: u16,
        tokens_prompt: Option<TokenCount>,
        tokens_completion: Option<TokenCount>,
        tokens_total: Option<TokenCount>,
        model_used: Option<String>,
        cache_create_tokens: Option<TokenCount>,
        cache_read_tokens: Option<TokenCount>,
        cost: Option<f64>,
        cost_currency: Option<String>,
    ) -> Result<()> {
        if let Some(tracer) = &self.tracer {
            let complete_params = crate::trace::immediate::CompleteTraceParams {
                status_code,
                is_success: true,
                tokens_prompt,
                tokens_completion,
                error_type: None,    // no error type for success
                error_message: None, // no error message for success
                cache_create_tokens,
                cache_read_tokens,
                cost,
                cost_currency: cost_currency.clone(),
            };
            tracer
                .complete_trace_with_stats(request_id, complete_params)
                .await
                .map_err(|e| {
                    lwarn!(
                        request_id,
                        LogStage::Error,
                        LogComponent::Tracing,
                        "success_trace_complete_failed",
                        "成功请求追踪完成失败（第二层）",
                        error = format!("{:?}", e)
                    );
                    ProxyError::internal_with_source("Failed to complete trace", e)
                })?;

            linfo!(
                request_id,
                LogStage::Response,
                LogComponent::Tracing,
                "success_trace_completed",
                "成功请求追踪完成（第二层：批量更新）",
                status_code = status_code,
                tokens_prompt = tokens_prompt,
                tokens_completion = tokens_completion,
                tokens_total = tokens_total,
                cache_create_tokens = cache_create_tokens,
                cache_read_tokens = cache_read_tokens,
                cost = cost,
                cost_currency = cost_currency,
                model_used = model_used
            );
        }

        Ok(())
    }

    /// 完成请求追踪（失败情况）（第二层：批量更新统计信息）
    ///
    /// 在请求处理失败时调用，一次性更新错误信息
    pub async fn complete_trace_failure(
        &self,
        request_id: &str,
        status_code: u16,
        error_type: Option<String>,
        error_message: Option<String>,
    ) -> Result<()> {
        if let Some(tracer) = &self.tracer {
            let params = crate::trace::immediate::SimpleCompleteTraceParams {
                request_id: request_id.to_string(),
                status_code,
                is_success: false,
                tokens_prompt: None,
                tokens_completion: None,
                error_type: error_type.clone(),
                error_message: error_message.clone(),
            };
            tracer.complete_trace(params).await.map_err(|e| {
                lwarn!(
                    request_id,
                    LogStage::Error,
                    LogComponent::Tracing,
                    "failure_trace_complete_failed",
                    "失败请求追踪完成失败（第二层）",
                    error = format!("{:?}", e)
                );
                ProxyError::internal_with_source("Failed to complete trace", e)
            })?;

            linfo!(
                request_id,
                LogStage::ResponseFailure,
                LogComponent::Tracing,
                "failure_trace_completed",
                "失败请求追踪完成（第二层：批量更新）",
                status_code = status_code,
                error_type = error_type,
                error_message = error_message
            );
        }

        Ok(())
    }

    /// 批量完成追踪（用于通用错误处理）
    ///
    /// 提供一个便捷的方法来完成失败的追踪，使用标准的错误码和消息
    pub async fn complete_trace_with_error(
        &self,
        request_id: &str,
        error: &ProxyError,
    ) -> Result<()> {
        let (status_code, error_type) = match error {
            ProxyError::Authentication(_) => (401, Some("authentication_error".to_string())),
            ProxyError::Network(NetworkError::RateLimitExceeded) => {
                (429, Some("rate_limit_exceeded".to_string()))
            }
            ProxyError::Network(
                NetworkError::UpstreamNotAvailable(_) | NetworkError::UpstreamNotFound(_)
            ) => {
                (502, Some("upstream_error".to_string()))
            }
            ProxyError::Config(_) => (500, Some("configuration_error".to_string())),
            ProxyError::Internal { .. } => (500, Some("internal_error".to_string())),
            ProxyError::Network(
                NetworkError::ConnectionTimeout(_)
                | NetworkError::ReadTimeout(_)
                | NetworkError::WriteTimeout(_)
            ) => {
                (504, Some("timeout_error".to_string()))
            }
            _ => (500, Some("unknown_error".to_string())),
        };

        let error_message = Some(error.to_string());

        self.complete_trace_failure(request_id, status_code, error_type, error_message)
            .await
    }

    /// 检查是否启用了追踪
    #[must_use]
    pub const fn is_tracing_enabled(&self) -> bool {
        self.tracer.is_some()
    }

    /// 完成认证失败的追踪
    pub async fn complete_trace_auth_failure(&self, request_id: &str, message: &str) -> Result<()> {
        self.complete_trace_failure(
            request_id,
            401,
            Some("authentication_failed".to_string()),
            Some(message.to_string()),
        )
        .await
    }

    /// 完成速率限制失败的追踪
    pub async fn complete_trace_rate_limit(&self, request_id: &str, message: &str) -> Result<()> {
        self.complete_trace_failure(
            request_id,
            429,
            Some("rate_limit_exceeded".to_string()),
            Some(message.to_string()),
        )
        .await
    }

    /// 完成配置错误的追踪
    pub async fn complete_trace_config_error(&self, request_id: &str, message: &str) -> Result<()> {
        self.complete_trace_failure(
            request_id,
            500,
            Some("configuration_error".to_string()),
            Some(message.to_string()),
        )
        .await
    }

    /// 完成API密钥选择失败的追踪
    pub async fn complete_trace_api_key_selection_failed(
        &self,
        request_id: &str,
        message: &str,
    ) -> Result<()> {
        self.complete_trace_failure(
            request_id,
            503,
            Some("api_key_selection_failed".to_string()),
            Some(message.to_string()),
        )
        .await
    }

    /// 完成上游服务错误的追踪
    pub async fn complete_trace_upstream_error(
        &self,
        request_id: &str,
        message: &str,
    ) -> Result<()> {
        self.complete_trace_failure(
            request_id,
            502,
            Some("upstream_error".to_string()),
            Some(message.to_string()),
        )
        .await
    }
}

/// 追踪上下文助手
///
/// `提供从ProxyContext中提取追踪所需信息的便捷方法`
pub struct TracingContextHelper;

impl TracingContextHelper {
    /// `从ProxyContext提取用户服务API信息`
    #[must_use]
    pub fn extract_user_service_api_info(ctx: &ProxyContext) -> Option<(i32, Option<i32>)> {
        ctx.user_service_api
            .as_ref()
            .map(|api| (api.id, Some(api.user_id)))
    }

    /// `从ProxyContext提取提供商信息`
    #[must_use]
    pub fn extract_provider_info(ctx: &ProxyContext) -> Option<ProviderTypeId> {
        ctx.provider_type.as_ref().map(|pt| pt.id)
    }

    /// `从ProxyContext提取后端API密钥信息`
    #[must_use]
    pub fn extract_backend_key_info(ctx: &ProxyContext) -> Option<i32> {
        ctx.selected_backend.as_ref().map(|backend| backend.id)
    }

    /// `从ProxyContext提取模型信息`
    #[must_use]
    pub fn extract_model_info(ctx: &ProxyContext) -> Option<String> {
        // 使用最新请求模型（统计阶段会同步更新）
        ctx.requested_model.clone()
    }

    /// `从ProxyContext提取token信息`
    #[must_use]
    pub fn extract_token_info(
        ctx: &ProxyContext,
    ) -> (Option<TokenCount>, Option<TokenCount>, Option<TokenCount>) {
        let usage = ctx.usage_final.as_ref();
        let prompt = usage.and_then(|u| u.prompt_tokens);
        let completion = usage.and_then(|u| u.completion_tokens);
        let total = usage.and_then(|u| u.total_tokens).or(Some(0));
        (prompt, completion, total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_service_creation() {
        let service = TracingService::new(None);
        assert!(!service.is_tracing_enabled());
    }

    #[test]
    fn test_tracing_context_helper() {
        let ctx = ProxyContext::default();

        // 测试默认上下文的提取
        assert!(TracingContextHelper::extract_user_service_api_info(&ctx).is_none());
        assert!(TracingContextHelper::extract_provider_info(&ctx).is_none());
        assert!(TracingContextHelper::extract_backend_key_info(&ctx).is_none());
        assert!(TracingContextHelper::extract_model_info(&ctx).is_none());

        let (prompt, completion, total) = TracingContextHelper::extract_token_info(&ctx);
        assert!(prompt.is_none());
        assert!(completion.is_none());
        assert_eq!(total, Some(0));
    }

    #[tokio::test]
    async fn test_tracing_service_without_tracer() {
        let service = TracingService::new(None);

        // 所有方法在没有tracer的情况下都应该成功返回
        assert!(
            service
                .start_trace(
                    "test",
                    1,
                    Some(1),
                    None,
                    None,
                    "GET",
                    Some("/test".to_string()),
                    None,
                    None
                )
                .await
                .is_ok()
        );
        assert!(
            service
                .update_trace_model_info("test", Some(1), Some("model".to_string()), Some(1))
                .await
                .is_ok()
        );
        assert!(
            service
                .complete_trace_success(
                    "test",
                    200,
                    Some(10),
                    Some(20),
                    Some(30),
                    Some("model".to_string()),
                    Some(1),
                    Some(2),
                    Some(0.5),
                    Some("USD".to_string())
                )
                .await
                .is_ok()
        );
        assert!(
            service
                .complete_trace_failure(
                    "test",
                    500,
                    Some("error".to_string()),
                    Some("message".to_string())
                )
                .await
                .is_ok()
        );
    }
}
