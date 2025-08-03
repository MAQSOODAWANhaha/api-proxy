//! # AI代理请求处理器
//!
//! 基于设计文档实现的AI代理处理器，负责身份验证、速率限制和转发策略

use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use chrono::Utc;
use pingora_core::upstreams::peer::{HttpPeer, Peer};
use pingora_core::protocols::tls::ALPN;
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::Session;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::auth::unified::UnifiedAuthManager;
use crate::config::AppConfig;
use crate::error::ProxyError;
use crate::cache::UnifiedCacheManager;
use crate::trace::unified::UnifiedProxyTracer;
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
    /// 统一认证管理器
    auth_manager: Arc<UnifiedAuthManager>,
    /// 负载均衡调度器注册表
    schedulers: Arc<SchedulerRegistry>,
    /// 统一追踪器
    tracer: Option<Arc<UnifiedProxyTracer>>,
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
    /// 使用的tokens
    pub tokens_used: u32,
    /// 是否启用追踪
    pub trace_enabled: bool,
    /// 选择的提供商名称
    pub selected_provider: Option<String>,
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
            trace_enabled: false,
            selected_provider: None,
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
        auth_manager: Arc<UnifiedAuthManager>,
        schedulers: Arc<SchedulerRegistry>,
        tracer: Option<Arc<UnifiedProxyTracer>>,
    ) -> Self {
        Self {
            db,
            cache,
            config,
            auth_manager,
            schedulers,
            tracer,
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

        // 开始追踪（如果启用）
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled {
                let method = session.req_header().method.as_str().to_string();
                // 我们还没有 user_service_api_id，所以先用默认值，稍后更新
                if let Err(e) = tracer.start_trace(ctx.request_id.clone(), 0, method).await {
                    tracing::warn!(
                        request_id = %ctx.request_id,
                        error = %e,
                        "Failed to start trace"
                    );
                    ctx.trace_enabled = false; // 禁用追踪
                } else {
                    let _ = tracer.start_phase(&ctx.request_id, "preparation").await;
                }
            }
        }

        // 步骤1: 身份验证 - 验证是哪个用户创建的哪种服务提供商的token
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled {
                let _ = tracer.start_phase(&ctx.request_id, "authentication").await;
            }
        }
        
        let api_key = self.extract_api_key(session)?;
        let user_service_api = self.authenticate_api_key(&api_key).await?;
        ctx.user_service_api = Some(user_service_api.clone());
        
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled {
                let _ = tracer.complete_phase(
                    &ctx.request_id, 
                    "authentication", 
                    true, 
                    Some("User authenticated successfully")
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
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled {
                let _ = tracer.start_phase(&ctx.request_id, "rate_limit_check").await;
            }
        }
        
        self.check_rate_limit(&user_service_api).await?;
        
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled {
                let _ = tracer.complete_phase(
                    &ctx.request_id, 
                    "rate_limit_check", 
                    true, 
                    Some("Rate limit check passed")
                ).await;
            }
        }
        
        tracing::debug!(
            request_id = %ctx.request_id,
            rate_limit = user_service_api.rate_limit,
            "Rate limit check passed"
        );

        // 步骤3: 获取提供商类型信息
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled {
                let _ = tracer.start_phase(&ctx.request_id, "provider_lookup").await;
            }
        }
        
        let provider_type = self.get_provider_type(user_service_api.provider_type_id).await?;
        ctx.provider_type = Some(provider_type.clone());
        ctx.selected_provider = Some(provider_type.name.clone());
        
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled {
                let _ = tracer.complete_phase(
                    &ctx.request_id, 
                    "provider_lookup", 
                    true, 
                    Some(&format!("Provider {} configured", provider_type.name))
                ).await;
            }
        }

        // 步骤4: 根据token查找数据库中配置的转发策略
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled {
                let _ = tracer.start_phase(&ctx.request_id, "backend_selection").await;
            }
        }
        
        let scheduler = self.get_scheduler(&user_service_api.scheduling_strategy)?;
        let selected_backend = scheduler.select_backend(&user_service_api).await?;
        ctx.selected_backend = Some(selected_backend.clone());
        
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled {
                let _ = tracer.complete_phase(
                    &ctx.request_id, 
                    "backend_selection", 
                    true, 
                    Some(&format!("Backend key {} selected", selected_backend.id))
                ).await;
            }
        }

        // 完成准备阶段
        if let Some(tracer) = &self.tracer {
            if ctx.trace_enabled {
                let _ = tracer.complete_phase(
                    &ctx.request_id, 
                    "preparation", 
                    true, 
                    Some("Request preparation completed")
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
    async fn authenticate_api_key(&self, api_key: &str) -> Result<user_service_apis::Model, ProxyError> {
        let cache_key = format!("user_service_api:{}", api_key);
        
        // 首先检查缓存
        if let Ok(Some(user_api)) = self.cache.provider().get::<user_service_apis::Model>(&cache_key).await {
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
        let _ = self.cache.provider().set(&cache_key, &user_api, Some(Duration::from_secs(300))).await;

        tracing::debug!(
            api_key_preview = %self.sanitize_api_key(api_key),
            user_id = user_api.user_id,
            provider_type_id = user_api.provider_type_id,
            "API key authenticated from database"
        );

        Ok(user_api)
    }

    /// 检查速率限制 - 基于统一缓存的滑动窗口算法
    async fn check_rate_limit(&self, user_api: &user_service_apis::Model) -> Result<(), ProxyError> {
        let rate_limit = user_api.rate_limit.unwrap_or(1000); // 默认每分钟1000次
        
        if rate_limit <= 0 {
            return Ok(()); // 无限制
        }

        let cache_key = format!("rate_limit:service_api:{}:minute", user_api.id);
        
        // 使用统一缓存的incr操作实现速率限制
        let current_count = self.cache.provider().incr(&cache_key, 1).await
            .map_err(|e| ProxyError::internal(format!("Cache incr error: {}", e)))?;

        // 如果是第一次请求，设置过期时间
        if current_count == 1 {
            let _ = self.cache.provider().expire(&cache_key, Duration::from_secs(60)).await;
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
    async fn get_provider_type(&self, provider_type_id: i32) -> Result<provider_types::Model, ProxyError> {
        let cache_key = format!("provider_type:{}", provider_type_id);
        
        // 首先检查缓存
        if let Ok(Some(provider_type)) = self.cache.provider().get::<provider_types::Model>(&cache_key).await {
            return Ok(provider_type);
        }

        // 从数据库查询
        let provider_type = ProviderTypes::find_by_id(provider_type_id)
            .one(&*self.db)
            .await
            .map_err(|e| ProxyError::internal(format!("Database error: {}", e)))?
            .ok_or(ProxyError::internal("Provider type not found"))?;

        // 缓存结果（30分钟）
        let _ = self.cache.provider().set(&cache_key, &provider_type, Some(Duration::from_secs(1800))).await;

        Ok(provider_type)
    }

    /// 获取调度器
    fn get_scheduler(&self, strategy: &Option<String>) -> Result<Arc<dyn LoadBalancer>, ProxyError> {
        let strategy_name = strategy.as_deref().unwrap_or("round_robin");
        self.schedulers.get(strategy_name)
    }

    /// 选择上游对等体
    pub async fn select_upstream_peer(&self, ctx: &ProxyContext) -> Result<Box<HttpPeer>, ProxyError> {
        let provider_type = ctx.provider_type.as_ref()
            .ok_or(ProxyError::internal("Provider type not set"))?;

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

        // 创建基础peer
        let mut peer = HttpPeer::new(upstream_addr, true, provider_type.base_url.clone());
        
        // 为Google API配置正确的选项
        if self.should_use_google_api_key_auth(provider_type) {
            if let Some(options) = peer.get_mut_peer_options() {
                // 设置ALPN - 允许HTTP/2和HTTP/1.1协商，优先HTTP/2
                options.alpn = ALPN::H2H1;
                
                // 设置连接超时
                options.connection_timeout = Some(Duration::from_secs(10));
                options.total_connection_timeout = Some(Duration::from_secs(15));
                options.read_timeout = Some(Duration::from_secs(30));
                options.write_timeout = Some(Duration::from_secs(30));
                
                // 设置TLS验证
                options.verify_cert = true;
                options.verify_hostname = true;
                
                // 设置HTTP/2特定选项
                options.h2_ping_interval = Some(Duration::from_secs(30));
                options.max_h2_streams = 100;
                
                tracing::debug!(
                    request_id = %ctx.request_id,
                    provider = %provider_type.name,
                    "Configured peer options for Google API with HTTP/2 support"
                );
            }
        }
        
        Ok(Box::new(peer))
    }

    /// 过滤上游请求 - 替换认证信息和隐藏源信息
    pub async fn filter_upstream_request(
        &self,
        _session: &Session,
        upstream_request: &mut RequestHeader,
        ctx: &ProxyContext,
    ) -> Result<(), ProxyError> {
        let selected_backend = ctx.selected_backend.as_ref()
            .ok_or(ProxyError::internal("Backend not selected"))?;
        let provider_type = ctx.provider_type.as_ref()
            .ok_or(ProxyError::internal("Provider type not set"))?;

        // 根据提供商类型处理认证信息
        upstream_request.remove_header("authorization");
        upstream_request.remove_header("x-goog-api-key");
        let auth_format = provider_type.auth_header_format.as_deref().unwrap_or("Bearer {key}");
        
        // 根据提供商类型选择认证方式
        if self.should_use_google_api_key_auth(provider_type) {
            // Gemini/Google APIs：使用 X-goog-api-key 头部认证
            upstream_request.insert_header("x-goog-api-key", &selected_backend.api_key)
                .map_err(|e| ProxyError::internal(format!("Failed to set x-goog-api-key header: {}", e)))?;
            
            tracing::debug!(
                request_id = %ctx.request_id,
                provider = %provider_type.name,
                base_url = %provider_type.base_url,
                "Using X-goog-api-key authentication for Google API"
            );
        } else {
            // 其他服务商：使用 Authorization 头部认证
            let auth_value = auth_format.replace("{key}", &selected_backend.api_key);
            upstream_request.insert_header("authorization", &auth_value)
                .map_err(|e| ProxyError::internal(format!("Failed to set auth header: {}", e)))?;
            
            tracing::debug!(
                request_id = %ctx.request_id,
                provider = %provider_type.name,
                auth_format = %auth_format,
                "Using Authorization header authentication"
            );
        }

        // 设置正确的Host头 - 只使用域名，不包含协议
        let host_name = provider_type.base_url.replace("https://", "").replace("http://", "");
        upstream_request.insert_header("host", &host_name)
            .map_err(|e| ProxyError::internal(format!("Failed to set host header: {}", e)))?;

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
            upstream_request.insert_header("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36")
                .map_err(|e| ProxyError::internal(format!("Failed to set user-agent: {}", e)))?;
        }

        // 为Google API添加期望的标准头部
        if self.should_use_google_api_key_auth(provider_type) {
            // 确保有Accept头
            if upstream_request.headers.get("accept").is_none() {
                upstream_request.insert_header("accept", "application/json")
                    .map_err(|e| ProxyError::internal(format!("Failed to set accept header: {}", e)))?;
            }
            
            // 确保有Accept-Encoding头
            if upstream_request.headers.get("accept-encoding").is_none() {
                upstream_request.insert_header("accept-encoding", "gzip, deflate")
                    .map_err(|e| ProxyError::internal(format!("Failed to set accept-encoding header: {}", e)))?;
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

    /// 过滤上游响应
    pub async fn filter_upstream_response(
        &self,
        upstream_response: &mut ResponseHeader,
        ctx: &mut ProxyContext,
    ) -> Result<(), ProxyError> {
        // 提取token使用信息
        ctx.tokens_used = self.extract_token_usage(upstream_response);

        // 移除可能暴露上游服务器信息的头部
        upstream_response.remove_header("server");
        upstream_response.remove_header("x-powered-by");
        upstream_response.remove_header("x-ratelimit-limit-requests");
        upstream_response.remove_header("x-ratelimit-limit-tokens");
        upstream_response.remove_header("x-ratelimit-remaining-requests");
        upstream_response.remove_header("x-ratelimit-remaining-tokens");

        // 保持上游服务器标识或使用通用标识
        if upstream_response.headers.get("server").is_none() {
            upstream_response.insert_header("server", "nginx/1.24.0")
                .map_err(|e| ProxyError::internal(format!("Failed to set server header: {}", e)))?;
        }

        tracing::debug!(
            request_id = %ctx.request_id,
            status = upstream_response.status.as_u16(),
            tokens_used = ctx.tokens_used,
            "Upstream response filtered - server info hidden"
        );

        Ok(())
    }

    /// 提取token使用信息
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

    /// 判断提供商是否使用 X-goog-api-key 认证方式
    fn should_use_google_api_key_auth(&self, provider_type: &provider_types::Model) -> bool {
        // 检查提供商名称
        let provider_name = provider_type.name.to_lowercase();
        if provider_name.contains("gemini") || provider_name.contains("google") {
            return true;
        }
        
        // 检查认证头格式配置
        if let Some(auth_format) = &provider_type.auth_header_format {
            let auth_format_lower = auth_format.to_lowercase();
            if auth_format_lower.contains("x-goog-api-key") {
                return true;
            }
        }
        
        // 检查基础 URL
        if provider_type.base_url.contains("googleapis.com") || 
           provider_type.base_url.contains("generativelanguage.googleapis.com") {
            return true;
        }
        
        false
    }



    /// 净化API密钥用于日志记录
    fn sanitize_api_key(&self, api_key: &str) -> String {
        if api_key.len() > 10 {
            format!("{}***{}", &api_key[..4], &api_key[api_key.len() - 4..])
        } else {
            "***".to_string()
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
    pub fn new(
        db: Arc<DatabaseConnection>,
        cache: Arc<UnifiedCacheManager>,
    ) -> Self {
        let mut schedulers: std::collections::HashMap<String, Arc<dyn LoadBalancer>> = std::collections::HashMap::new();

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
        self.schedulers
            .get(strategy)
            .cloned()
            .ok_or_else(|| ProxyError::internal(format!("Unknown scheduling strategy: {}", strategy)))
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
        use sea_orm::{QueryOrder};

        // 获取该用户该服务商的所有活跃API密钥
        let available_keys = UserProviderKeys::find()
            .filter(user_provider_keys::Column::UserId.eq(user_service_api.user_id))
            .filter(user_provider_keys::Column::ProviderTypeId.eq(user_service_api.provider_type_id))
            .filter(user_provider_keys::Column::IsActive.eq(true))
            .order_by_asc(user_provider_keys::Column::Id)
            .all(&*self.db)
            .await
            .map_err(|e| ProxyError::internal(format!("Database error: {}", e)))?;

        if available_keys.is_empty() {
            return Err(ProxyError::bad_gateway("No available API keys"));
        }

        // 从缓存获取当前轮询位置
        let cache_key = format!("round_robin:{}:{}", user_service_api.user_id, user_service_api.provider_type_id);
        let current_index = if let Ok(index) = self.cache.provider().incr(&cache_key, 1).await {
            let _ = self.cache.provider().expire(&cache_key, Duration::from_secs(3600)).await; // 1小时过期
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
            .filter(user_provider_keys::Column::ProviderTypeId.eq(user_service_api.provider_type_id))
            .filter(user_provider_keys::Column::IsActive.eq(true))
            .order_by_asc(user_provider_keys::Column::Id)
            .all(&*self.db)
            .await
            .map_err(|e| ProxyError::internal(format!("Database error: {}", e)))?;

        if available_keys.is_empty() {
            return Err(ProxyError::bad_gateway("No available API keys"));
        }

        // 计算权重总和
        let total_weight: i32 = available_keys.iter()
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
            .filter(user_provider_keys::Column::ProviderTypeId.eq(user_service_api.provider_type_id))
            .filter(user_provider_keys::Column::IsActive.eq(true))
            .order_by_asc(user_provider_keys::Column::Id)
            .all(&*self.db)
            .await
            .map_err(|e| ProxyError::internal(format!("Database error: {}", e)))?;

        if available_keys.is_empty() {
            return Err(ProxyError::bad_gateway("No available API keys"));
        }

        // 简化实现：选择最近使用时间最早的（假设使用频率低的更健康）
        let best_key = available_keys.into_iter()
            .min_by_key(|key| key.last_used.unwrap_or_else(|| {
                // 如果没有使用过，使用创建时间
                key.created_at
            }))
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