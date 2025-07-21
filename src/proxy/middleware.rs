//! # 中间件模块
//!
//! 实现各种请求处理中间件

use crate::config::AppConfig;
use crate::error::{ProxyError, Result};
use crate::auth::{
    service::AuthService,
    api_key::ApiKeyManager, 
    jwt::JwtManager,
    types::{AuthConfig, TokenType},
    AuthContext, AuthMethod
};
use async_trait::async_trait;
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::Session;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

/// 中间件 trait
#[async_trait]
pub trait Middleware: Send + Sync {
    /// 请求前处理
    async fn before_request(
        &self,
        session: &mut Session,
        req_header: &mut RequestHeader,
    ) -> Result<bool>;

    /// 响应后处理  
    async fn after_response(
        &self,
        session: &mut Session,
        resp_header: &mut ResponseHeader,
    ) -> Result<()>;

    /// 中间件名称
    fn name(&self) -> &'static str;
}

/// 认证中间件
pub struct AuthMiddleware {
    auth_service: Arc<AuthService>,
    config: Arc<AppConfig>,
}

impl AuthMiddleware {
    pub fn new(auth_service: Arc<AuthService>, config: Arc<AppConfig>) -> Self {
        Self { auth_service, config }
    }

    /// 验证认证令牌
    async fn validate_token(&self, auth_header: &str, client_ip: &str, user_agent: &str) -> Result<bool> {
        // 创建认证上下文
        let mut context = AuthContext {
            request_id: uuid::Uuid::new_v4().to_string(),
            resource_path: "/v1/chat/completions".to_string(), // 默认路径，后续可以从请求中获取
            method: "POST".to_string(),
            client_ip: client_ip.to_string(),
            user_agent: Some(user_agent.to_string()),
            auth_method: None,
            user_id: None,
            username: None,
            additional_data: HashMap::new(),
        };

        match self.auth_service.authenticate(auth_header, &mut context).await {
            Ok(_auth_result) => {
                tracing::debug!("Authentication successful for client: {}", client_ip);
                Ok(true)
            }
            Err(err) => {
                tracing::warn!("Authentication failed for client {}: {}", client_ip, err);
                Ok(false)
            }
        }
    }
}

#[async_trait]
impl Middleware for AuthMiddleware {
    async fn before_request(
        &self,
        session: &mut Session,
        req_header: &mut RequestHeader,
    ) -> Result<bool> {
        let path = req_header.uri.path();

        // 跳过健康检查和公开端点
        if path == "/health" || path == "/ping" {
            return Ok(false);
        }

        // 获取客户端信息
        let client_ip = session
            .client_addr()
            .map(|addr| addr.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        let user_agent = req_header.headers
            .get("User-Agent")
            .and_then(|ua| ua.to_str().ok())
            .unwrap_or("unknown");

        // 检查 Authorization 头
        if let Some(auth_header) = req_header.headers.get("Authorization") {
            let auth_value = auth_header
                .to_str()
                .map_err(|_| ProxyError::authentication("Invalid Authorization header"))?;

            // 使用实际的认证服务验证
            let is_valid = self.validate_token(auth_value, &client_ip, user_agent).await?;

            if !is_valid {
                // 返回 401 错误
                session.set_status(401).unwrap();
                session
                    .insert_header("Content-Type", "application/json")
                    .unwrap();
                let error_body = r#"{"error":{"message":"Invalid authentication credentials","type":"authentication_error"}}"#;
                session
                    .insert_header("Content-Length", &error_body.len().to_string())
                    .unwrap();
                session
                    .write_response_body(error_body.as_bytes())
                    .await
                    .unwrap();
                return Ok(true); // 终止请求
            }

            // 添加认证信息到请求头（供后续处理使用）
            req_header.insert_header("X-Authenticated", "true").unwrap();
            req_header.insert_header("X-Client-IP", &client_ip).unwrap();
        } else {
            // 缺少认证头
            session.set_status(401).unwrap();
            session
                .insert_header("Content-Type", "application/json")
                .unwrap();
            let error_body = r#"{"error":{"message":"Missing Authorization header","type":"authentication_error"}}"#;
            session
                .insert_header("Content-Length", &error_body.len().to_string())
                .unwrap();
            session
                .write_response_body(error_body.as_bytes())
                .await
                .unwrap();
            return Ok(true); // 终止请求
        }

        Ok(false) // 继续处理
    }

    async fn after_response(
        &self,
        _session: &mut Session,
        resp_header: &mut ResponseHeader,
    ) -> Result<()> {
        // 移除敏感的响应头
        resp_header.remove_header("Server");
        resp_header.remove_header("X-Powered-By");

        Ok(())
    }

    fn name(&self) -> &'static str {
        "AuthMiddleware"
    }
}

