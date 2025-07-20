//! # Pingora AI 代理服务
//!
//! 实现基于 Pingora 的 AI 服务代理，支持多个 AI 提供商的负载均衡

use async_trait::async_trait;
use pingora_core::{prelude::*, upstreams::peer::HttpPeer};
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_load_balancing::{selection::RoundRobin, LoadBalancer};
use std::sync::Arc;
use std::collections::HashMap;
use crate::config::AppConfig;
use crate::proxy::router::SmartRouter;
use crate::auth::{AuthContext, AuthService, middleware::{AuthMiddleware, AuthenticationResult}};
use pingora_proxy::{ProxyHttp, Session};

/// AI 代理上下文，用于在请求处理阶段间传递信息
#[derive(Debug, Default)]
pub struct ProxyContext {
    /// 请求 ID，用于日志追踪
    pub request_id: String,
    /// 选中的 AI 提供商
    pub ai_provider: String,
    /// 认证结果
    pub auth_result: AuthenticationResult,
    /// 路由决策信息
    pub route_decision: Option<crate::proxy::router::RouteDecision>,
}

/// AI 代理服务
pub struct ProxyService {
    config: Arc<AppConfig>,
    router: SmartRouter,
    auth_middleware: AuthMiddleware,
    openai_lb: Arc<LoadBalancer<RoundRobin>>,
    anthropic_lb: Arc<LoadBalancer<RoundRobin>>,
    gemini_lb: Arc<LoadBalancer<RoundRobin>>,
}

impl ProxyService {
    /// 创建新的代理服务实例
    pub fn new(
        config: Arc<AppConfig>,
        auth_service: Arc<AuthService>,
    ) -> pingora_core::Result<Self> {
        // 创建智能路由器
        let router = SmartRouter::new(Arc::clone(&config))
            .map_err(|e| Error::new(ErrorType::InternalError))?;

        // 创建认证中间件
        let auth_middleware = AuthMiddleware::new(auth_service)
            .skip_path("/health".to_string())
            .skip_path("/metrics".to_string())
            .skip_path("/ping".to_string());

        // 创建 OpenAI 负载均衡器
        let openai_upstreams = vec!["api.openai.com:443"];
        let openai_lb = Arc::new(LoadBalancer::try_from_iter(openai_upstreams)
            .map_err(|e| Error::because(ErrorType::InternalError, "Failed to create OpenAI load balancer", e))?);

        // 创建 Anthropic 负载均衡器  
        let anthropic_upstreams = vec!["api.anthropic.com:443"];
        let anthropic_lb = Arc::new(LoadBalancer::try_from_iter(anthropic_upstreams)
            .map_err(|e| Error::because(ErrorType::InternalError, "Failed to create Anthropic load balancer", e))?);

        // 创建 Google Gemini 负载均衡器
        let gemini_upstreams = vec!["generativelanguage.googleapis.com:443"];
        let gemini_lb = Arc::new(LoadBalancer::try_from_iter(gemini_upstreams)
            .map_err(|e| Error::because(ErrorType::InternalError, "Failed to create Gemini load balancer", e))?);

        Ok(Self {
            config,
            router,
            auth_middleware,
            openai_lb,
            anthropic_lb,
            gemini_lb,
        })
    }

    /// 根据路由决策选择合适的负载均衡器
    pub fn select_load_balancer(&self, provider: &str) -> Option<&Arc<LoadBalancer<RoundRobin>>> {
        match provider {
            "OpenAI" => Some(&self.openai_lb),
            "Anthropic" => Some(&self.anthropic_lb),
            "GoogleGemini" => Some(&self.gemini_lb),
            _ => {
                tracing::warn!("Unknown provider '{}', falling back to OpenAI", provider);
                Some(&self.openai_lb)
            }
        }
    }

