//! # 简化的 Pingora AI 代理服务
//!
//! 使用新的简化组件实现透明AI代理服务

use async_trait::async_trait;
use bytes::Bytes;
use pingora_core::protocols::Digest;
use pingora_core::{ErrorType, prelude::*, upstreams::peer::HttpPeer};
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::{FailToProxy, ProxyHttp, Session};
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::auth::RefactoredUnifiedAuthManager;
use crate::cache::UnifiedCacheManager;
use crate::config::{AppConfig, ProviderConfigManager};
use crate::proxy::request_handler::{ProxyContext, RequestHandler};
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
    // 为日志限制请求/响应体的最大输出长度
    const MAX_LOG_BODY_BYTES: usize = 32 * 1024; // 32KB

    // 脱敏 JSON 中疑似敏感字段（key/token/secret/authorization/cookie 等）
    fn sanitize_json_value(v: &mut Value) {
        match v {
            Value::Object(map) => {
                for (k, val) in map.iter_mut() {
                    let kl = k.to_ascii_lowercase();
                    let is_sensitive = ["key", "token", "secret", "authorization", "cookie"]
                        .iter()
                        .any(|m| kl.contains(m));
                    if is_sensitive {
                        if let Value::String(s) = val {
                            if s.len() > 8 {
                                let masked = format!("{}...{}", &s[..4], &s[s.len().saturating_sub(4)..]);
                                *val = Value::String(masked);
                            } else {
                                *val = Value::String("****".to_string());
                            }
                        } else {
                            *val = Value::String("****".to_string());
                        }
                    } else {
                        ProxyService::sanitize_json_value(val);
                    }
                }
            }
            Value::Array(arr) => {
                for item in arr.iter_mut() {
                    ProxyService::sanitize_json_value(item);
                }
            }
            _ => {}
        }
    }

    fn pretty_truncated(s: &str, max: usize) -> String {
        if s.len() > max {
            format!("{}\n...[truncated {} bytes]", &s[..max], s.len() - max)
        } else {
            s.to_string()
        }
    }

    fn pretty_json_bytes(bytes: &[u8], max: usize) -> Option<String> {
        if let Ok(mut v) = serde_json::from_slice::<Value>(bytes) {
            ProxyService::sanitize_json_value(&mut v);
            let pretty = serde_json::to_string_pretty(&v).unwrap_or_else(|_| String::from("<json pretty error>"));
            Some(ProxyService::pretty_truncated(&pretty, max))
        } else {
            None
        }
    }
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

        Ok(Self { ai_handler, tracer })
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

        tracing::info!(
            request_id = %ctx.request_id,
            method = %method,
            path = %path,
            flow = "request_start",
            "收到代理请求"
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
        // 如果是重试请求，添加短暂延迟避免立即重试
        if ctx.retry_count > 0 {
            let delay_ms = (ctx.retry_count * 100).min(1000); // 最多延迟1秒
            tracing::debug!(
                request_id = %ctx.request_id,
                retry_count = ctx.retry_count,
                delay_ms = delay_ms,
                "Adding retry delay before upstream selection"
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms as u64)).await;
        }

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

    async fn request_body_filter(
        &self,
        session: &mut Session,
        body_chunk: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        // 检查请求头，只处理 JSON 内容
        let content_type = session
            .req_header()
            .headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let is_json = content_type.contains("application/json");

        // `body_chunk` 是一个 Option<Bytes>，`end_of_stream` 表示是否为最后一块
        // Some(bytes) 代表一个数据块
        // end_of_stream=true 代表整个请求体已经接收完毕
        if is_json {
            if let Some(chunk) = body_chunk.take() {
                // JSON：拦截以便在结束时整体修改
                ctx.body.extend_from_slice(&chunk);
                tracing::debug!(
                    request_id = %ctx.request_id,
                    chunk_size = chunk.len(),
                    total_buffer_size = ctx.body.len(),
                    end_of_stream = end_of_stream,
                    "Accumulated JSON request body chunk"
                );
            }
        } else if let Some(chunk) = body_chunk.as_ref() {
            // 非JSON：透传，但复制一份用于日志
            ctx.body.extend_from_slice(chunk);
            tracing::debug!(
                request_id = %ctx.request_id,
                chunk_size = chunk.len(),
                total_buffer_size = ctx.body.len(),
                end_of_stream = end_of_stream,
                "Observed non-JSON request body chunk (pass-through)"
            );
        }

        if end_of_stream {
            // body_chunk 是 None，表示请求体已经全部到达 ctx.body 中
            tracing::info!(
                request_id = %ctx.request_id,
                original_body_size = ctx.body.len(),
                "Complete request body received, applying Google Code Assist modifications"
            );

            // 记录原始请求体（人类可读 + 安全脱敏 + 长度限制）
            let original_preview = if let Some(pretty) = ProxyService::pretty_json_bytes(&ctx.body, ProxyService::MAX_LOG_BODY_BYTES) {
                pretty
            } else if let Ok(text) = std::str::from_utf8(&ctx.body) {
                ProxyService::pretty_truncated(text, ProxyService::MAX_LOG_BODY_BYTES)
            } else {
                format!("<binary:{} bytes> {}", ctx.body.len(), hex::encode(&ctx.body[..ctx.body.len().min(1024)]))
            };
            tracing::info!(
                request_id = %ctx.request_id,
                size = ctx.body.len(),
                content_type = %content_type,
                body = %original_preview,
                "=== 客户端请求体（原始） ==="
            );

            if !is_json {
                // 非JSON：仅记录原始请求体，不做修改
                let original_preview = if let Some(pretty) = ProxyService::pretty_json_bytes(&ctx.body, ProxyService::MAX_LOG_BODY_BYTES) {
                    pretty
                } else if let Ok(text) = std::str::from_utf8(&ctx.body) {
                    ProxyService::pretty_truncated(text, ProxyService::MAX_LOG_BODY_BYTES)
                } else {
                    format!("<binary:{} bytes> {}", ctx.body.len(), hex::encode(&ctx.body[..ctx.body.len().min(1024)]))
                };
                tracing::info!(
                    request_id = %ctx.request_id,
                    size = ctx.body.len(),
                    content_type = %content_type,
                    body = %original_preview,
                    "=== 客户端请求体（原始，非JSON透传） ==="
                );

                return Ok(());
            }

            // --- 这里是核心的Google Code Assist API修改逻辑（JSON） ---
            let modified_body = match serde_json::from_slice::<Value>(&ctx.body) {
                Ok(mut json_value) => {
                    tracing::debug!(
                        request_id = %ctx.request_id,
                        "Successfully parsed request body as JSON, applying modifications"
                    );

                    // 调用AI处理器的Google Code Assist修改逻辑
                    // 这会根据路由和OAuth配置注入相应的字段
                    match self
                        .ai_handler
                        .modify_gemini_request_body_json(&mut json_value, session, ctx)
                        .await
                    {
                        Ok(modified) => {
                            if modified {
                                tracing::info!(
                                    request_id = %ctx.request_id,
                                    "Request body successfully modified for Google Code Assist API"
                                );
                            } else {
                                tracing::debug!(
                                    request_id = %ctx.request_id,
                                    "No modifications needed for this request"
                                );
                            }

                            // 将修改后的 JSON 对象序列化回 Vec<u8>
                            serde_json::to_vec(&json_value).unwrap_or_else(|e| {
                                tracing::error!(
                                    request_id = %ctx.request_id,
                                    error = %e,
                                    "Failed to serialize modified JSON, using original body"
                                );
                                ctx.body.clone()
                            })
                        }
                        Err(e) => {
                            tracing::error!(
                                request_id = %ctx.request_id,
                                error = %e,
                                "Failed to modify request body, using original"
                            );
                            ctx.body.clone()
                        }
                    }
                }
                Err(e) => {
                    // 如果无法解析为 JSON，则保持原始 body 不变
                    tracing::warn!(
                        request_id = %ctx.request_id,
                        error = %e,
                        "Failed to parse body as JSON, forwarding original body"
                    );
                    ctx.body.clone()
                }
            };

            tracing::info!(
                request_id = %ctx.request_id,
                original_size = ctx.body.len(),
                modified_size = modified_body.len(),
                "Request body processing complete, sending to upstream"
            );

            // 记录发送到上游的请求体（最终版本）
            let final_preview = if let Some(pretty) = ProxyService::pretty_json_bytes(&modified_body, ProxyService::MAX_LOG_BODY_BYTES) {
                pretty
            } else if let Ok(text) = std::str::from_utf8(&modified_body) {
                ProxyService::pretty_truncated(text, ProxyService::MAX_LOG_BODY_BYTES)
            } else {
                format!(
                    "<binary:{} bytes> {}",
                    modified_body.len(),
                    hex::encode(&modified_body[..modified_body.len().min(1024)])
                )
            };
            tracing::info!(
                request_id = %ctx.request_id,
                size = modified_body.len(),
                body = %final_preview,
                "=== 上游请求体（最终） ==="
            );

            // 将修改后的完整 body 放入 body_chunk 中
            // Pingora 会将这个 Some(Bytes) 一次性发送给上游服务器
            *body_chunk = Some(Bytes::from(modified_body));
        }

        Ok(())
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
        // 检测可重试的错误类型
        let is_retryable_error = matches!(
            &e.etype,
            ErrorType::ConnectTimedout
                | ErrorType::ReadTimedout
                | ErrorType::WriteTimedout
                | ErrorType::ConnectError
                | ErrorType::ConnectRefused
        );

        // 检查是否可以重试
        let max_retry_count = ctx
            .user_service_api
            .as_ref()
            .and_then(|api| api.retry_count)
            .unwrap_or(3) as u32;

        let should_retry = is_retryable_error
            && ctx.retry_count < max_retry_count
            && ctx.selected_backend.is_some();

        // 增加重试计数
        ctx.retry_count += 1;

        tracing::warn!(
            request_id = %ctx.request_id,
            retry_count = ctx.retry_count,
            max_retry_count = max_retry_count,
            should_retry = should_retry,
            error_type = ?e.etype,
            "Proxy connection failed, evaluating retry"
        );

        if should_retry {
            tracing::info!(
                request_id = %ctx.request_id,
                retry_attempt = ctx.retry_count,
                error_type = ?e.etype,
                "Attempting retry for network/timeout error with same backend"
            );

            // 对于网络错误和超时，使用相同的API密钥重试
            // 这类错误通常是临时的网络问题或服务商临时故障
            // 注意：由于Pingora架构限制，实际重试由Pingora内部处理
            // 这里主要记录重试意图，真正的重试通过返回适当的错误码触发
        }

        // 处理最终失败的情况
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
                retry_count = ctx.retry_count,
                max_retry_count = max_retry_count,
                original_error = %e,
                converted_error = %converted_error,
                "All retry attempts exhausted, returning error response"
            );

            // 上游连接失败时立即记录到数据库（包含重试次数信息）
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
                        Some(format!(
                            "{} (retry_count: {})",
                            converted_error, ctx.retry_count
                        )),
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
                can_reuse_downstream: false,
            };
        }

        // 对于其他错误，使用默认错误码并不重用连接
        if let Some(tracer) = &self.tracer {
            let _ = tracer
                .complete_trace(
                    &ctx.request_id,
                    500,
                    false,
                    None,
                    None,
                    Some("proxy_error".to_string()),
                    Some(format!(
                        "Pingora error: {} (retry_count: {})",
                        e, ctx.retry_count
                    )),
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

                // 如果响应非2xx/3xx，打印响应体用于排查
                if status_code >= 400 {
                    let content_type = ctx
                        .response_details
                        .content_type
                        .clone()
                        .unwrap_or_else(|| "application/json".to_string());
                    let body_preview = ctx
                        .response_details
                        .body
                        .clone()
                        .unwrap_or_else(|| "<empty>".to_string());
                    tracing::error!(
                        request_id = %ctx.request_id,
                        status = status_code,
                        content_type = %content_type,
                        body = %ProxyService::pretty_truncated(&body_preview, 64 * 1024),
                        "=== 上游响应体（失败） ==="
                    );
                }

                // 从响应体JSON中提取所有统计信息 - 使用StatisticsService
                match self
                    .ai_handler
                    .statistics_service()
                    .extract_stats_from_response_body(ctx)
                    .await
                {
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
