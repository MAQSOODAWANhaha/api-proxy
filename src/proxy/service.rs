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
use tokio_util::codec::Decoder as _; // bring decode into scope for SseCodec
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::auth::{AuthManager, types::AuthType};
use crate::cache::CacheManager;
use crate::config::{AppConfig, ProviderConfigManager};
use crate::proxy::provider_strategy;
use crate::proxy::request_handler::{ProxyContext, RequestHandler, ResolvedCredential};
use crate::trace::{TraceSystem, immediate::ImmediateProxyTracer};
use sea_orm::DatabaseConnection;

/// 简化的AI代理服务 - 保持完整业务逻辑
pub struct ProxyService {
    /// AI代理处理器 - 保持原有完整功能
    ai_handler: Arc<RequestHandler>,
    /// 即时写入追踪器
    tracer: Option<Arc<ImmediateProxyTracer>>,
    /// 早期阶段服务（在 early_request_filter 执行）
    early_services: Vec<Arc<dyn EarlyRequestService>>,
    /// 上游请求头构建阶段服务
    upstream_request_services: Vec<Arc<dyn UpstreamRequestService>>,
    /// 请求体阶段服务
    request_body_services: Vec<Arc<dyn RequestBodyService>>,
    /// 响应头阶段服务
    response_header_services: Vec<Arc<dyn ResponseHeaderService>>,
    /// 响应体阶段服务
    response_body_services: Vec<Arc<dyn ResponseBodyService>>,
    /// 上游选择阶段服务
    upstream_peer_services: Vec<Arc<dyn UpstreamPeerService>>,
    /// 上游连接建立回调服务
    connected_to_upstream_services: Vec<Arc<dyn ConnectedToUpstreamService>>,
    /// 代理失败处理服务
    proxy_failure_services: Vec<Arc<dyn ProxyFailureService>>,
    /// 日志阶段服务
    logging_services: Vec<Arc<dyn LoggingService>>,
}

