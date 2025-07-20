//! # 中间件模块
//!
//! 实现各种请求处理中间件

use crate::config::AppConfig;
use crate::error::{ProxyError, Result};
use async_trait::async_trait;
use pingora_http::{RequestHeader, ResponseHeader};
use pingora_proxy::Session;
use std::sync::Arc;

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
    config: Arc<AppConfig>,
}

impl AuthMiddleware {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self { config }
    }

    /// 验证 API 密钥
    async fn validate_api_key(&self, api_key: &str) -> Result<bool> {
        // TODO: 实现真正的数据库查询
        // 目前简单检查格式
        if api_key.starts_with("sk-") && api_key.len() >= 20 {
            tracing::debug!("API key validation passed for key: {}...", &api_key[..10]);
            Ok(true)
        } else {
            tracing::warn!("API key validation failed for key: {}", api_key);
            Ok(false)
        }
    }

    /// 验证 JWT 令牌
    async fn validate_jwt(&self, token: &str) -> Result<bool> {
        // TODO: 实现真正的 JWT 验证
        if token.len() > 10 {
            tracing::debug!("JWT validation passed for token: {}...", &token[..10]);
            Ok(true)
        } else {
            tracing::warn!("JWT validation failed");
            Ok(false)
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

        // 检查 Authorization 头
        if let Some(auth_header) = req_header.headers.get("Authorization") {
            let auth_value = auth_header
                .to_str()
                .map_err(|_| ProxyError::authentication("Invalid Authorization header"))?;

            if auth_value.starts_with("Bearer ") {
                let token = &auth_value[7..];

                // 判断是 API 密钥还是 JWT
                let is_valid = if token.starts_with("sk-") {
                    self.validate_api_key(token).await?
                } else {
                    self.validate_jwt(token).await?
                };

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

                // 添加用户信息到请求头（供后续处理使用）
                req_header.insert_header("X-User-Token", token).unwrap();
            } else {
                // 无效的认证格式
                session.set_status(401).unwrap();
                session
                    .insert_header("Content-Type", "application/json")
                    .unwrap();
                let error_body = r#"{"error":{"message":"Invalid authorization format. Use 'Bearer <token>'","type":"authentication_error"}}"#;
                session
                    .insert_header("Content-Length", &error_body.len().to_string())
                    .unwrap();
                session
                    .write_response_body(error_body.as_bytes())
                    .await
                    .unwrap();
                return Ok(true); // 终止请求
            }
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

/// 速率限制中间件
pub struct RateLimitMiddleware {
    config: Arc<AppConfig>,
}

impl RateLimitMiddleware {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self { config }
    }

    /// 检查速率限制
    async fn check_rate_limit(&self, client_id: &str) -> Result<bool> {
        // TODO: 实现真正的速率限制检查（使用 Redis）
        // 目前简单返回通过
        tracing::debug!("Rate limit check for client: {}", client_id);
        Ok(true)
    }
}

#[async_trait]
impl Middleware for RateLimitMiddleware {
    async fn before_request(
        &self,
        session: &mut Session,
        req_header: &mut RequestHeader,
    ) -> Result<bool> {
        // 获取客户端标识（从认证信息或 IP）
        let client_id = if let Some(token_header) = req_header.headers.get("X-User-Token") {
            token_header.to_str().unwrap_or("unknown")
        } else {
            // 从 IP 地址获取
            session
                .client_addr()
                .map(|addr| addr.to_string())
                .as_deref()
                .unwrap_or("unknown")
        };

        if !self.check_rate_limit(client_id).await? {
            // 返回 429 错误
            session.set_status(429).unwrap();
            session
                .insert_header("Content-Type", "application/json")
                .unwrap();
            session.insert_header("Retry-After", "60").unwrap();
            let error_body =
                r#"{"error":{"message":"Rate limit exceeded","type":"rate_limit_error"}}"#;
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
        // 添加速率限制相关的响应头
        resp_header
            .insert_header("X-RateLimit-Remaining", "999")
            .unwrap();
        resp_header
            .insert_header(
                "X-RateLimit-Reset",
                &(chrono::Utc::now().timestamp() + 3600).to_string(),
            )
            .unwrap();

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
pub fn create_default_middleware_chain(config: Arc<AppConfig>) -> MiddlewareChain {
    MiddlewareChain::new()
        .add_middleware(Box::new(LoggingMiddleware::new(Arc::clone(&config))))
        .add_middleware(Box::new(RateLimitMiddleware::new(Arc::clone(&config))))
        .add_middleware(Box::new(AuthMiddleware::new(config)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fixtures::TestConfig;
    use crate::testing::helpers::init_test_env;

    #[test]
    fn test_middleware_chain_creation() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let chain = create_default_middleware_chain(config);

        assert_eq!(chain.len(), 3); // LoggingMiddleware + RateLimitMiddleware + AuthMiddleware
        assert!(!chain.is_empty());
    }

    #[test]
    fn test_auth_middleware_creation() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let middleware = AuthMiddleware::new(config);

        assert_eq!(middleware.name(), "AuthMiddleware");
    }

    #[test]
    fn test_rate_limit_middleware_creation() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let middleware = RateLimitMiddleware::new(config);

        assert_eq!(middleware.name(), "RateLimitMiddleware");
    }

    #[test]
    fn test_logging_middleware_creation() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let middleware = LoggingMiddleware::new(config);

        assert_eq!(middleware.name(), "LoggingMiddleware");
    }

    #[tokio::test]
    async fn test_auth_middleware_api_key_validation() {
        init_test_env();

        let config = Arc::new(TestConfig::app_config());
        let middleware = AuthMiddleware::new(config);

        // 测试有效的 API 密钥
        assert!(middleware
            .validate_api_key("sk-1234567890abcdef12345")
            .await
            .unwrap());

        // 测试无效的 API 密钥
        assert!(!middleware.validate_api_key("invalid-key").await.unwrap());
        assert!(!middleware.validate_api_key("sk-short").await.unwrap());
    }
}
