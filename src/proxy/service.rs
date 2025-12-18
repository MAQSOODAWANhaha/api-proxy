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
use uuid::Uuid;

use crate::proxy::context::ProxyContext;
use crate::proxy::provider_strategy;
use crate::proxy::response::{JsonError, build_auth_error_response, write_json_error};
use crate::proxy::state::ProxyState;

/// 核心AI代理服务 - 作为编排器
pub struct ProxyService {
    state: Arc<ProxyState>,
}

impl ProxyService {
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

    /// 检测是否为部分响应错误（收到响应头但响应体不完整）
    fn is_partial_response_error(ctx: &ProxyContext, error: Option<&Error>) -> bool {
        // 检查是否有响应状态码但有连接错误
        let has_response_status = ctx.response_details.status_code.is_some();
        let has_connection_error = Self::is_connection_failure(error);

        has_response_status && has_connection_error
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
            && let Some(status) = ctx.response_details.status_code
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
        if let (Some(user_api), Some(provider_type)) =
            (ctx.user_service_api.as_ref(), ctx.provider_type.as_ref())
        {
            let timeout = user_api.timeout_seconds.unwrap_or(120).max(120);

            ctx.timeout_seconds = Some(timeout);

            let timeout_u64 = u64::try_from(timeout).unwrap_or(120);
            let timeout_duration = std::time::Duration::from_secs(timeout_u64 * 2);
            session.set_read_timeout(Some(timeout_duration));
            session.set_write_timeout(Some(timeout_duration));

            if let Some(name) = provider_strategy::ProviderRegistry::match_name(&provider_type.name)
            {
                ctx.strategy = provider_strategy::make_strategy(
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
        ctx.request_details = self
            .state
            .collect_service
            .collect_request_details(session, &req_stats);

        if let Some(user_api) = &ctx.user_service_api {
            let provider_type_id = ctx.provider_type.as_ref().map(|p| p.id);
            let user_provider_key_id = ctx.selected_backend.as_ref().map(|backend| backend.id);
            let _ = self
                .state
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
                .await;
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
            user_service_api_id = ctx.user_service_api.as_ref().map(|u| u.id)
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
            .user_service_api
            .as_ref()
            .is_some_and(|api| api.log_mode)
        {
            ctx.upstream_request_headers =
                Some(logging::headers_json_map_request(upstream_request));
            ctx.upstream_request_uri = Some(upstream_request.uri.to_string());
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
            // 缓存数据到上下文
            ctx.request_body.extend_from_slice(chunk);
            // 如果需要修改请求体且不是流结束，按照 Pingora 官方示例清空分块
            // 保持 HTTP 流式语义，避免原始与改写后的内容混合发送
            if ctx.will_modify_body
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
                body_size = ctx.request_body.len(),
                has_strategy = ctx.strategy.is_some(),
                will_modify = ctx.will_modify_body
            );

            // 确保有完整的 body 数据才进行 JSON 修改
            let mut chunk_replaced = false;
            if !ctx.request_body.is_empty() && ctx.will_modify_body {
                if let Some(strategy) = &ctx.strategy {
                    match serde_json::from_slice::<Value>(&ctx.request_body) {
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
                                            ctx.request_body = BytesMut::from(&serialized[..]);
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
                                body_preview = %String::from_utf8_lossy(&ctx.request_body[..std::cmp::min(500, ctx.request_body.len())])
                            );
                        }
                    }
                }
            } else if ctx.request_body.is_empty() && ctx.will_modify_body {
                lwarn!(
                    &ctx.request_id,
                    LogStage::RequestModify,
                    LogComponent::Proxy,
                    "request_body_empty_for_modify",
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
        self.state
            .resp_transform_service
            .filter_response(session, upstream_response, ctx)?;

        let resp_stats = self
            .state
            .collect_service
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
            // 简单记录响应体接收完成
            linfo!(
                &ctx.request_id,
                LogStage::Response,
                LogComponent::Proxy,
                "response_body_eom",
                "响应体接收完成",
                body_size = ctx.response_body.len()
            );
        }
        Ok(None)
    }

    async fn logging(&self, session: &mut Session, e: Option<&Error>, ctx: &mut Self::CTX) {
        let status_code = Self::resolve_status_code(ctx, e);

        if let Some(strategy) = &ctx.strategy
            && let Err(e) = strategy
                .handle_response_body(session, ctx, status_code, &ctx.response_body)
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

        self.state
            .trace_manager
            .update_model(
                &ctx.request_id,
                metrics.provider_type_id,
                metrics.model.clone(),
                ctx.selected_backend.as_ref().map(|k| k.id),
            )
            .await;

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