const COMPONENT: &str = "proxy.service";
const COMPONENT_STREAM: &str = "proxy.body.stream";
const COMPONENT_BUFFER: &str = "proxy.body.buffer";
const COMPONENT_UPSTREAM: &str = "proxy.upstream";

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
                                let masked =
                                    format!("{}...{}", &s[..4], &s[s.len().saturating_sub(4)..]);
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

    // 运行 early_request_filter 阶段所有服务
    async fn run_early_services(
        &self,
        session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()> {
        for svc in &self.early_services {
            tracing::debug!(component = COMPONENT, request_id = %ctx.request_id, step = svc.name(), "run early step");
            svc.exec(&self.ai_handler, session, ctx).await?;
        }
        Ok(())
    }
    /// 创建新的代理服务实例 - 保持原有完整功能
    pub fn new(
        config: Arc<AppConfig>,
        db: Arc<DatabaseConnection>,
        cache: Arc<CacheManager>,
        provider_config_manager: Arc<ProviderConfigManager>,
        trace_system: Option<Arc<TraceSystem>>,
        auth_manager: Arc<AuthManager>,
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

        // 构建 early 请求阶段的服务列表
        let early_services: Vec<Arc<dyn EarlyRequestService>> = vec![
            Arc::new(EarlyAuthService),
            Arc::new(EarlyTraceStartService),
            Arc::new(EarlyRateLimitService),
            Arc::new(EarlyProviderSetupService),
            Arc::new(EarlySelectBackendService),
            Arc::new(EarlyCredentialResolveService),
            Arc::new(EarlyDownstreamTimeoutService),
            Arc::new(EarlyTraceExtendService),
        ];

        // 其他阶段服务（现阶段各一项，方便后续扩展为多步）
        let upstream_request_services: Vec<Arc<dyn UpstreamRequestService>> =
            vec![Arc::new(DefaultUpstreamRequestService)];
        let request_body_services: Vec<Arc<dyn RequestBodyService>> =
            vec![Arc::new(DefaultRequestBodyService)];
        let response_header_services: Vec<Arc<dyn ResponseHeaderService>> =
            vec![Arc::new(DefaultResponseHeaderService)];
        let response_body_services: Vec<Arc<dyn ResponseBodyService>> =
            vec![Arc::new(DefaultResponseBodyService)];

        // 上游选择/连接/失败阶段默认服务
        let upstream_peer_services: Vec<Arc<dyn UpstreamPeerService>> =
            vec![Arc::new(DefaultUpstreamPeerService)];
        let connected_to_upstream_services: Vec<Arc<dyn ConnectedToUpstreamService>> =
            vec![Arc::new(DefaultConnectedToUpstreamService)];
        let proxy_failure_services: Vec<Arc<dyn ProxyFailureService>> =
            vec![Arc::new(DefaultProxyFailureService)];
        let logging_services: Vec<Arc<dyn LoggingService>> = vec![Arc::new(DefaultLoggingService)];

        Ok(Self {
            ai_handler,
            tracer,
            early_services,
            upstream_request_services,
            request_body_services,
            response_header_services,
            response_body_services,
            upstream_peer_services,
            connected_to_upstream_services,
            proxy_failure_services,
            logging_services,
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

    /// 创建provider特定的响应体处理服务
    fn provider_response_body_services(
        &self,
        ctx: &ProxyContext,
    ) -> Vec<Arc<dyn ResponseBodyService>> {
        let mut services = Vec::new();

        // 根据provider类型注册相应的响应体处理服务
        if let Some(provider) = ctx.provider_type.as_ref() {
            if let Some(strategy_name) =
                crate::proxy::provider_strategy::ProviderRegistry::match_name(&provider.name)
            {
                match strategy_name {
                    "openai" => {
                        // 创建OpenAI策略并设置数据库连接
                        let db_connection = self.ai_handler.db_connection();
                        let mut strategy = crate::proxy::provider_strategy::provider_strategy_openai::OpenAIStrategy::new();
                        strategy = strategy.with_db(db_connection);
                        services.push(Arc::new(strategy) as Arc<dyn ResponseBodyService>);
                    }
                    // 其他provider可以在这里扩展
                    _ => {}
                }
            }
        }

        services
    }
}

// =============== 阶段服务定义（仅用于 early_request_filter） ===============

#[async_trait]
trait EarlyRequestService: Send + Sync {
    fn name(&self) -> &'static str {
        "early_service"
    }
    async fn exec(
        &self,
        ai: &RequestHandler,
        session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()>;
}

// 1) 认证
struct EarlyAuthService;
#[async_trait]
impl EarlyRequestService for EarlyAuthService {
    fn name(&self) -> &'static str {
        "auth"
    }
    async fn exec(
        &self,
        ai: &RequestHandler,
        session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()> {
        let auth_svc = ai.auth_service().clone();
        match auth_svc
            .authenticate_entry_api(session, &ctx.request_id)
            .await
        {
            Ok(user_api) => {
                ctx.user_service_api = Some(user_api.clone());
                info!(
                    event = "auth_ok", component = COMPONENT, request_id = %ctx.request_id,
                    user_service_api_id = user_api.id, user_id = user_api.user_id, "认证通过"
                );
                Ok(())
            }
            Err(e) => {
                error!(event = "auth_fail", component = COMPONENT, request_id = %ctx.request_id, error = %e, "认证失败");
                let _ = ai
                    .tracing_service()
                    .complete_trace_with_error(&ctx.request_id, &e)
                    .await;
                Err(crate::pingora_error!(crate::proxy_err!(auth, "{}", e)))
            }
        }
    }
}

// 2) 启动追踪
struct EarlyTraceStartService;
#[async_trait]
impl EarlyRequestService for EarlyTraceStartService {
    fn name(&self) -> &'static str {
        "trace_start"
    }
    async fn exec(
        &self,
        ai: &RequestHandler,
        session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()> {
        if let Some(user_api) = ctx.user_service_api.as_ref() {
            let method = session.req_header().method.as_str();
            let path_owned = session.req_header().uri.path().to_string();
            let req_stats = ai.statistics_service().collect_request_stats(session);
            if let Err(e) = ai
                .tracing_service()
                .start_trace(
                    &ctx.request_id,
                    user_api.id,
                    Some(user_api.user_id),
                    method,
                    Some(path_owned),
                    Some(req_stats.client_ip.clone()),
                    req_stats.user_agent.clone(),
                )
                .await
            {
                warn!(component = COMPONENT, request_id = %ctx.request_id, error = %e, "Failed to start trace");
            }
        }
        Ok(())
    }
}

// =============== 其他阶段服务 trait 与默认实现 ===============

#[async_trait]
trait UpstreamRequestService: Send + Sync {
    fn name(&self) -> &'static str {
        "upstream_request"
    }
    async fn exec(
        &self,
        ai: &RequestHandler,
        session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()>;
}

struct DefaultUpstreamRequestService;
#[async_trait]
impl UpstreamRequestService for DefaultUpstreamRequestService {
    async fn exec(
        &self,
        ai: &RequestHandler,
        session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()> {
        ai.filter_upstream_request(session, upstream_request, ctx)
            .await
            .map_err(|e| {
                error!(
                    request_id = %ctx.request_id,
                    error = %e,
                    "Failed to filter upstream request"
                );
                // 统一错误追踪（异步）
                let req_id = ctx.request_id.clone();
                let tracer = ai.tracing_service().clone();
                let (code, etype, msg) = match &e {
                    crate::error::ProxyError::Network { message, .. } => (
                        502,
                        "network_error".to_string(),
                        format!("Network error: {}", message),
                    ),
                    _ => (500, "request_filter_error".to_string(), e.to_string()),
                };
                tokio::spawn(async move {
                    let _ = tracer
                        .complete_trace_failure(&req_id, code, Some(etype), Some(msg))
                        .await;
                });
                match e {
                    crate::error::ProxyError::Network { message, .. } => {
                        crate::pingora_error!(crate::proxy_err!(
                            network,
                            "Network error during request processing: {}",
                            message
                        ))
                    }
                    _ => crate::pingora_error!(crate::proxy_err!(
                        internal,
                        "Internal error during request processing"
                    )),
                }
            })?;

        // provider 特定策略的请求改写（如 Gemini 注入/补充 Header）
        if let Some(pt) = ctx.provider_type.as_ref() {
            if let Some(name) = provider_strategy::ProviderRegistry::match_name(&pt.name) {
                if let Some(strategy) = provider_strategy::make_strategy(name, None) {
                    strategy
                        .modify_request(session, upstream_request, ctx)
                        .await
                        .map_err(|e| {
                            crate::pingora_error!(crate::proxy_err!(
                                internal,
                                "provider modify_request error: {}",
                                e
                            ))
                        })?;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
trait RequestBodyService: Send + Sync {
    async fn exec(
        &self,
        ai: &RequestHandler,
        session: &mut Session,
        body_chunk: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()>;
}

struct DefaultRequestBodyService;
#[async_trait]
impl RequestBodyService for DefaultRequestBodyService {
    async fn exec(
        &self,
        ai: &RequestHandler,
        session: &mut Session,
        body_chunk: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()> {
        let content_type = session
            .req_header()
            .headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let is_json = content_type.contains("application/json");
        let should_modify = ctx.will_modify_body && is_json;

        if is_json && should_modify {
            if let Some(chunk) = body_chunk.take() {
                ctx.body.extend_from_slice(&chunk);
                debug!(
                    request_id = %ctx.request_id,
                    chunk_size = chunk.len(),
                    total_buffer_size = ctx.body.len(),
                    end_of_stream = end_of_stream,
                    "Accumulated JSON request body chunk (will modify)"
                );
            }
        } else if let Some(chunk) = body_chunk.as_ref() {
            ctx.body.extend_from_slice(chunk);
            debug!(
                request_id = %ctx.request_id,
                chunk_size = chunk.len(),
                total_buffer_size = ctx.body.len(),
                end_of_stream = end_of_stream,
                "Observed request body chunk (pass-through)"
            );
        }
        if end_of_stream {
            if !is_json || !should_modify {
                info!(
                    event = "upstream_request_body_final",
                    component = COMPONENT,
                    request_id = %ctx.request_id,
                    size = ctx.body.len(),
                    "上游请求体（最终）"
                );
                return Ok(());
            }

            let modified_body = match serde_json::from_slice::<Value>(&ctx.body) {
                Ok(mut json_value) => {
                    debug!(
                        request_id = %ctx.request_id,
                        "Successfully parsed request body as JSON, applying modifications"
                    );
                    match ai
                        .modify_provider_request_body_json(&mut json_value, session, ctx)
                        .await
                    {
                        Ok(modified) => {
                            if modified {
                                info!(
                                    request_id = %ctx.request_id,
                                    "Request body successfully modified for Google Code Assist API"
                                );
                                serde_json::to_vec(&json_value).unwrap_or_else(|e| {
                                    error!(
                                        request_id = %ctx.request_id,
                                        error = %e,
                                        "Failed to serialize modified JSON, using original body"
                                    );
                                    ctx.body.clone()
                                })
                            } else {
                                debug!(
                                    request_id = %ctx.request_id,
                                    "No modifications needed after parse; forwarding original body"
                                );
                                ctx.body.clone()
                            }
                        }
                        Err(e) => {
                            error!(
                                request_id = %ctx.request_id,
                                error = %e,
                                "Failed to modify request body, using original"
                            );
                            ctx.body.clone()
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        request_id = %ctx.request_id,
                        error = %e,
                        "Failed to parse body as JSON, forwarding original body"
                    );
                    ctx.body.clone()
                }
            };

            info!(
                event = "upstream_request_body_final",
                component = COMPONENT,
                request_id = %ctx.request_id,
                size = modified_body.len(),
                "上游请求体（最终）"
            );

            *body_chunk = Some(Bytes::from(modified_body));
        }

        Ok(())
    }
}

#[async_trait]
trait ResponseHeaderService: Send + Sync {
    async fn exec(
        &self,
        ai: &RequestHandler,
        session: &Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()>;
}

struct DefaultResponseHeaderService;
#[async_trait]
impl ResponseHeaderService for DefaultResponseHeaderService {
    async fn exec(
        &self,
        ai: &RequestHandler,
        session: &Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()> {
        ai.filter_upstream_response(session, upstream_response, ctx)
            .await
            .map_err(|e| {
                error!(
                    event = "upstream_response_filter_fail",
                    component = COMPONENT,
                    request_id = %ctx.request_id,
                    error = %e,
                    "处理上游响应头失败"
                );
                crate::pingora_error!(crate::proxy_err!(
                    internal,
                    "failed to filter upstream response: {}",
                    e
                ))
            })?;

        let response_time = ctx.start_time.elapsed();
        let status_code = upstream_response.status.as_u16();
        info!(
            event = "upstream_response_complete",
            component = COMPONENT,
            request_id = %ctx.request_id,
            status_code = status_code,
            response_time_ms = response_time.as_millis(),
            "上游响应处理完成"
        );
        Ok(())
    }
}

pub trait ResponseBodyService: Send + Sync {
    fn exec(
        &self,
        body: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<Option<std::time::Duration>>;
}

struct DefaultResponseBodyService;
impl ResponseBodyService for DefaultResponseBodyService {
    fn exec(
        &self,
        body: &mut Option<bytes::Bytes>,
        end_of_stream: bool,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<Option<std::time::Duration>> {
        if let Some(data) = body {
            // 尝试将 chunk 按 UTF-8 解码为字符串并解析为 JSON，再格式化为 pretty JSON 输出；否则输出文本预览或二进制调试信息
            if let Ok(text) = std::str::from_utf8(&data) {
                // 尝试将文本解析为 JSON 并 pretty print
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(text) {
                    if let Ok(body) = serde_json::to_string_pretty(&json_val) {
                        tracing::info!(
                            component = COMPONENT_STREAM,
                            event = "stream_line_json",
                            request_id = %ctx.request_id,
                            body_json = %body,
                            "Received upstream response body chunk as JSON"
                        );
                    }
                } else {
                    tracing::info!(
                        component = COMPONENT_STREAM,
                        event = "stream_line_text",
                        request_id = %ctx.request_id,
                        body_preview = %text,
                        "Received upstream response body chunk as text"
                    );
                }
            } else {
                // 非 UTF-8 二进制数据，记录为调试级别并保留长度信息
                tracing::debug!(
                    component = COMPONENT_STREAM,
                    event = "stream_line_binary",
                    request_id = %ctx.request_id,
                    chunk_size = data.len(),
                    "Received non-UTF8 binary upstream response body chunk"
                );
            }

            let (is_sse, is_json_stream) = ctx
                .response_details
                .content_type
                .as_deref()
                .map(|ct| {
                    (
                        ct.contains("text/event-stream"),
                        ct.contains("application/stream+json"),
                    )
                })
                .unwrap_or((false, false));

            if is_sse || is_json_stream {
                // 流式：仅保留尾窗，用事件/行解析（latest-wins），不累计全量
                ctx.response_details.push_tail_window(data);

                if is_sse {
                    // 严格 SSE：使用 codec 解码事件，再聚合 usage/model（流式一律累加）
                    let (codec, bytes_buf) = ctx.ensure_sse_state();
                    bytes_buf.extend_from_slice(data);
                    let mut parsed_events = 0usize;
                    let mut parse_errors = 0usize;
                    let mut events: Vec<crate::utils::event_stream::EventStream> = Vec::new();
                    loop {
                        match codec.decode(bytes_buf) {
                            Ok(Some(ev)) => events.push(ev),
                            Ok(None) => break,
                            Err(_) => break,
                        }
                    }
                    for ev in events {
                        let json = ev.data;
                        if json.is_null() {
                            parse_errors += 1;
                            continue;
                        }
                        parsed_events += 1;
                        let usage = crate::statistics::usage_model::extract_tokens_from_json(
                            ctx.provider_type.as_ref(),
                            &json,
                        );
                        if usage.prompt_tokens.is_some()
                            || usage.completion_tokens.is_some()
                            || usage.total_tokens.is_some()
                            || usage.cache_create_tokens.is_some()
                            || usage.cache_read_tokens.is_some()
                        {
                            let partial = crate::statistics::types::PartialUsage {
                                prompt_tokens: usage.prompt_tokens,
                                completion_tokens: usage.completion_tokens,
                                total_tokens: usage.total_tokens,
                                cache_create_tokens: usage.cache_create_tokens,
                                cache_read_tokens: usage.cache_read_tokens,
                            };
                            ctx.usage_partial.merge_sum(&partial);
                        }
                        if ctx.requested_model.is_none() {
                            if let Some(m) = json.get("model").and_then(|m| m.as_str()) {
                                ctx.requested_model = Some(m.to_string());
                            }
                        }

                        ctx.last_parsed_json = Some(json);
                    }
                    tracing::debug!(
                        component = COMPONENT_STREAM,
                        request_id = %ctx.request_id,
                        parsed_events = parsed_events,
                        parse_errors = parse_errors,
                        event = "streaming_chunk_parsed",
                        "Processed streaming chunk via codec",
                    );
                } else {
                    // application/stream+json：逐行 JSON 解析（每行一个事件）（流式一律累加）
                    if let Ok(text) = std::str::from_utf8(data) {
                        let line_buf = ctx.ensure_json_lines_state();
                        line_buf.push_str(text);

                        if let Some(last_nl) = line_buf.rfind('\n') {
                            let complete = line_buf[..=last_nl].to_string();
                            let leftover = line_buf[last_nl + 1..].to_string();
                            *line_buf = leftover;

                            let mut parsed_events = 0usize;
                            let mut parse_errors = 0usize;

                            for raw in complete.lines() {
                                let mut line = raw.trim();
                                if line.is_empty() || line.starts_with(':') {
                                    continue;
                                }
                                if let Some(rest) = line.strip_prefix("data:") {
                                    line = rest.trim_start();
                                }
                                if let Some(pos) = line.find('{') {
                                    let json_str = &line[pos..];
                                    match serde_json::from_str::<serde_json::Value>(json_str) {
                                        Ok(json) => {
                                            parsed_events += 1;
                                            let usage = crate::statistics::usage_model::extract_tokens_from_json(ctx.provider_type.as_ref(), &json);
                                            if usage.prompt_tokens.is_some()
                                                || usage.completion_tokens.is_some()
                                                || usage.total_tokens.is_some()
                                                || usage.cache_create_tokens.is_some()
                                                || usage.cache_read_tokens.is_some()
                                            {
                                                let partial =
                                                    crate::statistics::types::PartialUsage {
                                                        prompt_tokens: usage.prompt_tokens,
                                                        completion_tokens: usage.completion_tokens,
                                                        total_tokens: usage.total_tokens,
                                                        cache_create_tokens: usage
                                                            .cache_create_tokens,
                                                        cache_read_tokens: usage.cache_read_tokens,
                                                    };
                                                ctx.usage_partial.merge_sum(&partial);
                                            }
                                            if ctx.requested_model.is_none() {
                                                if let Some(m) =
                                                    json.get("model").and_then(|m| m.as_str())
                                                {
                                                    ctx.requested_model = Some(m.to_string());
                                                }
                                            }
                                            ctx.last_parsed_json = Some(json);
                                        }
                                        Err(_) => parse_errors += 1,
                                    }
                                }
                            }
                            tracing::info!(
                                component = COMPONENT_STREAM,
                                request_id = %ctx.request_id,
                                parsed_events = parsed_events,
                                parse_errors = parse_errors,
                                event = "streaming_chunk_parsed",
                                "Processed streaming JSON lines chunk",
                            );
                        }
                    }
                }
            } else {
                // 非流式：按块累计，供后续收口统计与（限量）解压
                ctx.response_details.add_body_chunk(data);
                tracing::debug!(
                    component = COMPONENT_BUFFER,
                    request_id = %ctx.request_id,
                    chunk_size = data.len(),
                    total_size = ctx.response_details.body_chunks.len(),
                    event = "response_chunk",
                    "Buffered response body chunk",
                );
            }
        }

        // 单入口收尾：在 DefaultResponseBodyService 内完成唯一收口
        if end_of_stream {
            let (is_sse, is_json_stream) = ctx
                .response_details
                .content_type
                .as_deref()
                .map(|ct| {
                    (
                        ct.contains("text/event-stream"),
                        ct.contains("application/stream+json"),
                    )
                })
                .unwrap_or((false, false));

            if is_sse || is_json_stream {
                // 流式：将增量聚合结果写入 usage_final，并覆盖最终模型
                let stats = crate::statistics::usage_model::finalize_streaming(ctx);
                ctx.usage_final = Some(stats.usage.clone());
                if let Some(m) = stats.model_name {
                    ctx.requested_model = Some(m);
                }
                tracing::info!(
                    component = COMPONENT_STREAM,
                    event = "stream_aggregate_done",
                    request_id = %ctx.request_id,
                    prompt_tokens = ?ctx.usage_final.as_ref().and_then(|u| u.prompt_tokens),
                    completion_tokens = ?ctx.usage_final.as_ref().and_then(|u| u.completion_tokens),
                    total_tokens = ?ctx.usage_final.as_ref().and_then(|u| u.total_tokens),
                    model_name = ?ctx.requested_model,
                    "Streaming usage aggregation completed",
                );
            } else {
                // 非流式：解析缓冲正文（限量解压 + 窗口）
                use crate::statistics::util::{decompress_for_stats, find_last_balanced_json};
                let encoding_owned = ctx.response_details.content_encoding.clone();
                let encoding = encoding_owned.as_deref();
                let raw = &ctx.response_details.body_chunks;

                let mut parsed_json: Option<serde_json::Value> = None;
                if !raw.is_empty() {
                    let decoded = decompress_for_stats(encoding, raw, 2 * 1024 * 1024);
                    if let Ok(body_str) = std::str::from_utf8(&decoded) {
                        let window = if body_str.len() > 1024 * 1024 {
                            &body_str[body_str.len() - 512 * 1024..]
                        } else {
                            body_str
                        };
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(window) {
                            parsed_json = Some(v);
                        } else {
                            let mut last_json: Option<serde_json::Value> = None;
                            for line in window.lines() {
                                let t = line.trim_start_matches("data:").trim();
                                if let Some(pos) = t.find('{') {
                                    if let Ok(j) =
                                        serde_json::from_str::<serde_json::Value>(&t[pos..])
                                    {
                                        last_json = Some(j);
                                    }
                                }
                            }
                            if last_json.is_some() {
                                parsed_json = last_json;
                            } else {
                                parsed_json = find_last_balanced_json(window);
                            }
                        }
                    }
                }

                if let Some(json) = parsed_json.clone() {
                    let usage = crate::statistics::usage_model::extract_tokens_from_json(
                        ctx.provider_type.as_ref(),
                        &json,
                    );
                    let mut model = crate::statistics::usage_model::extract_model_from_json(&json);
                    if model.is_none() {
                        model = ctx.requested_model.clone();
                    }
                    ctx.usage_final = Some(usage);
                    if let Some(m) = model {
                        ctx.requested_model = Some(m);
                    }

                    // 打印 body JSON（已脱敏）
                    let mut v = json.clone();
                    ProxyService::sanitize_json_value(&mut v);
                    if let Ok(pretty) = serde_json::to_string_pretty(&v) {
                        let preview = if pretty.len() > 8192 {
                            format!(
                                "{}...[truncated {} bytes]",
                                &pretty[..8192],
                                pretty.len() - 8192
                            )
                        } else {
                            pretty
                        };
                        tracing::debug!(
                            component = COMPONENT_BUFFER,
                            event = "buffer_body_json",
                            request_id = %ctx.request_id,
                            body_json = %preview,
                            "Buffered response JSON body",
                        );
                    }
                } else {
                    // 解析失败兜底
                    ctx.usage_final = Some(crate::statistics::types::TokenUsageMetrics {
                        prompt_tokens: Some(0),
                        completion_tokens: Some(0),
                        total_tokens: Some(0),
                        cache_create_tokens: None,
                        cache_read_tokens: None,
                    });
                }

                // 清理缓冲
                ctx.response_details.clear_body_chunks();

                tracing::info!(
                    component = COMPONENT_BUFFER,
                    event = "buffer_parsed",
                    request_id = %ctx.request_id,
                    prompt_tokens = ?ctx.usage_final.as_ref().and_then(|u| u.prompt_tokens),
                    completion_tokens = ?ctx.usage_final.as_ref().and_then(|u| u.completion_tokens),
                    total_tokens = ?ctx.usage_final.as_ref().and_then(|u| u.total_tokens),
                    model_name = ?ctx.requested_model,
                    "Buffered JSON parsed and usage computed",
                );
            }
        }

        Ok(None)
    }
}

#[async_trait]
trait UpstreamPeerService: Send + Sync {
    async fn select(
        &self,
        ai: &RequestHandler,
        session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<Option<Box<HttpPeer>>>;
}

struct DefaultUpstreamPeerService;
#[async_trait]
impl UpstreamPeerService for DefaultUpstreamPeerService {
    async fn select(
        &self,
        ai: &RequestHandler,
        _session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<Option<Box<HttpPeer>>> {
        ai.select_upstream_peer(ctx).await.map(Some).map_err(|e| {
            // 统一错误追踪（异步，不阻塞）：上游选择失败
            let req_id = ctx.request_id.clone();
            let tracer = ai.tracing_service().clone();
            let (code, etype, msg) = match &e {
                crate::error::ProxyError::ConnectionTimeout {
                    timeout_seconds, ..
                } => (
                    504,
                    "connection_timeout".to_string(),
                    format!("Connection timeout after {}s", timeout_seconds),
                ),
                crate::error::ProxyError::ReadTimeout {
                    timeout_seconds, ..
                } => (
                    504,
                    "read_timeout".to_string(),
                    format!("Read timeout after {}s", timeout_seconds),
                ),
                crate::error::ProxyError::Network { message, .. } => (
                    502,
                    "network_error".to_string(),
                    format!("Network error: {}", message),
                ),
                _ => (500, "upstream_error".to_string(), e.to_string()),
            };
            tokio::spawn(async move {
                let _ = tracer
                    .complete_trace_failure(&req_id, code, Some(etype), Some(msg))
                    .await;
            });
            match e {
                crate::error::ProxyError::ConnectionTimeout {
                    timeout_seconds, ..
                } => {
                    crate::pingora_error!(crate::proxy_err!(
                        connection_timeout,
                        "Connection timeout after {}s",
                        timeout_seconds
                    ))
                }
                crate::error::ProxyError::ReadTimeout {
                    timeout_seconds, ..
                } => {
                    crate::pingora_error!(crate::proxy_err!(
                        read_timeout,
                        "Read timeout after {}s",
                        timeout_seconds
                    ))
                }
                crate::error::ProxyError::Network { message, .. } => {
                    crate::pingora_error!(crate::proxy_err!(network, "Network error: {}", message))
                }
                _ => crate::pingora_error!(crate::proxy_err!(internal, "Internal server error")),
            }
        })
    }
}

#[async_trait]
trait ConnectedToUpstreamService: Send + Sync {
    async fn exec(
        &self,
        reused: bool,
        peer: &HttpPeer,
        digest: Option<&Digest>,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()>;
}

struct DefaultConnectedToUpstreamService;
#[async_trait]
impl ConnectedToUpstreamService for DefaultConnectedToUpstreamService {
    async fn exec(
        &self,
        reused: bool,
        peer: &HttpPeer,
        _digest: Option<&Digest>,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()> {
        info!(
            event = "upstream_connected",
            component = COMPONENT_UPSTREAM,
            request_id = %ctx.request_id,
            reused = reused,
            peer_addr = ?peer._address,
            sni = %peer.sni,
            "已连接上游"
        );
        Ok(())
    }
}

trait ProxyFailureService: Send + Sync {
    fn handle(
        &self,
        tracer: Option<Arc<ImmediateProxyTracer>>,
        ai: &RequestHandler,
        err: &Error,
        ctx: &mut ProxyContext,
    ) -> FailToProxy;
}

struct DefaultProxyFailureService;
impl ProxyFailureService for DefaultProxyFailureService {
    fn handle(
        &self,
        tracer: Option<Arc<ImmediateProxyTracer>>,
        ai: &RequestHandler,
        e: &Error,
        ctx: &mut ProxyContext,
    ) -> FailToProxy {
        let is_retryable_error = matches!(
            &e.etype,
            ErrorType::ConnectTimedout
                | ErrorType::ReadTimedout
                | ErrorType::WriteTimedout
                | ErrorType::ConnectError
                | ErrorType::ConnectRefused
        );

        let max_retry_count = ctx
            .user_service_api
            .as_ref()
            .and_then(|api| api.retry_count)
            .unwrap_or(3) as u32;

        let should_retry = is_retryable_error
            && ctx.retry_count < max_retry_count
            && ctx.selected_backend.is_some();

        ctx.retry_count += 1;

        warn!(
            event = "fail",
            component = COMPONENT,
            request_id = %ctx.request_id,
            error_type = ?e.etype,
            retry_count = ctx.retry_count,
            max_retry_count = max_retry_count,
            should_retry = should_retry,
            "代理失败，评估是否重试"
        );

        if should_retry {
            info!(
                event = "retry",
                component = COMPONENT,
                request_id = %ctx.request_id,
                retry_attempt = ctx.retry_count,
                error_type = ?e.etype,
                "对网络/超时错误执行重试（相同后端）"
            );
        }

        let is_timeout_or_network_error = matches!(
            &e.etype,
            ErrorType::ConnectTimedout
                | ErrorType::ReadTimedout
                | ErrorType::WriteTimedout
                | ErrorType::ConnectError
                | ErrorType::ConnectRefused
        );

        if is_timeout_or_network_error {
            let converted_error = ai.convert_pingora_error(e, ctx);

            error!(
                event = "fail_final",
                component = COMPONENT,
                request_id = %ctx.request_id,
                retry_count = ctx.retry_count,
                max_retry_count = max_retry_count,
                original_error = %e,
                converted_error = %converted_error,
                "重试已用尽，返回错误响应"
            );

            if let Some(tracer) = tracer {
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

                let req_id = ctx.request_id.clone();
                let retry = ctx.retry_count;
                let converted_error_msg = format!("{}", converted_error);
                let _ = tokio::spawn(async move {
                    let _ = tracer
                        .complete_trace(
                            &req_id,
                            error_code,
                            false,
                            None,
                            None,
                            Some(error_type.to_string()),
                            Some(format!("{} (retry_count: {})", converted_error_msg, retry)),
                        )
                        .await;
                });
            }

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

        if let Some(tracer) = tracer {
            let req_id = ctx.request_id.clone();
            let retry = ctx.retry_count;
            let e_msg = format!("{}", e);
            let _ = tokio::spawn(async move {
                let _ = tracer
                    .complete_trace(
                        &req_id,
                        500,
                        false,
                        None,
                        None,
                        Some("proxy_error".to_string()),
                        Some(format!("Pingora error: {} (retry_count: {})", e_msg, retry)),
                    )
                    .await;
            });
        }

        FailToProxy {
            error_code: 500,
            can_reuse_downstream: false,
        }
    }
}

#[async_trait]
trait LoggingService: Send + Sync {
    async fn exec(
        &self,
        ai: &RequestHandler,
        tracer: Option<Arc<ImmediateProxyTracer>>,
        session: &mut Session,
        e: Option<&Error>,
        ctx: &mut ProxyContext,
    );
}

struct DefaultLoggingService;
#[async_trait]
impl LoggingService for DefaultLoggingService {
    async fn exec(
        &self,
        _ai: &RequestHandler,
        _tracer: Option<Arc<ImmediateProxyTracer>>,
        _session: &mut Session,
        _e: Option<&Error>,
        _ctx: &mut ProxyContext,
    ) {
        // 默认不做额外处理，保留现有 logging 实现
    }
}

// =============== Provider Service Registry（按需注入） ===============

fn provider_upstream_request_services(ctx: &ProxyContext) -> Vec<Arc<dyn UpstreamRequestService>> {
    if let Some(pt) = ctx.provider_type.as_ref() {
        let name = pt.name.to_ascii_lowercase();
        // 示例：Gemini 可在此返回自定义的 UpstreamRequestService；当前默认由 DefaultUpstreamRequestService 调用 strategy.modify_request
        if name.contains("gemini") {
            // 这里暂不额外插入，避免重复调用；需要时可放开：
            // return vec![Arc::new(GeminiUpstreamRequestService)];
        }
    }
    Vec::new()
}

fn provider_response_header_services(_ctx: &ProxyContext) -> Vec<Arc<dyn ResponseHeaderService>> {
    Vec::new()
}

// 示例占位：如需对某 provider 进行更强定制，可实现如下结构体并在上面的注册函数中返回
// struct GeminiUpstreamRequestService;
// #[async_trait]
// impl UpstreamRequestService for GeminiUpstreamRequestService {
//     async fn exec(&self, ai: &RequestHandler, session: &mut Session, upstream_request: &mut RequestHeader, ctx: &mut ProxyContext) -> pingora_core::Result<()> {
//         if let Some(pt) = ctx.provider_type.as_ref() {
//             if let Some(name) = provider_strategy::ProviderRegistry::match_name(&pt.name) {
//                 if let Some(strategy) = provider_strategy::make_strategy(name, None) {
//                     strategy.modify_request(session, upstream_request, ctx)
//                         .await
//                         .map_err(|e| crate::pingora_error!(crate::proxy_err!(internal, "provider modify_request error: {}", e)))?;
//                 }
//             }
//         }
//         Ok(())
//     }
// }
// 3) 限流
struct EarlyRateLimitService;
#[async_trait]
impl EarlyRequestService for EarlyRateLimitService {
    fn name(&self) -> &'static str {
        "rate_limit"
    }
    async fn exec(
        &self,
        ai: &RequestHandler,
        _session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()> {
        if let Some(user_api) = ctx.user_service_api.as_ref() {
            if let Err(e) = ai.check_rate_limit(user_api).await {
                warn!(event = "rate_limited", component = COMPONENT, request_id = %ctx.request_id, error = %e, "命中限流");
                let _ = ai
                    .tracing_service()
                    .complete_trace_with_error(&ctx.request_id, &e)
                    .await;
                return Err(crate::pingora_error!(crate::proxy_err!(
                    rate_limit, "{}", e
                )));
            }
        }
        Ok(())
    }
}

// 4) 提供商配置 + 超时
struct EarlyProviderSetupService;
#[async_trait]
impl EarlyRequestService for EarlyProviderSetupService {
    fn name(&self) -> &'static str {
        "provider_setup"
    }
    async fn exec(
        &self,
        ai: &RequestHandler,
        _session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()> {
        let user_api = ctx
            .user_service_api
            .as_ref()
            .expect("user_service_api must exist after auth");
        let provider_type = ai
            .get_provider_type(user_api.provider_type_id)
            .await
            .map_err(|e| {
                crate::pingora_error!(crate::proxy_err!(internal, "provider config error: {}", e))
            })?;

        let timeout_from_dynamic = if let Ok(Some(pc)) = ai
            .provider_config_manager()
            .get_provider_by_name(&provider_type.name)
            .await
        {
            pc.timeout_seconds
        } else {
            None
        };

        let timeout = user_api
            .timeout_seconds
            .or(timeout_from_dynamic)
            .or(provider_type.timeout_seconds);

        ctx.provider_type = Some(provider_type.clone());
        ctx.selected_provider = Some(provider_type.name.clone());
        ctx.timeout_seconds = timeout;
        Ok(())
    }
}

// 5) 选择后端密钥
struct EarlySelectBackendService;
#[async_trait]
impl EarlyRequestService for EarlySelectBackendService {
    fn name(&self) -> &'static str {
        "select_backend_key"
    }
    async fn exec(
        &self,
        ai: &RequestHandler,
        _session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()> {
        let user_api = ctx.user_service_api.as_ref().unwrap();
        let selected_backend = ai
            .select_api_key(user_api, &ctx.request_id)
            .await
            .map_err(|e| {
                crate::pingora_error!(crate::proxy_err!(
                    upstream_not_available,
                    "no backend key available: {}",
                    e
                ))
            })?;
        ctx.selected_backend = Some(selected_backend);
        Ok(())
    }
}

// 6) 凭证解析
struct EarlyCredentialResolveService;
#[async_trait]
impl EarlyRequestService for EarlyCredentialResolveService {
    fn name(&self) -> &'static str {
        "resolve_credential"
    }
    async fn exec(
        &self,
        ai: &RequestHandler,
        _session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()> {
        let selected_backend = ctx
            .selected_backend
            .as_ref()
            .expect("selected_backend must exist");
        match AuthType::from(selected_backend.auth_type.as_str()) {
            AuthType::ApiKey => {
                ctx.resolved_credential =
                    Some(ResolvedCredential::ApiKey(selected_backend.api_key.clone()));
            }
            AuthType::OAuth => {
                let token = ai
                    .resolve_oauth_access_token(&selected_backend.api_key, &ctx.request_id)
                    .await
                    .map_err(|e| {
                        crate::pingora_error!(crate::proxy_err!(auth, "oauth session error: {}", e))
                    })?;
                ctx.resolved_credential = Some(ResolvedCredential::OAuthAccessToken(token));
            }
            other => {
                let err = crate::proxy_err!(business, "Unsupported auth type: {}", other);
                return Err(crate::pingora_error!(err));
            }
        }
        Ok(())
    }
}

// 7) 下游超时配置（与业务超时联动）
struct EarlyDownstreamTimeoutService;
#[async_trait]
impl EarlyRequestService for EarlyDownstreamTimeoutService {
    fn name(&self) -> &'static str {
        "downstream_timeout"
    }
    async fn exec(
        &self,
        _ai: &RequestHandler,
        session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()> {
        use std::time::Duration;
        let timeout_seconds = ctx.timeout_seconds.unwrap_or(30) as u64;
        let downstream_timeout_secs = timeout_seconds * 2;
        session.set_read_timeout(Some(Duration::from_secs(downstream_timeout_secs)));
        session.set_write_timeout(Some(Duration::from_secs(downstream_timeout_secs)));
        info!(
            event = "prep_ok", component = COMPONENT, request_id = %ctx.request_id,
            configured_timeout_s = timeout_seconds, downstream_timeout_s = downstream_timeout_secs,
            "代理准备完成"
        );
        Ok(())
    }
}

// 8) 追踪扩展字段更新
struct EarlyTraceExtendService;
#[async_trait]
impl EarlyRequestService for EarlyTraceExtendService {
    fn name(&self) -> &'static str {
        "trace_extend"
    }
    async fn exec(
        &self,
        ai: &RequestHandler,
        _session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<()> {
        if let (Some(pt), Some(backend)) =
            (ctx.provider_type.as_ref(), ctx.selected_backend.as_ref())
        {
            if let Err(err) = ai
                .tracing_service()
                .update_trace_model_info(
                    &ctx.request_id,
                    Some(pt.id),
                    ctx.requested_model.clone(),
                    Some(backend.id),
                )
                .await
            {
                warn!(component = COMPONENT, request_id = %ctx.request_id, error = %err, "Failed to update model info");
            }
        }
        Ok(())
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
            debug!(
                request_id = %ctx.request_id,
                "Trace will be started when request info is available"
            );
        }

        ctx
    }

    async fn early_request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        let path = session.req_header().uri.path();
        let method = session.req_header().method.as_str();

        // 收集部分客户端信息用于日志
        let req_stats = self
            .ai_handler
            .statistics_service()
            .collect_request_stats(session);

        // 下游请求开始
        info!(
            event = "downstream_request_start",
            component = COMPONENT,
            request_id = %ctx.request_id,
            method = %method,
            path = %path,
            client_ip = %req_stats.client_ip,
            user_agent = ?req_stats.user_agent,
            "收到下游请求"
        );

        // 记录原始请求信息（统一JSON头部）
        let request_url = session.req_header().uri.to_string();
        let client_headers_json = crate::logging::headers_json_string_request(session.req_header());

        // 下游请求头（JSON）
        info!(
            event = "downstream_request_headers",
            component = COMPONENT,
            request_id = %ctx.request_id,
            method = %method,
            url = %request_url,
            client_headers_json = client_headers_json,
            "下游请求头"
        );

        // 透明代理设计：仅处理代理请求，其他全部拒绝
        if !self.is_proxy_request(path) {
            if self.is_management_request(path) {
                warn!(
                    event = "wrong_port",
                    component = COMPONENT,
                    request_id = %ctx.request_id,
                    path = %path,
                    "管理接口请求被发送到代理端口，应使用管理端口(默认: 9090)"
                );
                let e = crate::proxy_err!(
                    upstream_not_found,
                    "请使用管理端口访问管理接口(默认端口: 9090)"
                );
                return Err(crate::pingora_error!(e));
            } else {
                warn!(
                    event = "not_proxy_endpoint",
                    component = COMPONENT,
                    request_id = %ctx.request_id,
                    path = %path,
                    "非代理端点：该端口仅处理 AI 代理请求"
                );
                let e = crate::proxy_err!(
                    upstream_not_found,
                    "该端口仅处理 AI 代理请求(非管理/非静态)"
                );
                return Err(crate::pingora_error!(e));
            }
        }

        // 处理CORS预检请求
        if method == "OPTIONS" {
            return Err(crate::pingora_http!(200, "CORS preflight"));
        }

        // 在早期阶段顺序执行：认证 -> 追踪 -> 限流 -> 提供商/密钥/凭证/超时/追踪扩展
        self.run_early_services(session, ctx).await?;

        // 早期处理完成
        Ok(())
    }

    async fn request_filter(
        &self,
        _session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> pingora_core::Result<bool> {
        // 主要工作已在 early_request_filter 完成，这里直接继续
        crate::pingora_continue!()
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<Box<HttpPeer>> {
        // 如果是重试请求，添加短暂延迟避免立即重试
        if ctx.retry_count > 0 {
            let delay_ms = (ctx.retry_count * 100).min(1000); // 最多延迟1秒
            debug!(
                request_id = %ctx.request_id,
                retry_count = ctx.retry_count,
                delay_ms = delay_ms,
                "Adding retry delay before upstream selection"
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms as u64)).await;
        }
        for svc in &self.upstream_peer_services {
            if let Some(peer) = svc.select(&self.ai_handler, _session, ctx).await? {
                return Ok(peer);
            }
        }

        // 理论上默认服务已返回 Some；到这里表示未能选择上游
        Err(crate::pingora_error!(crate::proxy_err!(
            upstream_not_found,
            "no upstream peer selected"
        )))
    }

    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        for svc in &self.upstream_request_services {
            tracing::debug!(component = COMPONENT, request_id = %ctx.request_id, step = svc.name(), "run upstream_request step");
            svc.exec(&self.ai_handler, session, upstream_request, ctx)
                .await?;
        }
        // provider 特定的上游请求服务（按需注入，默认在通用服务之后执行以便覆盖）
        for svc in provider_upstream_request_services(ctx) {
            tracing::debug!(component = COMPONENT, request_id = %ctx.request_id, step = svc.name(), provider = ?ctx.provider_type.as_ref().map(|p| p.name.clone()), "run provider upstream_request step");
            svc.exec(&self.ai_handler, session, upstream_request, ctx)
                .await?;
        }
        Ok(())
    }

    async fn request_body_filter(
        &self,
        session: &mut Session,
        body_chunk: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        for svc in &self.request_body_services {
            svc.exec(&self.ai_handler, session, body_chunk, end_of_stream, ctx)
                .await?;
        }
        Ok(())
    }

    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        // 统一由 request_handler 打印 upstream_response_headers 日志，避免重复与格式不一致

        for svc in &self.response_header_services {
            svc.exec(&self.ai_handler, _session, upstream_response, ctx)
                .await?;
        }
        // provider 特定的响应头服务
        for svc in provider_response_header_services(ctx) {
            svc.exec(&self.ai_handler, _session, upstream_response, ctx)
                .await?;
        }
        Ok(())
    }

    fn response_body_filter(
        &self,
        _session: &mut Session,
        body: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<Option<std::time::Duration>>
    where
        Self::CTX: Send + Sync,
    {
        let mut next: Option<std::time::Duration> = None;
        for svc in &self.response_body_services {
            let ret = svc.exec(body, end_of_stream, ctx)?;
            if next.is_none() {
                next = ret;
            }
        }

        // provider 特定的响应体服务
        for svc in self.provider_response_body_services(ctx) {
            let ret = svc.exec(body, end_of_stream, ctx)?;
            if next.is_none() {
                next = ret;
            }
        }

        Ok(next)
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
        for svc in &self.connected_to_upstream_services {
            svc.exec(reused, peer, _digest, ctx).await?;
        }
        Ok(())
    }

    async fn fail_to_proxy(
        &self,
        _session: &mut Session,
        e: &Error,
        ctx: &mut Self::CTX,
    ) -> FailToProxy {
        // 顺序执行失败处理服务，使用第一个服务的结果
        // 目前默认实现只有一个
        let tracer = self.tracer.clone();
        let mut result = None;
        for svc in &self.proxy_failure_services {
            result = Some(svc.handle(tracer.clone(), &self.ai_handler, e, ctx));
            break;
        }
        result.unwrap_or(FailToProxy {
            error_code: 500,
            can_reuse_downstream: false,
        })
    }

    async fn logging(&self, session: &mut Session, e: Option<&Error>, ctx: &mut Self::CTX) {
        // 可插拔日志服务（扩展点）
        for svc in &self.logging_services {
            svc.exec(&self.ai_handler, self.tracer.clone(), session, e, ctx)
                .await;
        }

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

            // 获取更多详细的上下文信息
            let request_url = session.req_header().uri.to_string();
            let request_headers_json =
                crate::logging::headers_json_string_request(session.req_header());
            let request_method = session.req_header().method.as_str();

            // 统一合并失败日志（结构化 JSON 字段）
            let selected_backend_id = ctx.selected_backend.as_ref().map(|b| b.id);
            let selected_backend_key_preview = ctx.selected_backend.as_ref().map(|b| {
                if b.api_key.len() > 8 {
                    format!(
                        "{}***{}",
                        &b.api_key[..4],
                        &b.api_key[b.api_key.len() - 4..]
                    )
                } else {
                    "***".to_string()
                }
            });

            error!(
                event = "request_failed",
                component = COMPONENT,
                request_id = %ctx.request_id,
                method = %request_method,
                url = %request_url,
                error_type = ?error.etype,
                error_source = ?error.esource,
                error_context = ?error.context,
                error_message = %error,
                duration_ms = duration.as_millis(),
                is_timeout_error = is_timeout_error,
                is_network_error = is_network_error,
                request_headers_json = request_headers_json,
                selected_backend_id = selected_backend_id,
                selected_backend_key_preview = ?selected_backend_key_preview,
                provider_type = ?ctx.provider_type.as_ref().map(|p| &p.name),
                timeout_seconds = ?ctx.timeout_seconds,
                "请求失败"
            );

            // 如果是超时或网络错误，使用AI处理器进行错误转换
            if is_timeout_error || is_network_error {
                let converted_error = self.ai_handler.convert_pingora_error(error, ctx);
                warn!(
                    request_id = %ctx.request_id,
                    original_error = %error,
                    converted_error = %converted_error,
                    "Converted Pingora error to ProxyError for better user experience"
                );
            }
        } else {
            // 成功请求完成，记录追踪信息（使用分块阶段已聚合的 usage/model，执行计费与追踪写入）
            if let Some(tracer) = &self.tracer {
                let status_code = session
                    .response_written()
                    .map(|resp| resp.status.as_u16())
                    .unwrap_or(200);

                // 如果响应非2xx/3xx，记录响应体用于排查
                if status_code >= 400 {
                    if ctx.response_details.body.is_none() {
                        ctx.response_details.finalize_body();
                    }
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

                    // 记录基本错误信息（error级别）
                    error!(
                        request_id = %ctx.request_id,
                        status = status_code,
                        content_type = %content_type,
                        body_length = body_preview.len(),
                        "上游响应失败"
                    );
                }

                // 使用分块阶段聚合的 usage/model 执行计费与追踪完成
                // 1) 读取最终 usage
                let final_usage = ctx
                    .usage_final
                    .clone()
                    .unwrap_or(crate::statistics::types::TokenUsageMetrics::default());
                // final_usage 已写入 usage_final

                // 2) 模型：优先使用请求阶段/流式聚合结果
                let final_model = ctx.requested_model.clone();
                if let Some(model) = final_model.clone() {
                    let _ = self
                        .ai_handler
                        .tracing_service()
                        .update_trace_model_info(&ctx.request_id, None, Some(model), None)
                        .await;
                }

                // 3) 费用计算（有模型与 provider），然后完成追踪
                let mut cost_val: Option<f64> = None;
                let mut currency_val: Option<String> = None;
                if let (Some(model), Some(provider)) =
                    (final_model.clone(), ctx.provider_type.as_ref())
                {
                    let pricing_usage = crate::pricing::TokenUsage {
                        prompt_tokens: final_usage.prompt_tokens,
                        completion_tokens: final_usage.completion_tokens,
                        cache_create_tokens: final_usage.cache_create_tokens,
                        cache_read_tokens: final_usage.cache_read_tokens,
                    };
                    match self
                        .ai_handler
                        .statistics_service()
                        .calculate_cost_direct(&model, provider.id, &pricing_usage, &ctx.request_id)
                        .await
                    {
                        Ok((c, cur)) => {
                            cost_val = c;
                            currency_val = cur;
                        }
                        Err(e) => {
                            warn!(component = "statistics.price", request_id = %ctx.request_id, error = %e, "Failed to calculate cost in logging phase");
                        }
                    }
                }

                if let Err(e) = tracer
                    .complete_trace_with_stats(
                        &ctx.request_id,
                        status_code,
                        true,
                        final_usage.prompt_tokens,
                        final_usage.completion_tokens,
                        None,
                        None,
                        final_usage.cache_create_tokens,
                        final_usage.cache_read_tokens,
                        cost_val,
                        currency_val.clone(),
                    )
                    .await
                {
                    error!(
                        request_id = %ctx.request_id,
                        error = %e,
                        "Failed to store complete trace with stats"
                    );
                }

                info!(
                    event = "stats_ok",
                    component = COMPONENT,
                    request_id = %ctx.request_id,
                    tokens_prompt = ?final_usage.prompt_tokens,
                    tokens_completion = ?final_usage.completion_tokens,
                    tokens_total = ?final_usage.total_tokens,
                    model_used = ?final_model,
                    cost = ?cost_val,
                    cost_currency = ?currency_val,
                    "统计与计费完成"
                );
            }

            debug!(
                request_id = %ctx.request_id,
                duration_ms = duration.as_millis(),
                // tokens_used 移除，统计以 usage_final 为准
                "AI proxy request completed successfully"
            );
        }
    }
}
