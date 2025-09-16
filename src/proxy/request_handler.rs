//! # AI代理请求处理器
//!
//! 基于设计文档实现的AI代理处理器，负责身份验证、速率限制和转发策略

use anyhow::Result;
use pingora_core::upstreams::peer::{HttpPeer, Peer, ALPN};
use url::form_urlencoded;
use pingora_core::{Error as PingoraError, ErrorType};
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::Session;
use sea_orm::prelude::Decimal;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use std::sync::Arc;
use std::time::Duration;

use crate::auth::{AuthUtils, RefactoredUnifiedAuthManager, types::AuthType};
use crate::cache::UnifiedCacheManager;
use crate::config::{AppConfig, ProviderConfigManager};
use crate::error::ProxyError;
use crate::pricing::PricingCalculatorService;
use crate::proxy::{AuthenticationService, StatisticsService, TracingService};
use crate::scheduler::{ApiKeyPoolManager, SelectionContext};
use crate::trace::immediate::ImmediateProxyTracer;
use entity::{
    oauth_client_sessions::{self, Entity as OAuthClientSessions},
    provider_types::{self, Entity as ProviderTypes},
    user_provider_keys::{self},
    user_service_apis::{self},
};

/// 请求处理器 - 负责AI代理请求的完整处理流程
///
/// 职责重构后专注于：
/// - 请求解析和验证
/// - 上游服务选择和负载均衡
/// - 请求转发和响应处理
/// - 追踪和统计记录
///
/// 认证职责已迁移到RefactoredUnifiedAuthManager
pub struct RequestHandler {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 统一缓存管理器
    cache: Arc<UnifiedCacheManager>,
    /// 配置 (未来使用)
    _config: Arc<AppConfig>,
    /// 服务商配置管理器
    provider_config_manager: Arc<ProviderConfigManager>,
    /// API密钥池管理器
    api_key_pool: Arc<ApiKeyPoolManager>,
    /// 认证服务 - 负责API密钥验证和完整provider配置获取
    auth_service: Arc<AuthenticationService>,
    /// 统计服务 - 负责请求/响应数据收集和分析
    statistics_service: Arc<StatisticsService>,
    /// 追踪服务 - 负责请求追踪的完整生命周期管理
    tracing_service: Arc<TracingService>,
}

/// Token使用详情
#[derive(Clone, Debug, Default)]
pub struct TokenUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: u32,
    pub model_used: Option<String>,
}

/// 请求详情
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct RequestDetails {
    pub headers: std::collections::HashMap<String, String>,
    pub body_size: Option<u64>,
    pub content_type: Option<String>,
    /// 真实客户端IP地址（考虑代理情况）
    pub client_ip: String,
    /// 用户代理字符串
    pub user_agent: Option<String>,
    /// 来源页面
    pub referer: Option<String>,
    /// 请求方法
    pub method: String,
    /// 请求路径
    pub path: String,
    /// 请求协议版本
    pub protocol_version: Option<String>,
}

/// 响应详情
#[derive(Clone, Debug, Default)]
pub struct ResponseDetails {
    pub headers: std::collections::HashMap<String, String>,
    pub body: Option<String>,
    pub body_size: Option<u64>,
    pub content_type: Option<String>,
    pub content_encoding: Option<String>,
    /// 响应体数据块累积(用于收集响应体数据)
    pub body_chunks: Vec<u8>,
}

/// 响应详情的序列化版本(不包含body_chunks)
#[derive(serde::Serialize)]
pub struct SerializableResponseDetails {
    pub headers: std::collections::HashMap<String, String>,
    pub body: Option<String>,
    pub body_size: Option<u64>,
    pub content_type: Option<String>,
    pub content_encoding: Option<String>,
}

impl From<&ResponseDetails> for SerializableResponseDetails {
    fn from(details: &ResponseDetails) -> Self {
        Self {
            headers: details.headers.clone(),
            body: details.body.clone(),
            body_size: details.body_size,
            content_type: details.content_type.clone(),
            content_encoding: details.content_encoding.clone(),
        }
    }
}

impl ResponseDetails {
    /// 添加响应体数据块
    pub fn add_body_chunk(&mut self, chunk: &[u8]) {
        let prev_size = self.body_chunks.len();
        self.body_chunks.extend_from_slice(chunk);

        // 只在累积大小达到特定阈值时记录日志（避免过多日志）
        let new_size = self.body_chunks.len();
        if new_size % 8192 == 0 || (prev_size < 1024 && new_size >= 1024) {
            tracing::debug!(
                chunk_size = chunk.len(),
                total_size = new_size,
                "Response body chunk added (milestone reached)"
            );
        }
    }

    /// 检测响应是否为SSE格式
    pub fn is_sse_format(&self) -> bool {
        // 检查Content-Type
        if let Some(content_type) = &self.content_type {
            if content_type.contains("text/event-stream") {
                return true;
            }
        }

        // 检查响应体内容格式（如果已经finalized）
        if let Some(body) = &self.body {
            let first_few_lines: Vec<&str> = body.lines().take(5).collect();
            let data_line_count = first_few_lines
                .iter()
                .filter(|line| line.trim().starts_with("data: "))
                .count();

            // 如果有多个"data: "开头的行，很可能是SSE格式
            return data_line_count > 1;
        }

        false
    }

    /// 获取SSE响应中的有效数据行数量
    pub fn get_sse_data_line_count(&self) -> usize {
        if let Some(body) = &self.body {
            return body
                .lines()
                .filter(|line| line.trim().starts_with("data: ") && !line.contains("[DONE]"))
                .count();
        }
        0
    }

    /// 完成响应体收集，将累积的数据转换为字符串
    pub fn finalize_body(&mut self) {
        let original_chunks_len = self.body_chunks.len();

        if !self.body_chunks.is_empty() {
            tracing::debug!(
                raw_body_size = original_chunks_len,
                "Starting response body finalization"
            );

            // 尝试将响应体转换为UTF-8字符串
            match String::from_utf8(self.body_chunks.clone()) {
                Ok(body_str) => {
                    let original_str_len = body_str.len();

                    // 对于大的响应体，只保留前64KB
                    if body_str.len() > 65536 {
                        self.body = Some(format!(
                            "{}...[truncated {} bytes]",
                            &body_str[..65536],
                            body_str.len() - 65536
                        ));
                        tracing::info!(
                            original_size = original_str_len,
                            stored_size = 65536,
                            truncated_bytes = original_str_len - 65536,
                            "Response body finalized as UTF-8 string (truncated)"
                        );
                    } else {
                        self.body = Some(body_str.clone());

                        // 检测是否为SSE格式并记录相关信息
                        let is_sse = body_str
                            .lines()
                            .any(|line| line.trim().starts_with("data: "));
                        if is_sse {
                            let data_line_count = body_str
                                .lines()
                                .filter(|line| {
                                    line.trim().starts_with("data: ") && !line.contains("[DONE]")
                                })
                                .count();

                            tracing::info!(
                                body_size = original_str_len,
                                is_sse_format = true,
                                sse_data_lines = data_line_count,
                                "Response body finalized as UTF-8 string (complete, SSE format detected)"
                            );
                        } else {
                            tracing::info!(
                                body_size = original_str_len,
                                is_sse_format = false,
                                "Response body finalized as UTF-8 string (complete)"
                            );
                        }
                    }
                }
                Err(utf8_error) => {
                    // 如果不是有效的UTF-8，保存为十六进制字符串（仅前1KB）
                    let truncated_chunks = if self.body_chunks.len() > 1024 {
                        &self.body_chunks[..1024]
                    } else {
                        &self.body_chunks
                    };
                    self.body = Some(format!("binary-data:{}", hex::encode(truncated_chunks)));

                    tracing::info!(
                        raw_size = original_chunks_len,
                        encoded_size = truncated_chunks.len(),
                        utf8_error = %utf8_error,
                        "Response body finalized as hex-encoded binary data"
                    );
                }
            }
            // 更新实际的body_size
            self.body_size = Some(self.body_chunks.len() as u64);
        } else {
            tracing::debug!("No response body chunks to finalize (empty response)");
        }
    }
}

/// 详细的请求统计信息
#[derive(Debug, Clone, Default)]
pub struct DetailedRequestStats {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub model_name: Option<String>,
    pub cache_create_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    pub cost: Option<f64>,
    pub cost_currency: Option<String>,
}

/// Gemini代理模式枚举 - 支持3种认证和路由模式
#[derive(Debug, Clone, PartialEq)]
pub enum GeminiProxyMode {
    /// 模式1: OAuth认证，无project_id - 路由到 generativelanguage.googleapis.com
    OAuthWithoutProject,
    /// 模式2: OAuth认证，有project_id - 路由到 cloudcode-pa.googleapis.com
    OAuthWithProject(String),
    /// 模式3: API Key认证 - 路由到 generativelanguage.googleapis.com
    ApiKey,
}

impl GeminiProxyMode {
    /// 获取对应的上游地址
    pub fn upstream_host(&self) -> &'static str {
        match self {
            Self::OAuthWithoutProject => "cloudcode-pa.googleapis.com",
            Self::OAuthWithProject(_) => "cloudcode-pa.googleapis.com",
            Self::ApiKey => "generativelanguage.googleapis.com",
        }
    }

    /// 判断是否需要请求体注入project_id（而不是路径注入）
    pub fn needs_body_injection(&self) -> bool {
        matches!(self, Self::OAuthWithProject(_))
    }

    /// 获取project_id（如果有）
    pub fn project_id(&self) -> Option<&str> {
        match self {
            Self::OAuthWithProject(project_id) => Some(project_id),
            _ => None,
        }
    }
}

/// 请求上下文
#[derive(Debug, Clone)]
pub struct ProxyContext {
    /// 请求ID
    pub request_id: String,
    /// 用户对外API配置
    pub user_service_api: Option<user_service_apis::Model>,
    /// 选择的后端API密钥
    pub selected_backend: Option<user_provider_keys::Model>,
    /// 提供商类型配置
    pub provider_type: Option<provider_types::Model>,
    /// 开始时间
    pub start_time: std::time::Instant,
    /// 重试次数
    pub retry_count: u32,
    /// 使用的tokens（向后兼容）
    pub tokens_used: u32,
    /// 详细的Token使用信息
    pub token_usage: TokenUsage,
    /// 请求详情
    pub request_details: RequestDetails,
    /// 响应详情
    pub response_details: ResponseDetails,
    /// 选择的提供商名称
    pub selected_provider: Option<String>,
    /// 连接超时时间(秒)
    pub timeout_seconds: Option<i32>,
    /// 请求体缓冲区 (用于request_body_filter中的数据收集)
    pub body: Vec<u8>,
    /// 是否计划修改请求体（供上游头部处理决策使用）
    pub will_modify_body: bool,
}

