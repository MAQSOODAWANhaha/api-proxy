//! # 简化的 Pingora AI 代理服务
//!
//! 使用新的简化组件实现透明AI代理服务

use async_trait::async_trait;
use bytes::Bytes;
use pingora_core::protocols::Digest;
use pingora_core::{ErrorType, prelude::*, upstreams::peer::HttpPeer};
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::{FailToProxy, ProxyHttp, Session};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::auth::RefactoredUnifiedAuthManager;
use crate::cache::UnifiedCacheManager;
use crate::config::{AppConfig, ProviderConfigManager};
use crate::proxy::request_handler::{RequestHandler, ProxyContext};
use crate::trace::{UnifiedTraceSystem, immediate::ImmediateProxyTracer};
use sea_orm::DatabaseConnection;

/// 简化的AI代理服务 - 保持完整业务逻辑
pub struct ProxyService {
    /// AI代理处理器 - 保持原有完整功能
    ai_handler: Arc<RequestHandler>,
    /// 即时写入追踪器
    tracer: Option<Arc<ImmediateProxyTracer>>,
}

impl ProxyService {
    /// 创建新的代理服务实例 - 保持原有完整功能
    pub fn new(
        config: Arc<AppConfig>,
        db: Arc<DatabaseConnection>,
        cache: Arc<UnifiedCacheManager>,
        provider_config_manager: Arc<ProviderConfigManager>,
        trace_system: Option<Arc<UnifiedTraceSystem>>,
        auth_manager: Arc<RefactoredUnifiedAuthManager>,
    ) -> pingora_core::Result<Self> {
        // 获取即时写入追踪器
        let tracer = trace_system.as_ref().and_then(|ts| ts.immediate_tracer());

        // 创建AI代理处理器 - 保持原有完整功能
        let ai_handler = Arc::new(RequestHandler::new(
            db,
            cache,
            config.clone(),
            tracer.clone(),
            provider_config_manager,
            auth_manager,
        ));

        // 保留trace_system引用获取的即时写入tracer
        let tracer = trace_system.and_then(|ts| ts.immediate_tracer());

        Ok(Self {
            ai_handler,
            tracer,
        })
    }

    /// 检查是否为代理请求（透明代理设计）
    fn is_proxy_request(&self, path: &str) -> bool {
        // 透明代理：除了管理API之外的所有请求都当作AI代理请求
        // 用户决定发送什么格式给什么提供商，系统只负责认证和密钥替换
        !self.is_management_request(path)
    }

    /// 检查是否为管理请求（应该发送到端口9090）
    fn is_management_request(&self, path: &str) -> bool {
        path.starts_with("/api/") || path.starts_with("/admin/") || path == "/"
    }
}

#[async_trait]
impl ProxyHttp for ProxyService {
    type CTX = ProxyContext;

