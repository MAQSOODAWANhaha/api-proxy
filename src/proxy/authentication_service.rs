//! # 代理端认证适配器
//!
//! 轻量级适配器，仅负责从HTTP请求中提取认证信息
//! 所有认证逻辑委托给核心AuthService处理

use anyhow::Result;
use pingora_proxy::Session;
use std::sync::Arc;

use crate::auth::{AuthHeaderParser, AuthParseError, AuthUtils, RefactoredUnifiedAuthManager};
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

    /// 解析客户端入站请求中的API密钥（入站认证 - 客户端→代理）
    ///
    /// 根据数据库中provider配置的auth_header_format动态解析客户端认证头:
    /// 1. 支持数组格式的auth_header_format（多种认证头格式）
    /// 2. 遍历所有配置的认证头格式
    /// 3. 从客户端请求头中查找匹配的头部
    /// 4. 使用fallback逻辑支持query参数
    ///
    /// 用途：从客户端HTTP请求中解析用户API密钥
    pub async fn parse_inbound_api_key_from_client(
        &self,
        session: &Session,
        provider: &entity::provider_types::Model,
    ) -> Result<String, ProxyError> {
        let req_header = session.req_header();

        // 尝试从provider配置的所有认证头格式中提取头名称
        let header_names =
            match AuthHeaderParser::extract_header_names_from_array(&provider.auth_header_format) {
                Ok(names) => names,
                Err(_) => {
                    // 如果不是数组格式，尝试作为单一格式解析（向后兼容）
                    match AuthHeaderParser::extract_header_name(&provider.auth_header_format) {
                        Ok(name) => vec![name],
                        Err(e) => {
                            return Err(ProxyError::authentication(&format!(
                                "Invalid auth header format in provider config: {}",
                                e
                            )));
                        }
                    }
                }
            };

        tracing::debug!(
            provider_name = %provider.name,
            auth_header_format = %provider.auth_header_format,
            extracted_header_names = ?header_names,
            "Extracted header names from provider auth format"
        );

        // 遍历所有配置的头名称，查找匹配的认证头
        for header_name in &header_names {
            if let Some(header_value) = req_header.headers.get(header_name) {
                if let Ok(header_str) = std::str::from_utf8(header_value.as_bytes()) {
                    // 尝试从当前头中提取API密钥 - 直接调用底层解析器，使用?操作符自动错误转换
                    match AuthHeaderParser::parse_api_key_from_inbound_headers_smart(
                        &provider.auth_header_format,
                        header_name,
                        header_str,
                    ) {
                        Ok(api_key) => {
                            tracing::debug!(
                                provider_name = %provider.name,
                                header_name = %header_name,
                                "API key extracted from header using unified parsing"
                            );
                            return Ok(api_key);
                        }
                        Err(e) => {
                            tracing::debug!(
                                provider_name = %provider.name,
                                header_name = %header_name,
                                error = %e,
                                "Failed to parse API key from header, trying next header"
                            );
                            // 继续尝试下一个header
                        }
                    }
                }
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
            "Missing API key for provider '{}'. Expected headers: {:?} with format: {}",
            provider.name, header_names, provider.auth_header_format
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
            supported_auth_types = %provider.supported_auth_types,
            "Starting proxy authentication with provider config"
        );

        // 步骤1: 使用provider配置从客户端请求中解析API密钥
        let api_key = self
            .parse_inbound_api_key_from_client(session, provider)
            .await?;

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

    /// 为上游AI服务商构建出站认证头（出站认证 - 代理→AI服务商）
    ///
    /// 根据数据库中provider的auth_header_format配置和内部API密钥构建发送给AI服务商的认证头
    /// 使用相同的auth_header_format配置，但填入内部API密钥发送给上游AI服务商
    ///
    /// 用途：构建发送给AI服务商的HTTP认证头，确保上游服务商收到正确格式的认证信息
    pub fn build_outbound_auth_headers_for_upstream(
        &self,
        provider: &entity::provider_types::Model,
        api_key: &str,
    ) -> Result<Vec<(String, String)>, ProxyError> {
        // 使用智能解析器支持数组和单一格式
        let headers = match AuthHeaderParser::parse_smart(&provider.auth_header_format, api_key) {
            Ok(headers) => headers,
            Err(AuthParseError::InvalidFormat(format)) => {
                return Err(ProxyError::internal(format!(
                    "Invalid authentication header format in database: {}",
                    format
                )));
            }
            Err(e) => {
                return Err(ProxyError::internal(format!(
                    "Authentication header parsing failed: {}",
                    e
                )));
            }
        };

        // 转换为 (name, value) 元组格式
        let mut auth_headers = Vec::new();
        for header in headers {
            auth_headers.push((header.name, header.value));
        }

        tracing::debug!(
            provider_name = %provider.name,
            auth_header_format = %provider.auth_header_format,
            generated_headers = ?auth_headers.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            "Generated authentication headers using unified logic"
        );

        Ok(auth_headers)
    }
}