/// 速率限制追踪器
#[derive(Debug, Clone)]
struct RateLimitTracker {
    requests_count: i32,
    window_start: DateTime<Utc>,
    window_duration_secs: i64,
    max_requests: i32,
}

impl RateLimitTracker {
    fn new(max_requests: i32, window_duration_secs: i64) -> Self {
        Self {
            requests_count: 0,
            window_start: Utc::now(),
            window_duration_secs,
            max_requests,
        }
    }

    fn is_allowed(&mut self) -> bool {
        let now = Utc::now();
        
        // 检查是否需要重置窗口
        if now.signed_duration_since(self.window_start).num_seconds() >= self.window_duration_secs {
            self.window_start = now;
            self.requests_count = 0;
        }

        // 检查是否超过限制
        if self.requests_count >= self.max_requests {
            false
        } else {
            self.requests_count += 1;
            true
        }
    }

    fn remaining_requests(&self) -> i32 {
        (self.max_requests - self.requests_count).max(0)
    }

    fn time_until_reset(&self) -> i64 {
        let elapsed = Utc::now().signed_duration_since(self.window_start).num_seconds();
        (self.window_duration_secs - elapsed).max(0)
    }
}

/// 速率限制中间件
pub struct RateLimitMiddleware {
    api_key_manager: Arc<ApiKeyManager>,
    config: Arc<AppConfig>,
    // 内存中的速率限制追踪器（生产环境应该使用Redis）
    trackers: Arc<RwLock<HashMap<String, RateLimitTracker>>>,
}

