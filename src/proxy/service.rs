//! # 核心代理服务 (Orchestrator)
//!
//! 实现了 Pingora 的 `ProxyHttp` trait，作为核心编排器，调用各个专有服务来处理请求。

use async_trait::async_trait;
use bytes::Bytes;
use pingora_core::prelude::*;
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::{ProxyHttp, Session};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::proxy::{
    AuthenticationService, StatisticsService, TracingService, context::ProxyContext,
    provider_strategy, request_transform_service::RequestTransformService,
    response_transform_service::ResponseTransformService, upstream_service::UpstreamService,
};

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

const COMPONENT: &str = "proxy.service";

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
        info!(
            event = "downstream_request_start",
            component = COMPONENT,
            request_id = %ctx.request_id,
            method = session.req_header().method.as_str(),
            path = session.req_header().uri.path(),
            "收到下游请求"
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
                info!(event = "auth_ok", component = COMPONENT, request_id = %ctx.request_id, user_service_api_id = auth_result.user_service_api.id, "认证授权成功");
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
                error!(event = "auth_fail", component = COMPONENT, request_id = %ctx.request_id, error = %e, "认证授权失败");
                let _ = self
                    .trace_service
                    .complete_trace_with_error(&ctx.request_id, &e)
                    .await;
                return Err(crate::pingora_error!(e));
            }
        }

        // 3. 启动追踪
        if let Some(user_api) = &ctx.user_service_api {
            let req_stats = self.stats_service.collect_request_stats(session);
            let _ = self
                .trace_service
                .start_trace(
                    &ctx.request_id,
                    user_api.id,
                    Some(user_api.user_id),
                    session.req_header().method.as_str(),
                    Some(session.req_header().uri.path().to_string()),
                    Some(req_stats.client_ip),
                    req_stats.user_agent,
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
        _session: &mut Session,
        body_chunk: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        if let Some(chunk) = body_chunk.take() {
            ctx.request_body.extend_from_slice(&chunk);
        }

        if end_of_stream && !ctx.request_body.is_empty() {
            if let (Ok(mut json_value), Some(strategy)) = (
                serde_json::from_slice::<Value>(&ctx.request_body),
                &ctx.strategy,
            ) {
                if let Ok(true) = strategy
                    .modify_request_body_json(_session, ctx, &mut json_value)
                    .await
                {
                    if let Ok(serialized) = serde_json::to_vec(&json_value) {
                        *body_chunk = Some(Bytes::from(serialized));
                    }
                }
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
            })
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

            let _cost = if let (Some(model), Some(usage)) =
                (ctx.requested_model.as_ref(), ctx.usage_final.as_ref())
            {
                if let Some(provider) = ctx.provider_type.as_ref() {
                    let pricing_usage = crate::pricing::TokenUsage {
                        prompt_tokens: usage.prompt_tokens,
                        completion_tokens: usage.completion_tokens,
                        cache_create_tokens: usage.cache_create_tokens,
                        cache_read_tokens: usage.cache_read_tokens,
                    };
                    self.stats_service
                        .calculate_cost_direct(model, provider.id, &pricing_usage, &ctx.request_id)
                        .await
                        .ok()
                } else {
                    None
                }
            } else {
                None
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

        info!(
            event = "request_complete",
            component = COMPONENT,
            request_id = %ctx.request_id,
            status_code = status_code,
            duration_ms = ctx.start_time.elapsed().as_millis(),
            "请求处理完成"
        );
    }
}
