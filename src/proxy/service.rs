//! # 核心代理服务 (Orchestrator)
//!
//! 实现了 Pingora 的 `ProxyHttp` trait，作为核心编排器，调用各个专有服务来处理请求。

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use pingora_core::prelude::*;
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::{ProxyHttp, Session};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;
use tracing::warn;
use uuid::Uuid;

use crate::logging::{LogComponent, LogStage};
use crate::proxy::{
    AuthenticationService, StatisticsService, TracingService, context::ProxyContext,
    provider_strategy, request_transform_service::RequestTransformService,
    response_transform_service::ResponseTransformService, upstream_service::UpstreamService,
};
use crate::{proxy_error, proxy_info};

/// 核心AI代理服务 - 作为编排器
pub struct ProxyService {
    db: Arc<DatabaseConnection>,
    auth_service: Arc<AuthenticationService>,
    stats_service: Arc<StatisticsService>,
    trace_service: Arc<TracingService>,
    upstream_service: Arc<UpstreamService>,
    req_transform_service: Arc<RequestTransformService>,
    resp_transform_service: Arc<ResponseTransformService>,
}

impl ProxyService {
    /// 创建新的代理服务实例
    pub fn new(
        db: Arc<DatabaseConnection>,
        auth_service: Arc<AuthenticationService>,
        stats_service: Arc<StatisticsService>,
        trace_service: Arc<TracingService>,
        upstream_service: Arc<UpstreamService>,
        req_transform_service: Arc<RequestTransformService>,
        resp_transform_service: Arc<ResponseTransformService>,
    ) -> pingora_core::Result<Self> {
        Ok(Self {
            db,
            auth_service,
            stats_service,
            trace_service,
            upstream_service,
            req_transform_service,
            resp_transform_service,
        })
    }
}

#[async_trait]
impl ProxyHttp for ProxyService {
    type CTX = ProxyContext;

    fn new_ctx(&self) -> Self::CTX {
        ProxyContext {
            request_id: Uuid::new_v4().to_string(),
            start_time: Instant::now(),
            ..Default::default()
        }
    }

    async fn early_request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        proxy_info!(
            &ctx.request_id,
            LogStage::RequestStart,
            LogComponent::Proxy,
            "downstream_request_start",
            "收到下游请求",
            method = session.req_header().method.as_str(),
            path = session.req_header().uri.path()
        );

        if session.req_header().method == "OPTIONS" {
            return Err(crate::pingora_http!(200, "CORS preflight"));
        }

        // 1. 执行完整的认证和授权流程
        match self
            .auth_service
            .authenticate_and_authorize(session, ctx)
            .await
        {
            Ok(auth_result) => {
                proxy_info!(
                    &ctx.request_id,
                    LogStage::Authentication,
                    LogComponent::AuthService,
                    "auth_ok",
                    "认证授权成功",
                    user_service_api_id = auth_result.user_service_api.id
                );
                let timeout = auth_result
                    .user_service_api
                    .timeout_seconds
                    .or(auth_result.provider_type.timeout_seconds)
                    .unwrap_or(30) as u64;
                ctx.timeout_seconds = Some(timeout as i32);
                session.set_read_timeout(Some(std::time::Duration::from_secs(timeout * 2)));
                session.set_write_timeout(Some(std::time::Duration::from_secs(timeout * 2)));

                // 2. 立即确定并存储ProviderStrategy
                if let Some(name) =
                    provider_strategy::ProviderRegistry::match_name(&auth_result.provider_type.name)
                {
                    ctx.strategy = provider_strategy::make_strategy(name, Some(self.db.clone()));
                }
            }
            Err(e) => {
                proxy_error!(
                    &ctx.request_id,
                    LogStage::Authentication,
                    LogComponent::AuthService,
                    "auth_fail",
                    "认证授权失败",
                    error = %e
                );
                let _ = self
                    .trace_service
                    .complete_trace_with_error(&ctx.request_id, &e)
                    .await;
                return Err(crate::pingora_error!(e));
            }
        }

        // 3. 收集请求统计信息并启动追踪
        let req_stats = self.stats_service.collect_request_stats(session);
        ctx.request_details = self
            .stats_service
            .collect_request_details(session, &req_stats);

