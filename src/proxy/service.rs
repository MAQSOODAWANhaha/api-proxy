//! # 核心代理服务 (Orchestrator)
//!
//! 实现了 Pingora 的 `ProxyHttp` trait，作为核心编排器，调用各个专有服务来处理请求。

use crate::error::ProxyError;
use crate::logging::{self, ErrorLogField, LogComponent, LogStage, log_proxy_error};
use crate::{ldebug, lerror, linfo, lwarn};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use pingora_core::prelude::*;
use pingora_core::{Error as PingoraError, ErrorType};
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::{ProxyHttp, Session};
use serde_json::{Value, json};
use std::convert::TryFrom;
use std::sync::Arc;
use std::time::Instant;
use tokio::time::Duration;
use uuid::Uuid;

use crate::proxy::context::ProxyContext;
use crate::proxy::provider_strategy;
use crate::proxy::response::{JsonError, build_auth_error_response, write_json_error};
use crate::proxy::retry_policy;
use crate::proxy::state::ProxyState;

/// 核心AI代理服务 - 作为编排器
pub struct ProxyService {
    state: Arc<ProxyState>,
}

impl ProxyService {
    const DEFAULT_BASE_RETRY_DELAY_MS: u64 = 500;
    const SSE_KEEPALIVE_PREFIX: &'static [u8] = b":\n\n";
    const SSE_CONTENT_TYPE: &'static str = "text/event-stream";
    const MAX_BODY_BUFFER_BYTES: usize = 2 * 1024 * 1024;

    /// 创建新的代理服务实例
    pub const fn new(state: Arc<ProxyState>) -> pingora_core::Result<Self> {
        Ok(Self { state })
    }

    /// 检测是否为连接失败错误
    fn is_connection_failure(error: Option<&Error>) -> bool {
        error.is_some_and(|err| match err.etype {
            ErrorType::ConnectionClosed
            | ErrorType::ConnectTimedout
            | ErrorType::ReadTimedout
            | ErrorType::WriteTimedout
            | ErrorType::HTTPStatus(0) => true,
            ErrorType::CustomCode(_, code) => (500..600).contains(&code),
            _ => false,
        })
    }

    /// 检测是否为部分响应错误（已收到响应体数据）
    const fn is_partial_response_error(ctx: &ProxyContext) -> bool {
        ctx.response.body_received_size > 0
    }

    /// 获取本次请求允许的最大重试次数（不包含首次尝试）
    fn max_retry_budget(ctx: &ProxyContext) -> u32 {
        let retry_count = ctx
            .routing
            .user_service_api
            .as_ref()
            .and_then(|api| api.retry_count)
            .unwrap_or(0)
            .max(0);
        u32::try_from(retry_count).unwrap_or(0)
    }

    /// 获取数据库配置的超时上限（秒），用于限制退避等待的最大时长
    ///
    /// - 若未设置或非法（<=0），按迁移默认值 30 秒处理
    fn db_timeout_seconds(ctx: &ProxyContext) -> u64 {
        let configured = ctx
            .routing
            .user_service_api
            .as_ref()
            .and_then(|api| api.timeout_seconds);
        let secs = configured.unwrap_or(30);
        let secs = if secs <= 0 { 30 } else { secs };
        u64::try_from(secs).unwrap_or(30)
    }

    fn max_retry_delay_ms(ctx: &ProxyContext) -> u64 {
        Self::db_timeout_seconds(ctx).saturating_mul(1000)
    }

    fn is_sse_content_type(content_type: Option<&str>) -> bool {
        content_type.is_some_and(|ct| ct.to_ascii_lowercase().contains(Self::SSE_CONTENT_TYPE))
    }

    fn append_body_with_limit(
        buffer: &mut BytesMut,
        total_size: &mut usize,
        truncated: &mut bool,
        chunk: &[u8],
        limit: usize,
    ) -> bool {
        *total_size = total_size.saturating_add(chunk.len());
        if *truncated {
            return false;
        }

        let remaining = limit.saturating_sub(buffer.len());
        if remaining == 0 {
            *truncated = true;
            return true;
        }

        if chunk.len() <= remaining {
            buffer.extend_from_slice(chunk);
            return false;
        }

        buffer.extend_from_slice(&chunk[..remaining]);
        *truncated = true;
        true
    }

    /// 判断是否应对上游状态码进行重试（仅基于状态码维度）
    const fn should_retry_upstream_status(status_code: u16) -> bool {
        // 最佳实践：仅对常见“临时性”错误码重试，避免对不可能成功的请求浪费资源与引入重复计费风险。
        matches!(status_code, 429 | 500 | 502 | 503 | 504)
    }

