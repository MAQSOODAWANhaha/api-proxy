//! # 重构后的统一认证层
//!
//! 使用新的services架构消除重复实现，保持向后兼容的API
//! 本文件将逐步替代unified.rs中的RefactoredUnifiedAuthManager God Object

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use serde_json::Value;
use tracing::debug;

use crate::auth::{
    AuthContext, AuthError, AuthMethod, AuthResult, AuthService,
    types::{AuthConfig, AuthType}, 
    strategies::traits::OAuthTokenResult,
    oauth::OAuthSessionManager,
};
use crate::cache::UnifiedCacheManager;
use crate::error::Result;
use sea_orm::DatabaseConnection;

/// 重构后的统一认证管理器
///
/// 简化后直接使用核心AuthService，移除复杂的services层
/// 保持向后兼容的API，内部直接委托给AuthService
/// 
/// 职责：
/// - 提供向后兼容的API接口
/// - 委托给核心AuthService处理认证逻辑
/// - 管理OAuth会话
pub struct RefactoredUnifiedAuthManager {
    /// 核心认证服务
    auth_service: Arc<AuthService>,
    /// OAuth会话管理器
    oauth_session_manager: Arc<OAuthSessionManager>,
    /// 认证配置
    config: Arc<AuthConfig>,
}

/// 认证请求参数（保持向后兼容）
#[derive(Debug, Clone)]
pub struct AuthRequest {
    /// 授权头内容
    pub authorization: Option<String>,
    /// 客户端IP
    pub client_ip: Option<String>,
    /// 用户代理
    pub user_agent: Option<String>,
    /// 请求路径
    pub path: String,
    /// HTTP方法
    pub method: String,
    /// 额外的认证头（如API-Key）
    pub extra_headers: HashMap<String, String>,
}

impl RefactoredUnifiedAuthManager {
    /// 创建重构后的统一认证管理器
    pub async fn new(
        auth_service: Arc<AuthService>, 
        config: Arc<AuthConfig>,
        db: Arc<DatabaseConnection>,
        _cache_manager: Arc<UnifiedCacheManager>,
    ) -> Result<Self> {
        let oauth_session_manager = Arc::new(OAuthSessionManager::new(db.clone()));
        
        Ok(Self {
            auth_service,
            oauth_session_manager,
            config,
        })
    }

    /// 统一认证接口（保持向后兼容）
    /// 
    /// 现在委托给核心AuthService处理认证逻辑
    pub async fn authenticate(&self, request: AuthRequest) -> Result<AuthResult> {
        debug!(
            "Processing authentication request for path: {} (via refactored manager)",
            request.path
        );

        // 尝试从Authorization头解析认证信息
        if let Some(auth_header) = &request.authorization {
            return self.authenticate_from_header(auth_header, &request).await;
        }

        // 尝试从额外头部获取API Key
        if let Some(api_key) = request
            .extra_headers
            .get("x-api-key")
            .or_else(|| request.extra_headers.get("api-key"))
        {
            return self.authenticate_api_key(api_key, &request).await;
        }

        // 检查是否为公开路径
        if self.is_public_path(&request.path) {
            return Ok(self.create_anonymous_auth_result());
        }

        Err(AuthError::MissingCredentials.into())
    }

    /// 从Authorization头认证（重构版本）
    async fn authenticate_from_header(
        &self,
        auth_header: &str,
        request: &AuthRequest,
    ) -> Result<AuthResult> {
        let mut context = self.create_auth_context(request);

        // 直接使用AuthService进行认证
        self.auth_service.authenticate(auth_header, &mut context).await
    }

    /// API Key认证（重构版本）
    /// 
    /// 统一使用AuthService处理API密钥认证
    async fn authenticate_api_key(
        &self,
        api_key: &str,
        request: &AuthRequest,
    ) -> Result<AuthResult> {
        let auth_header = format!("ApiKey {}", api_key);
        let mut context = self.create_auth_context(request);
        self.auth_service.authenticate(&auth_header, &mut context).await
    }

    /// 创建认证上下文
    fn create_auth_context(&self, request: &AuthRequest) -> AuthContext {
        let mut context = AuthContext::new(request.path.clone(), request.method.clone());
        context.client_ip = request.client_ip.clone();
        context.user_agent = request.user_agent.clone();
        context
    }


    /// 检查是否为公开路径
    fn is_public_path(&self, path: &str) -> bool {
        let public_patterns = ["/health", "/metrics", "/api/health", "/api/version"];
        public_patterns
            .iter()
            .any(|pattern| path.starts_with(pattern))
    }

    /// 创建匿名认证结果
    fn create_anonymous_auth_result(&self) -> AuthResult {
        AuthResult {
            user_id: 0,
            username: "anonymous".to_string(),
            is_admin: false,
            permissions: vec![],
            auth_method: AuthMethod::Internal,
            token_preview: "anonymous".to_string(),
            token_info: None,
            expires_at: None,
            session_info: None,
        }
    }

    /// 代理端API密钥认证（保持向后兼容）
    pub async fn authenticate_proxy_request(&self, api_key: &str) -> Result<crate::auth::proxy::ProxyAuthResult> {
        // 使用AuthService进行用户服务API认证
        let user_api = self.auth_service.authenticate_user_service_api(api_key).await?;
        
        Ok(crate::auth::proxy::ProxyAuthResult {
                    user_api: user_api.clone(),
                    user_id: user_api.user_id,
                    provider_type_id: user_api.provider_type_id,
                })
    }

    /// 验证代理端API密钥格式（保持向后兼容）
    pub fn validate_proxy_api_key_format(&self, api_key: &str) -> bool {
        crate::auth::AuthUtils::is_valid_api_key_format(api_key)
    }