impl Default for ProxyContext {
    fn default() -> Self {
        Self {
            request_id: String::new(),
            user_service_api: None,
            selected_backend: None,
            provider_type: None,
            start_time: std::time::Instant::now(),
            retry_count: 0,
            tokens_used: 0,
            token_usage: TokenUsage::default(),
            request_details: RequestDetails::default(),
            response_details: ResponseDetails::default(),
            selected_provider: None,
            timeout_seconds: None,
            body: Vec::new(),
            will_modify_body: false,
        }
    }
}

/// 认证结果
#[derive(Debug, Clone)]
pub struct AuthResult {
    /// 用户对外API配置
    pub user_service_api: user_service_apis::Model,
    /// 选择的后端API密钥
    pub selected_backend: user_provider_keys::Model,
    /// 提供商类型配置
    pub provider_type: provider_types::Model,
}

impl RequestHandler {
    /// 判断本次请求是否为 SSE（流式）请求：
    /// - 下游或上游 Accept 包含 text/event-stream 或 application/stream+json
    /// - URL 查询参数 alt=sse
    /// - URL 查询参数 stream=true（通用流标识）
    fn is_sse_request(&self, session: &Session, upstream_request: &RequestHeader) -> bool {
        // 1) 检查 Accept 头（优先下游，然后上游）
        let accept_downstream = session
            .req_header()
            .headers
            .get("accept")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        let accept_upstream = upstream_request
            .headers
            .get("accept")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        let accept_sse = |v: &str| v.contains("text/event-stream") || v.contains("application/stream+json");
        if accept_sse(&accept_downstream) || accept_sse(&accept_upstream) {
            return true;
        }

        // 2) 检查查询参数（alt=sse 或 stream=true）
        if let Some(query) = upstream_request.uri.query() {
            let mut is_sse = false;
            for (k, v) in form_urlencoded::parse(query.as_bytes()) {
                let key = k.to_string().to_ascii_lowercase();
                let val = v.to_string().to_ascii_lowercase();
                if (key == "alt" && val == "sse") || (key == "stream" && (val == "1" || val == "true")) {
                    is_sse = true;
                    break;
                }
            }
            if is_sse {
                return true;
            }
        }

        false
    }
    /// 获取统计服务的引用 - 用于外部访问
    pub fn statistics_service(&self) -> &Arc<StatisticsService> {
        &self.statistics_service
    }

    /// 创建新的AI代理处理器 - 协调器模式
    ///
    /// 现在RequestHandler作为协调器，将认证、统计和追踪职责委托给专门的服务
    pub fn new(
        db: Arc<DatabaseConnection>,
        cache: Arc<UnifiedCacheManager>,
        _config: Arc<AppConfig>,
        tracer: Option<Arc<ImmediateProxyTracer>>,
        provider_config_manager: Arc<ProviderConfigManager>,
        auth_manager: Arc<RefactoredUnifiedAuthManager>,
    ) -> Self {
        let health_checker = Arc::new(crate::scheduler::api_key_health::ApiKeyHealthChecker::new(
            db.clone(),
            None,
        ));
        let api_key_pool = Arc::new(ApiKeyPoolManager::new(db.clone(), health_checker));

        // 创建三个专门的服务（移除ProviderResolver，功能已集成到AuthenticationService）
        let auth_service = Arc::new(AuthenticationService::new(auth_manager.clone(), db.clone()));

        let pricing_calculator = Arc::new(PricingCalculatorService::new(db.clone()));
        let statistics_service = Arc::new(StatisticsService::new(pricing_calculator.clone()));

        let tracing_service = Arc::new(TracingService::new(tracer.clone()));

        Self {
            db,
            cache,
            _config,
            provider_config_manager,
            api_key_pool,
            auth_service,
            statistics_service,
            tracing_service,
        }
    }

