//! # AI代理请求处理器
//!
//! 提供纯业务能力（选择上游、过滤请求/响应、统计等）。
//! 认证/追踪/限流等副作用由 ProxyService 统一编排（已移除 Pipeline/Flow 概念）。

use anyhow::Result;
use pingora_core::upstreams::peer::{ALPN, HttpPeer, Peer};
use pingora_core::{Error as PingoraError, ErrorType};
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::Session;
use sea_orm::prelude::Decimal;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use std::sync::Arc;
use std::time::Duration;
use url::form_urlencoded;

use crate::auth::oauth_client::JWTParser;
use crate::auth::rate_limit_dist::DistributedRateLimiter;
use crate::auth::{AuthManager, types::AuthStatus};
use crate::cache::CacheManager;
use crate::config::ProviderConfigManager;
use crate::error::ProxyError;
use crate::logging::LogComponent;
use crate::pricing::PricingCalculatorService;
use crate::proxy::context::ResolvedCredential;
use crate::proxy::{AuthenticationService, ProxyContext, TracingService};
use crate::scheduler::{ApiKeyPoolManager, SelectionContext};
use crate::statistics::service::StatisticsService;
use crate::trace::immediate::ImmediateProxyTracer;
use crate::{proxy_debug, proxy_info, proxy_warn};
use entity::{
    oauth_client_sessions::{self, Entity as OAuthClientSessions},
    provider_types::{self, Entity as ProviderTypes},
    user_provider_keys::{self},
    user_service_apis::{self},
};

/// 请求处理器 - 纯业务实现
///
/// 职责：
/// - 请求/响应过滤与改写
/// - 上游服务选择
/// - 统计数据提取
/// - 构建上游认证头
///
/// 编排（认证/追踪/限流/错误追踪）由 ProxyService 负责
pub struct RequestHandler {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 统一缓存管理器
    cache: Arc<CacheManager>,
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

// RequestDetails / ResponseDetails 类型已迁移到 statistics::types

// SerializableResponseDetails 已合并到 ResponseDetails 的 serde 序列化中

// ResponseDetails 的方法已迁移至 statistics::response 模块的 impl

// Gemini 特定逻辑已迁移至 provider_strategy::GeminiStrategy

// 已统一为 statistics::types::PartialUsage / TokenUsageMetrics

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
    /// 获取认证服务引用（用于外部管道步骤）
    pub fn auth_service(&self) -> &Arc<AuthenticationService> {
        &self.auth_service
    }

    /// 获取数据库连接引用（用于外部管道步骤）
    pub fn db_connection(&self) -> Arc<DatabaseConnection> {
        self.db.clone()
    }

    /// 从 OpenAI access_token 中解析 chatgpt-account-id
    fn extract_chatgpt_account_id(&self, access_token: &str) -> Option<String> {
        let jwt_parser = JWTParser::new().ok()?;
        jwt_parser.extract_chatgpt_account_id(access_token).ok()?
    }
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
        let accept_sse =
            |v: &str| v.contains("text/event-stream") || v.contains("application/stream+json");
        if accept_sse(&accept_downstream) || accept_sse(&accept_upstream) {
            return true;
        }

