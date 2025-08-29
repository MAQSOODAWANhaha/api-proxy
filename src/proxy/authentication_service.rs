//! # 代理端认证适配器
//!
//! 轻量级适配器，仅负责从HTTP请求中提取认证信息
//! 所有认证逻辑委托给核心AuthService处理

use anyhow::Result;
use pingora_proxy::Session;
use std::sync::Arc;

use crate::auth::{AuthHeaderParser, AuthUtils, RefactoredUnifiedAuthManager};
use crate::error::ProxyError;
use crate::proxy::ProxyContext;
use entity;

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

    /// 从请求中提取API密钥（数据库驱动版本）
    ///
    /// 根据provider配置动态解析认证头:
    /// 1. 使用AuthHeaderParser解析provider的auth_header_format
    /// 2. 提取对应的头部名称
    /// 3. 从请求头中查找该头部
    /// 4. 使用fallback逻辑支持query参数
    pub async fn extract_api_key_from_request_with_provider(
        &self,
        session: &Session,
        provider: &entity::provider_types::Model,
    ) -> Result<String, ProxyError> {
        let req_header = session.req_header();

        // 首先尝试使用provider配置的认证头格式
        let header_name = AuthHeaderParser::extract_header_name(&provider.auth_header_format)
            .map_err(|e| ProxyError::authentication(&format!("Invalid auth header format in provider config: {}", e)))?;

        tracing::debug!(
            provider_name = %provider.name,
            auth_header_format = %provider.auth_header_format,
            extracted_header_name = %header_name,
            "Extracted header name from provider auth format"
        );

        // 查找对应的头部
        if let Some(header_value) = req_header.headers.get(&header_name) {
            if let Ok(header_str) = std::str::from_utf8(header_value.as_bytes()) {
                // 使用配置的格式解析API密钥
                return self.extract_key_from_auth_format(&provider.auth_header_format, header_str);
            }
        }

        // Fallback: 尝试从查询参数提取（保持向后兼容）
        if let Some(query) = req_header.uri.query() {
            for param_pair in query.split('&') {
                if let Some((key, value)) = param_pair.split_once('=') {
                    if key == "api_key" || key == "apikey" {
                        tracing::debug!("API key extracted from query parameter (fallback)");
                        return Ok(value.to_string());
                    }
                }
            }
        }

        Err(ProxyError::authentication(&format!(
            "Missing API key for provider '{}'. Expected header: {} with format: {}",
            provider.name, header_name, provider.auth_header_format
        )))
    }

    /// 从请求中提取API密钥（数据库驱动版本）
    pub async fn extract_api_key_from_request(
        &self,
        session: &Session,
        provider: &entity::provider_types::Model,
    ) -> Result<String, ProxyError> {
        self.extract_api_key_from_request_with_provider(session, provider).await
    }

    /// 从认证格式字符串中提取API密钥
    ///
    /// 根据配置的格式反向解析出密钥
    /// 例如：
    /// - format: "Authorization: Bearer {key}", header: "Bearer sk-123" -> "sk-123"
    /// - format: "X-goog-api-key: {key}", header: "AIza_123" -> "AIza_123"
    fn extract_key_from_auth_format(
        &self,
        auth_format: &str,
        header_value: &str,
    ) -> Result<String, ProxyError> {
        // 解析格式以获取模板
        let (_, value_template) = auth_format.split_once(": ")
            .ok_or_else(|| ProxyError::authentication("Invalid auth header format"))?;

        // 如果模板就是 {key}，直接返回整个头部值
        if value_template.trim() == "{key}" {
            return Ok(header_value.to_string());
        }

        // 处理带前缀的情况，如 "Bearer {key}"
        if let Some(prefix) = value_template.strip_suffix("{key}") {
            if let Some(key) = header_value.strip_prefix(prefix) {
                return Ok(key.to_string());
            }
        }

        // 处理带后缀的情况，如 "{key} suffix"
        if let Some(suffix) = value_template.strip_prefix("{key}") {
            if let Some(key) = header_value.strip_suffix(suffix) {
                return Ok(key.to_string());
            }
        }

        // 处理复杂格式，如 "prefix-{key}-suffix"
        // 这里使用简单的字符串替换逻辑
        let pattern_parts: Vec<&str> = value_template.split("{key}").collect();
        if pattern_parts.len() == 2 {
            let prefix = pattern_parts[0];
            let suffix = pattern_parts[1];
            
            if header_value.starts_with(prefix) && header_value.ends_with(suffix) {
                let start_pos = prefix.len();
                let end_pos = header_value.len() - suffix.len();
                if start_pos <= end_pos {
                    return Ok(header_value[start_pos..end_pos].to_string());
                }
            }
        }

        Err(ProxyError::authentication(&format!(
            "Could not extract API key from header value using format: {}",
            auth_format
        )))
    }


    /// 带Provider配置的认证流程（新版本）
    ///
    /// 执行完整的认证流程：
    /// 1. 使用provider配置提取API密钥
    /// 2. 验证密钥有效性
    /// 3. 获取用户和服务商信息
    ///
    /// 返回认证结果，不直接修改context
    pub async fn authenticate_with_provider(
        &self,
        session: &Session,
        request_id: &str,
        provider: &entity::provider_types::Model,
    ) -> Result<AuthenticationResult, ProxyError> {
        tracing::debug!(
            request_id = %request_id,
            provider_name = %provider.name,
            auth_type = %provider.auth_type,
            "Starting proxy authentication with provider config"
        );

        // 步骤1: 使用provider配置提取API密钥
        let api_key = self.extract_api_key_from_request_with_provider(session, provider).await?;

        // 步骤2: 使用统一认证管理器验证
        let proxy_auth_result = self
            .auth_manager
            .authenticate_proxy_request(&api_key)
            .await?;

        // 步骤3: 验证provider类型匹配
        if proxy_auth_result.provider_type_id != provider.id {
            return Err(ProxyError::authentication(&format!(
                "Provider type mismatch: API key belongs to provider_type_id {}, but request is for provider '{}' (id: {})",
                proxy_auth_result.provider_type_id, provider.name, provider.id
            )));
        }

        // 步骤4: 构造认证结果
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
            provider_name = %provider.name,
            user_service_api_id = auth_result.user_service_api.id,
            api_key_preview = %auth_result.api_key_preview,
            "Provider-aware proxy authentication successful"
        );

        Ok(auth_result)
    }

    /// 执行认证流程（数据库驱动版本）
    pub async fn authenticate(
        &self,
        session: &Session,
        request_id: &str,
        provider: &entity::provider_types::Model,
    ) -> Result<AuthenticationResult, ProxyError> {
        self.authenticate_with_provider(session, request_id, provider).await
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
    use super::*;
    
    // TODO: 添加数据库驱动认证的集成测试
    // 需要模拟provider配置和数据库查询
}