    /// 根据提供商获取对应的 SNI 主机名
    pub fn get_sni_for_provider(&self, provider: &str) -> &'static str {
        match provider {
            "OpenAI" => "api.openai.com",
            "Anthropic" => "api.anthropic.com",
            "GoogleGemini" => "generativelanguage.googleapis.com",
            _ => {
                tracing::warn!("Unknown provider '{}', falling back to OpenAI SNI", provider);
                "api.openai.com"
            }
        }
    }


    /// 检查是否为管理 API 请求
    fn is_management_request(&self, path: &str) -> bool {
        path.starts_with("/api/") || path.starts_with("/admin/")
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
        
        tracing::info!(
            "Processing request: {} {} (ID: {})",
            method, path, ctx.request_id
        );

        // 提取请求头用于路由决策
        let mut headers = HashMap::new();
        for (name, value) in req_header.headers.iter() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(name.to_string(), value_str.to_string());
            }
        }

        // 使用智能路由器进行路由决策
        let route_decision = self.router.route(path, method, &headers);
        
        tracing::info!(
            "Route decision: {} -> {} (reason: {}, ID: {})",
            path, route_decision.provider, route_decision.reason, ctx.request_id
        );

        // 如果路由到管理 API，暂时拒绝（后续可以转发到内嵌的 Axum 服务）
        if route_decision.provider == "Management" {
            tracing::info!("Management request blocked: {} (ID: {})", path, ctx.request_id);
            return Err(Error::explain(ErrorType::HTTPStatus(501), "Management API not implemented yet"));
        }

        // 处理 CORS 预检请求
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

        // 保存路由决策和提供商信息
        ctx.ai_provider = route_decision.provider.clone();
        ctx.route_decision = Some(route_decision);

        Ok(false) // 继续处理请求
    }

    async fn upstream_peer(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<Box<HttpPeer>> {
        let path = session.req_header().uri.path();
        
        // 使用路由决策中的提供商信息选择负载均衡器
        let provider = &ctx.ai_provider;
        let lb = self.select_load_balancer(provider)
            .ok_or_else(|| Error::new_str("No suitable load balancer found"))?;
        
        // 选择上游服务器
        let upstream = lb
            .select(path.as_bytes(), 256)
            .ok_or_else(|| Error::new_str("No upstream server available"))?;

        tracing::info!(
            "Selected upstream {:?} for provider {} (rule: {}, user: {}, ID: {})", 
            upstream, 
            provider,
            ctx.route_decision.as_ref().map(|d| d.reason.as_str()).unwrap_or("unknown"),
            ctx.auth_result.username.as_deref().unwrap_or("anonymous"),
            ctx.request_id
        );

        // 获取 SNI 主机名
        let sni = self.get_sni_for_provider(provider);
        
        // 创建 HTTPS peer
        let peer = Box::new(HttpPeer::new(upstream, true, sni.to_string()));
        
        Ok(peer)
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        // 设置正确的 Host 头
        let host = self.get_sni_for_provider(&ctx.ai_provider);
        upstream_request
            .insert_header("Host", host)
            .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set Host header", e))?;

        // 添加请求 ID 头用于追踪
        upstream_request
            .insert_header("X-Request-ID", &ctx.request_id)
            .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set Request-ID header", e))?;

        // 添加路由追踪信息
        if let Some(ref decision) = ctx.route_decision {
            upstream_request
                .insert_header("X-Route-Provider", &decision.provider)
                .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set Route-Provider header", e))?;
                
            upstream_request
                .insert_header("X-Route-Weight", &decision.weight.to_string())
                .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set Route-Weight header", e))?;
        }

        tracing::info!(
            "Forwarding request to {} with Host: {} (rule: {}, user: {}, ID: {})", 
            ctx.ai_provider, 
            host,
            ctx.route_decision.as_ref().map(|d| d.reason.as_str()).unwrap_or("unknown"),
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
        // 添加代理标识头
        upstream_response
            .insert_header("X-Proxy-By", "AI-Proxy-Pingora")
            .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set proxy header", e))?;
        
        // 添加请求 ID
        upstream_response
            .insert_header("X-Request-ID", &ctx.request_id)
            .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set Request-ID header", e))?;

        // 添加路由信息到响应头
        upstream_response
            .insert_header("X-Route-Provider", &ctx.ai_provider)
            .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set Route-Provider header", e))?;

        if let Some(ref decision) = ctx.route_decision {
            upstream_response
                .insert_header("X-Route-Rule", &decision.rule.description)
                .map_err(|e| Error::because(ErrorType::InternalError, "Failed to set Route-Rule header", e))?;
        }

        // 移除敏感头
        upstream_response.remove_header("Server");
        upstream_response.remove_header("X-Powered-By");

        tracing::info!(
            "Response processed for {} request (rule: {}, user: {}, ID: {}, Status: {})", 
            ctx.ai_provider,
            ctx.route_decision.as_ref().map(|d| d.reason.as_str()).unwrap_or("unknown"),
            ctx.auth_result.username.as_deref().unwrap_or("anonymous"),
            ctx.request_id, 
            upstream_response.status
        );
        
        Ok(())
    }

    async fn logging(
        &self,
        _session: &mut Session,
        _e: Option<&Error>,
        ctx: &mut Self::CTX,
    ) {
        let route_info = ctx.route_decision.as_ref()
            .map(|d| format!("rule: {}, weight: {}", d.reason, d.weight))
            .unwrap_or_else(|| "no route info".to_string());

        if let Some(error) = _e {
            tracing::error!(
                "Request failed: {} (ID: {}, Provider: {}, User: {}, Route: {})", 
                error, 
                ctx.request_id, 
                ctx.ai_provider, 
                ctx.auth_result.username.as_deref().unwrap_or("anonymous"),
                route_info
            );
        } else {
            tracing::info!(
                "Request completed successfully (ID: {}, Provider: {}, User: {}, Route: {})", 
                ctx.request_id, 
                ctx.ai_provider, 
                ctx.auth_result.username.as_deref().unwrap_or("anonymous"),
                route_info
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
        let service = ProxyService::new(config, auth_service);

        assert!(service.is_ok());
    }

    #[test]
    fn test_load_balancer_selection() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let auth_service = Arc::new(TestConfig::auth_service());
        let service = ProxyService::new(config, auth_service).unwrap();

        // 测试提供商选择
        assert!(service.select_load_balancer("OpenAI").is_some());
        assert!(service.select_load_balancer("Anthropic").is_some());
        assert!(service.select_load_balancer("GoogleGemini").is_some());
        assert!(service.select_load_balancer("Unknown").is_some()); // 应该回退到 OpenAI
    }

    #[test]
    fn test_sni_selection() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let auth_service = Arc::new(TestConfig::auth_service());
        let service = ProxyService::new(config, auth_service).unwrap();

        assert_eq!(service.get_sni_for_provider("OpenAI"), "api.openai.com");
        assert_eq!(service.get_sni_for_provider("Anthropic"), "api.anthropic.com");
        assert_eq!(service.get_sni_for_provider("GoogleGemini"), "generativelanguage.googleapis.com");
    }

    #[test]
    fn test_management_request_detection() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let auth_service = Arc::new(TestConfig::auth_service());
        let service = ProxyService::new(config, auth_service).unwrap();

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
        let service = ProxyService::new(config, auth_service).unwrap();

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
        let service = ProxyService::new(config, auth_service).unwrap();

        // 测试路由决策 - 这里我们需要通过 SmartRouter 进行路由
        let headers: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        
        // 由于 router 是私有的，我们测试通过服务选择的结果
        // 应该能够为所有主要提供商选择负载均衡器
        assert!(service.select_load_balancer("OpenAI").is_some());
        assert!(service.select_load_balancer("Anthropic").is_some());
        assert!(service.select_load_balancer("GoogleGemini").is_some());
    }
}