        if let Some(user_api) = &ctx.user_service_api {
            let _ = self
                .trace_service
                .start_trace(
                    &ctx.request_id,
                    user_api.id,
                    Some(user_api.user_id),
                    req_stats.method.as_str(),
                    Some(req_stats.path.clone()),
                    Some(req_stats.client_ip.clone()),
                    req_stats.user_agent.clone(),
                )
                .await;
        }

        Ok(())
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<Box<HttpPeer>> {
        self.upstream_service.select_peer(ctx).await.map_err(|e| {
            let _ = self
                .trace_service
                .complete_trace_with_error(&ctx.request_id, &e);
            crate::pingora_error!(e)
        })
    }

    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        self.req_transform_service
            .filter_request(session, upstream_request, ctx)
            .await
            .map_err(|e| {
                let _ = self
                    .trace_service
                    .complete_trace_with_error(&ctx.request_id, &e);
                crate::pingora_error!(e)
            })
    }

    async fn request_body_filter(
        &self,
        session: &mut Session,
        body_chunk: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        // 处理当前分块数据（如果有）
        if let Some(chunk) = body_chunk.as_ref() {
            // 缓存数据到上下文
            ctx.request_body.extend_from_slice(chunk);

            // 记录接收到的数据
            tracing::info!(
                request_id = %ctx.request_id,
                chunk_size = chunk.len(),
                total_size = ctx.request_body.len(),
                end_of_stream = end_of_stream,
                "接收请求体数据块"
            );

            // 如果需要修改请求体且不是流结束，按照 Pingora 官方示例清空分块
            // 保持 HTTP 流式语义，避免原始与改写后的内容混合发送
            if ctx.will_modify_body && !end_of_stream {
                if let Some(chunk) = body_chunk {
                    chunk.clear();
                }
            }
        }

        // 流结束处理：处理完整的请求体
        if end_of_stream {
            tracing::info!(
                request_id = %ctx.request_id,
                body_size = ctx.request_body.len(),
                has_strategy = ctx.strategy.is_some(),
                will_modify = ctx.will_modify_body,
                "请求体接收完成，准备处理"
            );

            // 确保有完整的 body 数据才进行 JSON 修改
            let mut chunk_replaced = false;
            if !ctx.request_body.is_empty() && ctx.will_modify_body {
                if let Some(strategy) = &ctx.strategy {
                    match serde_json::from_slice::<Value>(&ctx.request_body) {
                        Ok(mut json_value) => {
                            tracing::debug!(
                                request_id = %ctx.request_id,
                                body = json_value.to_string(),
                                "请求体 JSON 解析成功，尝试应用策略修改"
                            );
                            match strategy
                                .modify_request_body_json(session, ctx, &mut json_value)
                                .await
                            {
                                Ok(true) => {
                                    tracing::debug!(
                                        request_id = %ctx.request_id,
                                        body = json_value.to_string(),
                                        "策略选择修改请求体，正在序列化回字节"
                                    );
                                    match serde_json::to_vec(&json_value) {
                                        Ok(serialized) => {
                                            // 更新 body 并重新设置到 chunk
                                            ctx.request_body = BytesMut::from(&serialized[..]);
                                            *body_chunk = Some(Bytes::from(serialized));
                                            chunk_replaced = true;
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                request_id = %ctx.request_id,
                                                error = %e,
                                                "序列化修改后的 JSON 失败"
                                            );
                                        }
                                    }
                                }
                                Ok(false) => {
                                    tracing::info!(
                                        request_id = %ctx.request_id,
                                        "策略选择不修改请求体"
                                    );
                                }
                                Err(e) => {
                                    tracing::error!(
                                        request_id = %ctx.request_id,
                                        error = %e,
                                        "执行请求体修改策略失败"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                request_id = %ctx.request_id,
                                error = %e,
                                body_preview = %String::from_utf8_lossy(&ctx.request_body[..std::cmp::min(500, ctx.request_body.len())]),
                                "解析请求体 JSON 失败"
                            );
                        }
                    }
                }
            } else if ctx.request_body.is_empty() && ctx.will_modify_body {
                tracing::warn!(
                    request_id = %ctx.request_id,
                    "策略期望修改请求体，但请求体为空"
                );
            }

            // 如果提前吞掉了分块但未能改写，确保把原始数据再发送出去
            if ctx.will_modify_body && !chunk_replaced {
                let original_body = Bytes::copy_from_slice(ctx.request_body.as_ref());
                *body_chunk = Some(original_body);
            }
        }

        Ok(())
    }

    async fn response_filter(
        &self,
        session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        self.resp_transform_service
            .filter_response(session, upstream_response, ctx)
            .await
            .map_err(|e| {
                let _ = self
                    .trace_service
                    .complete_trace_with_error(&ctx.request_id, &e);
                crate::pingora_error!(e)
            })?;

        let resp_stats = self
            .stats_service
            .collect_response_details(upstream_response, ctx);
        ctx.response_details.headers = resp_stats.headers;

        Ok(())
    }

    fn response_body_filter(
        &self,
        _session: &mut Session,
        body: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<Option<std::time::Duration>> {
        if let Some(chunk) = body.as_ref() {
            ctx.response_body.extend_from_slice(chunk);
        }

        if end_of_stream {
            if let Some(status_code) = ctx.response_details.status_code {
                if status_code >= 400 {
                    crate::logging::log_error_response(
                        &ctx.request_id,
                        &ctx.request_details.path,
                        status_code,
                        &ctx.response_body,
                    );
                }
            }
        }
        Ok(None)
    }

    async fn logging(&self, session: &mut Session, e: Option<&Error>, ctx: &mut Self::CTX) {
        let status_code =
            ctx.response_details
                .status_code
                .unwrap_or(if e.is_some() { 500 } else { 200 });
        let success = e.is_none() && status_code < 400;

        if let Some(strategy) = &ctx.strategy {
            if let Err(e) = strategy
                .handle_response_body(session, ctx, status_code, &ctx.response_body)
                .await
            {
                warn!(request_id = %ctx.request_id, error = %e, "Provider strategy handle_response_body failed");
            }
        }

        if success {
            let stats = crate::statistics::usage_model::finalize_eos(ctx);
            ctx.usage_final = Some(stats.usage.clone());
            if let Some(model_name) = stats.model_name {
                ctx.requested_model = Some(model_name);
            }

            if let Some(model_used) = ctx.requested_model.clone() {
                let provider_type_id = ctx.provider_type.as_ref().map(|p| p.id);
                let user_provider_key_id = ctx.selected_backend.as_ref().map(|k| k.id);
                if let Err(err) = self
                    .trace_service
                    .update_trace_model_info(
                        &ctx.request_id,
                        provider_type_id,
                        Some(model_used.clone()),
                        user_provider_key_id,
                    )
                    .await
                {
                    warn!(
                        request_id = %ctx.request_id,
                        error = %err,
                        "Failed to update trace model info"
                    );
                }
            }

            let (cost_value, cost_currency) = if let (Some(model), Some(usage)) =
                (ctx.requested_model.as_ref(), ctx.usage_final.as_ref())
            {
                if let Some(provider) = ctx.provider_type.as_ref() {
                    let pricing_usage = crate::pricing::TokenUsage {
                        prompt_tokens: usage.prompt_tokens,
                        completion_tokens: usage.completion_tokens,
                        cache_create_tokens: usage.cache_create_tokens,
                        cache_read_tokens: usage.cache_read_tokens,
                    };
                    match self
                        .stats_service
                        .calculate_cost_direct(model, provider.id, &pricing_usage, &ctx.request_id)
                        .await
                    {
                        Ok(cost_tuple) => cost_tuple,
                        Err(_) => (None, None),
                    }
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            let _ = self
                .trace_service
                .complete_trace_success(
                    &ctx.request_id,
                    status_code,
                    ctx.usage_final.as_ref().and_then(|u| u.prompt_tokens),
                    ctx.usage_final.as_ref().and_then(|u| u.completion_tokens),
                    ctx.usage_final
                        .as_ref()
                        .and_then(|u| u.total_tokens.map(|t| t as u32)),
                    ctx.requested_model.clone(),
                    ctx.usage_final.as_ref().and_then(|u| u.cache_create_tokens),
                    ctx.usage_final.as_ref().and_then(|u| u.cache_read_tokens),
                    cost_value,
                    cost_currency,
                )
                .await;
        } else if let Some(err) = e {
            let _ = self
                .trace_service
                .complete_trace_failure(
                    &ctx.request_id,
                    status_code,
                    Some(format!("{:?}", err.etype)),
                    Some(err.to_string()),
                )
                .await;
        }

        proxy_info!(
            &ctx.request_id,
            LogStage::Response,
            LogComponent::Proxy,
            "request_complete",
            "请求处理完成",
            status_code = status_code,
            duration_ms = ctx.start_time.elapsed().as_millis()
        );
    }
}
