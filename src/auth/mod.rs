//! # 认证授权模块
//!
//! 提供完整的身份验证和权限控制功能

pub mod api_key;
pub mod header_parser;
pub mod jwt;
pub mod middleware;
pub mod permissions;
pub mod service;
pub mod types;
pub mod unified;

pub use api_key::ApiKeyManager;
pub use header_parser::{AuthHeader, AuthHeaderParser, AuthParseError};
pub use jwt::JwtManager;
pub use middleware::AuthMiddleware;
pub use permissions::{Permission, Role};
pub use service::AuthService;
pub use types::*;
pub use unified::{AuthRequest, CacheStats, UnifiedAuthManager, create_unified_auth_manager};

/// 认证结果
#[derive(Debug, Clone)]
pub struct AuthResult {
    /// 用户ID
    pub user_id: i32,
    /// 用户名
    pub username: String,
    /// 是否为管理员
    pub is_admin: bool,
    /// 权限列表
    pub permissions: Vec<Permission>,
    /// 认证方式
    pub auth_method: AuthMethod,
    /// 原始令牌（脱敏）
    pub token_preview: String,
}

/// 认证方式
#[derive(Debug, Clone, PartialEq)]
pub enum AuthMethod {
    /// API 密钥
    ApiKey,
    /// JWT 令牌
    Jwt,
    /// 基础认证 (用户名/密码)
    BasicAuth,
    /// 内部服务调用
    Internal,
}

/// 认证上下文
#[derive(Debug, Clone, Default)]
pub struct AuthContext {
    /// 认证结果
    pub auth_result: Option<AuthResult>,
    /// 请求的资源路径
    pub resource_path: String,
    /// HTTP 方法
    pub method: String,
    /// 客户端 IP
    pub client_ip: Option<String>,
    /// 用户代理
    pub user_agent: Option<String>,
}

impl AuthContext {
    /// 创建新的认证上下文
    pub fn new(resource_path: String, method: String) -> Self {
        Self {
            auth_result: None,
            resource_path,
            method,
            client_ip: None,
            user_agent: None,
        }
    }

    /// 设置认证结果
    pub fn set_auth_result(&mut self, result: AuthResult) {
        self.auth_result = Some(result);
    }

    /// 检查是否已认证
    pub fn is_authenticated(&self) -> bool {
        self.auth_result.is_some()
    }

    /// 检查是否为管理员
    pub fn is_admin(&self) -> bool {
        self.auth_result
            .as_ref()
            .map(|r| r.is_admin)
            .unwrap_or(false)
    }

    /// 检查是否有特定权限
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.auth_result
            .as_ref()
            .map(|r| r.permissions.contains(permission))
            .unwrap_or(false)
    }

    /// 获取用户ID
    pub fn get_user_id(&self) -> Option<i32> {
        self.auth_result.as_ref().map(|r| r.user_id)
    }

    /// 获取用户名
    pub fn get_username(&self) -> Option<&str> {
        self.auth_result.as_ref().map(|r| r.username.as_str())
    }
}
