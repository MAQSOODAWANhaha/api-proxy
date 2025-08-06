//! # AI代理请求处理器
//!
//! 基于设计文档实现的AI代理处理器，负责身份验证、速率限制和转发策略

use anyhow::Result;
use chrono::Utc;
use pingora_core::protocols::tls::ALPN;
use pingora_core::upstreams::peer::{HttpPeer, Peer};
use pingora_core::{Error as PingoraError, ErrorType};
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::Session;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::sync::Arc;
use std::time::Duration;

use crate::cache::UnifiedCacheManager;
use crate::config::{AppConfig, ProviderConfigManager};
use crate::error::ProxyError;
use crate::trace::immediate::ImmediateProxyTracer;
use entity::{
    provider_types::{self, Entity as ProviderTypes},
    user_provider_keys::{self, Entity as UserProviderKeys},
    user_service_apis::{self, Entity as UserServiceApis},
};

/// AI代理处理器
pub struct AIProxyHandler {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 统一缓存管理器
    cache: Arc<UnifiedCacheManager>,
    /// 配置
    config: Arc<AppConfig>,
    /// 负载均衡调度器注册表
    schedulers: Arc<SchedulerRegistry>,
    /// 即时写入追踪器
    tracer: Option<Arc<ImmediateProxyTracer>>,
    /// 服务商配置管理器
    provider_config_manager: Arc<ProviderConfigManager>,
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
                        self.body = Some(format!("{}...[truncated {} bytes]", 
                            &body_str[..65536], 
                            body_str.len() - 65536));
                        tracing::info!(
                            original_size = original_str_len,
                            stored_size = 65536,
                            truncated_bytes = original_str_len - 65536,
                            "Response body finalized as UTF-8 string (truncated)"
                        );
                    } else {
                        self.body = Some(body_str);
                        tracing::info!(
                            body_size = original_str_len,
                            "Response body finalized as UTF-8 string (complete)"
                        );
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
    /// 是否启用追踪
    pub trace_enabled: bool,
    /// 选择的提供商名称
    pub selected_provider: Option<String>,
    /// 连接超时时间(秒)
    pub timeout_seconds: Option<i32>,
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
            trace_enabled: false,
            selected_provider: None,
            timeout_seconds: None,
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

impl AIProxyHandler {
    /// 创建新的AI代理处理器
    pub fn new(
        db: Arc<DatabaseConnection>,
        cache: Arc<UnifiedCacheManager>,
        config: Arc<AppConfig>,
        schedulers: Arc<SchedulerRegistry>,
        tracer: Option<Arc<ImmediateProxyTracer>>,
        provider_config_manager: Arc<ProviderConfigManager>,
    ) -> Self {
        Self {
            db,
            cache,
            config,
            schedulers,
            tracer,
            provider_config_manager,
        }
    }

    /// 准备代理请求 - 核心三步骤：身份验证、速率限制、转发策略
    pub async fn prepare_proxy_request(
        &self,
        session: &Session,
        ctx: &mut ProxyContext,
    ) -> Result<(), ProxyError> {
        let start = std::time::Instant::now();

        tracing::debug!(
            request_id = %ctx.request_id,
            "Starting AI proxy request preparation"
        );

        // 追踪将在认证后开始，因为需要user_service_api_id

        // 步骤1: 身份验证 - 验证是哪个用户创建的哪种服务提供商的token
        let auth_start = std::time::Instant::now();
        let api_key = self.extract_api_key(session)?;
        let user_service_api = self.authenticate_api_key(&api_key).await?;
        ctx.user_service_api = Some(user_service_api.clone());
        let auth_duration = auth_start.elapsed();

        // 开始即时追踪（认证成功后，现在有了user_service_api_id）
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled {
                let method = session.req_header().method.as_str().to_string();
                let path = Some(session.req_header().uri.path().to_string());
                
                // 使用增强的客户端信息收集
                let (client_ip, user_agent, _referer) = self.collect_client_info(session);

                if let Err(e) = tracer.start_trace(
                    ctx.request_id.clone(),
                    user_service_api.id,
                    method,
                    path,
                    Some(client_ip.clone()),
                    user_agent.clone(),
                ).await {
                    tracing::warn!(
                        request_id = %ctx.request_id,
                        error = %e,
                        "Failed to start immediate trace"
                    );
                    ctx.trace_enabled = false; // 禁用追踪
                }
                
                // 记录客户端信息到日志
                tracing::info!(
                    request_id = %ctx.request_id,
                    client_ip = %client_ip,
                    user_agent = ?user_agent,
                    "Client information collected"
                );
                
                // 记录认证阶段追踪信息
                let _ = tracer.add_phase_info(
                    &ctx.request_id,
                    "authentication",
                    auth_duration.as_millis() as u64,
                    true,
                    Some(format!("Authenticated user_service_api_id: {}", user_service_api.id)),
                ).await;
            }
        }

        tracing::debug!(
            request_id = %ctx.request_id,
            user_id = user_service_api.user_id,
            provider_type_id = user_service_api.provider_type_id,
            api_key_preview = %self.sanitize_api_key(&api_key),
            "Authentication successful"
        );

        // 步骤2: 速率验证 - 对这个用户创建的服务商的速率限制
        let rate_limit_start = std::time::Instant::now();
        let rate_limit_result = self.check_rate_limit(&user_service_api).await;
        let rate_limit_duration = rate_limit_start.elapsed();
        
        if let Err(e) = rate_limit_result {
            // 速率限制失败时立即记录到数据库
            if let Some(tracer) = &self.tracer {
                if ctx.trace_enabled {
                    let _ = tracer.complete_trace(
                        &ctx.request_id,
                        429, // Rate limit exceeded
                        false,
                        None,
                        None,
                        None,
                        Some("rate_limit_exceeded".to_string()),
                        Some(e.to_string()),
                    ).await;
                }
            }
            return Err(e);
        }

        tracing::debug!(
            request_id = %ctx.request_id,
            rate_limit = user_service_api.rate_limit,
            "Rate limit check passed"
        );
        
        // 记录速率限制阶段追踪信息
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled {
                let _ = tracer.add_phase_info(
                    &ctx.request_id,
                    "rate_limit_check",
                    rate_limit_duration.as_millis() as u64,
                    true,
                    Some(format!("Rate limit: {:?}", user_service_api.rate_limit)),
                ).await;
            }
        }

        // 步骤3: 获取提供商类型信息和配置
        let provider_type = match self
            .get_provider_type(user_service_api.provider_type_id)
            .await {
            Ok(provider_type) => provider_type,
            Err(e) => {
                // 提供商类型获取失败时立即记录到数据库
                if let Some(tracer) = &self.tracer {
                    if ctx.trace_enabled {
                        let _ = tracer.complete_trace(
                            &ctx.request_id,
                            500, // Internal server error
                            false,
                            None,
                            None,
                            None,
                            Some("provider_type_not_found".to_string()),
                            Some(e.to_string()),
                        ).await;
                    }
                }
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


        // 步骤4: 根据token查找数据库中配置的转发策略
        let load_balancing_start = std::time::Instant::now();
        let scheduler = match self.get_scheduler(&user_service_api.scheduling_strategy) {
            Ok(scheduler) => scheduler,
            Err(e) => {
                // 调度器获取失败时立即记录到数据库
                if let Some(tracer) = &self.tracer {
                    if ctx.trace_enabled {
                        let _ = tracer.complete_trace(
                            &ctx.request_id,
                            500, // Internal server error
                            false,
                            None,
                            None,
                            None,
                            Some("scheduler_not_found".to_string()),
                            Some(e.to_string()),
                        ).await;
                    }
                }
                return Err(e);
            }
        };
        
        let selected_backend = match scheduler.select_backend(&user_service_api).await {
            Ok(backend) => backend,
            Err(e) => {
                // 后端选择失败时立即记录到数据库
                if let Some(tracer) = &self.tracer {
                    if ctx.trace_enabled {
                        let _ = tracer.complete_trace(
                            &ctx.request_id,
                            503, // Service unavailable
                            false,
                            None,
                            None,
                            None,
                            Some("backend_selection_failed".to_string()),
                            Some(e.to_string()),
                        ).await;
                    }
                }
                return Err(e);
            }
        };
        ctx.selected_backend = Some(selected_backend.clone());

        // 更新追踪信息（如果启用）- 使用扩展方法记录更多信息
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled {
                let _ = tracer.update_extended_trace_info(
                    &ctx.request_id,
                    Some(provider_type.name.clone()),
                    Some(provider_type.id),
                    Some(selected_backend.id),
                    None, // model_used将在响应处理时设置
                    None, // upstream_addr将在peer选择时设置
                    None, // request_size将在请求处理时设置
                    Some(user_service_api.id), // user_provider_key_id
                ).await;
                
                tracing::info!(
                    request_id = %ctx.request_id,
                    provider_type_id = provider_type.id,
                    backend_key_id = selected_backend.id,
                    user_service_api_id = user_service_api.id,
                    "Updated trace info with provider and backend details"
                );
                
                // 记录负载均衡阶段追踪信息
                let load_balancing_duration = load_balancing_start.elapsed();
                let _ = tracer.add_phase_info(
                    &ctx.request_id,
                    "load_balancing",
                    load_balancing_duration.as_millis() as u64,
                    true,
                    Some(format!(
                        "Selected backend_id: {}, strategy: {}", 
                        selected_backend.id,
                        user_service_api.scheduling_strategy.as_deref().unwrap_or("round_robin")
                    )),
                ).await;
            }
        }

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

    /// 从请求中提取API密钥
    fn extract_api_key(&self, session: &Session) -> Result<String, ProxyError> {
        // 从Authorization头提取API密钥
        if let Some(auth_header) = session.req_header().headers.get("authorization") {
            let auth_str = std::str::from_utf8(auth_header.as_bytes())
                .map_err(|_| ProxyError::authentication("Invalid authorization header encoding"))?;

            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Ok(token.to_string());
            }
        }

        // 从查询参数提取API密钥
        if let Some(query) = session.req_header().uri.query() {
            for param in query.split('&') {
                if let Some((key, value)) = param.split_once('=') {
                    if key == "api_key" {
                        return Ok(value.to_string());
                    }
                }
            }
        }

        Err(ProxyError::authentication("API key not found"))
    }

    /// 验证API密钥 - 基于user_service_apis表
    async fn authenticate_api_key(
        &self,
        api_key: &str,
    ) -> Result<user_service_apis::Model, ProxyError> {
        let cache_key = format!("user_service_api:{}", api_key);

        // 首先检查缓存
        if let Ok(Some(user_api)) = self
            .cache
            .provider()
            .get::<user_service_apis::Model>(&cache_key)
            .await
        {
            tracing::debug!("Found API key in cache: {}", self.sanitize_api_key(api_key));
            return Ok(user_api);
        }

        // 从数据库查询
        let user_api = UserServiceApis::find()
            .filter(user_service_apis::Column::ApiKey.eq(api_key))
            .filter(user_service_apis::Column::IsActive.eq(true))
            .one(&*self.db)
            .await
            .map_err(|e| ProxyError::internal(format!("Database error: {}", e)))?
            .ok_or(ProxyError::authentication("Invalid API key"))?;

        // 检查API密钥是否过期
        if let Some(expires_at) = user_api.expires_at {
            if expires_at < Utc::now().naive_utc() {
                return Err(ProxyError::authentication("API key expired"));
            }
        }

        // 缓存结果（5分钟）
        let _ = self
            .cache
            .provider()
            .set(&cache_key, &user_api, Some(Duration::from_secs(300)))
            .await;

        tracing::debug!(
            api_key_preview = %self.sanitize_api_key(api_key),
            user_id = user_api.user_id,
            provider_type_id = user_api.provider_type_id,
            "API key authenticated from database"
        );

        Ok(user_api)
    }

    /// 检查速率限制 - 基于统一缓存的滑动窗口算法
    async fn check_rate_limit(
        &self,
        user_api: &user_service_apis::Model,
    ) -> Result<(), ProxyError> {
        let rate_limit = user_api.rate_limit.unwrap_or(1000); // 默认每分钟1000次

        if rate_limit <= 0 {
            return Ok(()); // 无限制
        }

        let cache_key = format!("rate_limit:service_api:{}:minute", user_api.id);

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
                user_service_api_id = user_api.id,
                current_count = current_count,
                rate_limit = rate_limit,
                "Rate limit exceeded"
            );

            return Err(ProxyError::rate_limit(format!(
                "Rate limit exceeded: {} requests per minute",
                rate_limit
            )));
        }

        tracing::debug!(
            user_service_api_id = user_api.id,
            current_count = current_count,
            rate_limit = rate_limit,
            remaining = rate_limit as i64 - current_count,
            "Rate limit check passed"
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

    /// 获取调度器
    fn get_scheduler(
        &self,
        strategy: &Option<String>,
    ) -> Result<Arc<dyn LoadBalancer>, ProxyError> {
        let strategy_name = strategy.as_deref().unwrap_or("round_robin");
        self.schedulers.get(strategy_name)
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
                if let Some(tracer) = &self.tracer {
                    if ctx.trace_enabled {
                        let _ = tracer.complete_trace(
                            &ctx.request_id,
                            500, // Internal server error
                            false,
                            None,
                            None,
                            None,
                            Some("upstream_peer_selection_failed".to_string()),
                            Some(error.to_string()),
                        ).await;
                    }
                }
                return Err(error);
            }
        };

        // 构建上游地址，确保使用HTTPS
        let upstream_addr = if provider_type.base_url.contains(':') {
            provider_type.base_url.clone()
        } else {
            format!("{}:443", provider_type.base_url)
        };

        tracing::debug!(
            request_id = %ctx.request_id,
            upstream = %upstream_addr,
            provider = %provider_type.name,
            "Selected upstream peer"
        );

        // 更新追踪信息：记录upstream地址
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled {
                let _ = tracer.update_extended_trace_info(
                    &ctx.request_id,
                    None, // provider_name 已在之前设置
                    None, // provider_type_id 已在之前设置
                    None, // backend_key_id 已在之前设置
                    None, // model_used将在响应处理时设置
                    Some(upstream_addr.clone()), // 记录upstream地址
                    None, // request_size将在请求处理时设置
                    None, // user_provider_key_id 已在之前设置
                ).await;
                
                tracing::info!(
                    request_id = %ctx.request_id,
                    upstream_addr = %upstream_addr,
                    "Updated trace info with upstream address"
                );
            }
        }

        // 创建基础peer
        let mut peer = HttpPeer::new(upstream_addr, true, provider_type.base_url.clone());

        // 获取超时配置，如果前面的配置逻辑未设置则使用30秒fallback
        let connection_timeout_secs = ctx.timeout_seconds.unwrap_or(30) as u64;
        let total_timeout_secs = connection_timeout_secs + 5; // 总超时比连接超时多5秒
        let read_timeout_secs = connection_timeout_secs * 2; // 读取超时是连接超时的2倍

        // 为Google API配置正确的选项
        if self.should_use_google_api_key_auth(provider_type) {
            if let Some(options) = peer.get_mut_peer_options() {
                // 设置ALPN - 允许HTTP/2和HTTP/1.1协商，优先HTTP/2
                options.alpn = ALPN::H2H1;

                // 设置动态超时配置
                options.connection_timeout = Some(Duration::from_secs(connection_timeout_secs));
                options.total_connection_timeout = Some(Duration::from_secs(total_timeout_secs));
                options.read_timeout = Some(Duration::from_secs(read_timeout_secs));
                options.write_timeout = Some(Duration::from_secs(read_timeout_secs));

                // 设置TLS验证
                options.verify_cert = true;
                options.verify_hostname = true;

                // 设置HTTP/2特定选项
                options.h2_ping_interval = Some(Duration::from_secs(30));
                options.max_h2_streams = 100;

                tracing::debug!(
                    request_id = %ctx.request_id,
                    provider = %provider_type.name,
                    connection_timeout_s = connection_timeout_secs,
                    total_timeout_s = total_timeout_secs,
                    read_timeout_s = read_timeout_secs,
                    "Configured peer options for Google API with dynamic timeout"
                );
            }
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
        // 收集请求头信息
        self.collect_request_details(session, ctx);
        
        // 更新追踪信息：记录请求大小
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled && ctx.request_details.body_size.is_some() {
                let _ = tracer.update_extended_trace_info(
                    &ctx.request_id,
                    None, // provider_name 已设置
                    None, // provider_type_id 已设置
                    None, // backend_key_id 已设置
                    None, // model_used将在响应时设置
                    None, // upstream_addr 已设置
                    ctx.request_details.body_size, // 记录请求体大小
                    None, // user_provider_key_id 已设置
                ).await;
                
                tracing::info!(
                    request_id = %ctx.request_id,
                    request_size = ?ctx.request_details.body_size,
                    "Updated trace info with request size"
                );
            }
        }
        
        let selected_backend = match ctx.selected_backend.as_ref() {
            Some(backend) => backend,
            None => {
                let error = ProxyError::internal("Backend not selected");
                // 请求转发失败时立即记录到数据库
                if let Some(tracer) = &self.tracer {
                    if ctx.trace_enabled {
                        let _ = tracer.complete_trace(
                            &ctx.request_id,
                            500, // Internal server error
                            false,
                            None,
                            None,
                            None,
                            Some("request_forwarding_failed".to_string()),
                            Some(error.to_string()),
                        ).await;
                    }
                }
                return Err(error);
            }
        };
        
        let provider_type = match ctx.provider_type.as_ref() {
            Some(provider_type) => provider_type,
            None => {
                let error = ProxyError::internal("Provider type not set");
                // 请求转发失败时立即记录到数据库
                if let Some(tracer) = &self.tracer {
                    if ctx.trace_enabled {
                        let _ = tracer.complete_trace(
                            &ctx.request_id,
                            500, // Internal server error
                            false,
                            None,
                            None,
                            None,
                            Some("request_forwarding_failed".to_string()),
                            Some(error.to_string()),
                        ).await;
                    }
                }
                return Err(error);
            }
        };

        // 根据提供商类型处理认证信息
        upstream_request.remove_header("authorization");
        upstream_request.remove_header("x-goog-api-key");
        let auth_format = provider_type
            .auth_header_format
            .as_deref()
            .unwrap_or("Bearer {key}");

        // 根据提供商类型选择认证方式
        if self.should_use_google_api_key_auth(provider_type) {
            // Gemini/Google APIs：使用 X-goog-api-key 头部认证
            if let Err(e) = upstream_request.insert_header("x-goog-api-key", &selected_backend.api_key) {
                let error = ProxyError::internal(format!("Failed to set x-goog-api-key header: {}", e));
                // 头部设置失败时立即记录到数据库
                if let Some(tracer) = &self.tracer {
                    if ctx.trace_enabled {
                        let _ = tracer.complete_trace(
                            &ctx.request_id,
                            500, // Internal server error
                            false,
                            None,
                            None,
                            None,
                            Some("header_setting_failed".to_string()),
                            Some(error.to_string()),
                        ).await;
                    }
                }
                return Err(error);
            }

            tracing::debug!(
                request_id = %ctx.request_id,
                provider = %provider_type.name,
                base_url = %provider_type.base_url,
                "Using X-goog-api-key authentication for Google API"
            );
        } else {
            // 其他服务商：使用 Authorization 头部认证
            let auth_value = auth_format.replace("{key}", &selected_backend.api_key);
            if let Err(e) = upstream_request.insert_header("authorization", &auth_value) {
                let error = ProxyError::internal(format!("Failed to set auth header: {}", e));
                // 头部设置失败时立即记录到数据库
                if let Some(tracer) = &self.tracer {
                    if ctx.trace_enabled {
                        let _ = tracer.complete_trace(
                            &ctx.request_id,
                            500, // Internal server error
                            false,
                            None,
                            None,
                            None,
                            Some("header_setting_failed".to_string()),
                            Some(error.to_string()),
                        ).await;
                    }
                }
                return Err(error);
            }

            tracing::debug!(
                request_id = %ctx.request_id,
                provider = %provider_type.name,
                auth_format = %auth_format,
                "Using Authorization header authentication"
            );
        }

        // 设置正确的Host头 - 只使用域名，不包含协议
        let host_name = provider_type
            .base_url
            .replace("https://", "")
            .replace("http://", "");
        if let Err(e) = upstream_request.insert_header("host", &host_name) {
            let error = ProxyError::internal(format!("Failed to set host header: {}", e));
            // 头部设置失败时立即记录到数据库
            if let Some(tracer) = &self.tracer {
                if ctx.trace_enabled {
                    let _ = tracer.complete_trace(
                        &ctx.request_id,
                        500, // Internal server error
                        false,
                        None,
                        None,
                        None,
                        Some("header_setting_failed".to_string()),
                        Some(error.to_string()),
                    ).await;
                }
            }
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
                if let Some(tracer) = &self.tracer {
                    if ctx.trace_enabled {
                        let _ = tracer.complete_trace(
                            &ctx.request_id,
                            500, // Internal server error
                            false,
                            None,
                            None,
                            None,
                            Some("header_setting_failed".to_string()),
                            Some(error.to_string()),
                        ).await;
                    }
                }
                return Err(error);
            }
        }

        // 为Google API添加期望的标准头部
        if self.should_use_google_api_key_auth(provider_type) {
            // 确保有Accept头
            if upstream_request.headers.get("accept").is_none() {
                if let Err(e) = upstream_request.insert_header("accept", "application/json") {
                    let error = ProxyError::internal(format!("Failed to set accept header: {}", e));
                    // 头部设置失败时立即记录到数据库
                    if let Some(tracer) = &self.tracer {
                        if ctx.trace_enabled {
                            let _ = tracer.complete_trace(
                                &ctx.request_id,
                                500, // Internal server error
                                false,
                                None,
                                None,
                                None,
                                Some("header_setting_failed".to_string()),
                                Some(error.to_string()),
                            ).await;
                        }
                    }
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

            if client_supports_compression
                && upstream_request.headers.get("accept-encoding").is_none()
            {
                if let Err(e) = upstream_request.insert_header("accept-encoding", "gzip, deflate") {
                    let error = ProxyError::internal(format!("Failed to set accept-encoding header: {}", e));
                    // 头部设置失败时立即记录到数据库
                    if let Some(tracer) = &self.tracer {
                        if ctx.trace_enabled {
                            let _ = tracer.complete_trace(
                                &ctx.request_id,
                                500, // Internal server error
                                false,
                                None,
                                None,
                                None,
                                Some("header_setting_failed".to_string()),
                                Some(error.to_string()),
                            ).await;
                        }
                    }
                    return Err(error);
                }

                tracing::debug!(
                    request_id = %ctx.request_id,
                    "Client supports compression, requesting compressed response from upstream"
                );
            } else {
                // 客户端不支持压缩，移除任何Accept-Encoding头，确保上游返回未压缩响应
                upstream_request.remove_header("accept-encoding");

                tracing::debug!(
                    request_id = %ctx.request_id,
                    client_supports_compression = client_supports_compression,
                    "Client doesn't support compression, requesting uncompressed response from upstream"
                );
            }
        }

        // 注释掉可能导致问题的自定义头部
        // upstream_request.insert_header("x-request-id", &ctx.request_id)
        //     .map_err(|e| ProxyError::internal(format!("Failed to set request-id: {}", e)))?;

        // 添加详细的Pingora请求日志用于对比
        tracing::info!(
            request_id = %ctx.request_id,
            final_uri = %upstream_request.uri,
            method = %upstream_request.method,
            "=== PINGORA REQUEST DETAILS ==="
        );

        // 记录所有Pingora请求头用于与reqwest对比
        let mut pingora_headers = Vec::new();
        for (name, value) in upstream_request.headers.iter() {
            if let Ok(value_str) = std::str::from_utf8(value.as_bytes()) {
                pingora_headers.push(format!("{}: {}", name.as_str(), value_str));
            }
        }

        tracing::info!(
            request_id = %ctx.request_id,
            headers = ?pingora_headers,
            backend_key_id = selected_backend.id,
            provider = %provider_type.name,
            auth_preview = %self.sanitize_api_key(&selected_backend.api_key),
            "PINGORA HTTP REQUEST HEADERS"
        );

        Ok(())
    }

    /// 获取真实客户端IP地址（考虑代理情况）
    fn get_real_client_ip(&self, session: &Session) -> String {
        let req_header = session.req_header();
        
        // 1. 优先检查 X-Forwarded-For 头
        if let Some(forwarded_for) = req_header.headers.get("x-forwarded-for")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok()) {
            // X-Forwarded-For 可能包含多个IP，取第一个（最原始的客户端IP）
            if let Some(first_ip) = forwarded_for.split(',').next() {
                let ip = first_ip.trim();
                if !ip.is_empty() && ip != "unknown" {
                    return ip.to_string();
                }
            }
        }
        
        // 2. 检查 X-Real-IP 头
        if let Some(real_ip) = req_header.headers.get("x-real-ip")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok()) {
            let ip = real_ip.trim();
            if !ip.is_empty() && ip != "unknown" {
                return ip.to_string();
            }
        }
        
        // 3. 检查 CF-Connecting-IP (Cloudflare)
        if let Some(cf_ip) = req_header.headers.get("cf-connecting-ip")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok()) {
            let ip = cf_ip.trim();
            if !ip.is_empty() && ip != "unknown" {
                return ip.to_string();
            }
        }
        
        // 4. 最后使用直接连接的客户端地址
        session.client_addr()
            .map(|addr| addr.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
    
    /// 收集完整的客户端信息
    fn collect_client_info(&self, session: &Session) -> (String, Option<String>, Option<String>) {
        let req_header = session.req_header();
        
        // 获取真实客户端IP
        let client_ip = self.get_real_client_ip(session);
        
        // 获取User-Agent
        let user_agent = req_header.headers.get("user-agent")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .map(|s| s.to_string());
            
        // 获取Referer
        let referer = req_header.headers.get("referer")
            .or_else(|| req_header.headers.get("referrer")) // 支持两种拼写
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .map(|s| s.to_string());
        
        (client_ip, user_agent, referer)
    }
    
    /// 收集请求详情
    fn collect_request_details(&self, session: &Session, ctx: &mut ProxyContext) {
        let req_header = session.req_header();
        
        // 收集请求头
        let mut headers = std::collections::HashMap::new();
        for (name, value) in req_header.headers.iter() {
            if let Ok(value_str) = std::str::from_utf8(value.as_bytes()) {
                headers.insert(name.as_str().to_string(), value_str.to_string());
            }
        }
        
        // 获取Content-Type
        let content_type = req_header.headers.get("content-type")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .map(|s| s.to_string());
        
        // 获取Content-Length（请求体大小）
        let body_size = req_header.headers.get("content-length")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .and_then(|s| s.parse::<u64>().ok());
            
        // 收集完整的客户端信息
        let (client_ip, user_agent, referer) = self.collect_client_info(session);
        
        // 获取协议版本
        let protocol_version = Some(format!("{:?}", req_header.version));
        
        ctx.request_details = RequestDetails {
            headers,
            body_size,
            content_type,
            client_ip: client_ip.clone(),
            user_agent: user_agent.clone(),
            referer,
            method: req_header.method.as_str().to_string(),
            path: req_header.uri.path().to_string(),
            protocol_version,
        };
        
        tracing::info!(
            request_id = %ctx.request_id,
            headers_count = ctx.request_details.headers.len(),
            content_type = ?ctx.request_details.content_type,
            body_size = ?ctx.request_details.body_size,
            client_ip = %client_ip,
            user_agent = ?user_agent,
            method = %ctx.request_details.method,
            path = %ctx.request_details.path,
            "Collected comprehensive request details"
        );
    }

    /// 收集响应头信息
    fn collect_response_headers(&self, upstream_response: &ResponseHeader, ctx: &mut ProxyContext) {
        // 收集响应头
        let mut headers = std::collections::HashMap::new();
        for (name, value) in upstream_response.headers.iter() {
            if let Ok(value_str) = std::str::from_utf8(value.as_bytes()) {
                headers.insert(name.as_str().to_string(), value_str.to_string());
            }
        }
        
        // 获取Content-Type
        let content_type = upstream_response.headers.get("content-type")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .map(|s| s.to_string());
        
        // 获取Content-Length（响应体大小）
        let body_size = upstream_response.headers.get("content-length")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .and_then(|s| s.parse::<u64>().ok());
        
        // 获取Content-Encoding
        let content_encoding = upstream_response.headers.get("content-encoding")
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .map(|s| s.to_string());
        
        ctx.response_details = ResponseDetails {
            headers,
            body: None, // 响应体稍后在response body处理时收集
            body_size,
            content_type,
            content_encoding,
            body_chunks: Vec::new(), // 初始化为空的Vec
        };
        
        tracing::info!(
            request_id = %ctx.request_id,
            response_headers_count = ctx.response_details.headers.len(),
            content_type = ?ctx.response_details.content_type,
            content_encoding = ?ctx.response_details.content_encoding,
            body_size = ?ctx.response_details.body_size,
            "Collected response headers"
        );
    }

    /// 过滤上游响应
    pub async fn filter_upstream_response(
        &self,
        upstream_response: &mut ResponseHeader,
        ctx: &mut ProxyContext,
    ) -> Result<(), ProxyError> {
        // 提取token使用信息
        ctx.token_usage = self.extract_detailed_token_usage(upstream_response);
        ctx.tokens_used = ctx.token_usage.total_tokens;  // 向后兼容
        
        // 收集响应头信息
        self.collect_response_headers(upstream_response, ctx);
        
        // 更新数据库中的model信息
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled && ctx.token_usage.model_used.is_some() {
                let _ = tracer.update_extended_trace_info(
                    &ctx.request_id,
                    None, // provider_name 已设置
                    None, // provider_type_id 已设置
                    None, // backend_key_id 已设置
                    ctx.token_usage.model_used.clone(), // 更新model_used字段
                    None, // upstream_addr 已设置
                    None, // request_size 已设置
                    None, // user_provider_key_id 已设置
                ).await;
                
                tracing::info!(
                    request_id = %ctx.request_id,
                    model_used = ?ctx.token_usage.model_used,
                    "Updated trace info with model information"
                );
            }
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
        let is_streaming = content_type.contains("text/event-stream")
            || content_type.contains("text/plain")
            || ctx
                .provider_type
                .as_ref()
                .map(|p| p.name.to_lowercase().contains("openai"))
                .unwrap_or(false);

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

    /// 提取详细的token使用信息
    fn extract_detailed_token_usage(&self, response: &ResponseHeader) -> TokenUsage {
        // 尝试提取详细的token信息
        let prompt_tokens = self.extract_single_token_value(response, &[
            "x-openai-prompt-tokens",
            "x-anthropic-input-tokens", 
            "x-google-input-tokens",
            "x-prompt-tokens",
        ]);
        
        let completion_tokens = self.extract_single_token_value(response, &[
            "x-openai-completion-tokens",
            "x-anthropic-output-tokens",
            "x-google-output-tokens", 
            "x-completion-tokens",
        ]);
        
        let total_tokens = self.extract_single_token_value(response, &[
            "x-openai-total-tokens",
            "x-anthropic-total-tokens",
            "x-google-total-tokens",
            "x-total-tokens",
        ]).unwrap_or_else(|| {
            // 如果没有total_tokens头，尝试计算
            match (prompt_tokens, completion_tokens) {
                (Some(p), Some(c)) => p + c,
                (Some(p), None) => p,
                (None, Some(c)) => c,
                (None, None) => 0,
            }
        });
        
        // 尝试提取model信息
        let model_used = self.extract_model_info(response);
        
        TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            model_used,
        }
    }
    
    /// 提取单个token值
    fn extract_single_token_value(&self, response: &ResponseHeader, header_names: &[&str]) -> Option<u32> {
        for header_name in header_names {
            if let Some(header_value) = response.headers.get(*header_name) {
                if let Ok(tokens_str) = std::str::from_utf8(header_value.as_bytes()) {
                    if let Ok(tokens) = tokens_str.parse::<u32>() {
                        return Some(tokens);
                    }
                }
            }
        }
        None
    }
    
    /// 提取AI模型信息
    fn extract_model_info(&self, response: &ResponseHeader) -> Option<String> {
        // 尝试从各种可能的响应头中提取model信息
        let model_headers = [
            "x-openai-model",
            "x-anthropic-model",
            "x-google-model",
            "x-model",
            "model",
            "ai-model",
        ];
        
        for header_name in &model_headers {
            if let Some(header_value) = response.headers.get(*header_name) {
                if let Ok(model_str) = std::str::from_utf8(header_value.as_bytes()) {
                    let model = model_str.trim().to_string();
                    if !model.is_empty() {
                        tracing::info!(
                            "Extracted model info from header '{}': '{}'",
                            header_name, model
                        );
                        return Some(model);
                    }
                }
            }
        }
        
        tracing::debug!("No model information found in response headers");
        None
    }
    
    /// 提取token使用信息（向后兼容方法）
    fn extract_token_usage(&self, response: &ResponseHeader) -> u32 {
        // 尝试从不同的响应头中提取token使用信息
        let token_headers = [
            "x-openai-total-tokens",
            "x-anthropic-total-tokens",
            "x-google-total-tokens",
            "x-total-tokens",
        ];

        for header_name in &token_headers {
            if let Some(header_value) = response.headers.get(*header_name) {
                if let Ok(tokens_str) = std::str::from_utf8(header_value.as_bytes()) {
                    if let Ok(tokens) = tokens_str.parse::<u32>() {
                        return tokens;
                    }
                }
            }
        }

        0
    }

    /// 判断提供商是否使用 X-goog-api-key 认证方式（使用动态配置）
    fn should_use_google_api_key_auth(&self, provider_type: &provider_types::Model) -> bool {
        // 创建ProviderConfig以供ProviderConfigManager使用
        let provider_config = crate::config::ProviderConfig {
            id: provider_type.id,
            name: provider_type.name.clone(),
            display_name: provider_type.display_name.clone(),
            base_url: provider_type.base_url.clone(),
            https_url: if provider_type.base_url.starts_with("http") {
                provider_type.base_url.clone()
            } else {
                format!("https://{}", provider_type.base_url)
            },
            upstream_address: if provider_type.base_url.contains(':') {
                provider_type.base_url.clone()
            } else {
                format!("{}:443", provider_type.base_url)
            },
            api_format: provider_type.api_format.clone(),
            default_model: provider_type.default_model.clone(),
            max_tokens: provider_type.max_tokens,
            rate_limit: provider_type.rate_limit,
            timeout_seconds: provider_type.timeout_seconds,
            health_check_path: provider_type
                .health_check_path
                .clone()
                .unwrap_or_else(|| "/models".to_string()),
            auth_header_format: provider_type
                .auth_header_format
                .clone()
                .unwrap_or_else(|| "Bearer {key}".to_string()),
            is_active: provider_type.is_active,
            config_json: provider_type
                .config_json
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok()),
        };

        // 使用ProviderConfigManager的动态判断逻辑
        let uses_google_auth = self
            .provider_config_manager
            .uses_google_api_key_auth(&provider_config);

        tracing::debug!(
            provider_name = %provider_type.name,
            base_url = %provider_type.base_url,
            auth_format = ?provider_type.auth_header_format,
            uses_google_auth = uses_google_auth,
            "Dynamic Google API key authentication check completed"
        );

        uses_google_auth
    }

    /// 净化API密钥用于日志记录
    fn sanitize_api_key(&self, api_key: &str) -> String {
        if api_key.len() > 10 {
            format!("{}***{}", &api_key[..4], &api_key[api_key.len() - 4..])
        } else {
            "***".to_string()
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

/// 负载均衡器trait
#[async_trait::async_trait]
pub trait LoadBalancer: Send + Sync {
    /// 选择后端API密钥
    async fn select_backend(
        &self,
        user_service_api: &user_service_apis::Model,
    ) -> Result<user_provider_keys::Model, ProxyError>;
}

/// 调度器注册表
pub struct SchedulerRegistry {
    schedulers: std::collections::HashMap<String, Arc<dyn LoadBalancer>>,
}

impl SchedulerRegistry {
    /// 创建新的调度器注册表
    pub fn new(db: Arc<DatabaseConnection>, cache: Arc<UnifiedCacheManager>) -> Self {
        let mut schedulers: std::collections::HashMap<String, Arc<dyn LoadBalancer>> =
            std::collections::HashMap::new();

        // 注册轮询调度器
        schedulers.insert(
            "round_robin".to_string(),
            Arc::new(RoundRobinScheduler::new(db.clone(), cache.clone())),
        );

        // 注册权重调度器
        schedulers.insert(
            "weighted".to_string(),
            Arc::new(WeightedScheduler::new(db.clone(), cache.clone())),
        );

        // 注册健康度最佳调度器
        schedulers.insert(
            "health_best".to_string(),
            Arc::new(HealthBestScheduler::new(db.clone(), cache.clone())),
        );

        Self { schedulers }
    }

    /// 获取调度器
    pub fn get(&self, strategy: &str) -> Result<Arc<dyn LoadBalancer>, ProxyError> {
        self.schedulers.get(strategy).cloned().ok_or_else(|| {
            ProxyError::internal(format!("Unknown scheduling strategy: {}", strategy))
        })
    }
}

/// 轮询调度器
pub struct RoundRobinScheduler {
    db: Arc<DatabaseConnection>,
    cache: Arc<UnifiedCacheManager>,
}

impl RoundRobinScheduler {
    pub fn new(db: Arc<DatabaseConnection>, cache: Arc<UnifiedCacheManager>) -> Self {
        Self { db, cache }
    }
}

#[async_trait::async_trait]
impl LoadBalancer for RoundRobinScheduler {
    async fn select_backend(
        &self,
        user_service_api: &user_service_apis::Model,
    ) -> Result<user_provider_keys::Model, ProxyError> {
        use sea_orm::QueryOrder;

        // 获取该用户该服务商的所有活跃API密钥
        let available_keys = UserProviderKeys::find()
            .filter(user_provider_keys::Column::UserId.eq(user_service_api.user_id))
            .filter(
                user_provider_keys::Column::ProviderTypeId.eq(user_service_api.provider_type_id),
            )
            .filter(user_provider_keys::Column::IsActive.eq(true))
            .order_by_asc(user_provider_keys::Column::Id)
            .all(&*self.db)
            .await
            .map_err(|e| ProxyError::internal(format!("Database error: {}", e)))?;

        if available_keys.is_empty() {
            return Err(ProxyError::bad_gateway("No available API keys"));
        }

        // 从缓存获取当前轮询位置
        let cache_key = format!(
            "round_robin:{}:{}",
            user_service_api.user_id, user_service_api.provider_type_id
        );
        let current_index = if let Ok(index) = self.cache.provider().incr(&cache_key, 1).await {
            let _ = self
                .cache
                .provider()
                .expire(&cache_key, Duration::from_secs(3600))
                .await; // 1小时过期
            (index as usize) % available_keys.len()
        } else {
            0 // 缓存操作失败时使用第一个
        };

        let selected_key = available_keys[current_index].clone();

        tracing::debug!(
            user_id = user_service_api.user_id,
            provider_type_id = user_service_api.provider_type_id,
            selected_key_id = selected_key.id,
            selected_index = current_index,
            total_keys = available_keys.len(),
            "Round robin selection completed"
        );

        Ok(selected_key)
    }
}

/// 权重调度器
pub struct WeightedScheduler {
    db: Arc<DatabaseConnection>,
    cache: Arc<UnifiedCacheManager>,
}

impl WeightedScheduler {
    pub fn new(db: Arc<DatabaseConnection>, cache: Arc<UnifiedCacheManager>) -> Self {
        Self { db, cache }
    }
}

#[async_trait::async_trait]
impl LoadBalancer for WeightedScheduler {
    async fn select_backend(
        &self,
        user_service_api: &user_service_apis::Model,
    ) -> Result<user_provider_keys::Model, ProxyError> {
        use sea_orm::QueryOrder;

        // 获取该用户该服务商的所有活跃API密钥
        let available_keys = UserProviderKeys::find()
            .filter(user_provider_keys::Column::UserId.eq(user_service_api.user_id))
            .filter(
                user_provider_keys::Column::ProviderTypeId.eq(user_service_api.provider_type_id),
            )
            .filter(user_provider_keys::Column::IsActive.eq(true))
            .order_by_asc(user_provider_keys::Column::Id)
            .all(&*self.db)
            .await
            .map_err(|e| ProxyError::internal(format!("Database error: {}", e)))?;

        if available_keys.is_empty() {
            return Err(ProxyError::bad_gateway("No available API keys"));
        }

        // 计算权重总和
        let total_weight: i32 = available_keys
            .iter()
            .map(|key| key.weight.unwrap_or(1))
            .sum();

        if total_weight <= 0 {
            return Ok(available_keys[0].clone()); // 如果所有权重都是0，返回第一个
        }

        // 生成随机数
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let random_weight = rng.gen_range(1..=total_weight);

        // 根据权重选择
        let mut current_weight = 0;
        for key in available_keys {
            current_weight += key.weight.unwrap_or(1);
            if current_weight >= random_weight {
                tracing::debug!(
                    user_id = user_service_api.user_id,
                    provider_type_id = user_service_api.provider_type_id,
                    selected_key_id = key.id,
                    key_weight = key.weight.unwrap_or(1),
                    total_weight = total_weight,
                    random_weight = random_weight,
                    "Weighted selection completed"
                );
                return Ok(key);
            }
        }

        Err(ProxyError::internal("Weight selection failed"))
    }
}

/// 健康度最佳调度器
pub struct HealthBestScheduler {
    db: Arc<DatabaseConnection>,
    cache: Arc<UnifiedCacheManager>,
}

impl HealthBestScheduler {
    pub fn new(db: Arc<DatabaseConnection>, cache: Arc<UnifiedCacheManager>) -> Self {
        Self { db, cache }
    }
}

#[async_trait::async_trait]
impl LoadBalancer for HealthBestScheduler {
    async fn select_backend(
        &self,
        user_service_api: &user_service_apis::Model,
    ) -> Result<user_provider_keys::Model, ProxyError> {
        use sea_orm::QueryOrder;

        // 获取该用户该服务商的所有活跃API密钥
        let available_keys = UserProviderKeys::find()
            .filter(user_provider_keys::Column::UserId.eq(user_service_api.user_id))
            .filter(
                user_provider_keys::Column::ProviderTypeId.eq(user_service_api.provider_type_id),
            )
            .filter(user_provider_keys::Column::IsActive.eq(true))
            .order_by_asc(user_provider_keys::Column::Id)
            .all(&*self.db)
            .await
            .map_err(|e| ProxyError::internal(format!("Database error: {}", e)))?;

        if available_keys.is_empty() {
            return Err(ProxyError::bad_gateway("No available API keys"));
        }

        // 简化实现：选择最近使用时间最早的（假设使用频率低的更健康）
        let best_key = available_keys
            .into_iter()
            .min_by_key(|key| {
                key.last_used.unwrap_or_else(|| {
                    // 如果没有使用过，使用创建时间
                    key.created_at
                })
            })
            .unwrap();

        tracing::debug!(
            user_id = user_service_api.user_id,
            provider_type_id = user_service_api.provider_type_id,
            selected_key_id = best_key.id,
            last_used = ?best_key.last_used,
            "Health best selection completed"
        );

        Ok(best_key)
    }
}
