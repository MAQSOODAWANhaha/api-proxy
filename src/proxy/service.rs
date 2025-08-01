//! # Pingora AI 代理服务
//!
//! 基于设计文档实现的透明AI代理服务，专注身份验证、速率限制和转发策略

use async_trait::async_trait;
use pingora_core::{prelude::*, upstreams::peer::HttpPeer, ErrorType};
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::{ProxyHttp, Session};
use pingora_core::protocols::Digest;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::auth::unified::UnifiedAuthManager;
use crate::proxy::ai_handler::{AIProxyHandler, ProxyContext};
use crate::cache::UnifiedCacheManager;
use crate::trace::{UnifiedTraceSystem, unified::UnifiedProxyTracer};
use sea_orm::DatabaseConnection;

/// AI 代理服务 - 透明代理设计
pub struct ProxyService {
    /// 配置
    config: Arc<AppConfig>,
    /// AI代理处理器
    ai_handler: Arc<AIProxyHandler>,
    /// 统一追踪器
    tracer: Option<Arc<UnifiedProxyTracer>>,
}

impl ProxyService {
    /// 创建新的代理服务实例
    pub fn new(
        config: Arc<AppConfig>,
        db: Arc<DatabaseConnection>,
        cache: Arc<UnifiedCacheManager>,
        auth_manager: Arc<UnifiedAuthManager>,
        trace_system: Option<Arc<UnifiedTraceSystem>>,
    ) -> pingora_core::Result<Self> {
        // 创建调度器注册表
        let schedulers = Arc::new(crate::proxy::ai_handler::SchedulerRegistry::new(
            db.clone(),
            cache.clone(),
        ));

        // 获取追踪器
        let tracer = trace_system.as_ref().map(|ts| ts.tracer());

        // 创建AI代理处理器
        let ai_handler = Arc::new(AIProxyHandler::new(
            db,
            cache,
            config.clone(),
            auth_manager,
            schedulers,
            tracer.clone(),
        ));

        // 保留trace_system引用获取的tracer
        let tracer = trace_system.map(|ts| ts.tracer());

        Ok(Self {
            config,
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
        let mut ctx = ProxyContext {
            request_id: Uuid::new_v4().to_string(),
            start_time: Instant::now(),
            ..Default::default()
        };

        // 设置追踪启用标志（实际追踪将在 request_filter 中开始）
        if let Some(_tracer) = &self.tracer {
            ctx.trace_enabled = true;
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
                    r#"{"error":"Management APIs are available on management port (default: 9090)","code":"WRONG_PORT"}"#
                ));
            } else {
                return Err(Error::explain(
                    ErrorType::HTTPStatus(404),
                    r#"{"error":"Unknown endpoint - this port handles AI proxy requests (any format)","code":"NOT_PROXY_ENDPOINT"}"#
                ));
            }
        }

        // 处理CORS预检请求
        if method == "OPTIONS" {
            return Err(Error::explain(ErrorType::HTTPStatus(200), "CORS preflight"));
        }

        // 使用AI代理处理器进行身份验证、速率限制和转发策略
        match self.ai_handler.prepare_proxy_request(session, ctx).await {
            Ok(_) => {
                tracing::debug!(
                    request_id = %ctx.request_id,
                    "AI proxy request preparation completed successfully - using Pingora upstream"
                );
                
                // Ok(false) pingora 暂时实现有问题
                // 使用reqwest混合实现以对比请求日志
                match self.ai_handler.handle_request_with_reqwest(session, ctx).await {
                    Ok(_) => {
                        tracing::debug!(
                            request_id = %ctx.request_id,
                            "Request handled successfully with reqwest"
                        );
                        Ok(true) // 请求已处理完成，不需要继续
                    }
                    Err(e) => {
                        tracing::error!(
                            request_id = %ctx.request_id,
                            error = %e,
                            "Failed to handle request with reqwest"
                        );
                        Err(Error::explain(ErrorType::HTTPStatus(502), format!("Proxy error: {}", e)))
                    }
                }
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
                        let msg = e.to_string();
                        Err(Error::explain(ErrorType::HTTPStatus(401), msg))
                    }
                    crate::error::ProxyError::RateLimit { .. } => {
                        let msg = e.to_string();
                        Err(Error::explain(ErrorType::HTTPStatus(429), msg))
                    }
                    crate::error::ProxyError::BadGateway { .. } => {
                        let msg = e.to_string();
                        Err(Error::explain(ErrorType::HTTPStatus(502), msg))
                    }
                    _ => {
                        Err(Error::explain(ErrorType::HTTPStatus(500), "Internal server error"))
                    }
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
        self.ai_handler.select_upstream_peer(ctx).await
            .map_err(|_e| Error::new(ErrorType::InternalError))
    }

    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        // 使用AI代理处理器过滤上游请求 - 替换认证信息和隐藏源信息
        self.ai_handler.filter_upstream_request(session, upstream_request, ctx).await
            .map_err(|_e| Error::new(ErrorType::InternalError))
    }

    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        // 使用AI代理处理器过滤上游响应
        self.ai_handler.filter_upstream_response(upstream_response, ctx).await
            .map_err(|_e| Error::new(ErrorType::InternalError))?;

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

        // 完成追踪（如果启用）
        if ctx.trace_enabled {
            if let Some(tracer) = &self.tracer {
                let is_success = status_code < 400;
                if let Err(e) = tracer.complete_trace(
                    &ctx.request_id,
                    status_code,
                    is_success,
                ).await {
                    tracing::warn!(
                        request_id = %ctx.request_id,
                        error = %e,
                        "Failed to complete trace"
                    );
                }
            }
        }
        
        Ok(())
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

    async fn logging(
        &self,
        session: &mut Session,
        e: Option<&Error>,
        ctx: &mut Self::CTX,
    ) {
        let duration = ctx.start_time.elapsed();
        
        if let Some(error) = e {
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
                "AI proxy request failed with detailed context"
            );
        } else {
            tracing::debug!(
                request_id = %ctx.request_id,
                duration_ms = duration.as_millis(),
                tokens_used = ctx.tokens_used,
                "AI proxy request completed successfully"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_detection() {
        let config = Arc::new(crate::config::AppConfig::default());
        let db = Arc::new(sea_orm::DatabaseConnection::default());
        
        // 创建内存缓存管理器用于测试
        let cache_config = crate::config::CacheConfig {
            cache_type: crate::config::CacheType::Memory,
            memory_max_entries: 1000,
            default_ttl: 300,
            enabled: true,
        };
        let cache = Arc::new(UnifiedCacheManager::new(&cache_config, "").unwrap());
        let auth_manager = Arc::new(crate::auth::unified::UnifiedAuthManager::default());
        
        let service = ProxyService::new(config, db, cache, auth_manager).unwrap();

        // 测试代理请求检测
        assert!(service.is_proxy_request("/v1/chat/completions"));
        assert!(service.is_proxy_request("/proxy/openai/models"));
        assert!(!service.is_proxy_request("/api/health"));
        assert!(!service.is_proxy_request("/admin/dashboard"));
        
        // 测试管理请求检测
        assert!(service.is_management_request("/api/users"));
        assert!(service.is_management_request("/admin/dashboard"));
        assert!(service.is_management_request("/"));
        assert!(!service.is_management_request("/v1/chat/completions"));
    }
}