    /// 判断当前请求是否具备“可安全重试”的前提（请求体可重放）
    fn is_safe_to_retry(session: &mut Session) -> bool {
        // Pingora 的重试依赖 request retry buffer；如果缓冲被截断则不应重试。
        if session.retry_buffer_truncated() {
            return false;
        }
        // 无 body 的请求天然可重试；有 body 时需要确保 retry buffer 存在。
        if session.is_body_empty() {
            return true;
        }
        session.get_retry_buffer().is_some()
    }

    /// 在允许的情况下标记错误可重试，并推进上下文重试计数
    fn apply_retry_policy(
        session: &mut Session,
        ctx: &mut ProxyContext,
        err: &mut Error,
        reason: &'static str,
        status_code: Option<u16>,
    ) {
        // 检查部分响应错误（需要在调用重试策略前单独检查）
        if Self::is_partial_response_error(ctx) {
            if !ctx.control.retry.try_mark_policy_applied() {
                return;
            }
            err.set_retry(false);
            ldebug!(
                &ctx.request_id,
                LogStage::ResponseFailure,
                LogComponent::Proxy,
                "retry_skipped",
                "未触发重试（已收到部分响应）",
                reason = reason,
                status_code = status_code,
                response_body_size = ctx.response.body_received_size
            );
            return;
        }

        // 调用简化的重试策略评估
        retry_policy::apply_retry_policy(
            session,
            ctx,
            err,
            reason,
            status_code,
            Self::DEFAULT_BASE_RETRY_DELAY_MS,
            u32::try_from(Self::max_retry_delay_ms(ctx)).unwrap_or(u32::MAX),
        );
    }

    /// 每次重试开始前重置上下文中与上一轮响应相关的缓存
    fn reset_ctx_for_retry(ctx: &mut ProxyContext) {
        ctx.response.details = crate::collect::types::ResponseDetails::default();
        ctx.response.body = BytesMut::new();
        ctx.response.body_received_size = 0;
        ctx.response.body_truncated = false;
        ctx.response.is_sse = false;
        ctx.response.sse_keepalive_sent = false;
        // 注意：重试时 Pingora 会从内部 retry buffer 重放请求体，并再次调用 `request_body_filter`。
        // 这里清空 `ctx.request.body` 仅影响本地缓存/日志与“基于完整 body 的改写逻辑”，不会导致上游请求体丢失。
        ctx.request.body = BytesMut::new();
        ctx.request.body_received_size = 0;
        ctx.request.body_truncated = false;
        ctx.trace.upstream_request_headers = None;
        ctx.trace.upstream_request_uri = None;
        ctx.response.usage_final = None;
        ctx.request.requested_model = None;
        ctx.control.retry.reset_for_new_attempt();
    }

    /// 重构的状态码解析函数 - 优先使用Pingora错误信息
    fn resolve_status_code(ctx: &ProxyContext, error: Option<&Error>) -> u16 {
        // 第一优先级：检查连接失败错误
        if Self::is_connection_failure(error) {
            // 如果有连接失败，优先返回502/504而不是缓存的HTTP状态码
            if let Some(err) = error {
                return match err.etype {
                    ErrorType::ReadTimedout | ErrorType::WriteTimedout => 504,
                    ErrorType::CustomCode(_, code) if (500..600).contains(&code) => code,
                    _ => 502,
                };
            }
            return 502;
        }

        // 第二优先级：检查Pingora错误中的HTTP状态码
        if let Some(err) = error {
            match err.etype {
                ErrorType::HTTPStatus(code) | ErrorType::CustomCode(_, code) => return code,
                _ => {}
            }
        }

        // 第三优先级：只有在没有错误时才使用上下文中的状态码
        if error.is_none()
            && let Some(status) = ctx.response.details.status_code
        {
            return status;
        }

        // 最后回退：根据错误情况返回默认状态码
        if error.is_some() {
            500 // 服务器内部错误
        } else {
            200 // 没有错误且没有状态码时默认成功
        }
    }

    fn configure_timeouts_and_strategy(&self, session: &mut Session, ctx: &mut ProxyContext) {
        if let (Some(user_api), Some(provider_type)) = (
            ctx.routing.user_service_api.as_ref(),
            ctx.routing.provider_type.as_ref(),
        ) {
            // 允许显式配置较小超时；非正数回退默认值
            let configured = user_api.timeout_seconds.unwrap_or(120);
            let timeout = if configured <= 0 { 120 } else { configured };

            ctx.control.timeout_seconds = Some(timeout);

            let timeout_u64 = u64::try_from(timeout).unwrap_or(120);
            let timeout_duration = std::time::Duration::from_secs(timeout_u64 * 2);
            session.set_read_timeout(Some(timeout_duration));
            session.set_write_timeout(Some(timeout_duration));

            if let Some(name) = provider_strategy::ProviderRegistry::match_name(&provider_type.name)
            {
                ctx.routing.strategy = provider_strategy::make_strategy(
                    name,
                    Some(
                        self.state
                            .key_scheduler_service
                            .api_key_health_service()
                            .clone(),
                    ),
                );
            }
        }
    }

