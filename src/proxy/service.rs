//! # Pingora AI 代理服务
//!
//! 实现基于 Pingora 的 AI 服务代理，支持多个 AI 提供商的负载均衡

use async_trait::async_trait;
use pingora_core::{prelude::*, upstreams::peer::HttpPeer};
use pingora_http::{RequestHeader, ResponseHeader};
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Instant;
use crate::config::AppConfig;
use crate::proxy::router::SmartRouter;
use crate::proxy::upstream::{UpstreamManager, UpstreamType};
use crate::proxy::forwarding::{RequestForwarder, ForwardingContext, ForwardingConfig, ForwardingResult};
use crate::proxy::statistics::{StatisticsCollector, StatisticsConfig};
use crate::health::HealthCheckService;
use crate::auth::{AuthContext, AuthService, middleware::{AuthMiddleware, AuthenticationResult}};
use crate::providers::{AdapterManager, AdapterRequest, ProviderError};
use pingora_proxy::{ProxyHttp, Session};

/// AI 代理上下文，用于在请求处理阶段间传递信息
#[derive(Debug, Default)]
pub struct ProxyContext {
    /// 请求 ID，用于日志追踪
    pub request_id: String,
    /// 选中的上游类型
    pub upstream_type: Option<UpstreamType>,
    /// 选中的服务器地址
    pub selected_server: Option<String>,
    /// 认证结果
    pub auth_result: AuthenticationResult,
    /// 路由决策信息
    pub route_decision: Option<crate::proxy::router::RouteDecision>,
    /// 请求开始时间
    pub request_start: Option<Instant>,
    /// 适配器处理的请求
    pub adapter_request: Option<AdapterRequest>,
    /// 转发上下文
    pub forwarding_context: Option<ForwardingContext>,
    /// 转发结果
    pub forwarding_result: Option<ForwardingResult>,
}

/// AI 代理服务
pub struct ProxyService {
    config: Arc<AppConfig>,
    router: SmartRouter,
    auth_middleware: AuthMiddleware,
    upstream_manager: Arc<UpstreamManager>,
    adapter_manager: Arc<AdapterManager>,
    request_forwarder: RequestForwarder,
    statistics_collector: Arc<StatisticsCollector>,
}

impl ProxyService {
    /// 创建新的代理服务实例
    pub fn new(
        config: Arc<AppConfig>,
        auth_service: Arc<AuthService>,
        health_service: Arc<HealthCheckService>,
    ) -> pingora_core::Result<Self> {
        // 创建智能路由器
        let router = SmartRouter::new(Arc::clone(&config))
            .map_err(|_e| Error::new(ErrorType::InternalError))?;

        // 创建认证中间件
        let auth_middleware = AuthMiddleware::new(auth_service)
            .skip_path("/health".to_string())
            .skip_path("/metrics".to_string())
            .skip_path("/ping".to_string());

        // 创建上游管理器
        let upstream_manager = Arc::new(UpstreamManager::new(Arc::clone(&config)));

        // 创建适配器管理器
        let adapter_manager = Arc::new(AdapterManager::new());

        // 创建统计收集器
        let statistics_collector = Arc::new(StatisticsCollector::new(StatisticsConfig::default()));

        // 创建请求转发器
        let request_forwarder = RequestForwarder::new(
            Arc::clone(&upstream_manager),
            health_service,
            Arc::clone(&adapter_manager),
            ForwardingConfig::default(),
        );

        Ok(Self {
            config,
            router,
            auth_middleware,
            upstream_manager,
            adapter_manager,
            request_forwarder,
            statistics_collector,
        })
    }

    /// 选择上游服务器
    fn select_upstream_server(&self, upstream_type: &UpstreamType) -> pingora_core::Result<crate::proxy::upstream::UpstreamServer> {
        self.upstream_manager.select_upstream(upstream_type)
            .map_err(|e| Error::because(ErrorType::InternalError, "Failed to select upstream server", Box::new(e)))
    }

