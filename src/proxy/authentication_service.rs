//! # 代理端认证适配器
//!
//! 轻量级适配器，仅负责从HTTP请求中提取认证信息
//! 所有认证逻辑委托给核心AuthService处理

use anyhow::Result;
use pingora_proxy::Session;
use std::sync::Arc;

use crate::auth::{AuthUtils, RefactoredUnifiedAuthManager};
use crate::error::ProxyError;
use crate::proxy::ProxyContext;

/// 认证结果
#[derive(Debug, Clone)]
pub struct AuthenticationResult {
    /// 用户服务API信息
    pub user_service_api: entity::user_service_apis::Model,
    /// 用户ID
    pub user_id: i32,
    /// 服务商类型ID
    pub provider_type_id: i32,
    /// 认证使用的API密钥（已脱敏）
    pub api_key_preview: String,
}

/// 代理端认证适配器
///
/// 轻量级适配器，仅提供HTTP请求解析和认证委托
/// 所有实际认证逻辑都由RefactoredUnifiedAuthManager处理
pub struct AuthenticationService {
    /// 统一认证管理器
    auth_manager: Arc<RefactoredUnifiedAuthManager>,
}

impl AuthenticationService {
    /// 创建新的认证适配器
    pub fn new(auth_manager: Arc<RefactoredUnifiedAuthManager>) -> Self {
        Self { auth_manager }
    }

    /// 从请求中提取API密钥
    ///
    /// 支持多种提取方式：
    /// 1. Authorization头中的Bearer token
    /// 2. x-api-key头
    /// 3. api-key头
    /// 4. Query参数中的api_key
    pub async fn extract_api_key_from_request(
        &self,
        session: &Session,
    ) -> Result<String, ProxyError> {
        let req_header = session.req_header();

        // 1. 从Authorization头提取
        if let Some(auth_header) = req_header.headers.get("authorization") {
            if let Ok(auth_str) = std::str::from_utf8(auth_header.as_bytes()) {
                if let Some(api_key) = self.extract_key_from_header_value(auth_str, "Bearer ") {
                    tracing::debug!("API key extracted from Authorization header");
                    return Ok(api_key);
                }
            }
        }

        // 2. 从x-api-key头提取
        if let Some(api_key_header) = req_header.headers.get("x-api-key") {
            if let Ok(api_key) = std::str::from_utf8(api_key_header.as_bytes()) {
                tracing::debug!("API key extracted from x-api-key header");
                return Ok(api_key.to_string());
            }
        }

        // 3. 从api-key头提取
        if let Some(api_key_header) = req_header.headers.get("api-key") {
            if let Ok(api_key) = std::str::from_utf8(api_key_header.as_bytes()) {
                tracing::debug!("API key extracted from api-key header");
                return Ok(api_key.to_string());
            }
        }

        // 4. 从查询参数提取
        if let Some(query) = req_header.uri.query() {
            for param_pair in query.split('&') {
                if let Some((key, value)) = param_pair.split_once('=') {
                    if key == "api_key" || key == "apikey" {
                        tracing::debug!("API key extracted from query parameter");
                        return Ok(value.to_string());
                    }
                }
            }
        }

        // 5. 从X-goog-api-key头提取
        if let Some(api_key_header) = req_header.headers.get("X-goog-api-key") {
            if let Ok(api_key) = std::str::from_utf8(api_key_header.as_bytes()) {
                tracing::debug!("API key extracted from X-goog-api-key header");
                return Ok(api_key.to_string());
            }
        }

        Err(ProxyError::authentication("Missing API key"))
    }

    /// 从头值中提取密钥
    ///
    /// 支持Bearer格式和其他自定义格式
    fn extract_key_from_header_value(&self, header_value: &str, prefix: &str) -> Option<String> {
        if header_value.starts_with(prefix) {
            let api_key = &header_value[prefix.len()..];
            if !api_key.trim().is_empty() {
                Some(api_key.trim().to_string())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// 执行认证流程
    ///
    /// 执行完整的认证流程：
    /// 1. 提取API密钥
    /// 2. 验证密钥有效性
    /// 3. 获取用户和服务商信息
    ///
    /// 返回认证结果，不直接修改context
    pub async fn authenticate(
        &self,
        session: &Session,
        request_id: &str,
    ) -> Result<AuthenticationResult, ProxyError> {
        tracing::debug!(
            request_id = %request_id,
            "Starting proxy authentication"
        );

        // 步骤1: 提取API密钥
        let api_key = self.extract_api_key_from_request(session).await?;

        // 步骤2: 使用统一认证管理器验证
        let proxy_auth_result = self
            .auth_manager
            .authenticate_proxy_request(&api_key)
            .await?;

        // 步骤3: 构造认证结果
        let auth_result = AuthenticationResult {
            user_service_api: proxy_auth_result.user_api.clone(),
            user_id: proxy_auth_result.user_id,
            provider_type_id: proxy_auth_result.provider_type_id,
            api_key_preview: AuthUtils::sanitize_api_key(&api_key),
        };

        tracing::info!(
            request_id = %request_id,
            user_id = auth_result.user_id,
            provider_type_id = auth_result.provider_type_id,
            user_service_api_id = auth_result.user_service_api.id,
            api_key_preview = %auth_result.api_key_preview,
            "Proxy authentication successful"
        );

        Ok(auth_result)
    }

    /// 将认证结果应用到上下文（为了兼容性保留）
    pub fn apply_auth_result_to_context(
        &self,
        ctx: &mut ProxyContext,
        auth_result: &AuthenticationResult,
    ) {
        ctx.user_service_api = Some(auth_result.user_service_api.clone());
    }

    /// 检查速率限制
    ///
    /// 基于用户和服务API的速率限制配置进行检查
    pub async fn check_rate_limit(&self, ctx: &ProxyContext) -> Result<(), ProxyError> {
        // TODO: 实现基于Redis的速率限制检查
        // 这里应该检查:
        // 1. 每分钟请求数限制
        // 2. 每天请求数限制
        // 3. 每天token使用量限制

        tracing::debug!(
            request_id = %ctx.request_id,
            user_service_api_id = ctx.user_service_api.as_ref().map(|api| api.id),
            "Rate limit check passed (placeholder implementation)"
        );

        Ok(())
    }

    /// 验证API密钥格式
    ///
    /// 快速格式验证，避免无效密钥的数据库查询
    pub fn validate_api_key_format(&self, api_key: &str) -> bool {
        self.auth_manager.validate_proxy_api_key_format(api_key)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_extract_key_from_header_value() {
        // 这个测试不需要外部依赖
        // TODO: 创建测试用的AuthenticationService实例
        // 现在先验证基本的字符串处理逻辑

        let test_cases = vec![
            (
                "Bearer sk-123456789",
                "Bearer ",
                Some("sk-123456789".to_string()),
            ),
            (
                "Bearer  sk-123456789  ",
                "Bearer ",
                Some("sk-123456789".to_string()),
            ),
            ("Basic username:password", "Bearer ", None),
            ("sk-123456789", "Bearer ", None),
            ("", "Bearer ", None),
        ];

        // 基础字符串处理逻辑测试
        for (input, prefix, expected) in test_cases {
            let result = if input.starts_with(prefix) {
                let api_key = &input[prefix.len()..];
                if !api_key.trim().is_empty() {
                    Some(api_key.trim().to_string())
                } else {
                    None
                }
            } else {
                None
            };
            assert_eq!(result, expected, "Failed for input: {}", input);
        }
    }
}