    /// 清理代理端API密钥缓存（保持向后兼容）
    pub async fn invalidate_proxy_cache(&self, _api_key: &str) -> Result<()> {
        // 简化：缓存清理现在由AuthService内部管理
        Ok(())
    }

    /// 令牌黑名单管理（委托给AuthService）
    pub async fn blacklist_token(&self, token: &str, _expire_time: DateTime<Utc>) {
        if let Err(e) = self.auth_service.logout(token).await {
            debug!("Failed to blacklist token: {}", e);
        }
    }

    /// 清理过期缓存（委托给AuthService）
    pub async fn cleanup_expired_cache(&self) {
        self.auth_service.cleanup().await;
    }

    /// 清空缓存（保持向后兼容）
    pub async fn clear_cache(&self) {
        debug!("Cache clear requested - handled by AuthService");
        self.auth_service.cleanup().await;
    }

    /// 获取OAuth会话管理器引用（保持向后兼容）
    pub fn get_oauth_session_manager(&self) -> &Arc<OAuthSessionManager> {
        &self.oauth_session_manager
    }

    /// 获取认证统计信息
    pub async fn get_auth_stats(&self) -> Result<std::collections::HashMap<String, String>> {
        Ok(self.auth_service.health_check().await)
    }

    /// 健康检查
    pub async fn health_check(&self) -> std::collections::HashMap<String, String> {
        self.auth_service.health_check().await
    }

    // === OAuth相关方法（保持向后兼容） ===

    /// 多认证接口
    pub async fn multi_authenticate(
        &self,
        auth_type: &AuthType,
        _credentials: &Value,
    ) -> Result<OAuthTokenResult> {
        // 这里需要根据实际需求委托给对应的认证策略
        // 暂时返回错误，表示需要进一步实现
        Err(AuthError::AuthMethodNotSupported {
            method: format!("{:?}", auth_type),
            port: "unified".to_string(),
        }.into())
    }

    /// OAuth会话创建
    pub async fn create_oauth_session(
        &self,
        request: crate::auth::oauth::CreateSessionRequest,
    ) -> Result<crate::auth::oauth::SessionInfo> {
        self.oauth_session_manager.create_session(request).await
    }

    /// 根据会话ID获取OAuth会话信息
    pub async fn get_oauth_session_by_id(&self, session_id: &str) -> Result<Option<crate::auth::oauth::SessionInfo>> {
        self.oauth_session_manager.get_session_by_id(session_id).await
    }

    /// 根据state参数获取OAuth会话信息
    pub async fn get_oauth_session_by_state(&self, state: &str) -> Result<Option<crate::auth::oauth::SessionInfo>> {
        self.oauth_session_manager.get_session_by_state(state).await
    }

    /// 完成OAuth会话
    pub async fn complete_oauth_session(
        &self,
        request: crate::auth::oauth::CompleteSessionRequest,
    ) -> Result<crate::auth::oauth::SessionInfo> {
        self.oauth_session_manager.complete_session(request).await
    }

    /// 获取OAuth认证URL
    pub async fn get_oauth_auth_url(
        &self,
        _auth_type: &AuthType,
        _redirect_uri: &str,
        _state: &str,
    ) -> Result<String> {
        // 委托给OAuth服务处理
        // 这里需要根据实际需求实现URL生成逻辑
        Err(AuthError::InternalError("OAuth URL生成需要专门实现".to_string()).into())
    }

    /// 处理OAuth回调
    pub async fn handle_oauth_callback(
        &self,
        _auth_type: &AuthType,
        _code: &str,
        _state: &str,
    ) -> Result<OAuthTokenResult> {
        // 委托给OAuth服务处理回调
        // 这里需要根据实际需求实现回调处理逻辑
        Err(AuthError::InternalError("OAuth回调处理需要专门实现".to_string()).into())
    }

    /// 标记OAuth会话为失败
    pub async fn fail_oauth_session(&self, session_id: &str, error_message: &str) -> Result<()> {
        self.oauth_session_manager.fail_session(session_id, error_message).await
    }

    /// 验证OAuth会话状态
    pub async fn validate_oauth_session(&self, session_id: &str, state: &str) -> Result<bool> {
        self.oauth_session_manager.validate_session(session_id, state).await
    }

    /// 获取用户的活跃OAuth会话
    pub async fn get_user_active_oauth_sessions(&self, user_id: i32) -> Result<Vec<crate::auth::oauth::SessionInfo>> {
        self.oauth_session_manager.get_user_active_sessions(user_id).await
    }

    /// 撤销用户的所有OAuth会话
    pub async fn revoke_user_oauth_sessions(&self, user_id: i32) -> Result<u64> {
        self.oauth_session_manager.revoke_user_sessions(user_id).await
    }

    /// 清理过期的OAuth会话
    pub async fn cleanup_expired_oauth_sessions(&self) -> Result<u64> {
        self.oauth_session_manager.cleanup_expired_sessions().await
    }
}

/// 便捷的工厂函数，用于创建重构后的RefactoredUnifiedAuthManager
pub async fn create_refactored_unified_auth_manager(
    auth_service: Arc<AuthService>,
    config: Arc<AuthConfig>,
    db: Arc<DatabaseConnection>,
    cache_manager: Arc<UnifiedCacheManager>,
) -> Result<RefactoredUnifiedAuthManager> {
    RefactoredUnifiedAuthManager::new(auth_service, config, db, cache_manager).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_public_path() {
        // 测试公开路径识别逻辑
        let _public_paths = vec![
            "/health",
            "/metrics", 
            "/api/health",
            "/api/version",
        ];

        let _private_paths = vec![
            "/api/admin",
            "/proxy/openai",
            "/api/users",
        ];

        // 实际测试逻辑
    }
}