    /// 处理适配器请求
    fn process_adapter_request(&self, session: &Session, ctx: &mut ProxyContext) -> pingora_core::Result<()> {
        let req_header = session.req_header();
        let path = req_header.uri.path();
        let method = req_header.method.as_str();

        // 提取请求头
        let mut headers = HashMap::new();
        for (name, value) in req_header.headers.iter() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(name.to_string(), value_str.to_string());
            }
        }

        // 创建适配器请求
        let mut adapter_request = AdapterRequest::new(method, path);
        adapter_request.headers = headers;

        // 检测上游类型
        if let Some(upstream_type) = self.adapter_manager.detect_upstream_type(path) {
            ctx.upstream_type = Some(upstream_type.clone());

            // 处理请求
            match self.adapter_manager.process_request(&upstream_type, adapter_request) {
                Ok(processed_request) => {
                    ctx.adapter_request = Some(processed_request);
                    Ok(())
                }
                Err(ProviderError::AuthenticationFailed(msg)) => {
                    tracing::warn!("Authentication failed: {}", msg);
                    Err(Error::explain(ErrorType::HTTPStatus(401), msg))
                }
                Err(ProviderError::InvalidRequest(msg)) => {
                    tracing::warn!("Invalid request: {}", msg);
                    Err(Error::new(ErrorType::InvalidHTTPHeader))
                }
                Err(e) => {
                    tracing::error!("Adapter error: {}", e);
                    Err(Error::new(ErrorType::InternalError))
                }
            }
        } else {
            tracing::warn!("No adapter found for path: {}", path);
            Err(Error::new(ErrorType::InvalidHTTPHeader))
        }
    }


    /// 检查是否为管理 API 请求
    fn is_management_request(&self, path: &str) -> bool {
        path.starts_with("/api/") || path.starts_with("/admin/")
    }

    /// 创建转发上下文
    fn create_forwarding_context(
        &self,
        session: &Session,
        ctx: &ProxyContext,
    ) -> ForwardingContext {
        let mut forwarding_ctx = ForwardingContext::new(
            ctx.request_id.clone(),
            ctx.upstream_type.clone().unwrap_or(UpstreamType::OpenAI),
        );

        // 设置用户信息
        if let Some(ref username) = ctx.auth_result.username {
            forwarding_ctx = forwarding_ctx.with_user_id(username.clone());
        }

        // 设置客户端IP
        if let Some(client_addr) = session.client_addr() {
            forwarding_ctx = forwarding_ctx.with_client_ip(format!("{:?}", client_addr));
        }

        // 设置适配器请求
        if let Some(ref adapter_request) = ctx.adapter_request {
            forwarding_ctx = forwarding_ctx.with_adapter_request(adapter_request.clone());
        }

        forwarding_ctx
    }

    /// 获取统计信息
    pub async fn get_statistics(&self) -> crate::proxy::statistics::StatsSummary {
        self.statistics_collector.get_stats_summary().await
    }

    /// 重置统计信息
    pub async fn reset_statistics(&self) -> pingora_core::Result<()> {
        self.statistics_collector.reset_all_stats().await
            .map_err(|_| Error::new(ErrorType::InternalError))?;
        Ok(())
    }
}

#[async_trait]
impl ProxyHttp for ProxyService {
    type CTX = ProxyContext;