impl RateLimitMiddleware {
    pub fn new(api_key_manager: Arc<ApiKeyManager>, config: Arc<AppConfig>) -> Self {
        Self { 
            api_key_manager,
            config,
            trackers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 检查速率限制
    async fn check_rate_limit(&self, client_id: &str, auth_header: Option<&str>) -> Result<(bool, Option<i32>, Option<i64>)> {
        // 如果有API密钥，使用数据库中的限制设置
        if let Some(auth) = auth_header {
            if auth.starts_with("Bearer sk-") {
                let api_key = &auth[7..]; // 移除 "Bearer "
                
                match self.api_key_manager.check_rate_limit(api_key, 1).await {
                    Ok(rate_limit_status) => {
                        return Ok((
                            rate_limit_status.allowed,
                            rate_limit_status.remaining_requests,
                            Some(rate_limit_status.reset_time.timestamp()),
                        ));
                    }
                    Err(err) => {
                        tracing::warn!("Failed to check API key rate limit: {}", err);
                        // 降级到基于IP的限制
                    }
                }
            }
        }

        // 基于IP的速率限制（默认配置）
        let max_requests_per_minute = 100; // 可以从配置中读取
        let window_duration = 60; // 60秒窗口

        let mut trackers = self.trackers.write().await;
        let tracker = trackers
            .entry(client_id.to_string())
            .or_insert_with(|| RateLimitTracker::new(max_requests_per_minute, window_duration));

        let allowed = tracker.is_allowed();
        let remaining = tracker.remaining_requests();
        let reset_time = tracker.time_until_reset();

        tracing::debug!(
            "Rate limit check for client {}: allowed={}, remaining={}, reset_in={}s",
            client_id, allowed, remaining, reset_time
        );

        Ok((allowed, Some(remaining), Some(reset_time)))
    }

    /// 清理过期的追踪器
    pub async fn cleanup_expired_trackers(&self) {
        let mut trackers = self.trackers.write().await;
        let now = Utc::now();
        
        trackers.retain(|_, tracker| {
            now.signed_duration_since(tracker.window_start).num_seconds() < tracker.window_duration_secs * 2
        });
    }
}

#[async_trait]
impl Middleware for RateLimitMiddleware {
    async fn before_request(
        &self,
        session: &mut Session,
        req_header: &mut RequestHeader,
    ) -> Result<bool> {
        // 获取客户端标识和认证信息
        let client_ip = session
            .client_addr()
            .map(|addr| addr.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let auth_header = req_header.headers
            .get("Authorization")
            .and_then(|auth| auth.to_str().ok());

        let client_id = if let Some(auth) = auth_header {
            // 使用认证信息作为客户端标识
            if auth.starts_with("Bearer sk-") {
                format!("api_key:{}", &auth[7..20]) // 使用API密钥前缀
            } else {
                format!("jwt:{}", client_ip) // JWT用户按IP区分
            }
        } else {
            format!("ip:{}", client_ip) // 匿名用户按IP区分
        };

        match self.check_rate_limit(&client_id, auth_header).await? {
            (false, remaining, Some(reset_time)) => {
                // 返回 429 错误
                session.set_status(429).unwrap();
                session
                    .insert_header("Content-Type", "application/json")
                    .unwrap();
                session.insert_header("Retry-After", &reset_time.to_string()).unwrap();
                
                // 添加速率限制信息到响应头
                if let Some(remaining) = remaining {
                    session.insert_header("X-RateLimit-Remaining", &remaining.to_string()).unwrap();
                }
                session.insert_header("X-RateLimit-Reset", &reset_time.to_string()).unwrap();
                
                let error_body = r#"{"error":{"message":"Rate limit exceeded","type":"rate_limit_error"}}"#;
                session
                    .insert_header("Content-Length", &error_body.len().to_string())
                    .unwrap();
                session
                    .write_response_body(error_body.as_bytes())
                    .await
                    .unwrap();
                return Ok(true); // 终止请求
            }
            (true, remaining, reset_time) => {
                // 添加速率限制信息到请求头，供后续中间件使用
                if let Some(remaining) = remaining {
                    req_header.insert_header("X-RateLimit-Remaining", &remaining.to_string()).unwrap();
                }
                if let Some(reset_time) = reset_time {
                    req_header.insert_header("X-RateLimit-Reset", &reset_time.to_string()).unwrap();
                }
            }
            (false, remaining, None) => {
                // 没有重置时间信息，使用默认值
                session.set_status(429).unwrap();
                session
                    .insert_header("Content-Type", "application/json")
                    .unwrap();
                session.insert_header("Retry-After", "60").unwrap();
                
                if let Some(remaining) = remaining {
                    session.insert_header("X-RateLimit-Remaining", &remaining.to_string()).unwrap();
                }
                
                let error_body = r#"{"error":{"message":"Rate limit exceeded","type":"rate_limit_error"}}"#;
                session
                    .insert_header("Content-Length", &error_body.len().to_string())
                    .unwrap();
                session
                    .write_response_body(error_body.as_bytes())
                    .await
                    .unwrap();
                return Ok(true); // 终止请求
            }
        }

        Ok(false) // 继续处理
    }

    async fn after_response(
        &self,
        _session: &mut Session,
        resp_header: &mut ResponseHeader,
    ) -> Result<()> {
        // 速率限制信息已经在 before_request 中添加到请求头
        // 这里可以添加一些通用的速率限制响应头
        if resp_header.headers.get("X-RateLimit-Remaining").is_none() {
            resp_header
                .insert_header("X-RateLimit-Remaining", "N/A")
                .unwrap();
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "RateLimitMiddleware"
    }
}

/// 日志中间件
pub struct LoggingMiddleware {
    config: Arc<AppConfig>,
}

impl LoggingMiddleware {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn before_request(
        &self,
        session: &mut Session,
        req_header: &mut RequestHeader,
    ) -> Result<bool> {
        let start_time = std::time::Instant::now();

        // 记录请求开始
        tracing::info!(
            "Request started: {} {} from {}",
            req_header.method,
            req_header.uri.path(),
            session
                .client_addr()
                .map(|a| a.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        );

        // 在会话中存储开始时间（如果 Pingora 支持的话）
        req_header
            .insert_header(
                "X-Request-Start-Time",
                &start_time.elapsed().as_millis().to_string(),
            )
            .unwrap();

        Ok(false) // 继续处理
    }

    async fn after_response(
        &self,
        session: &mut Session,
        resp_header: &mut ResponseHeader,
    ) -> Result<()> {
        // 记录响应完成
        tracing::info!(
            "Request completed: status={} client={}",
            resp_header.status,
            session
                .client_addr()
                .map(|a| a.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        );

        Ok(())
    }

    fn name(&self) -> &'static str {
        "LoggingMiddleware"
    }
}

/// 中间件链
pub struct MiddlewareChain {
    middlewares: Vec<Box<dyn Middleware>>,
}

impl MiddlewareChain {
    /// 创建新的中间件链
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    /// 添加中间件
    pub fn add_middleware(mut self, middleware: Box<dyn Middleware>) -> Self {
        tracing::info!("Added middleware: {}", middleware.name());
        self.middlewares.push(middleware);
        self
    }

    /// 执行请求前中间件
    pub async fn before_request(
        &self,
        session: &mut Session,
        req_header: &mut RequestHeader,
    ) -> Result<bool> {
        for middleware in &self.middlewares {
            if middleware.before_request(session, req_header).await? {
                // 如果中间件返回 true，终止处理
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// 执行响应后中间件
    pub async fn after_response(
        &self,
        session: &mut Session,
        resp_header: &mut ResponseHeader,
    ) -> Result<()> {
        // 反向执行响应中间件
        for middleware in self.middlewares.iter().rev() {
            middleware.after_response(session, resp_header).await?;
        }
        Ok(())
    }

    /// 获取中间件数量
    pub fn len(&self) -> usize {
        self.middlewares.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.middlewares.is_empty()
    }
}

impl Default for MiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

/// 创建默认中间件链
pub fn create_default_middleware_chain(
    config: Arc<AppConfig>,
    auth_service: Arc<AuthService>,
    api_key_manager: Arc<ApiKeyManager>,
) -> MiddlewareChain {
    MiddlewareChain::new()
        .add_middleware(Box::new(LoggingMiddleware::new(Arc::clone(&config))))
        .add_middleware(Box::new(RateLimitMiddleware::new(
            Arc::clone(&api_key_manager),
            Arc::clone(&config)
        )))
        .add_middleware(Box::new(AuthMiddleware::new(auth_service, config)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fixtures::TestConfig;
    use crate::testing::helpers::init_test_env;

    // 注释掉需要实际数据库连接的测试
    // 这些测试需要完整的认证服务和数据库设置
    /*
    #[test]
    fn test_middleware_chain_creation() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        // 需要模拟 auth_service 和 api_key_manager
        // let chain = create_default_middleware_chain(config, auth_service, api_key_manager);

        // assert_eq!(chain.len(), 3); // LoggingMiddleware + RateLimitMiddleware + AuthMiddleware
        // assert!(!chain.is_empty());
    }
    */

    #[test]
    fn test_logging_middleware_creation() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let middleware = LoggingMiddleware::new(config);

        assert_eq!(middleware.name(), "LoggingMiddleware");
    }

    #[test]
    fn test_rate_limit_tracker() {
        let mut tracker = RateLimitTracker::new(5, 60); // 5 requests per 60 seconds
        
        // 前5个请求应该被允许
        for _ in 0..5 {
            assert!(tracker.is_allowed());
        }
        
        // 第6个请求应该被拒绝
        assert!(!tracker.is_allowed());
        
        // 检查剩余请求数
        assert_eq!(tracker.remaining_requests(), 0);
    }
}