    fn new_ctx(&self) -> Self::CTX {
        let ctx = ProxyContext {
            request_id: Uuid::new_v4().to_string(),
            start_time: Instant::now(),
            ..Default::default()
        };

        // 追踪将在 request_filter 中开始
        if let Some(_tracer) = &self.tracer {
            tracing::debug!(
                request_id = %ctx.request_id,
                "Trace will be started when request info is available"
            );
        }

        ctx
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<bool> {
        let path = session.req_header().uri.path();
        let method = session.req_header().method.as_str();

        tracing::debug!(
            request_id = %ctx.request_id,
            method = %method,
            path = %path,
            "Processing AI proxy request"
        );

        // 透明代理设计：仅处理代理请求，其他全部拒绝
        if !self.is_proxy_request(path) {
            if self.is_management_request(path) {
                tracing::warn!(
                    request_id = %ctx.request_id,
                    path = %path,
                    "Management API request received on proxy port - should use port 9090"
                );
                return Err(Error::explain(
                    ErrorType::HTTPStatus(404),
                    r#"{"error":"Management APIs are available on management port (default: 9090)","code":"WRONG_PORT"}"#,
                ));
            } else {
                return Err(Error::explain(
                    ErrorType::HTTPStatus(404),
                    r#"{"error":"Unknown endpoint - this port handles AI proxy requests (any format)","code":"NOT_PROXY_ENDPOINT"}"#,
                ));
            }
        }

        // 处理CORS预检请求
        if method == "OPTIONS" {
            return Err(Error::explain(ErrorType::HTTPStatus(200), "CORS preflight"));
        }

        // 使用AI代理处理器进行身份验证、速率限制和转发策略
        // 这会设置 ctx.timeout_seconds 从数据库配置
        match self.ai_handler.prepare_proxy_request(session, ctx).await {
            Ok(_) => {
                // 使用数据库配置的超时时间设置下游超时
                let timeout_seconds = ctx.timeout_seconds.unwrap_or(30) as u64;
                // 下游超时设置为配置时间的2倍，确保有足够时间处理AI请求
                let downstream_timeout_secs = timeout_seconds * 2;
                
                use std::time::Duration;
                session.set_read_timeout(Some(Duration::from_secs(downstream_timeout_secs)));
                session.set_write_timeout(Some(Duration::from_secs(downstream_timeout_secs)));
                
                tracing::debug!(
                    request_id = %ctx.request_id,
                    configured_timeout_s = timeout_seconds,
                    downstream_timeout_s = downstream_timeout_secs,
                    "Set downstream timeouts from database configuration"
                );

                tracing::debug!(
                    request_id = %ctx.request_id,
                    "AI proxy request preparation completed successfully - using Pingora native proxy"
                );

                // 返回 false 让 Pingora 继续处理请求转发
                // 后续由 upstream_peer, upstream_request_filter, response_filter 等方法完成代理
                Ok(false)
            }
            Err(e) => {
                tracing::error!(
                    request_id = %ctx.request_id,
                    error = %e,
                    "AI proxy request preparation failed"
                );

                // 根据错误类型返回相应的HTTP状态码
                match e {
                    crate::error::ProxyError::Authentication { .. } => {
                        let msg = format!(r#"{{"error":"{}","code":"AUTH_ERROR"}}"#, e);
                        Err(Error::explain(ErrorType::HTTPStatus(401), msg))
                    }
                    crate::error::ProxyError::RateLimit { .. } => {
                        let msg = format!(r#"{{"error":"{}","code":"RATE_LIMIT"}}"#, e);
                        Err(Error::explain(ErrorType::HTTPStatus(429), msg))
                    }
                    crate::error::ProxyError::ConnectionTimeout {
                        timeout_seconds, ..
                    } => {
                        let msg = format!(
                            r#"{{"error":"Connection timeout after {}s","code":"CONNECTION_TIMEOUT","timeout_configured":{}}}"#,
                            timeout_seconds, timeout_seconds
                        );
                        Err(Error::explain(ErrorType::HTTPStatus(504), msg))
                    }
                    crate::error::ProxyError::ReadTimeout {
                        timeout_seconds, ..
                    } => {
                        let msg = format!(
                            r#"{{"error":"Read timeout after {}s","code":"READ_TIMEOUT","timeout_configured":{}}}"#,
                            timeout_seconds, timeout_seconds
                        );
                        Err(Error::explain(ErrorType::HTTPStatus(504), msg))
                    }
                    crate::error::ProxyError::Network { message, .. } => {
                        let msg = format!(
                            r#"{{"error":"Network error: {}","code":"NETWORK_ERROR"}}"#,
                            message
                        );
                        Err(Error::explain(ErrorType::HTTPStatus(502), msg))
                    }
                    crate::error::ProxyError::BadGateway { .. } => {
                        let msg = format!(r#"{{"error":"{}","code":"BAD_GATEWAY"}}"#, e);
                        Err(Error::explain(ErrorType::HTTPStatus(502), msg))
                    }
                    _ => Err(Error::explain(
                        ErrorType::HTTPStatus(500),
                        r#"{"error":"Internal server error","code":"INTERNAL_ERROR"}"#,
                    )),
                }
            }
        }
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<Box<HttpPeer>> {
        // 使用AI代理处理器选择上游对等体
        self.ai_handler
            .select_upstream_peer(ctx)
            .await
            .map_err(|e| {
                match e {
                    crate::error::ProxyError::ConnectionTimeout { timeout_seconds, .. } => {
                        Error::explain(
                            ErrorType::HTTPStatus(504),
                            format!(r#"{{"error":"Connection timeout after {}s","code":"UPSTREAM_TIMEOUT","timeout_configured":{}}}"#, timeout_seconds, timeout_seconds)
                        )
                    }
                    crate::error::ProxyError::ReadTimeout { timeout_seconds, .. } => {
                        Error::explain(
                            ErrorType::HTTPStatus(504),
                            format!(r#"{{"error":"Read timeout after {}s","code":"READ_TIMEOUT","timeout_configured":{}}}"#, timeout_seconds, timeout_seconds)
                        )
                    }
                    crate::error::ProxyError::Network { message, .. } => {
                        Error::explain(
                            ErrorType::HTTPStatus(502),
                            format!(r#"{{"error":"Network error: {}","code":"NETWORK_ERROR"}}"#, message)
                        )
                    }
                    _ => Error::explain(
                        ErrorType::HTTPStatus(500),
                        r#"{"error":"Internal server error","code":"INTERNAL_ERROR"}"#
                    )
                }
            })
    }

    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        // 使用AI代理处理器过滤上游请求 - 替换认证信息和隐藏源信息
        self.ai_handler
            .filter_upstream_request(session, upstream_request, ctx)
            .await
            .map_err(|e| {
                tracing::error!(
                    request_id = %ctx.request_id,
                    error = %e,
                    "Failed to filter upstream request"
                );
                match e {
                    crate::error::ProxyError::Network { .. } => Error::explain(
                        ErrorType::HTTPStatus(502),
                        "Network error during request processing",
                    ),
                    _ => Error::new(ErrorType::InternalError),
                }
            })
    }

    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        // 使用AI代理处理器过滤上游响应
        self.ai_handler
            .filter_upstream_response(upstream_response, ctx)
            .await
            .map_err(|e| {
                tracing::error!(
                    request_id = %ctx.request_id,
                    error = %e,
                    "Failed to filter upstream response"
                );
                Error::new(ErrorType::InternalError)
            })?;

        // 记录响应时间和状态
        let response_time = ctx.start_time.elapsed();
        let status_code = upstream_response.status.as_u16();

        tracing::info!(
            request_id = %ctx.request_id,
            status = status_code,
            response_time_ms = response_time.as_millis(),
            tokens_used = ctx.tokens_used,
            "AI proxy response processed"
        );

        Ok(())
    }

    fn response_body_filter(
        &self,
        _session: &mut Session,
        body: &mut Option<Bytes>,
        _end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<Option<std::time::Duration>>
    where
        Self::CTX: Send + Sync,
    {
        // 收集响应体数据块
        if let Some(data) = body {
            ctx.response_details.add_body_chunk(data);

            tracing::info!(
                request_id = %ctx.request_id,
                chunk_size = data.len(),
                total_size = ctx.response_details.body_chunks.len(),
                "Collected response body chunk"
            );
        }

        Ok(None)
    }

    async fn connected_to_upstream(
        &self,
        _session: &mut Session,
        reused: bool,
        peer: &HttpPeer,
        #[cfg(unix)] _fd: std::os::unix::io::RawFd,
        #[cfg(windows)] _sock: std::os::windows::io::RawSocket,
        _digest: Option<&Digest>,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        tracing::debug!(
            request_id = %ctx.request_id,
            reused = reused,
            peer_addr = ?peer._address,
            sni = %peer.sni,
            "Connected to upstream - monitoring protocol negotiation"
        );

        // 这里可以获取协商的协议信息
        // 不幸的是，Session的upstream_session在这个时候可能还没有完全建立
        // 但我们可以记录连接状态

        Ok(())
    }

    async fn fail_to_proxy(
        &self,
        _session: &mut Session,
        e: &Error,
        ctx: &mut Self::CTX,
    ) -> FailToProxy {
        // 检测超时和网络错误，进行错误转换
        let is_timeout_or_network_error = matches!(
            &e.etype,
            ErrorType::ConnectTimedout
                | ErrorType::ReadTimedout
                | ErrorType::WriteTimedout
                | ErrorType::ConnectError
                | ErrorType::ConnectRefused
        );

        if is_timeout_or_network_error {
            let converted_error = self.ai_handler.convert_pingora_error(e, ctx);

            tracing::error!(
                request_id = %ctx.request_id,
                original_error = %e,
                converted_error = %converted_error,
                "Converting network/timeout error to user-friendly response"
            );

            // 上游连接失败时立即记录到数据库
            if let Some(tracer) = &self.tracer {
                let error_code = match converted_error {
                    crate::error::ProxyError::ConnectionTimeout { .. } => 504,
                    crate::error::ProxyError::ReadTimeout { .. } => 504,
                    crate::error::ProxyError::Network { .. } => 502,
                    crate::error::ProxyError::UpstreamNotAvailable { .. } => 503,
                    _ => 502,
                };

                let error_type = match converted_error {
                    crate::error::ProxyError::ConnectionTimeout { .. } => "connection_timeout",
                    crate::error::ProxyError::ReadTimeout { .. } => "read_timeout",
                    crate::error::ProxyError::Network { .. } => "network_error",
                    crate::error::ProxyError::UpstreamNotAvailable { .. } => "upstream_unavailable",
                    _ => "upstream_connection_failed",
                };

                let _ = tracer
                    .complete_trace(
                        &ctx.request_id,
                        error_code,
                        false,
                        None,
                        None,
                        Some(error_type.to_string()),
                        Some(converted_error.to_string()),
                    )
                    .await;
            }

            // 返回转换后的错误信息，让 Pingora 处理 HTTP 响应
            let error_code = match converted_error {
                crate::error::ProxyError::ConnectionTimeout { .. } => 504,
                crate::error::ProxyError::ReadTimeout { .. } => 504,
                crate::error::ProxyError::Network { .. } => 502,
                crate::error::ProxyError::UpstreamNotAvailable { .. } => 503,
                _ => 502,
            };

            return FailToProxy {
                error_code,
                can_reuse_downstream: false, // 对于超时和网络错误，不重用连接
            };
        }

        // 对于其他错误，使用默认错误码并不重用连接
        // 其他类型的连接失败也记录
        if let Some(tracer) = &self.tracer {
            let _ = tracer
                .complete_trace(
                    &ctx.request_id,
                    500,
                    false,
                    None,
                    None,
                    Some("proxy_error".to_string()),
                    Some(format!("Pingora error: {}", e)),
                )
                .await;
        }

        FailToProxy {
            error_code: 500,
            can_reuse_downstream: false,
        }
    }

    async fn logging(&self, session: &mut Session, e: Option<&Error>, ctx: &mut Self::CTX) {
        let duration = ctx.start_time.elapsed();

        if let Some(error) = e {
            // 检测是否为超时或网络错误，并进行详细记录
            let is_timeout_error = matches!(
                &error.etype,
                ErrorType::ConnectTimedout | ErrorType::ReadTimedout | ErrorType::WriteTimedout
            );
            let is_network_error = matches!(
                &error.etype,
                ErrorType::ConnectError | ErrorType::ConnectRefused
            );

            // 获取更多的上下文信息
            let request_info = format!(
                "method={} uri={} headers={:?}",
                session.req_header().method,
                session.req_header().uri,
                session.req_header().headers
            );

            tracing::error!(
                request_id = %ctx.request_id,
                error = %error,
                error_type = ?error.etype,
                error_source = ?error.esource,
                error_context = ?error.context,
                duration_ms = duration.as_millis(),
                request_info = %request_info,
                selected_backend = ?ctx.selected_backend.as_ref().map(|b| format!("id={} key_preview={}", b.id,
                    if b.api_key.len() > 8 { format!("{}***{}", &b.api_key[..4], &b.api_key[b.api_key.len()-4..]) } else { "***".to_string() })),
                provider_type = ?ctx.provider_type.as_ref().map(|p| &p.name),
                timeout_seconds = ?ctx.timeout_seconds,
                is_timeout_error = is_timeout_error,
                is_network_error = is_network_error,
                "AI proxy request failed with detailed context"
            );

            // 如果是超时或网络错误，使用AI处理器进行错误转换
            if is_timeout_error || is_network_error {
                let converted_error = self.ai_handler.convert_pingora_error(error, ctx);
                tracing::warn!(
                    request_id = %ctx.request_id,
                    original_error = %error,
                    converted_error = %converted_error,
                    "Converted Pingora error to ProxyError for better user experience"
                );
            }
        } else {
            // 成功请求完成，记录追踪信息
            if let Some(tracer) = &self.tracer {
                let status_code = session
                    .response_written()
                    .map(|resp| resp.status.as_u16())
                    .unwrap_or(200);

                ctx.response_details.finalize_body();

                // 从响应体JSON中提取所有统计信息 - 使用StatisticsService
                match self.ai_handler.statistics_service().extract_stats_from_response_body(ctx).await {
                    Ok(new_stats) => {
                        // 更新上下文中的token使用信息
                        ctx.token_usage.prompt_tokens = new_stats.input_tokens;
                        ctx.token_usage.completion_tokens = new_stats.output_tokens;
                        ctx.token_usage.total_tokens = new_stats.total_tokens.unwrap_or(0);
                        ctx.token_usage.model_used = new_stats.model_name.clone();
                        ctx.tokens_used = ctx.token_usage.total_tokens;

                        // 使用完整的统计信息完成追踪
                        if let Err(e) = tracer
                            .complete_trace_with_stats(
                                &ctx.request_id,
                                status_code,
                                true, // is_success
                                new_stats.input_tokens,
                                new_stats.output_tokens,
                                None, // error_type
                                None, // error_message
                                new_stats.cache_create_tokens,
                                new_stats.cache_read_tokens,
                                new_stats.cost,
                                new_stats.cost_currency,
                            )
                            .await
                        {
                            tracing::error!(
                                request_id = %ctx.request_id,
                                error = %e,
                                "Failed to store complete trace with stats"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            request_id = %ctx.request_id,
                            error = %e,
                            "Failed to extract stats from response body, using header-based data"
                        );
                        // Fallback to header-based data
                        let _ = tracer
                            .complete_trace_with_stats(
                                &ctx.request_id,
                                status_code,
                                true,
                                ctx.token_usage.prompt_tokens,
                                ctx.token_usage.completion_tokens,
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                            )
                            .await;
                    }
                }
            }

            tracing::debug!(
                request_id = %ctx.request_id,
                duration_ms = duration.as_millis(),
                tokens_used = ctx.tokens_used,
                "AI proxy request completed successfully"
            );
        }
    }
}