    fn new_ctx(&self) -> Self::CTX {
        ProxyContext {
            request_id: format!("req_{}", fastrand::u64(..)),
            ..Default::default()
        }
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<bool> {
        let req_header = session.req_header();
        let path = req_header.uri.path();
        let method = req_header.method.as_str();
        
        ctx.request_start = Some(Instant::now());
        
        tracing::info!(
            "Processing request: {} {} (ID: {})",
            method, path, ctx.request_id
        );

        // 检查是否为管理API请求
        if self.is_management_request(path) {
            tracing::info!("Management request blocked: {} (ID: {})", path, ctx.request_id);
            return Err(Error::explain(ErrorType::HTTPStatus(501), "Management API not implemented yet"));
        }

        // 处理CORS预检请求
        if method == "OPTIONS" {
            return Err(Error::explain(ErrorType::HTTPStatus(200), "CORS preflight"));
        }

        // 检查是否需要跳过认证
        if self.auth_middleware.should_skip_auth(path) {
            tracing::debug!("Skipping authentication for path: {} (ID: {})", path, ctx.request_id);
            ctx.auth_result = AuthenticationResult::default();
        } else {
            // 执行认证
            if let Some(auth_header) = req_header.headers.get("Authorization") {
                if let Ok(auth_str) = auth_header.to_str() {
                    // 创建认证上下文
                    let mut auth_context = AuthContext {
                        auth_result: None,
                        resource_path: path.to_string(),
                        method: method.to_string(),
                        client_ip: session.client_addr()
                            .map(|addr| format!("{:?}", addr)),
                        user_agent: req_header.headers.get("User-Agent")
                            .and_then(|h| h.to_str().ok())
                            .map(|s| s.to_string()),
                    };

                    // 执行认证
                    match self.auth_middleware.auth_service().authenticate(auth_str, &mut auth_context).await {
                        Ok(auth_result) => {
                            // 执行授权检查
                            match self.auth_middleware.auth_service().authorize(&auth_result, &auth_context).await {
                                Ok(_) => {
                                    ctx.auth_result = auth_result.into();
                                    tracing::debug!(
                                        "Authentication and authorization successful for user: {} (ID: {})", 
                                        ctx.auth_result.username.as_deref().unwrap_or("unknown"),
                                        ctx.request_id
                                    );
                                }
                                Err(auth_error) => {
                                    tracing::warn!("Authorization failed: {} (ID: {})", auth_error, ctx.request_id);
                                    return Err(Error::explain(ErrorType::HTTPStatus(403), "Access denied"));
                                }
                            }
                        }
                        Err(auth_error) => {
                            tracing::warn!("Authentication failed: {} (ID: {})", auth_error, ctx.request_id);
                            return Err(Error::explain(ErrorType::HTTPStatus(401), "Authentication failed"));
                        }
                    }
                } else {
                    tracing::warn!("Invalid authorization header (ID: {})", ctx.request_id);
                    return Err(Error::explain(ErrorType::HTTPStatus(401), "Invalid authorization header"));
                }
            } else {
                tracing::warn!("Missing Authorization header (ID: {})", ctx.request_id);
                return Err(Error::explain(ErrorType::HTTPStatus(401), "Missing Authorization header"));
            }
        }

        // 处理适配器请求
        self.process_adapter_request(session, ctx)?;

        // 创建转发上下文
        let forwarding_context = self.create_forwarding_context(session, ctx);
        ctx.forwarding_context = Some(forwarding_context);

        Ok(false) // 继续处理请求
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<Box<HttpPeer>> {
        let _path = session.req_header().uri.path();
        
        // 获取上游类型
        let upstream_type = ctx.upstream_type.as_ref()
            .ok_or_else(|| Error::new_str("No upstream type detected"))?;
        
        // 选择上游服务器
        let server = self.select_upstream_server(upstream_type)?;
        
        // 保存选中的服务器地址用于统计
        ctx.selected_server = Some(server.address());

        tracing::info!(
            "Selected upstream {} for type {:?} (user: {}, ID: {})", 
            server.address(),
            upstream_type,
            ctx.auth_result.username.as_deref().unwrap_or("anonymous"),
            ctx.request_id
        );

        // 创建HttpPeer
        let sni = server.host.clone();
        let peer = Box::new(HttpPeer::new(&server.address(), server.use_tls, sni));
        
        Ok(peer)
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        // 从适配器请求中应用修改
        if let Some(ref adapter_req) = ctx.adapter_request {
            // 更新请求头
            for (name, value) in &adapter_req.headers {
                upstream_request
                    .insert_header(name.clone(), value.clone())
                    .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set adapter header", e))?;
            }

            // 更新路径
            if adapter_req.path != upstream_request.uri.path() {
                // 在实际实现中可能需要重写URI
                tracing::debug!("Path rewrite: {} -> {}", upstream_request.uri.path(), adapter_req.path);
            }
        }

        // 添加请求ID头用于追踪
        upstream_request
            .insert_header("X-Request-ID", &ctx.request_id)
            .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set Request-ID header", e))?;

        // 添加上游类型信息
        if let Some(ref upstream_type) = ctx.upstream_type {
            upstream_request
                .insert_header("X-Upstream-Type", &format!("{:?}", upstream_type))
                .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set Upstream-Type header", e))?;
        }

        // 添加选中的服务器信息
        if let Some(ref server) = ctx.selected_server {
            upstream_request
                .insert_header("X-Selected-Server", server)
                .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set Selected-Server header", e))?;
        }

        tracing::info!(
            "Forwarding request to {:?} server {} (user: {}, ID: {})", 
            ctx.upstream_type.as_ref().unwrap_or(&UpstreamType::OpenAI),
            ctx.selected_server.as_deref().unwrap_or("unknown"),
            ctx.auth_result.username.as_deref().unwrap_or("anonymous"),
            ctx.request_id
        );
        
        Ok(())
    }

    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        // 记录响应时间
        let response_time = ctx.request_start.map(|start| start.elapsed());

        // 添加代理标识头
        upstream_response
            .insert_header("X-Proxy-By", "AI-Proxy-Pingora")
            .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set proxy header", e))?;
        
        // 添加请求ID
        upstream_response
            .insert_header("X-Request-ID", &ctx.request_id)
            .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set Request-ID header", e))?;

        // 添加上游类型信息
        if let Some(ref upstream_type) = ctx.upstream_type {
            upstream_response
                .insert_header("X-Upstream-Type", &format!("{:?}", upstream_type))
                .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set Upstream-Type header", e))?;
        }

        // 添加选中的服务器信息
        if let Some(ref server) = ctx.selected_server {
            upstream_response
                .insert_header("X-Selected-Server", server)
                .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set Selected-Server header", e))?;
        }

        // 移除敏感头
        upstream_response.remove_header("Server");
        upstream_response.remove_header("X-Powered-By");

        // 创建转发结果并收集统计
        if let Some(ref forwarding_context) = ctx.forwarding_context {
            let forwarding_result = ForwardingResult {
                success: upstream_response.status.is_success(),
                response_time: response_time.unwrap_or_default(),
                status_code: Some(upstream_response.status.as_u16()),
                error_message: if upstream_response.status.is_success() { None } else { 
                    Some(format!("HTTP {}", upstream_response.status.as_u16()))
                },
                retry_count: 0, // 这里应该从转发上下文获取
                bytes_transferred: 0, // 这里应该计算实际传输的字节数
                upstream_server: ctx.selected_server.clone(),
            };

            // 更新转发结果到上下文
            ctx.forwarding_result = Some(forwarding_result.clone());

            // 使用请求转发器处理响应
            if let Err(e) = self.request_forwarder.process_response(
                upstream_response, 
                forwarding_context, 
                &forwarding_result
            ).await {
                tracing::error!("Failed to process response: {}", e);
            }

            // 收集统计信息
            if let Err(e) = self.statistics_collector.record_request_completion(
                forwarding_context, 
                &forwarding_result
            ).await {
                tracing::error!("Failed to record statistics: {}", e);
            }
        }

        // 记录成功或失败统计到上游管理器
        if let (Some(upstream_type), Some(server)) = (&ctx.upstream_type, &ctx.selected_server) {
            if upstream_response.status.is_success() {
                if let Some(duration) = response_time {
                    self.upstream_manager.record_success(upstream_type, server, duration);
                }
            } else {
                self.upstream_manager.record_failure(upstream_type, server);
            }
        }

        tracing::info!(
            "Response processed for {:?} request (server: {}, user: {}, ID: {}, Status: {}, Duration: {:?})", 
            ctx.upstream_type.as_ref().unwrap_or(&UpstreamType::OpenAI),
            ctx.selected_server.as_deref().unwrap_or("unknown"),
            ctx.auth_result.username.as_deref().unwrap_or("anonymous"),
            ctx.request_id, 
            upstream_response.status,
            response_time
        );
        
        Ok(())
    }

    async fn logging(
        &self,
        _session: &mut Session,
        _e: Option<&Error>,
        ctx: &mut Self::CTX,
    ) {
        let upstream_info = ctx.upstream_type.as_ref()
            .map(|t| format!("type: {:?}", t))
            .unwrap_or_else(|| "no upstream info".to_string());

        let server_info = ctx.selected_server.as_deref().unwrap_or("unknown");
        let duration = ctx.request_start.map(|start| start.elapsed());

        // 记录失败统计
        if let Some(error) = _e {
            if let (Some(upstream_type), Some(server)) = (&ctx.upstream_type, &ctx.selected_server) {
                self.upstream_manager.record_failure(upstream_type, server);
            }

            tracing::error!(
                "Request failed: {} (ID: {}, Upstream: {}, Server: {}, User: {}, Duration: {:?})", 
                error, 
                ctx.request_id, 
                upstream_info,
                server_info,
                ctx.auth_result.username.as_deref().unwrap_or("anonymous"),
                duration
            );
        } else {
            tracing::info!(
                "Request completed successfully (ID: {}, Upstream: {}, Server: {}, User: {}, Duration: {:?})", 
                ctx.request_id, 
                upstream_info,
                server_info,
                ctx.auth_result.username.as_deref().unwrap_or("anonymous"),
                duration
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fixtures::TestConfig;
    use crate::testing::helpers::init_test_env;

    #[test]
    fn test_proxy_service_creation() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let auth_service = Arc::new(TestConfig::auth_service());
        let health_service = Arc::new(crate::health::HealthCheckService::new(None));
        let service = ProxyService::new(config, auth_service, health_service);

        assert!(service.is_ok());
    }

    #[test]
    fn test_upstream_server_selection() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let auth_service = Arc::new(TestConfig::auth_service());
        let health_service = Arc::new(crate::health::HealthCheckService::new(None));
        let service = ProxyService::new(config, auth_service, health_service).unwrap();

        // 测试上游服务器选择
        assert!(service.select_upstream_server(&UpstreamType::OpenAI).is_ok());
        assert!(service.select_upstream_server(&UpstreamType::Anthropic).is_ok());
        assert!(service.select_upstream_server(&UpstreamType::GoogleGemini).is_ok());
    }

    #[test]
    fn test_adapter_manager_integration() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let auth_service = Arc::new(TestConfig::auth_service());
        let health_service = Arc::new(crate::health::HealthCheckService::new(None));
        let service = ProxyService::new(config, auth_service, health_service).unwrap();

        // 测试适配器检测
        assert!(service.adapter_manager.supports_endpoint(&UpstreamType::OpenAI, "/v1/chat/completions"));
        assert!(!service.adapter_manager.supports_endpoint(&UpstreamType::OpenAI, "/unknown/endpoint"));
    }

    #[test]
    fn test_management_request_detection() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let auth_service = Arc::new(TestConfig::auth_service());
        let health_service = Arc::new(crate::health::HealthCheckService::new(None));
        let service = ProxyService::new(config, auth_service, health_service).unwrap();

        assert!(service.is_management_request("/api/users"));
        assert!(service.is_management_request("/admin/dashboard"));
        assert!(!service.is_management_request("/health")); // health 不再是管理请求，而是跳过认证的路径
        assert!(!service.is_management_request("/v1/chat/completions"));
    }