        // 2) 检查查询参数（alt=sse 或 stream=true）
        if let Some(query) = upstream_request.uri.query() {
            let mut is_sse = false;
            for (k, v) in form_urlencoded::parse(query.as_bytes()) {
                let key = k.to_string().to_ascii_lowercase();
                let val = v.to_string().to_ascii_lowercase();
                if (key == "alt" && val == "sse")
                    || (key == "stream" && (val == "1" || val == "true"))
                {
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

    // JSON 头部工具已迁移到 crate::logging：
    // headers_json_string_request / headers_json_string_response
    /// 获取统计服务的引用 - 用于外部访问
    pub fn statistics_service(&self) -> &Arc<StatisticsService> {
        &self.statistics_service
    }
    /// 获取追踪服务引用
    pub fn tracing_service(&self) -> &Arc<TracingService> {
        &self.tracing_service
    }
    /// 获取数据库连接引用 - 用于外部访问
    pub fn db(&self) -> &Arc<DatabaseConnection> {
        &self.db
    }
    pub fn provider_config_manager(&self) -> &Arc<ProviderConfigManager> {
        &self.provider_config_manager
    }

    /// 创建新的AI代理处理器 - 协调器模式
    ///
    /// 现在RequestHandler作为协调器，将认证、统计和追踪职责委托给专门的服务
    pub fn new(
        db: Arc<DatabaseConnection>,
        cache: Arc<CacheManager>,
        tracer: Option<Arc<ImmediateProxyTracer>>,
        provider_config_manager: Arc<ProviderConfigManager>,
        auth_manager: Arc<AuthManager>,
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
            provider_config_manager,
            api_key_pool,
            auth_service,
            statistics_service,
            tracing_service,
        }
    }

    /// 动态识别Gemini代理模式
    ///
    /// 根据用户密钥配置动态判断应该使用的代理模式：
    /// - OAuth + 无project_id => 路由到 cloudcode-pa.googleapis.com
    /// - OAuth + 有project_id => 路由到 cloudcode-pa.googleapis.com  
    /// - API Key => 路由到 generativelanguage.googleapis.com
    // Gemini 模式识别逻辑已迁移至 provider_strategy::GeminiStrategy

    /// 将project_id注入到API路径中
    ///
    /// 将形如 `/v1/models` 的路径转换为 `/v1/projects/{project_id}/models`
    /// 用于支持Google Cloud Code Assist API的路径格式
    ///
    /// 特殊处理：
    /// - `v1internal:` 路径不需要project_id注入，直接返回原路径
    /// - 标准 `/v1/` 路径会进行project_id注入
    #[allow(dead_code)]
    // Gemini 路径注入逻辑已迁移

    // Gemini Query 参数修改逻辑已迁移

    // Gemini Header 注入逻辑已迁移

    // Gemini 请求体修改逻辑已迁移

    /// Google Code Assist API JSON请求体修改器 (公开方法供service.rs调用)
    ///
    /// 实际修改JSON对象，根据不同路由注入相应的project_id字段
    pub async fn modify_provider_request_body_json(
        &self,
        json_value: &mut serde_json::Value,
        session: &Session,
        ctx: &ProxyContext,
    ) -> Result<bool, crate::error::ProxyError> {
        // 统一入口：由 ProviderStrategy 处理各提供商的 JSON 注入/改写
        if let Some(pt) = &ctx.provider_type {
            if let Some(name) =
                crate::proxy::provider_strategy::ProviderRegistry::match_name(&pt.name)
            {
                if let Some(strategy) =
                    crate::proxy::provider_strategy::make_strategy(name, Some(self.db().clone()))
                {
                    return strategy
                        .modify_request_body_json(session, ctx, json_value)
                        .await;
                }
            }
        }
        Ok(false)
    }

    /// 检查所有限制 - 包括速率限制、每日限制、过期时间等
    pub(crate) async fn check_rate_limit(
        &self,
        user_api: &user_service_apis::Model,
    ) -> Result<(), ProxyError> {
        self.check_rate_limit_with_id(user_api, "unknown").await
    }

    /// 检查所有限制 - 包括速率限制、每日限制、过期时间等（带request_id版本）
    pub(crate) async fn check_rate_limit_with_id(
        &self,
        user_api: &user_service_apis::Model,
        request_id: &str,
    ) -> Result<(), ProxyError> {
        // 1. 检查API过期时间
        if let Some(expires_at) = &user_api.expires_at {
            let now = chrono::Utc::now().naive_utc();
            if now > *expires_at {
                proxy_warn!(
                    request_id,
                    LogStage::Authentication,
                    LogComponent::RequestHandler,
                    "api_expired",
                    "API已过期",
                    user_service_api_id = user_api.id,
                    expires_at = expires_at.format("%Y-%m-%d %H:%M:%S").to_string()
                );
                return Err(ProxyError::rate_limit("API has expired".to_string()));
            }
        }

        // 2. 分布式限流（统一）：每分钟/每天
        let rl = DistributedRateLimiter::new(self.cache.clone());
        let endpoint_key = format!("service_api:{}", user_api.id);

        if let Some(rate_limit) = user_api.max_request_per_min {
            if rate_limit > 0 {
                let out = rl
                    .check_per_minute(user_api.user_id, &endpoint_key, rate_limit as i64)
                    .await
                    .map_err(|e| ProxyError::internal(format!("Rate limiter error: {}", e)))?;
                if !out.allowed {
                    proxy_warn!(
                        request_id,
                        LogStage::Authentication,
                        LogComponent::RequestHandler,
                        "rate_limit_exceeded_per_minute",
                        "每分钟速率限制超出（分布式）",
                        service_api_id = user_api.id,
                        user_id = user_api.user_id,
                        current = out.current,
                        limit = out.limit
                    );
                    return Err(ProxyError::rate_limit(format!(
                        "Rate limit exceeded: {} requests per minute",
                        rate_limit
                    )));
                }
            }
        }

        if let Some(daily_limit) = user_api.max_requests_per_day {
            if daily_limit > 0 {
                let out = rl
                    .check_per_day(user_api.user_id, &endpoint_key, daily_limit as i64)
                    .await
                    .map_err(|e| ProxyError::internal(format!("Rate limiter error: {}", e)))?;
                if !out.allowed {
                    proxy_warn!(
                        request_id,
                        LogStage::Authentication,
                        LogComponent::RequestHandler,
                        "daily_limit_exceeded",
                        "每日请求限制超出（分布式）",
                        service_api_id = user_api.id,
                        user_id = user_api.user_id,
                        current = out.current,
                        limit = out.limit
                    );
                    return Err(ProxyError::rate_limit(format!(
                        "Daily request limit exceeded: {} requests per day",
                        daily_limit
                    )));
                }
            }
        }

        // 4. 检查每日token限制 (基于历史数据预检查)
        if let Some(token_limit) = user_api.max_tokens_per_day {
            if token_limit > 0 {
                self.check_daily_token_limit(user_api.id, token_limit.into(), request_id)
                    .await?;
            }
        }

        // 5. 检查每日成本限制 (基于历史数据预检查)
        if let Some(cost_limit) = user_api.max_cost_per_day {
            if cost_limit > Decimal::ZERO {
                self.check_daily_cost_limit(user_api.id, cost_limit, request_id)
                    .await?;
            }
        }

        Ok(())
    }

    // 统一分布式限流：具体实现见 DistributedRateLimiter；此处不再保留逐函数实现

    /// 检查每日token限制 (基于数据库实际统计)
    async fn check_daily_token_limit(
        &self,
        service_api_id: i32,
        token_limit: i64,
        request_id: &str,
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

        if current_usage >= token_limit {
            proxy_warn!(
                request_id,
                LogStage::Authentication,
                LogComponent::RequestHandler,
                "daily_token_limit_exceeded",
                "每日token限制超出（数据库验证）",
                service_api_id = service_api_id,
                current_usage = current_usage,
                token_limit = token_limit,
                date = today.format("%Y-%m-%d").to_string()
            );

            return Err(ProxyError::rate_limit(format!(
                "Daily token limit exceeded: {} tokens per day (used: {})",
                token_limit, current_usage
            )));
        }

        proxy_debug!(
            request_id,
            LogStage::Authentication,
            LogComponent::RequestHandler,
            "daily_token_check_passed",
            "每日token限制检查通过（数据库验证）",
            service_api_id = service_api_id,
            current_usage = current_usage,
            token_limit = token_limit,
            remaining = token_limit - current_usage,
            date = today.format("%Y-%m-%d").to_string()
        );

        Ok(())
    }

    /// 检查每日成本限制 (基于数据库实际统计)
    async fn check_daily_cost_limit(
        &self,
        service_api_id: i32,
        cost_limit: Decimal,
        request_id: &str,
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
            proxy_warn!(
                request_id,
                LogStage::Authentication,
                LogComponent::RequestHandler,
                "daily_cost_limit_exceeded",
                "每日成本限制超出（数据库验证）",
                service_api_id = service_api_id,
                current_usage = current_usage.to_string(),
                cost_limit = cost_limit.to_string(),
                date = today.format("%Y-%m-%d").to_string()
            );

            return Err(ProxyError::rate_limit(format!(
                "Daily cost limit exceeded: ${} per day (used: ${})",
                cost_limit, current_usage
            )));
        }

        proxy_debug!(
            request_id,
            LogStage::Authentication,
            LogComponent::RequestHandler,
            "daily_cost_check_passed",
            "每日成本限制检查通过（数据库验证）",
            service_api_id = service_api_id,
            current_usage = current_usage.to_string(),
            cost_limit = cost_limit.to_string(),
            remaining = (cost_limit - current_usage).to_string(),
            date = today.format("%Y-%m-%d").to_string()
        );

        Ok(())
    }

    /// 获取提供商类型配置
    pub(crate) async fn get_provider_type(
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

    /// 解析 OAuth 会话，返回 access_token（供 CredentialResolutionStep 使用）
    pub(crate) async fn resolve_oauth_access_token(
        &self,
        session_id: &str,
        request_id: &str,
    ) -> Result<String, ProxyError> {
        proxy_debug!(
            request_id,
            LogStage::Authentication,
            LogComponent::RequestHandler,
            "resolve_oauth_token",
            "解析OAuth访问令牌",
            session_id = session_id
        );

        let oauth_session = OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
            .one(self.db.as_ref())
            .await
            .map_err(|e| {
                ProxyError::database(&format!("Failed to query oauth_client_sessions: {}", e))
            })?;

        let session = oauth_session.ok_or_else(|| {
            ProxyError::authentication(format!("OAuth session not found: {}", session_id))
        })?;

        if session.status != AuthStatus::Authorized.to_string() {
            return Err(ProxyError::authentication(format!(
                "OAuth session {} is not authorized (status: {})",
                session_id, session.status
            )));
        }

        let token = session
            .access_token
            .clone()
            .ok_or_else(|| ProxyError::authentication("OAuth session has no access_token"))?;

        let now = chrono::Utc::now().naive_utc();
        if session.expires_at <= now {
            return Err(ProxyError::authentication(format!(
                "OAuth access token expired at {}",
                session.expires_at
            )));
        }

        proxy_info!(
            request_id,
            LogStage::Authentication,
            LogComponent::RequestHandler,
            "oauth_token_resolved",
            "OAuth访问令牌解析成功",
            session_id = session_id,
            provider_name = session.provider_name,
            expires_at = session.expires_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            access_token = token
        );

        Ok(token)
    }

    /// 根据用户API配置选择合适的API密钥
    pub(crate) async fn select_api_key(
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

        proxy_debug!(
            request_id,
            LogStage::Authentication,
            LogComponent::RequestHandler,
            "api_key_selected",
            "API密钥选择完成（使用ApiKeyPoolManager）",
            user_id = user_service_api.user_id,
            provider_type_id = user_service_api.provider_type_id,
            selected_key_id = result.selected_key.id,
            strategy = result.strategy.as_str(),
            reason = result.reason
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
                return Err(ProxyError::internal("Provider type not set"));
            }
        };

        // 优先由 ProviderStrategy 决定上游地址（便于迁移提供商特定逻辑）
        let mut upstream_addr: Option<String> = None;
        if let Some(name) =
            crate::proxy::provider_strategy::ProviderRegistry::match_name(&provider_type.name)
        {
            if let Some(strategy) =
                crate::proxy::provider_strategy::make_strategy(name, Some(self.db.clone()))
            {
                if let Ok(Some(host)) = strategy.select_upstream_host(ctx).await {
                    upstream_addr = Some(if host.contains(':') {
                        host
                    } else {
                        format!("{}:443", host)
                    });
                }
            }
        }

        // 回退：使用 provider_types.base_url
        let upstream_addr = upstream_addr.unwrap_or_else(|| {
            if provider_type.base_url.contains(':') {
                provider_type.base_url.clone()
            } else {
                format!("{}:443", provider_type.base_url)
            }
        });

        proxy_debug!(
            &ctx.request_id,
            LogStage::UpstreamRequest,
            LogComponent::RequestHandler,
            "upstream_peer_selected",
            "上游节点选择完成",
            upstream = upstream_addr,
            provider = provider_type.name
        );

        // Upstream address no longer stored in simplified trace schema
        proxy_info!(
            &ctx.request_id,
            LogStage::UpstreamRequest,
            LogComponent::RequestHandler,
            "upstream_address_selected",
            "上游地址选择完成（不存储在追踪中）",
            upstream_addr = upstream_addr
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

            proxy_debug!(
                &ctx.request_id,
                LogStage::UpstreamRequest,
                LogComponent::RequestHandler,
                "peer_options_configured",
                "配置通用peer选项（动态超时）",
                provider = provider_type.name,
                provider_id = provider_type.id,
                connection_timeout_s = connection_timeout_secs,
                total_timeout_s = total_timeout_secs,
                read_timeout_s = read_timeout_secs
            );
        } else {
            // 为其他服务商也应用动态超时配置
            if let Some(options) = peer.get_mut_peer_options() {
                options.connection_timeout = Some(Duration::from_secs(connection_timeout_secs));
                options.total_connection_timeout = Some(Duration::from_secs(total_timeout_secs));
                options.read_timeout = Some(Duration::from_secs(read_timeout_secs));
                options.write_timeout = Some(Duration::from_secs(read_timeout_secs));

                tracing::debug!(
                    request_id = ctx.request_id,
                    provider = provider_type.name,
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
            request_id = ctx.request_id,
            method = session.req_header().method.to_string(),
            path = original_path,
            flow = "before_modify_request",
            "修改请求信息前"
        );

        // 先尝试使用可插拔 ProviderStrategy 做最小改写（不改变现有行为）
        if let Some(provider_name) = ctx.provider_type.as_ref().map(|p| p.name.clone()) {
            if let Some(name) =
                crate::proxy::provider_strategy::ProviderRegistry::match_name(&provider_name)
            {
                if let Some(strategy) =
                    crate::proxy::provider_strategy::make_strategy(name, Some(self.db().clone()))
                {
                    // 忽略策略内部的无害改写失败，避免影响主流程
                    if let Err(e) = strategy
                        .modify_request(session, upstream_request, ctx)
                        .await
                    {
                        tracing::debug!(
                            request_id = ctx.request_id,
                            provider = provider_name,
                            error = format!("{:?}", e),
                            "Provider strategy modify_request returned error, continue with default path"
                        );
                    }
                }
            }
        }

        // Gemini 特殊逻辑已迁移到 ProviderStrategy::modify_request（上方已调用）
        if let Some(provider_type) = &ctx.provider_type {
            if !provider_type.name.to_lowercase().contains("gemini") {
                tracing::debug!(
                    request_id = ctx.request_id,
                    original_path = original_path,
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

        // 提取请求中的模型信息
        if let Some(model_name) = self
            .statistics_service
            .extract_model_from_request(session, ctx)
        {
            ctx.requested_model = Some(model_name.clone());
            tracing::info!(
                request_id = ctx.request_id,
                model = model_name,
                extraction_method = "unified_service",
                "Model extracted successfully using StatisticsService"
            );
            // 第一层：即时更新追踪中的模型信息，避免后续阶段丢失
            if let Err(err) = self
                .tracing_service()
                .update_trace_model_info(&ctx.request_id, None, Some(model_name.clone()), None)
                .await
            {
                tracing::warn!(
                    component = "tracing_service",
                    request_id = ctx.request_id,
                    error = %err,
                    "Failed to update trace with extracted model (immediate)"
                );
            }
        }

        // Request size no longer stored in simplified trace schema
        if ctx.request_details.body_size.is_some() {
            tracing::info!(
                request_id = ctx.request_id,
                request_size = ?ctx.request_details.body_size,
                "Request size collected (not stored in trace)"
            );
        }

        let selected_backend = match ctx.selected_backend.as_ref() {
            Some(backend) => backend,
            None => {
                return Err(ProxyError::internal("Backend not selected"));
            }
        };

        let provider_type = match ctx.provider_type.as_ref() {
            Some(provider_type) => provider_type,
            None => {
                return Err(ProxyError::internal("Provider type not set"));
            }
        };

        let client_headers = crate::logging::headers_json_string_request(session.req_header());
        let upstream_headers = crate::logging::headers_json_string_request(upstream_request);

        tracing::debug!(
            request_id = ctx.request_id,
            stage = "before_auth",
            client_headers = client_headers,
            upstream_headers = upstream_headers,
            "Client and upstream headers (before auth)"
        );

        // 使用 CredentialResolutionStep 解析出的凭证构建上游认证头
        match ctx.resolved_credential.clone() {
            Some(ResolvedCredential::ApiKey(api_key)) => {
                // 使用 ProviderStrategy 构建认证头
                let auth_headers = if let Some(name) =
                    crate::proxy::provider_strategy::ProviderRegistry::match_name(
                        &provider_type.name,
                    ) {
                    if let Some(strategy) =
                        crate::proxy::provider_strategy::make_strategy(name, Some(self.db.clone()))
                    {
                        strategy.build_auth_headers(&api_key)
                    } else {
                        // 回退到默认逻辑
                        vec![("Authorization".to_string(), format!("Bearer {}", api_key))]
                    }
                } else {
                    // 回退到默认逻辑
                    vec![("Authorization".to_string(), format!("Bearer {}", api_key))]
                };
                self.clear_auth_headers(upstream_request);
                let mut applied = Vec::new();
                for (header_name, header_value) in &auth_headers {
                    let static_header_name = get_static_header_name(header_name);
                    upstream_request
                        .insert_header(static_header_name, header_value)
                        .map_err(|e| {
                            ProxyError::internal(format!(
                                "Failed to set authentication header '{}': {}",
                                header_name, e
                            ))
                        })?;
                    applied.push(header_name.clone());
                }
                tracing::info!(
                    request_id = ctx.request_id,
                    provider = provider_type.name,
                    auth_type = "api_key",
                    auth_headers = ?applied,
                    api_key = api_key,
                    "Applied API key authentication"
                );
            }
            Some(ResolvedCredential::OAuthAccessToken(access_token)) => {
                self.clear_auth_headers(upstream_request);
                let auth_value = format!("Bearer {}", access_token);
                upstream_request
                    .insert_header("authorization", &auth_value)
                    .map_err(|e| {
                        ProxyError::internal(format!(
                            "Failed to set OAuth authorization header: {}",
                            e
                        ))
                    })?;

                // 对于 OpenAI OAuth token，解析 JWT 获取 chatgpt-account-id
                if provider_type.name == "openai" {
                    if let Some(account_id) = self.extract_chatgpt_account_id(&access_token) {
                        upstream_request
                            .insert_header("chatgpt-account-id", &account_id)
                            .map_err(|e| {
                                ProxyError::internal(format!(
                                    "Failed to set chatgpt-account-id header: {}",
                                    e
                                ))
                            })?;
                        upstream_request
                            .insert_header("host", "chatgpt.com")
                            .map_err(|e| {
                                ProxyError::internal(format!(
                                    "Failed to set host header for ChatGPT: {}",
                                    e
                                ))
                            })?;
                        tracing::debug!(
                            request_id = ctx.request_id,
                            account_id = account_id,
                            "Added ChatGPT specific headers for OpenAI OAuth token"
                        );
                    }
                }

                tracing::info!(
                    request_id = ctx.request_id,
                    provider = provider_type.name,
                    auth_type = "oauth",
                    access_token = access_token,
                    "Applied OAuth access token"
                );
            }
            None => {
                return Err(ProxyError::internal("resolved_credential not set"));
            }
        }

        // 设置正确的Host头 - 只使用域名，不包含协议
        let host_name = provider_type
            .base_url
            .replace("https://", "")
            .replace("http://", "");
        if let Err(e) = upstream_request.insert_header("host", &host_name) {
            let error = ProxyError::internal(format!("Failed to set host header: {}", e));
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
                    request_id = ctx.request_id,
                    "SSE endpoint detected, removed accept-encoding for stability"
                );
            } else if client_supports_compression
                && upstream_request.headers.get("accept-encoding").is_none()
            {
                if let Err(e) = upstream_request.insert_header("accept-encoding", "gzip, deflate") {
                    let error = ProxyError::internal(format!(
                        "Failed to set accept-encoding header: {}",
                        e
                    ));
                    return Err(error);
                }

                tracing::debug!(
                    request_id = ctx.request_id,
                    "Client supports compression, requesting compressed response from upstream"
                );
            } else if !is_sse_endpoint {
                // 客户端不支持压缩，移除任何Accept-Encoding头，确保上游返回未压缩响应
                upstream_request.remove_header("accept-encoding");

                tracing::debug!(
                    request_id = ctx.request_id,
                    client_supports_compression = client_supports_compression,
                    "Client doesn't support compression, requesting uncompressed response from upstream"
                );
            }
        }

        tracing::debug!(
            request_id = ctx.request_id,
            method = upstream_request.method.to_string(),
            final_uri = upstream_request.uri.to_string(),
            flow = "after_auth_replacement",
            "Authentication replacement finished"
        );

        // Content-Length 处理策略：
        // - 对将要修改请求体的路由（如 generateContent/streamGenerateContent/onboardUser），移除原始 Content-Length，避免长度不一致
        // - 对所有流式（SSE）请求，移除 Content-Length，启用 Transfer-Encoding: chunked 来正确处理 HTTP/2 帧传输
        // - 否则若方法为 POST/PUT/PATCH 且缺少 Content-Length/Transfer-Encoding，则显式设置 Content-Length: 0，避免上游 411
        let method_upper = upstream_request.method.to_string().to_uppercase();
        let path_for_len = upstream_request.uri.path().to_string();
        let is_sse_endpoint = self.is_sse_request(session, upstream_request);

        if ctx.will_modify_body || is_sse_endpoint {
            upstream_request.remove_header("content-length");
            tracing::debug!(
                request_id = ctx.request_id,
                path = path_for_len,
                will_modify_body = ctx.will_modify_body,
                is_sse = is_sse_endpoint,
                "将修改请求体或为SSE请求，移除原始 Content-Length 以启用分块传输"
            );
        } else {
            // 优先以下游客户端请求头为准判断是否“无请求体”
            let has_cl_client = session.req_header().headers.get("content-length").is_some();
            let has_te_client = session
                .req_header()
                .headers
                .get("transfer-encoding")
                .is_some();

            // 其次再看当前上游请求头（通常与下游相同，除非我们前面改动过）
            let has_cl = has_cl_client || upstream_request.headers.get("content-length").is_some();
            let has_te =
                has_te_client || upstream_request.headers.get("transfer-encoding").is_some();
            let is_body_method = matches!(method_upper.as_str(), "POST" | "PUT" | "PATCH");
            if is_body_method && !has_cl && !has_te {
                // 上游有些端点（如 cloudcode-pa）要求 Content-Length，即使没有请求体
                if let Err(e) = upstream_request.insert_header("content-length", "0") {
                    let error = ProxyError::internal(format!(
                        "Failed to set content-length: 0 header: {}",
                        e
                    ));
                    return Err(error);
                }
                tracing::debug!(
                    request_id = ctx.request_id,
                    method = method_upper,
                    path = path_for_len,
                    "无请求体路由，显式设置 Content-Length: 0"
                );
            }
        }

        // 精简上游请求日志（去除大体量头部与敏感信息）
        tracing::info!(
            event = "upstream_request_ready",
            component = "proxy.headers",
            request_id = ctx.request_id,
            method = upstream_request.method.to_string(),
            uri = upstream_request.uri.to_string(),
            provider = provider_type.name,
            provider_type_id = provider_type.id,
            backend_key_id = selected_backend.id,
            body = ?String::from_utf8_lossy(&ctx.request_body),
            "上游请求已构建"
        );

        Ok(())
    }

    /// 过滤上游响应 - 协调器模式：委托给专门服务
    pub async fn filter_upstream_response(
        &self,
        _session: &Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut ProxyContext,
    ) -> Result<(), ProxyError> {
        // 记录响应头信息（关键头 + JSON 全量头）
        let response_headers = crate::logging::headers_json_string_response(upstream_response);

        tracing::info!(
            event = "upstream_response_headers",
            component = "proxy.headers",
            request_id = ctx.request_id,
            status_code = upstream_response.status.as_u16(),
            response_headers = response_headers,
            "收到上游响应头"
        );

        // 如果状态码为 4xx/5xx，标记失败阶段（响应体会在后续阶段打印）
        let status_code = upstream_response.status.as_u16();
        if status_code >= 400 {
            tracing::info!(
                event = "fail",
                component = "proxy.headers",
                request_id = ctx.request_id,
                status_code = status_code,
                response_body = ?String::from_utf8_lossy(&ctx.response_body),
                "上游响应失败"
            );
        }

        // 设置响应状态码
        ctx.response_details.status_code = Some(upstream_response.status.as_u16());

        // 收集响应详情 - 委托给StatisticsService
        self.statistics_service
            .collect_response_details(upstream_response, ctx);

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

        // 同步响应头中的关键字段到上下文，便于后续阶段判断
        // 若上游未提供 content-type，使用默认 application/json
        ctx.response_details.content_type = Some(content_type.to_string());
        if let Some(enc) = &content_encoding {
            ctx.response_details.content_encoding = Some(enc.clone());
        } else {
            ctx.response_details.content_encoding = None;
        }

        // 日志记录响应信息
        tracing::info!(
            request_id = ctx.request_id,
            status = upstream_response.status.as_u16(),
            content_type = content_type,
            content_encoding = ?content_encoding,
            content_length = upstream_response.headers.get("content-length")
                .and_then(|v| std::str::from_utf8(v.as_bytes()).ok()),
            "Processing upstream response"
        );

        // ========== 安全头部处理 ==========
        // 只移除可能暴露服务器信息的头部，保留传输相关的核心头部
        let headers_to_remove = ["x-powered-by"];

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
            request_id = ctx.request_id,
            status = upstream_response.status.as_u16(),
            preserved_encoding = ?content_encoding,
            "Upstream response processed successfully"
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
                    request_id = ctx.request_id,
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
                    request_id = ctx.request_id,
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
                    request_id = ctx.request_id,
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
                    request_id = ctx.request_id,
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
                    request_id = ctx.request_id,
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
                    request_id = ctx.request_id,
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
                // 添加 HTTP/2 特定的错误诊断信息
                let error_details =
                    format!("Network error: {:?} - {:?}", error.etype, error.esource);
                let is_h2_protocol = format!("{:?}", error.etype).contains("h2")
                    || format!("{:?}", error.esource).contains("h2");

                tracing::error!(
                    request_id = ctx.request_id,
                    provider = provider_name,
                    error_type = ?error.etype,
                    error_source = ?error.esource,
                    error_details = error_details,
                    is_h2_protocol = is_h2_protocol,
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