    async fn collect_request_metadata(&self, session: &Session, ctx: &mut ProxyContext) {
        let req_stats = self.state.collect_service.collect_request_stats(session);
        ctx.request.details = self
            .state
            .collect_service
            .collect_request_details(session, &req_stats);

        if let Some(user_api) = &ctx.routing.user_service_api {
            let provider_type_id = ctx.routing.provider_type.as_ref().map(|p| p.id);
            let user_provider_key_id = ctx
                .routing
                .selected_backend
                .as_ref()
                .map(|backend| backend.id);
            if matches!(
                self.state
                    .trace_manager
                    .start_trace(
                        &ctx.request_id,
                        user_api.id,
                        Some(user_api.user_id),
                        provider_type_id,
                        user_provider_key_id,
                        req_stats.method.as_str(),
                        Some(req_stats.path.clone()),
                        Some(req_stats.client_ip.clone()),
                        req_stats.user_agent.clone(),
                    )
                    .await,
                Ok(true)
            ) {
                ctx.mark_trace_started();
            }
        }
    }

    async fn send_auth_error_response(
        &self,
        session: &mut Session,
        request_id: &str,
        error: &ProxyError,
    ) -> pingora_core::Result<Option<u16>> {
        if let ProxyError::Authentication(auth_err) = error {
            let JsonError {
                status,
                payload,
                message,
            } = build_auth_error_response(auth_err);
            if status == 401 {
                lwarn!(
                    request_id,
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "authentication_failed",
                    "认证失败",
                    error = message
                );
            } else {
                lwarn!(
                    request_id,
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "usage_limit_reached",
                    "速率限制触发，返回结构化错误",
                    error = message
                );
            }
            write_json_error(session, status, payload).await?;
            return Ok(Some(status));
        }
        Ok(None)
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
        linfo!(
            &ctx.request_id,
            LogStage::RequestStart,
            LogComponent::Proxy,
            "downstream_request_start",
            "收到下游请求",
            method = session.req_header().method.as_str(),
            path = session.req_header().uri.path()
        );

        if session.req_header().method == "OPTIONS" {
            let mut resp = ResponseHeader::build(204, Some(4))
                .map_err(|err| PingoraError::explain(ErrorType::InternalError, err.to_string()))?;
            let _ = resp.insert_header("access-control-allow-origin", "*");
            let _ = resp.insert_header(
                "access-control-allow-methods",
                "GET, POST, PUT, DELETE, OPTIONS",
            );
            let _ = resp.insert_header(
                "access-control-allow-headers",
                "Content-Type, Authorization",
            );
            session.write_response_header(Box::new(resp), false).await?;
            session.write_response_body(None, true).await?;
            return Err(PingoraError::explain(
                ErrorType::HTTPStatus(204),
                "Preflight handled by proxy".to_string(),
            ));
        }

        // 1. 执行完整的认证和授权流程
        if let Err(e) = self
            .state
            .auth_service
            .authenticate_and_authorize(session, ctx)
            .await
        {
            log_proxy_error(
                &ctx.request_id,
                LogStage::Authentication,
                LogComponent::Auth,
                "auth_fail",
                "认证授权失败",
                &e,
                &[
                    ErrorLogField::new("path", json!(session.req_header().uri.path())),
                    ErrorLogField::new("method", json!(session.req_header().method.as_str())),
                ],
            );
            if let Some(status) = self
                .send_auth_error_response(session, &ctx.request_id, &e)
                .await?
            {
                let context = format!("{}:{}", e.error_code(), e);
                return Err(PingoraError::explain(
                    ErrorType::HTTPStatus(status),
                    context,
                ));
            }
            return Err(e.into());
        }

        linfo!(
            &ctx.request_id,
            LogStage::Authentication,
            LogComponent::Auth,
            "auth_ok",
            "认证授权成功",
            user_service_api_id = ctx.routing.user_service_api.as_ref().map(|u| u.id)
        );

        self.configure_timeouts_and_strategy(session, ctx);
        self.collect_request_metadata(session, ctx).await;

        Ok(())
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<Box<HttpPeer>> {
        // 每次重试前执行退避（如果上一次失败设置了 delay）
        if let Some(delay_ms) = ctx.control.retry.next_retry_delay_ms.take()
            && delay_ms > 0
        {
            linfo!(
                &ctx.request_id,
                LogStage::UpstreamRequest,
                LogComponent::Proxy,
                "retry_backoff_sleep",
                "重试退避等待",
                delay_ms = delay_ms,
                attempt = ctx.control.retry.retry_count,
                status_code = ctx.control.retry.last_retry_status_code
            );
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }

        // Pingora 在重试时会再次调用 upstream_peer，这里清理上一轮尝试的响应/请求缓存，避免混淆统计与日志。
        if ctx.control.retry.retry_count > 0 {
            Self::reset_ctx_for_retry(ctx);
        }
        let peer = self.state.upstream_service.select_peer(ctx).await?;
        Ok(peer)
    }

    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        self.state
            .req_transform_service
            .filter_request(session, upstream_request, ctx)
            .await?;

        if ctx
            .routing
            .user_service_api
            .as_ref()
            .is_some_and(|api| api.log_mode)
        {
            ctx.trace.upstream_request_headers =
                Some(logging::headers_json_map_request(upstream_request));
            ctx.trace.upstream_request_uri = Some(upstream_request.uri.to_string());
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    async fn request_body_filter(
        &self,
        session: &mut Session,
        body_chunk: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        // 处理当前分块数据（如果有）
        if let Some(chunk) = body_chunk.as_ref() {
            if ctx.request.will_modify_body {
                ctx.request.body_received_size =
                    ctx.request.body_received_size.saturating_add(chunk.len());
                ctx.request.body.extend_from_slice(chunk);
            } else {
                let newly_truncated = Self::append_body_with_limit(
                    &mut ctx.request.body,
                    &mut ctx.request.body_received_size,
                    &mut ctx.request.body_truncated,
                    chunk,
                    Self::MAX_BODY_BUFFER_BYTES,
                );
                if newly_truncated {
                    lwarn!(
                        &ctx.request_id,
                        LogStage::RequestModify,
                        LogComponent::Proxy,
                        "request_body_truncated",
                        "请求体缓存超过上限，已截断",
                        buffer_limit_bytes = Self::MAX_BODY_BUFFER_BYTES,
                        received_bytes = ctx.request.body_received_size
                    );
                }
            }
            // 如果需要修改请求体且不是流结束，按照 Pingora 官方示例清空分块
            // 保持 HTTP 流式语义，避免原始与改写后的内容混合发送
            if ctx.request.will_modify_body
                && !end_of_stream
                && let Some(chunk) = body_chunk
            {
                chunk.clear();
            }
        }

        // 流结束处理：处理完整的请求体
        if end_of_stream {
            linfo!(
                &ctx.request_id,
                LogStage::RequestModify,
                LogComponent::Proxy,
                "request_body_eom",
                "请求体接收完成，准备处理",
                body_size = ctx.request.body_received_size,
                body_truncated = ctx.request.body_truncated,
                has_strategy = ctx.routing.strategy.is_some(),
                will_modify = ctx.request.will_modify_body
            );

            // 确保有完整的 body 数据才进行 JSON 修改
            let mut chunk_replaced = false;
            if !ctx.request.body.is_empty() && ctx.request.will_modify_body {
                if let Some(strategy) = &ctx.routing.strategy {
                    match serde_json::from_slice::<Value>(&ctx.request.body) {
                        Ok(mut json_value) => {
                            ldebug!(
                                &ctx.request_id,
                                LogStage::RequestModify,
                                LogComponent::Proxy,
                                "request_body_parse_ok",
                                "请求体 JSON 解析成功，尝试应用策略修改",
                                body = json_value.to_string()
                            );
                            match strategy
                                .modify_request_body_json(session, ctx, &mut json_value)
                                .await
                            {
                                Ok(true) => {
                                    ldebug!(
                                        &ctx.request_id,
                                        LogStage::RequestModify,
                                        LogComponent::Proxy,
                                        "request_body_modified",
                                        "策略选择修改请求体，正在序列化回字节",
                                        body = json_value.to_string()
                                    );
                                    match serde_json::to_vec(&json_value) {
                                        Ok(serialized) => {
                                            // 更新 body 并重新设置到 chunk
                                            ctx.request.body = BytesMut::from(&serialized[..]);
                                            *body_chunk = Some(Bytes::from(serialized));
                                            chunk_replaced = true;
                                        }
                                        Err(e) => {
                                            lerror!(
                                                &ctx.request_id,
                                                LogStage::RequestModify,
                                                LogComponent::Proxy,
                                                "request_body_serialize_fail",
                                                &format!("序列化修改后的 JSON 失败: {e}")
                                            );
                                        }
                                    }
                                }
                                Ok(false) => {
                                    linfo!(
                                        &ctx.request_id,
                                        LogStage::RequestModify,
                                        LogComponent::Proxy,
                                        "request_body_not_modified",
                                        "策略选择不修改请求体"
                                    );
                                }
                                Err(e) => {
                                    lerror!(
                                        &ctx.request_id,
                                        LogStage::RequestModify,
                                        LogComponent::Proxy,
                                        "request_body_modify_fail",
                                        &format!("执行请求体修改策略失败: {e}")
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            lerror!(
                                &ctx.request_id,
                                LogStage::RequestModify,
                                LogComponent::Proxy,
                                "request_body_parse_fail",
                                &format!("解析请求体 JSON 失败: {e}"),
                                body_preview = %String::from_utf8_lossy(&ctx.request.body[..std::cmp::min(500, ctx.request.body.len())])
                            );
                        }
                    }
                }
            } else if ctx.request.body.is_empty() && ctx.request.will_modify_body {
                lwarn!(
                    &ctx.request_id,
                    LogStage::RequestModify,
                    LogComponent::Proxy,
                    "request_body_empty_for_modify",
                    "策略期望修改请求体，但请求体为空"
                );
            }

            // 如果提前吞掉了分块但未能改写，确保把原始数据再发送出去
            if ctx.request.will_modify_body && !chunk_replaced {
                let original_body = Bytes::copy_from_slice(ctx.request.body.as_ref());
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
        self.state
            .resp_transform_service
            .filter_response(session, upstream_response, ctx)?;

        let resp_stats = self
            .state
            .collect_service
            .collect_response_details(upstream_response, ctx);
        ctx.response.details.headers = resp_stats.headers;
        ctx.response.is_sse =
            Self::is_sse_content_type(ctx.response.details.content_type.as_deref());

        Ok(())
    }

    fn upstream_response_filter(
        &self,
        session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        let status_code = upstream_response.status.as_u16();

        if !Self::should_retry_upstream_status(status_code) {
            return Ok(());
        }

        // 解析 Retry-After（仅用于 429）
        if status_code == 429
            && let Some(value) = upstream_response.headers.get("retry-after")
            && let Ok(value_str) = std::str::from_utf8(value.as_bytes())
        {
            ctx.control
                .retry
                .set_retry_after_from_header_value(&ctx.request_id, value_str);
        }

        let reason = if status_code == 429 {
            "rate_limited"
        } else {
            "upstream_5xx"
        };

        // 仅在“预算允许且可安全重试”时，把该响应视为 error 触发 Pingora 重试；
        // 否则保持原样把上游响应（含 body）透传给下游。
        let mut err = PingoraError::new_up(ErrorType::HTTPStatus(status_code));
        Self::apply_retry_policy(session, ctx, err.as_mut(), reason, Some(status_code));

        if err.retry() {
            return Err(err);
        }

        // 未触发重试：重置标记，避免后续路径误判“已应用重试策略”
        ctx.control.retry.clear_policy_after_no_retry();
        Ok(())
    }

    fn fail_to_connect(
        &self,
        session: &mut Session,
        peer: &HttpPeer,
        ctx: &mut Self::CTX,
        e: Box<Error>,
    ) -> Box<Error> {
        let mut err = e.more_context(format!("Peer: {peer}"));
        // 连接建立阶段失败：通常属于可重试范畴，交由预算控制。
        Self::apply_retry_policy(session, ctx, err.as_mut(), "connect_failure", None);
        err
    }

    fn error_while_proxy(
        &self,
        peer: &HttpPeer,
        session: &mut Session,
        e: Box<Error>,
        ctx: &mut Self::CTX,
        client_reused: bool,
    ) -> Box<Error> {
        let policy_already_applied = ctx.control.retry.retry_policy_applied;

        // 基于 Pingora 默认逻辑补充上下文，并决定“复用连接时才重试”这类错误的最终 retry 值。
        let mut err = e.more_context(format!("Peer: {peer}"));
        let retry_buffer_truncated = session.as_ref().retry_buffer_truncated();
        err.retry
            .decide_reuse(client_reused && !retry_buffer_truncated);

        if matches!(err.etype, ErrorType::ConnectionClosed) {
            let (event, message) = match err.esource {
                pingora_core::ErrorSource::Upstream => {
                    ("upstream_connection_closed", "上游连接被关闭")
                }
                pingora_core::ErrorSource::Downstream => {
                    ("downstream_connection_closed", "下游连接被关闭")
                }
                _ => ("connection_closed", "连接被关闭"),
            };
            let response_started = ctx.response.details.status_code.is_some();
            lwarn!(
                &ctx.request_id,
                LogStage::ResponseFailure,
                LogComponent::Proxy,
                event,
                message,
                error_source = format!("{:?}", err.esource),
                error_type = format!("{:?}", err.etype),
                response_started = response_started,
                response_body_size = ctx.response.body_received_size,
                response_body_truncated = ctx.response.body_truncated,
                client_reused = client_reused,
                retry_buffer_truncated = retry_buffer_truncated,
                retry_decision = err.retry(),
                retry_count = ctx.control.retry.retry_count,
                policy_already_applied = policy_already_applied
            );
        }

        // 上游响应在 upstream_response_filter 阶段已完成“预算消耗/计数”决策时，
        // 这里必须避免重复应用策略导致双计数。
        if policy_already_applied {
            return err;
        }

        // 仅对上游连接类错误进行重试预算控制；其他内部错误默认不重试。
        let is_upstream_error = err.esource == pingora_core::ErrorSource::Upstream
            || matches!(err.etype, ErrorType::HTTPStatus(_));

        if !is_upstream_error {
            err.set_retry(false);
            return err;
        }

        // 对于已收到部分响应的场景，强制不重试（避免重复输出）。
        if Self::is_partial_response_error(ctx) {
            err.set_retry(false);
            return err;
        }

        if let ErrorType::HTTPStatus(code) = err.etype
            && Self::should_retry_upstream_status(code)
        {
            let reason = if code == 429 {
                "rate_limited"
            } else {
                "upstream_5xx"
            };
            Self::apply_retry_policy(session, ctx, err.as_mut(), reason, Some(code));
            return err;
        }

        // 对上游连接类错误（Pingora 已判断可重试的）应用预算控制。
        if err.retry() {
            Self::apply_retry_policy(session, ctx, err.as_mut(), "proxy_error", None);
        }

        err
    }

    fn response_body_filter(
        &self,
        _session: &mut Session,
        body: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<Option<std::time::Duration>> {
        if let Some(chunk) = body.as_ref() {
            let newly_truncated = Self::append_body_with_limit(
                &mut ctx.response.body,
                &mut ctx.response.body_received_size,
                &mut ctx.response.body_truncated,
                chunk,
                Self::MAX_BODY_BUFFER_BYTES,
            );
            if newly_truncated {
                lwarn!(
                    &ctx.request_id,
                    LogStage::Response,
                    LogComponent::Proxy,
                    "response_body_truncated",
                    "响应体缓存超过上限，已截断",
                    buffer_limit_bytes = Self::MAX_BODY_BUFFER_BYTES,
                    received_bytes = ctx.response.body_received_size
                );
            }
        }
        if !ctx.response.sse_keepalive_sent
            && ctx.response.is_sse
            && ctx.response.details.status_code == Some(200)
            && !end_of_stream
            && let Some(chunk) = body.take()
        {
            let mut combined =
                BytesMut::with_capacity(Self::SSE_KEEPALIVE_PREFIX.len() + chunk.len());
            combined.extend_from_slice(Self::SSE_KEEPALIVE_PREFIX);
            combined.extend_from_slice(&chunk);
            *body = Some(combined.freeze());
            ctx.response.sse_keepalive_sent = true;
            ldebug!(
                &ctx.request_id,
                LogStage::Response,
                LogComponent::Proxy,
                "sse_keepalive_injected",
                "已注入 SSE keep-alive 注释帧"
            );
        }
        if end_of_stream {
            // 简单记录响应体接收完成
            linfo!(
                &ctx.request_id,
                LogStage::Response,
                LogComponent::Proxy,
                "response_body_eom",
                "响应体接收完成",
                body_size = ctx.response.body_received_size,
                body_truncated = ctx.response.body_truncated
            );
        }
        Ok(None)
    }

    async fn logging(&self, session: &mut Session, e: Option<&Error>, ctx: &mut Self::CTX) {
        let status_code = Self::resolve_status_code(ctx, e);

        if let Some(strategy) = &ctx.routing.strategy
            && let Err(e) = strategy
                .handle_response_body(session, ctx, status_code, &ctx.response.body)
                .await
        {
            lwarn!(
                &ctx.request_id,
                LogStage::Response,
                LogComponent::Proxy,
                "strategy_response_fail",
                &format!("Provider strategy handle_response_body failed: {e}")
            );
        }

        let metrics = self
            .state
            .collect_service
            .finalize_metrics(ctx, status_code)
            .await;

        if ctx.is_trace_started() {
            self.state
                .trace_manager
                .update_model(
                    &ctx.request_id,
                    metrics.provider_type_id,
                    metrics.model.clone(),
                    ctx.routing.selected_backend.as_ref().map(|k| k.id),
                )
                .await;
        }

        if status_code < 400 {
            if let Err(err) = self.state.trace_manager.record_success(&metrics, ctx).await {
                lwarn!(
                    &ctx.request_id,
                    LogStage::Error,
                    LogComponent::Tracing,
                    "trace_record_fail",
                    &format!("Failed to record success trace: {err}")
                );
            }
        } else {
            self.state
                .trace_manager
                .record_failure(Some(&metrics), status_code, e, ctx)
                .await;
        }

        // 根据 user_service_api.log_mode 输出完整请求/响应日志（包含 body schema，内容可截断）
        logging::log_user_service_api_log_mode(ctx, status_code);

        linfo!(
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

#[cfg(test)]
mod tests {
    use super::*;

    use entity::user_service_apis;
    use pingora_core::protocols::l4::stream::Stream;
    use tokio::io::AsyncWriteExt;
    use tokio::net::{TcpListener, TcpStream};

    async fn make_test_session_with_client(request: &str) -> (Session, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");

        let mut client = TcpStream::connect(addr).await.expect("connect");
        let (server, _) = listener.accept().await.expect("accept");

        client
            .write_all(request.as_bytes())
            .await
            .expect("write request");

        let stream = Stream::from(server);
        let mut session = Session::new_h1(Box::new(stream));

        session
            .downstream_session
            .read_request()
            .await
            .expect("read request");

        (session, client)
    }

    async fn make_test_session(request: &str) -> Session {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");

        let mut client = TcpStream::connect(addr).await.expect("connect");
        let (server, _) = listener.accept().await.expect("accept");

        client
            .write_all(request.as_bytes())
            .await
            .expect("write request");

        let stream = Stream::from(server);
        let mut session = Session::new_h1(Box::new(stream));

        session
            .downstream_session
            .read_request()
            .await
            .expect("read request");

        session
    }

    fn make_test_user_service_api(retry_count: i32) -> user_service_apis::Model {
        let now = chrono::Utc::now().naive_utc();
        user_service_apis::Model {
            id: 1,
            user_id: 1,
            provider_type_id: 1,
            user_provider_keys_ids: serde_json::json!([]),
            api_key: "test-api-key".to_string(),
            name: None,
            description: None,
            scheduling_strategy: None,
            retry_count: Some(retry_count),
            timeout_seconds: None,
            max_request_per_min: None,
            max_requests_per_day: None,
            max_tokens_per_day: None,
            max_cost_per_day: None,
            log_mode: false,
            expires_at: None,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    fn reset_retry_policy_state(ctx: &mut ProxyContext) {
        ctx.control.retry.reset_for_new_attempt();
    }

    #[tokio::test]
    async fn test_retry_budget_consumption_for_upstream_5xx() {
        let mut session = make_test_session("GET / HTTP/1.1\r\nHost: example.com\r\n\r\n").await;

        let mut ctx = ProxyContext {
            request_id: "test-request".to_string(),
            start_time: Instant::now(),
            ..Default::default()
        };
        ctx.routing.user_service_api = Some(make_test_user_service_api(2));

        let mut err = PingoraError::new_up(ErrorType::HTTPStatus(500));
        ProxyService::apply_retry_policy(
            &mut session,
            &mut ctx,
            err.as_mut(),
            "test_upstream_5xx",
            Some(500),
        );
        assert!(err.retry());
        assert_eq!(ctx.control.retry.retry_count, 1);
        reset_retry_policy_state(&mut ctx);

        let mut err = PingoraError::new_up(ErrorType::HTTPStatus(502));
        ProxyService::apply_retry_policy(
            &mut session,
            &mut ctx,
            err.as_mut(),
            "test_upstream_5xx",
            Some(502),
        );
        assert!(err.retry());
        assert_eq!(ctx.control.retry.retry_count, 2);
        reset_retry_policy_state(&mut ctx);

        // 达到预算上限后不再重试
        let mut err = PingoraError::new_up(ErrorType::HTTPStatus(503));
        ProxyService::apply_retry_policy(
            &mut session,
            &mut ctx,
            err.as_mut(),
            "test_upstream_5xx",
            Some(503),
        );
        assert!(!err.retry());
        assert_eq!(ctx.control.retry.retry_count, 2);
    }

    #[tokio::test]
    async fn test_retry_policy_is_not_double_counted_when_called_twice() {
        let mut session = make_test_session("GET / HTTP/1.1\r\nHost: example.com\r\n\r\n").await;

        let mut ctx = ProxyContext {
            request_id: "test-request".to_string(),
            start_time: Instant::now(),
            ..Default::default()
        };
        ctx.routing.user_service_api = Some(make_test_user_service_api(2));

        let mut err1 = PingoraError::new_up(ErrorType::HTTPStatus(500));
        ProxyService::apply_retry_policy(
            &mut session,
            &mut ctx,
            err1.as_mut(),
            "test_upstream_5xx",
            Some(500),
        );
        assert!(err1.retry());
        assert_eq!(ctx.control.retry.retry_count, 1);

        // 同一轮失败不应重复计数
        let mut err2 = PingoraError::new_up(ErrorType::HTTPStatus(500));
        ProxyService::apply_retry_policy(
            &mut session,
            &mut ctx,
            err2.as_mut(),
            "test_upstream_5xx",
            Some(500),
        );
        assert_eq!(ctx.control.retry.retry_count, 1);
    }

    #[tokio::test]
    async fn test_retry_after_is_capped_by_db_timeout_seconds() {
        let mut session = make_test_session("GET / HTTP/1.1\r\nHost: example.com\r\n\r\n").await;

        let mut user_api = make_test_user_service_api(1);
        user_api.timeout_seconds = Some(1);

        let mut ctx = ProxyContext {
            request_id: "test-request".to_string(),
            start_time: Instant::now(),
            ..Default::default()
        };
        ctx.routing.user_service_api = Some(user_api);
        ctx.control.retry.retry_after_ms = Some(2_000);

        let mut err = PingoraError::new_up(ErrorType::HTTPStatus(429));
        ProxyService::apply_retry_policy(
            &mut session,
            &mut ctx,
            err.as_mut(),
            "rate_limited",
            Some(429),
        );

        assert!(err.retry());
        assert_eq!(ctx.control.retry.retry_count, 1);
        assert_eq!(ctx.control.retry.next_retry_delay_ms, Some(1_000));
    }

    #[tokio::test]
    async fn test_retry_after_is_capped_by_default_timeout_when_timeout_seconds_is_non_positive() {
        let mut session = make_test_session("GET / HTTP/1.1\r\nHost: example.com\r\n\r\n").await;

        let mut user_api = make_test_user_service_api(1);
        user_api.timeout_seconds = Some(0);

        let mut ctx = ProxyContext {
            request_id: "test-request".to_string(),
            start_time: Instant::now(),
            ..Default::default()
        };
        ctx.routing.user_service_api = Some(user_api);
        ctx.control.retry.retry_after_ms = Some(120_000);

        let mut err = PingoraError::new_up(ErrorType::HTTPStatus(429));
        ProxyService::apply_retry_policy(
            &mut session,
            &mut ctx,
            err.as_mut(),
            "rate_limited",
            Some(429),
        );

        assert!(err.retry());
        assert_eq!(ctx.control.retry.retry_count, 1);
        assert_eq!(ctx.control.retry.next_retry_delay_ms, Some(30_000));
    }

    #[tokio::test]
    async fn test_is_safe_to_retry_false_when_retry_buffer_truncated() {
        let body = "a".repeat(70_000);
        let request = format!(
            "POST / HTTP/1.1\r\nHost: example.com\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let (mut session, _client_guard) = make_test_session_with_client(&request).await;
        session.enable_retry_buffering();

        loop {
            let chunk = session
                .read_request_body()
                .await
                .expect("read request body");
            if chunk.is_none() {
                break;
            }
        }

        assert!(session.retry_buffer_truncated());
        assert!(!ProxyService::is_safe_to_retry(&mut session));
    }

    #[tokio::test]
    async fn test_retry_disabled_when_retry_count_is_zero() {
        let mut session = make_test_session("GET / HTTP/1.1\r\nHost: example.com\r\n\r\n").await;

        let mut ctx = ProxyContext {
            request_id: "test-request".to_string(),
            start_time: Instant::now(),
            ..Default::default()
        };
        ctx.routing.user_service_api = Some(make_test_user_service_api(0));

        let mut err = PingoraError::new_up(ErrorType::HTTPStatus(500));
        ProxyService::apply_retry_policy(
            &mut session,
            &mut ctx,
            err.as_mut(),
            "test_upstream_5xx",
            Some(500),
        );
        assert!(!err.retry());
        assert_eq!(ctx.control.retry.retry_count, 0);
    }
}