    #[test]
    fn test_auth_middleware_integration() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let auth_service = Arc::new(TestConfig::auth_service());
        let health_service = Arc::new(crate::health::HealthCheckService::new(None));
        let service = ProxyService::new(config, auth_service, health_service).unwrap();

        // 测试跳过认证的路径
        assert!(service.auth_middleware.should_skip_auth("/health"));
        assert!(service.auth_middleware.should_skip_auth("/metrics"));
        assert!(service.auth_middleware.should_skip_auth("/ping"));
        assert!(!service.auth_middleware.should_skip_auth("/v1/chat/completions"));
    }

    #[test]
    fn test_smart_routing_integration() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let auth_service = Arc::new(TestConfig::auth_service());
        let health_service = Arc::new(crate::health::HealthCheckService::new(None));
        let service = ProxyService::new(config, auth_service, health_service).unwrap();

        // 测试路由决策 - 这里我们需要通过 SmartRouter 进行路由
        let headers: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        
        // 由于 router 是私有的，我们测试通过服务选择的结果
        // 应该能够为所有主要提供商选择负载均衡器
        // TODO: 添加正确的路由测试
        // assert!(service.select_load_balancer("OpenAI").is_some());
        // assert!(service.select_load_balancer("Anthropic").is_some());
        // assert!(service.select_load_balancer("GoogleGemini").is_some());
    }
}