    /// 准备代理请求 - 协调器模式：委托给专门服务
    pub async fn prepare_proxy_request(
        &self,
        session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> Result<(), ProxyError> {
        let start = std::time::Instant::now();

        tracing::info!(
            request_id = %ctx.request_id,
            method = %session.req_header().method,
            path = %session.req_header().uri.path(),
            flow = "before_auth",
            "准备代理请求（认证前）"
        );

        // 步骤1: 身份验证和完整配置获取 - 替代原来的步骤0+步骤1
        // AuthenticationService现在会一次性获取所有必要信息，包括完整的provider配置
        let auth_start = std::time::Instant::now();
        let auth_result = self
            .auth_service
            .authenticate_and_replace_credentials(session, &ctx.request_id)
            .await?;
        let _auth_duration = auth_start.elapsed();

        // 应用认证结果到上下文
        self.auth_service
            .apply_auth_result_to_context(ctx, &auth_result);
        let user_service_api = ctx.user_service_api.as_ref().unwrap();

        tracing::info!(
            request_id = %ctx.request_id,
            user_id = user_service_api.user_id,
            provider_name = %auth_result.provider_type.name,
            provider_base_url = %auth_result.provider_type.base_url,
            timeout_seconds = ctx.timeout_seconds.unwrap_or(30),
            flow = "after_auth",
            "认证与服务商配置完成"
        );

        // 步骤2: 开始请求追踪 - 委托给TracingService
        let method = session.req_header().method.as_str();
        let path = Some(session.req_header().uri.path().to_string());
        let request_stats = self.statistics_service.collect_request_stats(session);
        let client_ip = request_stats.client_ip.clone();
        let user_agent = request_stats.user_agent.clone();

        self.tracing_service
            .start_trace(
                &ctx.request_id,
                user_service_api.id,
                Some(user_service_api.user_id),
                method,
                path,
                Some(client_ip),
                user_agent,
            )
            .await?;

        // 步骤3: 速率验证 - 仍由RequestHandler处理（业务逻辑）
        let rate_limit_start = std::time::Instant::now();
        let rate_limit_result = self.check_rate_limit(user_service_api).await;
        let _rate_limit_duration = rate_limit_start.elapsed();

        if let Err(e) = rate_limit_result {
            // 速率限制失败时立即记录到数据库
            self.tracing_service
                .complete_trace_rate_limit(&ctx.request_id, &e.to_string())
                .await?;
            return Err(e);
        }

        tracing::debug!(
            request_id = %ctx.request_id,
            rate_limit = user_service_api.max_request_per_min,
            "Rate limit check passed"
        );

        // 步骤4: 获取提供商类型信息和配置
        let provider_type = match self
            .get_provider_type(user_service_api.provider_type_id)
            .await
        {
            Ok(provider_type) => provider_type,
            Err(e) => {
                // 提供商类型获取失败时立即记录到数据库
                self.tracing_service
                    .complete_trace_config_error(&ctx.request_id, &e.to_string())
                    .await?;
                return Err(e);
            }
        };
        ctx.provider_type = Some(provider_type.clone());
        ctx.selected_provider = Some(provider_type.name.clone());

        // 设置超时配置，优先级：用户配置 > 动态配置 > 默认配置
        ctx.timeout_seconds = if let Some(user_timeout) = user_service_api.timeout_seconds {
            // 优先使用用户配置的超时时间
            Some(user_timeout)
        } else if let Ok(Some(provider_config)) = self
            .provider_config_manager
            .get_provider_by_name(&provider_type.name)
            .await
        {
            // 其次使用动态配置的超时时间
            provider_config.timeout_seconds
        } else {
            // 最后使用provider_types表中的默认超时时间
            provider_type.timeout_seconds
        };

        let timeout_source = if user_service_api.timeout_seconds.is_some() {
            "user_service_api configuration (highest priority)"
        } else if let Ok(Some(_)) = self
            .provider_config_manager
            .get_provider_by_name(&provider_type.name)
            .await
        {
            "dynamic provider configuration"
        } else {
            "provider_types default configuration"
        };

        tracing::debug!(
            request_id = %ctx.request_id,
            provider = %provider_type.name,
            timeout_seconds = ?ctx.timeout_seconds,
            source = timeout_source,
            user_config = ?user_service_api.timeout_seconds,
            "Applied timeout configuration with correct priority"
        );

        // 步骤5: 根据用户配置选择合适的API密钥
        let _api_key_selection_start = std::time::Instant::now();
        let selected_backend = match self.select_api_key(user_service_api, &ctx.request_id).await {
            Ok(backend) => backend,
            Err(e) => {
                // API密钥选择失败时立即记录到数据库
                self.tracing_service
                    .complete_trace_api_key_selection_failed(&ctx.request_id, &e.to_string())
                    .await?;
                return Err(e);
            }
        };
        ctx.selected_backend = Some(selected_backend.clone());

        // 更新追踪信息 - 使用TracingService记录更多信息
        self.tracing_service
            .update_extended_trace_info(
                &ctx.request_id,
                Some(provider_type.id),    // provider_type_id
                None,                      // model_used将在响应处理时设置
                Some(selected_backend.id), // user_provider_key_id
            )
            .await?;

        let elapsed = start.elapsed();
        tracing::info!(
            request_id = %ctx.request_id,
            user_id = user_service_api.user_id,
            provider = %provider_type.name,
            backend_key_id = selected_backend.id,
            strategy = %user_service_api.scheduling_strategy.as_deref().unwrap_or("round_robin"),
            elapsed_ms = elapsed.as_millis(),
            "AI proxy request preparation completed"
        );

        Ok(())
    }

    /// 动态识别Gemini代理模式
    ///
    /// 根据用户密钥配置动态判断应该使用的代理模式：
    /// - OAuth + 无project_id => 路由到 cloudcode-pa.googleapis.com
    /// - OAuth + 有project_id => 路由到 cloudcode-pa.googleapis.com  
    /// - API Key => 路由到 generativelanguage.googleapis.com
    async fn identify_gemini_proxy_mode(
        &self,
        ctx: &ProxyContext,
    ) -> Result<GeminiProxyMode, ProxyError> {
        let selected_backend = ctx
            .selected_backend
            .as_ref()
            .ok_or_else(|| ProxyError::internal("Backend not selected in context"))?;

        let auth_type = &selected_backend.auth_type;
        let project_id = &selected_backend.project_id;

        let mode = match auth_type.as_str() {
            "oauth" => {
                // OAuth认证始终路由到 cloudcode-pa.googleapis.com
                if let Some(project_id) = project_id {
                    if !project_id.is_empty() {
                        // OAuth + 有project_id => 路由到 cloudcode-pa.googleapis.com，并在请求体中注入project字段
                        GeminiProxyMode::OAuthWithProject(project_id.clone())
                    } else {
                        // OAuth + 无project_id => 路由到 cloudcode-pa.googleapis.com，不注入project字段
                        GeminiProxyMode::OAuthWithoutProject
                    }
                } else {
                    // OAuth + project_id为None => 路由到 cloudcode-pa.googleapis.com，不注入project字段
                    GeminiProxyMode::OAuthWithoutProject
                }
            }
            "api_key" => {
                // API Key认证路由到 generativelanguage.googleapis.com
                GeminiProxyMode::ApiKey
            }
            _ => {
                // 其他认证类型（service_account, adc）默认使用API Key模式路由到 generativelanguage.googleapis.com
                tracing::warn!(
                    request_id = %ctx.request_id,
                    auth_type = auth_type,
                    "Unsupported auth_type for Gemini, defaulting to ApiKey mode (generativelanguage.googleapis.com)"
                );
                GeminiProxyMode::ApiKey
            }
        };

        let upstream_host = mode.upstream_host();
        tracing::info!(
            request_id = %ctx.request_id,
            auth_type = auth_type,
            project_id = ?project_id,
            identified_mode = ?mode,
            upstream_host = upstream_host,
            "Gemini proxy mode identified"
        );

        Ok(mode)
    }

    /// 将project_id注入到API路径中
    ///
    /// 将形如 `/v1/models` 的路径转换为 `/v1/projects/{project_id}/models`
    /// 用于支持Google Cloud Code Assist API的路径格式
    ///
    /// 特殊处理：
    /// - `v1internal:` 路径不需要project_id注入，直接返回原路径
    /// - 标准 `/v1/` 路径会进行project_id注入
    #[allow(dead_code)]
    fn inject_project_id_into_path(&self, original_path: &str, project_id: &str) -> String {
        // 检查是否是 v1internal: 路径（如 /v1internal:loadCodeAssist）
        if original_path.contains("v1internal:") {
            tracing::debug!(
                path = original_path,
                project_id = project_id,
                "Detected v1internal: path, skipping project_id injection"
            );
            return original_path.to_string();
        }

        // 检查路径是否以 /v1/ 开头
        if original_path.starts_with("/v1/") {
            // 提取 /v1/ 后面的部分
            let remainder = &original_path[4..]; // 跳过 "/v1/"
            format!("/v1/projects/{}/{}", project_id, remainder)
        } else {
            // 如果不是标准的 /v1/ 路径，直接返回原路径
            tracing::warn!(
                path = original_path,
                project_id = project_id,
                "Path does not start with /v1/, skipping project_id injection"
            );
            original_path.to_string()
        }
    }

    /// Gemini Query 参数修改器
    ///
    /// 根据不同的代理模式为请求添加必要的 query 参数
    async fn modify_gemini_query_parameters(
        &self,
        ctx: &ProxyContext,
        _upstream_request: &mut RequestHeader,
        gemini_mode: &GeminiProxyMode,
    ) -> Result<(), ProxyError> {
        tracing::info!(
            request_id = %ctx.request_id,
            gemini_mode = ?gemini_mode,
            headers = ?_upstream_request.headers,
            query = ?_upstream_request.uri.query(),
            "Modifying Gemini query parameters"
        );

        Ok(())
    }

    /// Gemini Headers 修改器
    ///
    /// 根据不同的代理模式添加 Google 特定的头部
    async fn modify_gemini_headers(
        &self,
        ctx: &ProxyContext,
        _upstream_request: &mut RequestHeader,
        gemini_mode: &GeminiProxyMode,
    ) -> Result<(), ProxyError> {
        tracing::info!(
            request_id = %ctx.request_id,
            gemini_mode = ?gemini_mode,
            headers = ?_upstream_request.headers,
            query = ?_upstream_request.uri.query(),
            "Modifying Gemini headers"
        );

        Ok(())
    }

    /// Gemini 请求体修改器
    ///
    /// 根据路由匹配进行不同的请求体字段注入
    /// 实现实际的请求体JSON修改，根据不同路由注入相应的project_id字段
    async fn modify_gemini_request_body(
        &self,
        ctx: &mut ProxyContext,
        session: &Session,
        _upstream_request: &mut RequestHeader,
        gemini_mode: &GeminiProxyMode,
    ) -> Result<(), ProxyError> {
        // 获取当前请求路径和请求体数据用于分析
        let request_path = session.req_header().uri.path();
        let method = session.req_header().method.as_str();

        // 打印请求体数据用于调试（注意：在实际生产环境中应该小心处理敏感数据）
        tracing::info!(
            request_id = %ctx.request_id,
            method = method,
            path = request_path,
            gemini_mode = ?gemini_mode,
            uri = %session.req_header().uri,
            "=== GEMINI REQUEST BODY ANALYZER START ==="
        );

        // 只有当使用OAuth且有project_id时才进行请求体修改
        if let GeminiProxyMode::OAuthWithProject(project_id) = gemini_mode {
            // TODO: 实际的请求体读取和修改逻辑
            // 由于Pingora的架构限制，请求体的实际修改需要在request body处理阶段完成
            // 这里我们记录需要进行的修改类型，供后续处理阶段使用

            // 路由匹配和对应的请求体字段注入规划
            let (route_type, fields_to_inject) = if request_path.contains("loadCodeAssist") {
                // 路由1: /v1internal:loadCodeAssist 或 /v1beta/models/{model}:loadCodeAssist
                // 需要注入: metadata.duetProject = project_id, body.cloudaicompanionProject = project_id
                (
                    "loadCodeAssist",
                    vec!["metadata.duetProject", "body.cloudaicompanionProject"],
                )
            } else if request_path.contains("onboardUser") {
                // 路由2: /v1internal:onboardUser 或 /v1beta/models/{model}:onboardUser
                // 需要注入: body.cloudaicompanionProject = project_id
                ("onboardUser", vec!["body.cloudaicompanionProject"])
            } else if request_path.contains("generateContent")
                && !request_path.contains("streamGenerateContent")
            {
                // 路由3: /v1internal:generateContent 或 /v1beta/models/{model}:generateContent
                // 需要注入: body.project = project_id
                ("generateContent", vec!["body.project"])
            } else if request_path.contains("streamGenerateContent") {
                // 路由4: 流式端点 - 为提高兼容性，不进行 project 字段注入，依赖账户默认项目
                ("streamGenerateContent", vec![])
            } else {
                // 其他路由不需要特殊处理
                ("other", vec![])
            };

            if !fields_to_inject.is_empty() || route_type == "countTokens" {
                // 标记：本次请求将修改请求体（通用标记，不按路由分支）
                ctx.will_modify_body = true;
                tracing::info!(
                    request_id = %ctx.request_id,
                    project_id = project_id,
                    route_type = route_type,
                    fields_to_inject = ?fields_to_inject,
                    "📋 Gemini request body modification plan"
                );

                // TODO: 在这里实现实际的JSON修改逻辑
                // 步骤：
                // 1. 读取完整的请求体数据
                // 2. 解析JSON
                // 3. 根据route_type和fields_to_inject规则修改JSON
                // 4. 重新序列化JSON并设置到upstream request

                tracing::info!(
                    request_id = %ctx.request_id,
                    project_id = project_id,
                    route_type = route_type,
                    modification_count = fields_to_inject.len(),
                    "🔧 Project ID injection planned for Code Assist API request"
                );

                // 存储修改信息到上下文中，供请求体处理阶段使用
                // 注意：这需要在ProxyContext中添加相应的字段来存储这些信息
                tracing::debug!(
                    request_id = %ctx.request_id,
                    project_id = project_id,
                    route_type = route_type,
                    fields = ?fields_to_inject,
                    "Stored body modification plan in context for later processing"
                );
            } else {
                ctx.will_modify_body = false;
                tracing::debug!(
                    request_id = %ctx.request_id,
                    project_id = project_id,
                    route_type = route_type,
                    request_path = request_path,
                    "No specific field injection needed for this Code Assist API route"
                );
            }
        } else {
            // 非OAuth或无project_id的情况
            ctx.will_modify_body = false;
            tracing::debug!(
                request_id = %ctx.request_id,
                gemini_mode = ?gemini_mode,
                "No body modification needed - not OAuth with project_id mode"
            );
        }

        tracing::info!(
            request_id = %ctx.request_id,
            path = request_path,
            "=== GEMINI REQUEST BODY ANALYZER END ==="
        );

        Ok(())
    }

    /// Google Code Assist API JSON请求体修改器 (公开方法供service.rs调用)
    ///
    /// 实际修改JSON对象，根据不同路由注入相应的project_id字段
    pub async fn modify_gemini_request_body_json(
        &self,
        json_value: &mut serde_json::Value,
        session: &Session,
        ctx: &ProxyContext,
    ) -> Result<bool, crate::error::ProxyError> {
        // 获取当前请求路径
        let request_path = session.req_header().uri.path();

        // 识别Gemini代理模式 (复用现有逻辑)
        let gemini_mode = self.identify_gemini_proxy_mode(ctx).await?;

        // 只有当使用OAuth且有project_id时才进行请求体修改
        if let crate::proxy::request_handler::GeminiProxyMode::OAuthWithProject(project_id) =
            gemini_mode
        {
            // 根据路由类型进行不同的字段注入
            let modified = if request_path.contains("loadCodeAssist") {
                // 路由1: /v1internal:loadCodeAssist 或 /v1beta/models/{model}:loadCodeAssist
                // 需要注入: metadata.duetProject = project_id, body.cloudaicompanionProject = project_id
                self.inject_loadcodeassist_fields(json_value, &project_id, &ctx.request_id)
            } else if request_path.contains("onboardUser") {
                // 路由2: /v1internal:onboardUser 或 /v1beta/models/{model}:onboardUser
                // 需要注入: body.cloudaicompanionProject = project_id
                self.inject_onboarduser_fields(json_value, &project_id, &ctx.request_id)
            } else if request_path.contains("countTokens") {
                // 路由5: /v1internal:countTokens 或 /v1beta/models/{model}:countTokens
                // 需要标准化请求体结构: { request: { model: "models/{model}", contents: [...] } }
                self.inject_counttokens_fields(json_value, &ctx.request_id)
            } else if request_path.contains("generateContent")
                && !request_path.contains("streamGenerateContent")
            {
                // 路由3: /v1internal:generateContent 或 /v1beta/models/{model}:generateContent
                // 需要注入: body.project = project_id
                self.inject_generatecontent_fields(json_value, &project_id, &ctx.request_id)
            } else if request_path.contains("streamGenerateContent") {
                // 路由4: 流式端点 - 为提高兼容性，不注入 project 字段
                tracing::debug!(
                    request_id = %ctx.request_id,
                    route_path = request_path,
                    "Skip project injection for streamGenerateContent"
                );
                false
            } else {
                // 其他路由不需要特殊处理
                tracing::debug!(
                    request_id = %ctx.request_id,
                    route_path = request_path,
                    "No field injection needed for this Code Assist API route"
                );
                false
            };

            if modified {
                tracing::info!(
                    request_id = %ctx.request_id,
                    project_id = project_id,
                    route_path = request_path,
                    "Successfully injected project_id fields into Google Code Assist request"
                );
            }

            Ok(modified)
        } else {
            // 非OAuth或无project_id的情况
            tracing::debug!(
                request_id = %ctx.request_id,
                gemini_mode = ?gemini_mode,
                "No JSON body modification needed - not OAuth with project_id mode"
            );
            Ok(false)
        }
    }

    /// 为 loadCodeAssist API 注入字段
    fn inject_loadcodeassist_fields(
        &self,
        json_value: &mut serde_json::Value,
        project_id: &str,
        request_id: &str,
    ) -> bool {
        let mut modified = false;

        // 1. 注入 metadata.duetProject = project_id
        if let Some(obj) = json_value.as_object_mut() {
            let metadata = obj
                .entry("metadata")
                .or_insert_with(|| serde_json::json!({}));
            if let Some(metadata_obj) = metadata.as_object_mut() {
                metadata_obj.insert(
                    "duetProject".to_string(),
                    serde_json::Value::String(project_id.to_owned()),
                );
                modified = true;
                tracing::debug!(
                    request_id = %request_id,
                    project_id = project_id,
                    "Injected metadata.duetProject field"
                );
            }

            // 2. 注入 cloudaicompanionProject = project_id（顶层字段，非 body 下）
            obj.insert(
                "cloudaicompanionProject".to_string(),
                serde_json::Value::String(project_id.to_owned()),
            );
            modified = true;
            tracing::debug!(
                request_id = %request_id,
                project_id = project_id,
                "Injected top-level cloudaicompanionProject field"
            );
        }

        modified
    }

    /// 为 onboardUser API 注入字段
    fn inject_onboarduser_fields(
        &self,
        json_value: &mut serde_json::Value,
        project_id: &str,
        request_id: &str,
    ) -> bool {
        let mut modified = false;

        // 注入 cloudaicompanionProject = project_id（顶层字段）
        if let Some(obj) = json_value.as_object_mut() {
            obj.insert(
                "cloudaicompanionProject".to_string(),
                serde_json::Value::String(project_id.to_owned()),
            );
            modified = true;
            tracing::debug!(
                request_id = %request_id,
                project_id = project_id,
                "Injected top-level cloudaicompanionProject field for onboardUser"
            );
        }

        modified
    }

    /// 为 generateContent 和 streamGenerateContent API 注入字段
    fn inject_generatecontent_fields(
        &self,
        json_value: &mut serde_json::Value,
        project_id: &str,
        request_id: &str,
    ) -> bool {
        let mut modified = false;

        // 注入 project = project_id（顶层字段）
        if let Some(obj) = json_value.as_object_mut() {
            obj.insert(
                "project".to_string(),
                serde_json::Value::String(project_id.to_owned()),
            );
            modified = true;
            tracing::debug!(
                request_id = %request_id,
                project_id = project_id,
                "Injected top-level project field for generateContent API"
            );
        }

        modified
    }

    /// 为 countTokens API 标准化请求体结构
    /// 目标结构: { "request": { "model": "models/{model}", "contents": [...] } }
    fn inject_counttokens_fields(
        &self,
        json_value: &mut serde_json::Value,
        request_id: &str,
    ) -> bool {
        let mut modified = false;

        // 确保有一个对象
        if let Some(root) = json_value.as_object_mut() {
            // 提取已有的 request 对象或创建新的
            let mut request_obj = if let Some(request_val) = root.get_mut("request") {
                if let Some(obj) = request_val.as_object_mut() {
                    obj.clone()
                } else {
                    serde_json::Map::new()
                }
            } else {
                serde_json::Map::new()
            };

            // 处理 model 字段：优先从 request.model，其次从根 model
            if let Some(model_val) = request_obj.get("model").and_then(|v| v.as_str())
                .or_else(|| root.get("model").and_then(|v| v.as_str()))
            {
                let model_str = if model_val.starts_with("models/") {
                    model_val.to_string()
                } else {
                    format!("models/{}", model_val)
                };
                request_obj.insert("model".to_string(), serde_json::Value::String(model_str));
                modified = true;
            }

            // 处理 contents：优先从 request.contents，其次从根 contents
            if let Some(contents_val) = request_obj.get("contents").cloned()
                .or_else(|| root.get("contents").cloned())
            {
                request_obj.insert("contents".to_string(), contents_val);
                modified = true;
            }

            // 将标准化的 request 对象写回根
            root.insert("request".to_string(), serde_json::Value::Object(request_obj));
        }

        if modified {
            tracing::info!(
                request_id = %request_id,
                "Standardized countTokens request body structure"
            );
        } else {
            tracing::debug!(
                request_id = %request_id,
                "No changes made for countTokens request body"
            );
        }

        modified
    }

    /// 检查所有限制 - 包括速率限制、每日限制、过期时间等
    async fn check_rate_limit(
        &self,
        user_api: &user_service_apis::Model,
    ) -> Result<(), ProxyError> {
        // 1. 检查API过期时间
        if let Some(expires_at) = &user_api.expires_at {
            let now = chrono::Utc::now().naive_utc();
            if now > *expires_at {
                tracing::warn!(
                    user_service_api_id = user_api.id,
                    expires_at = %expires_at,
                    "API has expired"
                );
                return Err(ProxyError::rate_limit("API has expired".to_string()));
            }
        }

        // 2. 检查每分钟请求数限制
        if let Some(rate_limit) = user_api.max_request_per_min {
            if rate_limit > 0 {
                self.check_minute_rate_limit(user_api.id, rate_limit)
                    .await?;
            }
        }

        // 3. 检查每日请求数限制
        if let Some(daily_limit) = user_api.max_requests_per_day {
            if daily_limit > 0 {
                self.check_daily_request_limit(user_api.id, daily_limit)
                    .await?;
            }
        }

        // 4. 检查每日token限制 (基于历史数据预检查)
        if let Some(token_limit) = user_api.max_tokens_per_day {
            if token_limit > 0 {
                self.check_daily_token_limit(user_api.id, token_limit)
                    .await?;
            }
        }

        // 5. 检查每日成本限制 (基于历史数据预检查)
        if let Some(cost_limit) = user_api.max_cost_per_day {
            if cost_limit > Decimal::ZERO {
                self.check_daily_cost_limit(user_api.id, cost_limit).await?;
            }
        }

        Ok(())
    }

    /// 检查每分钟速率限制
    async fn check_minute_rate_limit(
        &self,
        service_api_id: i32,
        rate_limit: i32,
    ) -> Result<(), ProxyError> {
        let cache_key = format!("rate_limit:service_api:{}:minute", service_api_id);

        // 使用统一缓存的incr操作实现速率限制
        let current_count = self
            .cache
            .provider()
            .incr(&cache_key, 1)
            .await
            .map_err(|e| ProxyError::internal(format!("Cache incr error: {}", e)))?;

        // 如果是第一次请求，设置过期时间
        if current_count == 1 {
            let _ = self
                .cache
                .provider()
                .expire(&cache_key, Duration::from_secs(60))
                .await;
        }

        if current_count > rate_limit as i64 {
            tracing::warn!(
                service_api_id = service_api_id,
                current_count = current_count,
                rate_limit = rate_limit,
                "Per-minute rate limit exceeded"
            );

            return Err(ProxyError::rate_limit(format!(
                "Rate limit exceeded: {} requests per minute",
                rate_limit
            )));
        }

        tracing::debug!(
            service_api_id = service_api_id,
            current_count = current_count,
            rate_limit = rate_limit,
            remaining = rate_limit as i64 - current_count,
            "Per-minute rate limit check passed"
        );

        Ok(())
    }

    /// 检查每日请求数限制
    async fn check_daily_request_limit(
        &self,
        service_api_id: i32,
        daily_limit: i32,
    ) -> Result<(), ProxyError> {
        let today = chrono::Utc::now().date_naive();
        let cache_key = format!("rate_limit:service_api:{}:day:{}", service_api_id, today);

        // 使用统一缓存的incr操作实现每日限制
        let current_count = self
            .cache
            .provider()
            .incr(&cache_key, 1)
            .await
            .map_err(|e| ProxyError::internal(format!("Cache incr error: {}", e)))?;

        // 如果是第一次请求，设置过期时间为当天结束
        if current_count == 1 {
            let tomorrow = today + chrono::Duration::days(1);
            let seconds_until_tomorrow = (tomorrow.and_hms_opt(0, 0, 0).unwrap()
                - chrono::Utc::now().naive_utc())
            .num_seconds()
            .max(0) as u64;

            let _ = self
                .cache
                .provider()
                .expire(&cache_key, Duration::from_secs(seconds_until_tomorrow))
                .await;
        }

        if current_count > daily_limit as i64 {
            tracing::warn!(
                service_api_id = service_api_id,
                current_count = current_count,
                daily_limit = daily_limit,
                date = %today,
                "Daily request limit exceeded"
            );

            return Err(ProxyError::rate_limit(format!(
                "Daily request limit exceeded: {} requests per day",
                daily_limit
            )));
        }

        tracing::debug!(
            service_api_id = service_api_id,
            current_count = current_count,
            daily_limit = daily_limit,
            remaining = daily_limit as i64 - current_count,
            date = %today,
            "Daily request limit check passed"
        );

        Ok(())
    }

    /// 检查每日token限制 (基于数据库实际统计)
    async fn check_daily_token_limit(
        &self,
        service_api_id: i32,
        token_limit: i32,
    ) -> Result<(), ProxyError> {
        let today = chrono::Utc::now().date_naive();
        let today_start = today.and_hms_opt(0, 0, 0).unwrap();
        let today_end = (today + chrono::Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap();

        // 查询当天数据库中实际的token消耗
        use entity::proxy_tracing::{Column, Entity as ProxyTracing};

        let total_tokens_used: Option<i64> = ProxyTracing::find()
            .filter(Column::UserServiceApiId.eq(service_api_id))
            .filter(Column::CreatedAt.gte(today_start))
            .filter(Column::CreatedAt.lt(today_end))
            .filter(Column::IsSuccess.eq(true)) // 只计算成功请求的token
            .select_only()
            .column_as(Column::TokensTotal.sum(), "total_tokens")
            .into_tuple::<Option<i64>>()
            .one(&*self.db)
            .await
            .map_err(|e| ProxyError::internal(format!("Database query error: {}", e)))?
            .flatten();

        let current_usage = total_tokens_used.unwrap_or(0);

        if current_usage >= token_limit as i64 {
            tracing::warn!(
                service_api_id = service_api_id,
                current_usage = current_usage,
                token_limit = token_limit,
                date = %today,
                "Daily token limit exceeded (database-verified)"
            );

            return Err(ProxyError::rate_limit(format!(
                "Daily token limit exceeded: {} tokens per day (used: {})",
                token_limit, current_usage
            )));
        }

        tracing::debug!(
            service_api_id = service_api_id,
            current_usage = current_usage,
            token_limit = token_limit,
            remaining = token_limit as i64 - current_usage,
            date = %today,
            "Daily token limit check passed (database-verified)"
        );

        Ok(())
    }

    /// 检查每日成本限制 (基于数据库实际统计)
    async fn check_daily_cost_limit(
        &self,
        service_api_id: i32,
        cost_limit: Decimal,
    ) -> Result<(), ProxyError> {
        let today = chrono::Utc::now().date_naive();
        let today_start = today.and_hms_opt(0, 0, 0).unwrap();
        let today_end = (today + chrono::Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap();

        // 查询当天数据库中实际的成本消耗
        use entity::proxy_tracing::{Column, Entity as ProxyTracing};

        let total_cost_used: Option<f64> = ProxyTracing::find()
            .filter(Column::UserServiceApiId.eq(service_api_id))
            .filter(Column::CreatedAt.gte(today_start))
            .filter(Column::CreatedAt.lt(today_end))
            .filter(Column::IsSuccess.eq(true)) // 只计算成功请求的成本
            .select_only()
            .column_as(Column::Cost.sum(), "total_cost")
            .into_tuple::<Option<f64>>()
            .one(&*self.db)
            .await
            .map_err(|e| ProxyError::internal(format!("Database query error: {}", e)))?
            .flatten();

        let current_usage = total_cost_used
            .map(|f| f.to_string().parse::<Decimal>().unwrap_or(Decimal::ZERO))
            .unwrap_or(Decimal::ZERO);

        if current_usage >= cost_limit {
            tracing::warn!(
                service_api_id = service_api_id,
                current_usage = %current_usage.to_string(),
                cost_limit = %cost_limit.to_string(),
                date = %today,
                "Daily cost limit exceeded (database-verified)"
            );

            return Err(ProxyError::rate_limit(format!(
                "Daily cost limit exceeded: ${} per day (used: ${})",
                cost_limit, current_usage
            )));
        }

        tracing::debug!(
            service_api_id = service_api_id,
            current_usage = %current_usage.to_string(),
            cost_limit = %cost_limit.to_string(),
            remaining = %(cost_limit - current_usage).to_string(),
            date = %today,
            "Daily cost limit check passed (database-verified)"
        );

        Ok(())
    }

    /// 获取提供商类型配置
    async fn get_provider_type(
        &self,
        provider_type_id: i32,
    ) -> Result<provider_types::Model, ProxyError> {
        let cache_key = format!("provider_type:{}", provider_type_id);

        // 首先检查缓存
        if let Ok(Some(provider_type)) = self
            .cache
            .provider()
            .get::<provider_types::Model>(&cache_key)
            .await
        {
            return Ok(provider_type);
        }

        // 从数据库查询
        let provider_type = ProviderTypes::find_by_id(provider_type_id)
            .one(&*self.db)
            .await
            .map_err(|e| ProxyError::internal(format!("Database error: {}", e)))?
            .ok_or(ProxyError::internal("Provider type not found"))?;

        // 缓存结果（30分钟）
        let _ = self
            .cache
            .provider()
            .set(&cache_key, &provider_type, Some(Duration::from_secs(1800)))
            .await;

        Ok(provider_type)
    }

    /// 根据用户API配置选择合适的API密钥
    async fn select_api_key(
        &self,
        user_service_api: &user_service_apis::Model,
        request_id: &str,
    ) -> Result<user_provider_keys::Model, ProxyError> {
        // 创建选择上下文
        let context = SelectionContext::new(
            request_id.to_string(),
            user_service_api.user_id,
            user_service_api.id,
            user_service_api.provider_type_id,
        );

        // 使用ApiKeyPoolManager处理密钥选择 - 正确使用user_provider_keys_ids约束
        let result = self
            .api_key_pool
            .select_api_key_from_service_api(user_service_api, &context)
            .await?;

        tracing::debug!(
            request_id = %request_id,
            user_id = user_service_api.user_id,
            provider_type_id = user_service_api.provider_type_id,
            selected_key_id = result.selected_key.id,
            strategy = %result.strategy.as_str(),
            reason = %result.reason,
            "API key selection completed using ApiKeyPoolManager"
        );

        Ok(result.selected_key)
    }

    /// 选择上游对等体
    pub async fn select_upstream_peer(
        &self,
        ctx: &ProxyContext,
    ) -> Result<Box<HttpPeer>, ProxyError> {
        let provider_type = match ctx.provider_type.as_ref() {
            Some(provider_type) => provider_type,
            None => {
                let error = ProxyError::internal("Provider type not set");
                // 上游对等体选择失败时立即记录到数据库
                self.tracing_service
                    .complete_trace_upstream_error(&ctx.request_id, &error.to_string())
                    .await?;
                return Err(error);
            }
        };

        // 动态确定上游地址 - 对Gemini进行特殊处理
        let upstream_addr = if provider_type.name.to_lowercase().contains("gemini") {
            // Gemini代理模式识别
            let gemini_mode = self.identify_gemini_proxy_mode(ctx).await?;
            let upstream_host = gemini_mode.upstream_host();

            tracing::info!(
                request_id = %ctx.request_id,
                provider = %provider_type.name,
                gemini_mode = ?gemini_mode,
                upstream_host = upstream_host,
                "Identified Gemini proxy mode and upstream host"
            );

            format!("{}:443", upstream_host)
        } else {
            // 其他提供商使用配置中的base_url
            if provider_type.base_url.contains(':') {
                provider_type.base_url.clone()
            } else {
                format!("{}:443", provider_type.base_url)
            }
        };

        tracing::debug!(
            request_id = %ctx.request_id,
            upstream = %upstream_addr,
            provider = %provider_type.name,
            "Selected upstream peer"
        );

        // Upstream address no longer stored in simplified trace schema
        tracing::info!(
            request_id = %ctx.request_id,
            upstream_addr = %upstream_addr,
            "Selected upstream address (not stored in trace)"
        );

        // 创建基础peer
        let mut peer = HttpPeer::new(upstream_addr, true, provider_type.base_url.clone());

        // 获取超时配置，如果前面的配置逻辑未设置则使用30秒fallback
        let connection_timeout_secs = ctx.timeout_seconds.unwrap_or(30) as u64;
        let total_timeout_secs = connection_timeout_secs + 5; // 总超时比连接超时多5秒
        let read_timeout_secs = connection_timeout_secs * 2; // 读取超时是连接超时的2倍

        // 为所有提供商配置通用选项
        if let Some(options) = peer.get_mut_peer_options() {
            // 优先协商 HTTP/2，避免部分上游在 HTTP/1.1 下要求 Content-Length 的限制
            // 注意：如 Pingora 版本不支持该字段，请根据实际 API 调整。
            // 尝试设置 ALPN 优先顺序：h2 -> http/1.1（如该字段在当前版本不可用，请按版本调整或忽略）
            options.alpn = ALPN::H2H1;

            // 设置动态超时配置
            options.connection_timeout = Some(Duration::from_secs(connection_timeout_secs));
            options.total_connection_timeout = Some(Duration::from_secs(total_timeout_secs));
            options.read_timeout = Some(Duration::from_secs(read_timeout_secs));
            options.write_timeout = Some(Duration::from_secs(read_timeout_secs));

            // 已移除 TLS 验证设置

            // 设置HTTP/2特定选项
            options.h2_ping_interval = Some(Duration::from_secs(30));
            options.max_h2_streams = 100;

            tracing::debug!(
                request_id = %ctx.request_id,
                provider = %provider_type.name,
                provider_id = provider_type.id,
                connection_timeout_s = connection_timeout_secs,
                total_timeout_s = total_timeout_secs,
                read_timeout_s = read_timeout_secs,
                "Configured universal peer options with dynamic timeout"
            );
        } else {
            // 为其他服务商也应用动态超时配置
            if let Some(options) = peer.get_mut_peer_options() {
                options.connection_timeout = Some(Duration::from_secs(connection_timeout_secs));
                options.total_connection_timeout = Some(Duration::from_secs(total_timeout_secs));
                options.read_timeout = Some(Duration::from_secs(read_timeout_secs));
                options.write_timeout = Some(Duration::from_secs(read_timeout_secs));

                tracing::debug!(
                    request_id = %ctx.request_id,
                    provider = %provider_type.name,
                    connection_timeout_s = connection_timeout_secs,
                    total_timeout_s = total_timeout_secs,
                    read_timeout_s = read_timeout_secs,
                    "Configured peer options with dynamic timeout"
                );
            }
        }

        Ok(Box::new(peer))
    }

    /// 过滤上游请求 - 替换认证信息和隐藏源信息
    pub async fn filter_upstream_request(
        &self,
        session: &Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut ProxyContext,
    ) -> Result<(), ProxyError> {
        // 获取原始路径
        let original_path = session.req_header().uri.path();

        tracing::info!(
            request_id = %ctx.request_id,
            method = %session.req_header().method,
            path = %original_path,
            flow = "before_modify_request",
            "修改请求信息前"
        );

        // Gemini代理处理
        if let Some(provider_type) = &ctx.provider_type {
            if provider_type.name.to_lowercase().contains("gemini") {
                let gemini_mode = self.identify_gemini_proxy_mode(ctx).await?;

                // 注意：不再进行路径注入，project_id 将在请求体处理阶段注入
                // 路径保持原样，如 /v1internal:loadCodeAssist
                tracing::debug!(
                    request_id = %ctx.request_id,
                    original_path = original_path,
                    gemini_mode = ?gemini_mode,
                    "Gemini request - path unchanged, project_id injection will happen in request body"
                );

                // 处理 Query 参数
                self.modify_gemini_query_parameters(ctx, upstream_request, &gemini_mode)
                    .await?;

                // 处理 Headers
                self.modify_gemini_headers(ctx, upstream_request, &gemini_mode)
                    .await?;

                // 处理 Body（需要在后续实现时取消注释）
                self.modify_gemini_request_body(ctx, session, upstream_request, &gemini_mode)
                    .await?;

                tracing::debug!(
                    request_id = %ctx.request_id,
                    gemini_mode = ?gemini_mode,
                    final_path = upstream_request.uri.path(),
                    "Applied Gemini proxy path processing"
                );
            } else {
                tracing::debug!(
                    request_id = %ctx.request_id,
                    original_path = %original_path,
                    "Using original path for non-Gemini provider"
                );
            }
        }

        // 收集请求详情 - 委托给StatisticsService
        let request_stats_for_details = self.statistics_service.collect_request_stats(session);
        let request_details = self
            .statistics_service
            .collect_request_details(session, &request_stats_for_details);
        ctx.request_details = request_details;

        // Request size no longer stored in simplified trace schema
        if ctx.request_details.body_size.is_some() {
            tracing::info!(
                request_id = %ctx.request_id,
                request_size = ?ctx.request_details.body_size,
                "Request size collected (not stored in trace)"
            );
        }

        let selected_backend = match ctx.selected_backend.as_ref() {
            Some(backend) => backend,
            None => {
                let error = ProxyError::internal("Backend not selected");
                // 请求转发失败时立即记录到数据库
                self.tracing_service
                    .complete_trace_upstream_error(&ctx.request_id, &error.to_string())
                    .await?;
                return Err(error);
            }
        };

        let provider_type = match ctx.provider_type.as_ref() {
            Some(provider_type) => provider_type,
            None => {
                let error = ProxyError::internal("Provider type not set");
                // 请求转发失败时立即记录到数据库
                self.tracing_service
                    .complete_trace_config_error(&ctx.request_id, &error.to_string())
                    .await?;
                return Err(error);
            }
        };

        // 记录未认证之前的请求头信息（关键头 + 全量头）
        let client_headers_before_auth =
            self.extract_key_headers_from_request(session.req_header());
        let upstream_headers_before_auth = self.extract_key_headers_from_request(upstream_request);
        let client_all_headers = self.format_all_request_headers(session.req_header());
        let upstream_all_headers_before = self.format_all_request_headers(upstream_request);

        let client_all_headers_str = if client_all_headers.is_empty() {
            "<none>".to_string()
        } else {
            format!("\n  - {}", client_all_headers.join("\n  - "))
        };
        let upstream_all_headers_before_str = if upstream_all_headers_before.is_empty() {
            "<none>".to_string()
        } else {
            format!(
                "\n  - {}",
                upstream_all_headers_before.join("\n  - ")
            )
        };

        tracing::info!(
            request_id = %ctx.request_id,
            stage = "before_auth",
            client_headers_key = %client_headers_before_auth,
            upstream_headers_key = %upstream_headers_before_auth,
            client_headers_all = %client_all_headers_str,
            upstream_headers_all = %upstream_all_headers_before_str,
            "=== 客户端与上游请求头（认证前） ==="
        );

        // 应用统一的数据库驱动认证
        self.apply_authentication(
            ctx,
            upstream_request,
            provider_type,
            &selected_backend.api_key,
        )
        .await?;

        // 设置正确的Host头 - 只使用域名，不包含协议
        let host_name = provider_type
            .base_url
            .replace("https://", "")
            .replace("http://", "");
        if let Err(e) = upstream_request.insert_header("host", &host_name) {
            let error = ProxyError::internal(format!("Failed to set host header: {}", e));
            // 头部设置失败时立即记录到数据库
            self.tracing_service
                .complete_trace_config_error(&ctx.request_id, &error.to_string())
                .await?;
            return Err(error);
        }

        // 移除可能暴露客户端信息的头部 - 完全隐藏源信息
        let headers_to_remove = [
            "x-forwarded-for",
            "x-real-ip",
            "x-forwarded-proto",
            "x-original-forwarded-for",
            "x-client-ip",
            "cf-connecting-ip",
            "x-forwarded-host",
            "x-forwarded-port",
            "via",
        ];

        for header in &headers_to_remove {
            upstream_request.remove_header(*header);
        }

        // 保持原始用户代理或使用标准浏览器用户代理
        if upstream_request.headers.get("user-agent").is_none() {
            if let Err(e) = upstream_request.insert_header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36") {
                let error = ProxyError::internal(format!("Failed to set user-agent: {}", e));
                // 头部设置失败时立即记录到数据库
                self.tracing_service.complete_trace_config_error(
                    &ctx.request_id,
                    &error.to_string(),
                ).await?;
                return Err(error);
            }
        }

        // 为所有AI服务添加标准头部（移除硬编码的Google特判）
        {
            // 确保有Accept头
            let is_sse_endpoint = self.is_sse_request(session, upstream_request);

            if upstream_request.headers.get("accept").is_none() {
                let accept_value = if is_sse_endpoint {
                    "text/event-stream"
                } else {
                    "application/json"
                };
                if let Err(e) = upstream_request.insert_header("accept", accept_value) {
                    let error = ProxyError::internal(format!("Failed to set accept header: {}", e));
                    // 头部设置失败时立即记录到数据库
                    self.tracing_service
                        .complete_trace_config_error(&ctx.request_id, &error.to_string())
                        .await?;
                    return Err(error);
                }
            }

            // 智能处理Accept-Encoding：只有当原始客户端请求支持压缩时才请求压缩
            // 这样可以避免普通客户端收到压缩响应的问题
            let client_supports_compression = session
                .req_header()
                .headers
                .get("accept-encoding")
                .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
                .map(|s| s.contains("gzip") || s.contains("deflate"))
                .unwrap_or(false);

            if is_sse_endpoint {
                // 对于 SSE，移除任何压缩协商，确保事件流稳定
                upstream_request.remove_header("accept-encoding");
                tracing::debug!(
                    request_id = %ctx.request_id,
                    "SSE endpoint detected, removed accept-encoding for stability"
                );
            } else if client_supports_compression
                && upstream_request.headers.get("accept-encoding").is_none() {
                if let Err(e) = upstream_request.insert_header("accept-encoding", "gzip, deflate") {
                    let error = ProxyError::internal(format!(
                        "Failed to set accept-encoding header: {}",
                        e
                    ));
                    // 头部设置失败时立即记录到数据库
                    self.tracing_service
                        .complete_trace_config_error(&ctx.request_id, &error.to_string())
                        .await?;
                    return Err(error);
                }

                tracing::debug!(
                    request_id = %ctx.request_id,
                    "Client supports compression, requesting compressed response from upstream"
                );
            } else if !is_sse_endpoint {
                // 客户端不支持压缩，移除任何Accept-Encoding头，确保上游返回未压缩响应
                upstream_request.remove_header("accept-encoding");

                tracing::debug!(
                    request_id = %ctx.request_id,
                    client_supports_compression = client_supports_compression,
                    "Client doesn't support compression, requesting uncompressed response from upstream"
                );
            }
        }

        // 记录认证后的头部信息变化
        let client_headers_after_auth = self.extract_key_headers_from_request(session.req_header());
        let upstream_headers_after_auth = self.extract_key_headers_from_request(upstream_request);

        tracing::info!(
            request_id = %ctx.request_id,
            stage = "after_auth",
            client_headers = %client_headers_after_auth,
            upstream_headers = %upstream_headers_after_auth,
            provider = %provider_type.name,
            backend_id = selected_backend.id,
            "Headers after authentication and processing"
        );

        tracing::info!(
            request_id = %ctx.request_id,
            method = %upstream_request.method,
            final_uri = %upstream_request.uri,
            flow = "after_auth_replacement",
            "替换认证信息完成"
        );

        // Content-Length 处理策略：
        // - 对将要修改请求体的路由（如 generateContent/streamGenerateContent/onboardUser），移除原始 Content-Length，避免长度不一致
        // - 否则若方法为 POST/PUT/PATCH 且缺少 Content-Length/Transfer-Encoding，则显式设置 Content-Length: 0，避免上游 411
        let method_upper = upstream_request.method.to_string().to_uppercase();
        let path_for_len = upstream_request.uri.path().to_string();

        if ctx.will_modify_body {
            upstream_request.remove_header("content-length");
            tracing::debug!(
                request_id = %ctx.request_id,
                path = %path_for_len,
                "将修改请求体，移除原始 Content-Length"
            );
        } else {
            // 优先以下游客户端请求头为准判断是否“无请求体”
            let has_cl_client = session
                .req_header()
                .headers
                .get("content-length")
                .is_some();
            let has_te_client = session
                .req_header()
                .headers
                .get("transfer-encoding")
                .is_some();

            // 其次再看当前上游请求头（通常与下游相同，除非我们前面改动过）
            let has_cl = has_cl_client
                || upstream_request.headers.get("content-length").is_some();
            let has_te = has_te_client
                || upstream_request.headers.get("transfer-encoding").is_some();
            let is_body_method = matches!(method_upper.as_str(), "POST" | "PUT" | "PATCH");
            if is_body_method && !has_cl && !has_te {
                // 上游有些端点（如 cloudcode-pa）要求 Content-Length，即使没有请求体
                if let Err(e) = upstream_request.insert_header("content-length", "0") {
                    let error = ProxyError::internal(format!(
                        "Failed to set content-length: 0 header: {}",
                        e
                    ));
                    self.tracing_service
                        .complete_trace_config_error(&ctx.request_id, &error.to_string())
                        .await?;
                    return Err(error);
                }
                tracing::debug!(
                    request_id = %ctx.request_id,
                    method = %method_upper,
                    path = %path_for_len,
                    "无请求体路由，显式设置 Content-Length: 0"
                );
            }
        }

        // 注释掉可能导致问题的自定义头部
        // upstream_request.insert_header("x-request-id", &ctx.request_id)
        //     .map_err(|e| ProxyError::internal(format!("Failed to set request-id: {}", e)))?;

        // 添加详细的上游请求日志（更友好的多行格式）
        let upstream_all_headers_after = self.format_all_request_headers(upstream_request);
        let upstream_all_headers_after_str = if upstream_all_headers_after.is_empty() {
            "<none>".to_string()
        } else {
            format!(
                "\n  - {}",
                upstream_all_headers_after.join("\n  - ")
            )
        };

        tracing::info!(
            request_id = %ctx.request_id,
            final_uri = %upstream_request.uri,
            method = %upstream_request.method,
            backend_key_id = selected_backend.id,
            provider = %provider_type.name,
            auth_preview = %AuthUtils::sanitize_api_key(&selected_backend.api_key),
            headers = %upstream_all_headers_after_str,
            flow = "after_modify_request",
            "修改请求信息后（上游HTTP请求详情）"
        );

        Ok(())
    }

    /// 过滤上游响应 - 协调器模式：委托给专门服务
    pub async fn filter_upstream_response(
        &self,
        upstream_response: &mut ResponseHeader,
        ctx: &mut ProxyContext,
    ) -> Result<(), ProxyError> {
        // 记录响应头信息（关键头 + 全量头）
        let response_headers = self.extract_key_headers_from_response(upstream_response);
        let response_all_headers = self.format_all_response_headers(upstream_response);
        let response_all_headers_str = if response_all_headers.is_empty() {
            "<none>".to_string()
        } else {
            format!("\n  - {}", response_all_headers.join("\n  - "))
        };

        tracing::info!(
            request_id = %ctx.request_id,
            stage = "response",
            status = %upstream_response.status,
            response_headers_key = %response_headers,
            response_headers_all = %response_all_headers_str,
            "=== 上游HTTP响应头 ==="
        );

        // 如果状态码为 4xx/5xx，标记失败阶段（响应体会在后续阶段打印）
        let status_code = upstream_response.status.as_u16();
        if status_code >= 400 {
            tracing::error!(
                request_id = %ctx.request_id,
                status = status_code,
                flow = "response_failure",
                "响应失败，稍后打印响应体"
            );
        }

        // 收集响应详情 - 委托给StatisticsService
        self.statistics_service
            .collect_response_details(upstream_response, ctx);

        // 初始化token使用信息 - 委托给StatisticsService
        let token_usage = self.statistics_service.initialize_token_usage(ctx).await?;
        ctx.token_usage = token_usage;

        // 更新数据库中的model信息 - 委托给TracingService
        if let Some(model_used) = &ctx.token_usage.model_used {
            self.tracing_service
                .update_extended_trace_info(
                    &ctx.request_id,
                    None,                     // provider_type_id 已设置
                    Some(model_used.clone()), // 更新model_used字段
                    None,                     // user_provider_key_id 已设置
                )
                .await?;

            tracing::info!(
                request_id = %ctx.request_id,
                model_used = ?model_used,
                "Updated trace info with model information via TracingService"
            );
        }

        // ========== 压缩响应处理 ==========
        // 检测响应是否被压缩
        let content_encoding = upstream_response
            .headers
            .get("content-encoding")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .map(|s| s.to_lowercase());

        // 检测内容类型
        let content_type = upstream_response
            .headers
            .get("content-type")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .unwrap_or("application/json");

        // 检测是否为流式响应
        // 默认支持流式，主要通过Content-Type自动检测
        let is_streaming = content_type.contains("text/event-stream")
            || content_type.contains("application/stream+json")
            || content_type.contains("text/plain");

        // 日志记录响应信息
        tracing::info!(
            request_id = %ctx.request_id,
            status = upstream_response.status.as_u16(),
            content_type = content_type,
            content_encoding = ?content_encoding,
            is_streaming = is_streaming,
            content_length = upstream_response.headers.get("content-length")
                .and_then(|v| std::str::from_utf8(v.as_bytes()).ok()),
            "Processing upstream response"
        );

        // ========== 透明响应传递 ==========
        // 对于压缩响应，确保完整透传所有相关头部
        if content_encoding.is_some() {
            tracing::debug!(
                request_id = %ctx.request_id,
                encoding = ?content_encoding,
                "Preserving compressed response with all headers"
            );
            // 保持压缩相关的所有头部，让客户端处理解压
            // 不移除 Content-Encoding, Content-Length, Transfer-Encoding 等关键头部
        }

        // 对于流式响应，确保支持chunk传输
        if is_streaming {
            tracing::debug!(
                request_id = %ctx.request_id,
                "Configuring for streaming response"
            );
            // 保持流式传输相关头部
            // Transfer-Encoding: chunked 应该保持
        }

        // ========== 安全头部处理 ==========
        // 只移除可能暴露服务器信息的头部，保留传输相关的核心头部
        let headers_to_remove = [
            "x-powered-by",
            "x-ratelimit-limit-requests",
            "x-ratelimit-limit-tokens",
            "x-ratelimit-remaining-requests",
            "x-ratelimit-remaining-tokens",
        ];

        for header in &headers_to_remove {
            upstream_response.remove_header(*header);
        }

        // 谨慎处理server头部 - 保持原有或使用通用标识
        if upstream_response.headers.get("server").is_none() {
            upstream_response
                .insert_header("server", "nginx/1.24.0")
                .map_err(|e| ProxyError::internal(format!("Failed to set server header: {}", e)))?;
        }

        // ========== 跨域支持 ==========
        // 为API响应添加基本的CORS头部
        if upstream_response
            .headers
            .get("access-control-allow-origin")
            .is_none()
        {
            upstream_response
                .insert_header("access-control-allow-origin", "*")
                .map_err(|e| ProxyError::internal(format!("Failed to set CORS header: {}", e)))?;
        }

        tracing::debug!(
            request_id = %ctx.request_id,
            status = upstream_response.status.as_u16(),
            tokens_used = ctx.tokens_used,
            preserved_encoding = ?content_encoding,
            "Upstream response processed successfully"
        );

        Ok(())
    }

    /// 统一的认证头处理方法 - 支持多认证类型
    async fn apply_authentication(
        &self,
        ctx: &ProxyContext,
        upstream_request: &mut RequestHeader,
        provider_type: &provider_types::Model,
        api_key: &str,
    ) -> Result<(), ProxyError> {
        // 获取用户配置的认证类型
        let selected_backend = ctx
            .selected_backend
            .as_ref()
            .ok_or_else(|| ProxyError::internal("Backend not selected in context"))?;

        let auth_type = &selected_backend.auth_type;

        // 根据认证类型应用不同的认证策略
        let parsed_auth_type = AuthType::from(auth_type.as_str());
        match parsed_auth_type {
            AuthType::ApiKey => {
                // 传统API Key认证 - 根据provider类型使用相应的认证头
                self.apply_api_key_authentication(ctx, upstream_request, provider_type, api_key)
                    .await
            }
            AuthType::OAuth => {
                // 统一OAuth认证 - 支持所有OAuth 2.0提供商
                // 对于OAuth，api_key实际包含session_id，需要查询实际的access_token
                let session_id = api_key; // 为了代码清晰性重命名

                // 从oauth_client_sessions表查询actual access_token
                let oauth_session = OAuthClientSessions::find()
                    .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
                    .one(self.db.as_ref())
                    .await
                    .map_err(|e| {
                        let error =
                            ProxyError::internal(format!("Failed to query OAuth session: {}", e));
                        error
                    })?;

                let access_token = match oauth_session {
                    Some(session) => {
                        if let Some(access_token) = &session.access_token {
                            access_token.clone()
                        } else {
                            return Err(ProxyError::internal("OAuth session has no access_token"));
                        }
                    }
                    None => {
                        return Err(ProxyError::internal("OAuth session not found"));
                    }
                };

                self.apply_oauth_authentication(ctx, upstream_request, provider_type, &access_token)
                    .await
            }
            AuthType::ServiceAccount => {
                // Google服务账户认证 - JWT格式
                self.apply_service_account_authentication(
                    ctx,
                    upstream_request,
                    provider_type,
                    api_key,
                )
                .await
            }
            AuthType::Adc => {
                // Google ADC认证 - 使用环境凭据
                self.apply_adc_authentication(ctx, upstream_request, provider_type, api_key)
                    .await
            }
        }
    }

    /// 应用API Key认证
    async fn apply_api_key_authentication(
        &self,
        ctx: &ProxyContext,
        upstream_request: &mut RequestHeader,
        provider_type: &provider_types::Model,
        api_key: &str,
    ) -> Result<(), ProxyError> {
        // 使用统一的出站认证头构建逻辑，为上游AI服务商构建正确的认证头
        let auth_headers = match self
            .auth_service
            .build_outbound_auth_headers_for_upstream(provider_type, api_key)
        {
            Ok(headers) => headers,
            Err(error) => {
                self.tracing_service
                    .complete_trace_config_error(&ctx.request_id, &error.to_string())
                    .await?;
                return Err(error);
            }
        };

        // 清除所有可能的认证头，确保干净的状态
        self.clear_auth_headers(upstream_request);

        // 设置所有认证头
        let mut applied_header_names = Vec::new();
        for (header_name, header_value) in &auth_headers {
            let static_header_name = get_static_header_name(header_name);
            if let Err(e) = upstream_request.insert_header(static_header_name, header_value) {
                let error = ProxyError::internal(format!(
                    "Failed to set authentication header '{}': {}",
                    header_name, e
                ));
                self.tracing_service
                    .complete_trace_config_error(&ctx.request_id, &error.to_string())
                    .await?;
                return Err(error);
            }
            applied_header_names.push(header_name.clone());
        }

        tracing::info!(
            request_id = %ctx.request_id,
            provider = %provider_type.name,
            auth_type = "api_key",
            auth_headers = ?applied_header_names,
            api_key_preview = %AuthUtils::sanitize_api_key(api_key),
            "Applied API key authentication with {} headers", auth_headers.len()
        );

        Ok(())
    }

    /// 应用统一OAuth认证
    async fn apply_oauth_authentication(
        &self,
        ctx: &ProxyContext,
        upstream_request: &mut RequestHeader,
        provider_type: &provider_types::Model,
        access_token: &str,
    ) -> Result<(), ProxyError> {
        // 清除所有可能的认证头
        self.clear_auth_headers(upstream_request);

        // OAuth 2.0标准使用Authorization: Bearer格式
        let auth_value = format!("Bearer {}", access_token);
        if let Err(e) = upstream_request.insert_header("authorization", &auth_value) {
            let error =
                ProxyError::internal(format!("Failed to set OAuth authorization header: {}", e));
            self.tracing_service
                .complete_trace_config_error(&ctx.request_id, &error.to_string())
                .await?;
            return Err(error);
        }

        tracing::info!(
            request_id = %ctx.request_id,
            provider = %provider_type.name,
            auth_type = "oauth",
            token_preview = %AuthUtils::sanitize_api_key(access_token),
            "Applied OAuth authentication"
        );

        Ok(())
    }

    /// 应用服务账户认证
    async fn apply_service_account_authentication(
        &self,
        ctx: &ProxyContext,
        upstream_request: &mut RequestHeader,
        provider_type: &provider_types::Model,
        jwt_token: &str,
    ) -> Result<(), ProxyError> {
        // 清除所有可能的认证头
        self.clear_auth_headers(upstream_request);

        // 服务账户使用Authorization: Bearer JWT格式
        let auth_value = format!("Bearer {}", jwt_token);
        if let Err(e) = upstream_request.insert_header("authorization", &auth_value) {
            let error = ProxyError::internal(format!(
                "Failed to set service account authorization header: {}",
                e
            ));
            self.tracing_service
                .complete_trace_config_error(&ctx.request_id, &error.to_string())
                .await?;
            return Err(error);
        }

        tracing::info!(
            request_id = %ctx.request_id,
            provider = %provider_type.name,
            auth_type = "service_account",
            jwt_preview = %AuthUtils::sanitize_api_key(jwt_token),
            "Applied service account authentication"
        );

        Ok(())
    }

    /// 应用ADC认证
    async fn apply_adc_authentication(
        &self,
        ctx: &ProxyContext,
        upstream_request: &mut RequestHeader,
        provider_type: &provider_types::Model,
        token: &str,
    ) -> Result<(), ProxyError> {
        // 清除所有可能的认证头
        self.clear_auth_headers(upstream_request);

        // ADC使用Authorization: Bearer格式
        let auth_value = format!("Bearer {}", token);
        if let Err(e) = upstream_request.insert_header("authorization", &auth_value) {
            let error =
                ProxyError::internal(format!("Failed to set ADC authorization header: {}", e));
            self.tracing_service
                .complete_trace_config_error(&ctx.request_id, &error.to_string())
                .await?;
            return Err(error);
        }

        tracing::info!(
            request_id = %ctx.request_id,
            provider = %provider_type.name,
            auth_type = "adc",
            token_preview = %AuthUtils::sanitize_api_key(token),
            "Applied ADC authentication"
        );

        Ok(())
    }

    /// 清除所有可能的认证头
    fn clear_auth_headers(&self, upstream_request: &mut RequestHeader) {
        upstream_request.remove_header("authorization");
        upstream_request.remove_header("x-goog-api-key");
        upstream_request.remove_header("x-api-key");
        upstream_request.remove_header("api-key");
    }

    /// 获取关键头部信息用于日志记录 (RequestHeader 版本)
    fn extract_key_headers_from_request(&self, req_header: &RequestHeader) -> String {
        let mut key_headers = Vec::new();

        // 模仿现有代码的方式直接遍历头部
        for (name, value) in req_header.headers.iter() {
            if let Ok(value_str) = std::str::from_utf8(value.as_bytes()) {
                let name_str = name.as_str().to_lowercase();

                match name_str.as_str() {
                    "authorization" => {
                        let sanitized = if value_str.len() > 20 {
                            format!(
                                "{}***{}",
                                &value_str[..10],
                                &value_str[value_str.len() - 4..]
                            )
                        } else {
                            "***".to_string()
                        };
                        key_headers.push(format!("auth: {}", sanitized));
                    }
                    "content-type" => key_headers.push(format!("content-type: {}", value_str)),
                    "host" => key_headers.push(format!("host: {}", value_str)),
                    "user-agent" => {
                        let truncated = if value_str.len() > 50 {
                            format!("{}...", &value_str[..47])
                        } else {
                            value_str.to_string()
                        };
                        key_headers.push(format!("user-agent: {}", truncated));
                    }
                    _ => {}
                }
            }
        }

        if key_headers.is_empty() {
            "none".to_string()
        } else {
            key_headers.join(", ")
        }
    }

    /// 将所有请求头格式化为人类可读的列表（会对敏感字段做脱敏）
    fn format_all_request_headers(&self, req_header: &RequestHeader) -> Vec<String> {
        let mut all = Vec::new();
        for (name, value) in req_header.headers.iter() {
            let name_str = name.as_str();
            let value_str = std::str::from_utf8(value.as_bytes()).unwrap_or("<binary>");

            let masked = match name_str.to_ascii_lowercase().as_str() {
                "authorization" | "proxy-authorization" | "x-api-key" | "api-key"
                | "x-goog-api-key" | "set-cookie" | "cookie" => {
                    // 只保留前后少量字符，避免日志泄露敏感信息
                    if value_str.len() > 16 {
                        format!(
                            "{}: {}...{}",
                            name_str,
                            &value_str[..8],
                            &value_str[value_str.len().saturating_sub(4)..]
                        )
                    } else {
                        format!("{}: ****", name_str)
                    }
                }
                _ => format!("{}: {}", name_str, value_str),
            };
            all.push(masked);
        }
        all
    }

    /// 将所有响应头格式化为人类可读的列表
    fn format_all_response_headers(&self, resp_header: &ResponseHeader) -> Vec<String> {
        let mut all = Vec::new();
        for (name, value) in resp_header.headers.iter() {
            let name_str = name.as_str();
            let value_str = std::str::from_utf8(value.as_bytes()).unwrap_or("<binary>");
            all.push(format!("{}: {}", name_str, value_str));
        }
        all
    }

    /// 获取关键头部信息用于日志记录 (ResponseHeader 版本)
    fn extract_key_headers_from_response(&self, resp_header: &ResponseHeader) -> String {
        let mut key_headers = Vec::new();

        // 模仿现有代码的方式直接遍历头部
        for (name, value) in resp_header.headers.iter() {
            if let Ok(value_str) = std::str::from_utf8(value.as_bytes()) {
                let name_str = name.as_str().to_lowercase();

                match name_str.as_str() {
                    "content-type" => key_headers.push(format!("content-type: {}", value_str)),
                    "content-length" => key_headers.push(format!("content-length: {}", value_str)),
                    "content-encoding" => {
                        key_headers.push(format!("content-encoding: {}", value_str))
                    }
                    "cache-control" => key_headers.push(format!("cache-control: {}", value_str)),
                    _ => {}
                }
            }
        }

        if key_headers.is_empty() {
            "none".to_string()
        } else {
            key_headers.join(", ")
        }
    }

    /// 检测并转换Pingora错误为ProxyError
    pub fn convert_pingora_error(&self, error: &PingoraError, ctx: &ProxyContext) -> ProxyError {
        let timeout_secs = ctx.timeout_seconds.unwrap_or(30) as u64; // 使用配置的超时或30秒fallback
        let provider_name = ctx
            .provider_type
            .as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("unknown");
        let provider_url = ctx
            .provider_type
            .as_ref()
            .map(|p| p.base_url.as_str())
            .unwrap_or("unknown");

        match &error.etype {
            ErrorType::ConnectTimedout => {
                tracing::error!(
                    request_id = %ctx.request_id,
                    provider = provider_name,
                    timeout_seconds = timeout_secs,
                    "Connection timeout to upstream provider"
                );
                ProxyError::connection_timeout(
                    format!(
                        "Failed to connect to {} ({}) within {}s",
                        provider_name, provider_url, timeout_secs
                    ),
                    timeout_secs,
                )
            }
            ErrorType::ReadTimedout => {
                tracing::error!(
                    request_id = %ctx.request_id,
                    provider = provider_name,
                    timeout_seconds = timeout_secs,
                    "Read timeout from upstream provider"
                );
                ProxyError::read_timeout(
                    format!(
                        "Read timeout when communicating with {} ({}) after {}s",
                        provider_name, provider_url, timeout_secs
                    ),
                    timeout_secs,
                )
            }
            ErrorType::WriteTimedout => {
                tracing::error!(
                    request_id = %ctx.request_id,
                    provider = provider_name,
                    timeout_seconds = timeout_secs,
                    "Write timeout to upstream provider"
                );
                ProxyError::read_timeout(
                    format!(
                        "Write timeout when sending data to {} ({}) after {}s",
                        provider_name, provider_url, timeout_secs
                    ),
                    timeout_secs,
                )
            }
            ErrorType::ConnectError => {
                tracing::error!(
                    request_id = %ctx.request_id,
                    provider = provider_name,
                    "Failed to connect to upstream provider"
                );
                ProxyError::network(format!(
                    "Failed to connect to {} ({})",
                    provider_name, provider_url
                ))
            }
            ErrorType::ConnectRefused => {
                tracing::error!(
                    request_id = %ctx.request_id,
                    provider = provider_name,
                    "Connection refused by upstream provider"
                );
                ProxyError::upstream_not_available(format!(
                    "Connection refused by {} ({})",
                    provider_name, provider_url
                ))
            }
            ErrorType::HTTPStatus(status) if *status >= 500 => {
                tracing::error!(
                    request_id = %ctx.request_id,
                    provider = provider_name,
                    status = *status,
                    "Upstream provider returned server error"
                );
                ProxyError::bad_gateway(format!(
                    "Upstream {} returned server error: {}",
                    provider_name, status
                ))
            }
            _ => {
                tracing::error!(
                    request_id = %ctx.request_id,
                    provider = provider_name,
                    error_type = ?error.etype,
                    error_source = ?error.esource,
                    "Upstream error"
                );
                ProxyError::network(format!(
                    "Network error when communicating with {} ({})",
                    provider_name, provider_url
                ))
            }
        }
    }
}

/// 将动态header name映射为静态字符串引用，解决Rust生命周期问题
///
/// Pingora的insert_header方法需要'static生命周期的字符串引用，
/// 但AuthHeader返回的是String类型。这个函数将常见的header names
/// 映射为静态字符串常量，对于未知header则使用Box::leak作为fallback。
fn get_static_header_name(header_name: &str) -> &'static str {
    match header_name {
        "authorization" => "authorization",
        "x-goog-api-key" => "x-goog-api-key",
        "x-api-key" => "x-api-key",
        "api-key" => "api-key",
        "x-custom-auth" => "x-custom-auth",
        "bearer" => "bearer",
        "token" => "token",
        // 对于未知的header name，使用Box::leak创建静态引用
        // 注意：这会造成少量内存泄漏，但对于HTTP headers这种少量且固定的情况可以接受
        unknown => {
            tracing::warn!("Using Box::leak for unknown header name: {}", unknown);
            Box::leak(unknown.to_string().into_boxed_str())
        }
    }
}
