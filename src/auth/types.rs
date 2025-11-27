//! # 认证类型定义
//!
//! 定义认证相关的数据结构和常量

use chrono::{DateTime, Utc};
use entity::user_service_apis;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::types::ProviderTypeId;

/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// 用户ID
    pub id: i32,
    /// 用户名
    pub username: String,
    /// 邮箱
    pub email: String,
    /// 是否为管理员
    pub is_admin: bool,
    /// 是否激活
    pub is_active: bool,
    /// 权限列表（现在使用UserRole）
    pub permissions: Vec<crate::auth::permissions::UserRole>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后登录时间
    pub last_login: Option<DateTime<Utc>>,
}

/// API 密钥信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    /// 密钥ID
    pub id: i32,
    /// 用户ID
    pub user_id: i32,
    /// 提供商类型ID
    pub provider_type_id: ProviderTypeId,
    /// 认证类型 (`api_key`, oauth)
    pub auth_type: String,
    /// 密钥名称
    pub name: String,
    /// API 密钥 (通常是脱敏的)
    pub api_key: String,
    /// 权重
    pub weight: Option<i32>,
    /// 每分钟最大请求数
    pub max_requests_per_minute: Option<i32>,
    /// 每分钟最大 Token 提示数
    pub max_tokens_prompt_per_minute: Option<i32>,
    /// 每日最大请求数
    pub max_requests_per_day: Option<i32>,
    /// 是否激活
    pub is_active: bool,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// OAuth 提供商配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProviderConfig {
    pub provider_name: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub authorize_url: String,
    pub token_url: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub pkce_required: bool,
    pub extra_params: HashMap<String, String>,
}

/// JWT 载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// 用户ID
    pub sub: String,
    /// 用户名
    pub username: String,
    /// 是否为管理员
    pub is_admin: bool,
    /// 权限列表
    pub permissions: Vec<String>,
    /// 签发时间
    pub iat: i64,
    /// 过期时间
    pub exp: i64,
    /// 签发者
    pub iss: String,
    /// 受众
    pub aud: String,
    /// JWT ID
    pub jti: String,
}

impl JwtClaims {
    /// 创建新的 JWT 载荷
    #[must_use]
    pub fn new(
        user_id: i32,
        username: String,
        is_admin: bool,
        permissions: Vec<String>,
        expires_in_seconds: i64,
    ) -> Self {
        let now = Utc::now().timestamp();
        Self {
            sub: user_id.to_string(),
            username,
            is_admin,
            permissions,
            iat: now,
            exp: now + expires_in_seconds,
            iss: "ai-proxy".to_string(),
            aud: "ai-proxy-users".to_string(),
            jti: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// 检查 JWT 是否过期
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp() > self.exp
    }

    /// 获取用户ID
    pub fn user_id(&self) -> crate::error::Result<i32> {
        match self.sub.parse::<i32>() {
            Ok(id) => Ok(id),
            Err(err) => Err(crate::error::auth::AuthError::Message(format!(
                "JWT sub 字段解析失败: {err}"
            ))
            .into()),
        }
    }
}

/// 代理端认证结果（原位于 auth/proxy.rs）
#[derive(Debug, Clone)]
pub struct Authentication {
    /// 用户服务API信息
    pub user_api: user_service_apis::Model,
    /// 用户ID
    pub user_id: i32,
    /// 服务商类型ID
    pub provider_type_id: ProviderTypeId,
}

/// 认证方式 - 表示已完成认证的方式（认证结果状态）
///
/// 注意：与 `AuthType` 的区别：
/// - `AuthMethod` 表示请求经过哪种方式完成了认证（结果状态）
/// - `AuthType` 表示认证策略的具体类型（配置输入）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthMethod {
    /// 通过API密钥认证
    ApiKey,
    /// 通过JWT令牌认证
    Jwt,
    /// 通过基础认证（用户名/密码）
    BasicAuth,
    /// 内部服务调用认证
    Internal,
    /// `通过OAuth流程完成认证`
    OAuth,
}

/// `OAuth令牌信息`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌（可选）
    pub refresh_token: Option<String>,
    /// 令牌类型
    pub token_type: String,
    /// 过期时间（秒）
    pub expires_in: Option<i64>,
    /// 作用域
    pub scope: Option<String>,
}

/// 统一认证结果
/// 表示用户认证成功后的完整信息，包括用户身份、角色和可选的令牌信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResult {
    /// 用户ID
    pub user_id: i32,
    /// 用户名
    pub username: String,
    /// 是否为管理员
    pub is_admin: bool,
    /// 用户角色
    pub role: crate::auth::permissions::UserRole,
    /// 认证方式
    pub auth_method: AuthMethod,
    /// 原始令牌（脱敏）
    pub token_preview: String,
    /// OAuth令牌信息（可选，仅当通过OAuth认证时包含）
    pub token_info: Option<TokenInfo>,
    /// 令牌过期时间（可选）
    pub expires_at: Option<DateTime<Utc>>,
    /// 会话信息（可选，主要用于OAuth会话）
    pub session_info: Option<serde_json::Value>,
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

/// 认证令牌类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenType {
    /// Bearer 令牌
    Bearer(String),
    /// API 密钥
    ApiKey(String),
}

impl TokenType {
    /// 从 Authorization 头解析令牌
    #[must_use]
    pub fn from_auth_header(auth_header: &str) -> Option<Self> {
        auth_header.strip_prefix("Bearer ").map_or_else(
            || {
                if auth_header.starts_with("sk-") {
                    Some(Self::ApiKey(auth_header.to_string()))
                } else {
                    None
                }
            },
            |token| Some(Self::Bearer(token.to_string())),
        )
    }
}

/// 认证配置（仅包含时效等非敏感信息）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// JWT 密钥（从环境变量注入，配置文件不保存）
    #[serde(skip)]
    pub jwt_secret: String,
    /// JWT 过期时间（秒）
    pub jwt_expires_in: i64,
    /// 刷新令牌过期时间（秒）
    pub refresh_expires_in: i64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "development-secret-key-change-me-in-production".to_string(),
            jwt_expires_in: 86400,       // 1 天
            refresh_expires_in: 604_800, // 7 天
        }
    }
}

impl AuthConfig {
    /// 从环境变量注入 JWT 密钥
    pub fn load_jwt_secret_from_env(&mut self) -> crate::error::Result<()> {
        let Ok(jwt_secret) = std::env::var("JWT_SECRET") else {
            return Err(crate::error::auth::AuthError::Message(
                "JWT_SECRET environment variable is required for authentication".to_string(),
            )
            .into());
        };

        if jwt_secret.len() < 32 {
            return Err(crate::error::auth::AuthError::Message(format!(
                "JWT_SECRET must be at least 32 characters, got {}",
                jwt_secret.len()
            ))
            .into());
        }

        self.jwt_secret = jwt_secret;
        Ok(())
    }

    /// 测试专用配置
    #[cfg(test)]
    #[must_use]
    pub fn test() -> Self {
        Self {
            jwt_secret: "test-secret-key-for-jwt-testing-only-do-not-use-in-production".to_string(),
            jwt_expires_in: 3600,
            refresh_expires_in: 86400,
        }
    }
}

/// 支持的认证类型 - 表示具体的认证策略类型（配置输入）
///
/// 注意：与 `AuthMethod` 的区别：
/// - `AuthType` 表示认证策略的具体类型，用于配置和策略选择
/// - `AuthMethod` 表示请求经过哪种方式完成了认证（结果状态）
///
/// 例如：`AuthType::GoogleOAuth` 策略执行后，可能产生 `AuthMethod::OAuth` 结果
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    /// API密钥认证策略
    ApiKey,
    /// 统一OAuth认证策略（支持所有OAuth 2.0提供商）
    OAuth,
}

impl fmt::Display for AuthType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey => write!(f, "api_key"),
            Self::OAuth => write!(f, "oauth"),
        }
    }
}

impl AuthType {
    /// 安全解析认证类型字符串，未知类型返回 None
    #[must_use]
    pub fn from(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "api_key" => Some(Self::ApiKey),
            "oauth" => Some(Self::OAuth),
            _ => None,
        }
    }
}

/// 认证状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    /// 待认证
    Pending,
    /// 已授权
    Authorized,
    /// 已过期
    Expired,
    /// 错误状态
    Error,
    /// 已撤销
    Revoked,
}

impl fmt::Display for AuthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Authorized => write!(f, "authorized"),
            Self::Expired => write!(f, "expired"),
            Self::Error => write!(f, "error"),
            Self::Revoked => write!(f, "revoked"),
        }
    }
}

impl From<&str> for AuthStatus {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "authorized" => Self::Authorized,
            "expired" => Self::Expired,
            "error" => Self::Error,
            "revoked" => Self::Revoked,
            _ => Self::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_type_parsing() {
        // Bearer token
        let auth_header = "Bearer abc123";
        assert_eq!(
            TokenType::from_auth_header(auth_header),
            Some(TokenType::Bearer("abc123".to_string()))
        );

        // API key
        let auth_header = "sk-1234567890abcdef";
        assert_eq!(
            TokenType::from_auth_header(auth_header),
            Some(TokenType::ApiKey("sk-1234567890abcdef".to_string()))
        );
    }

    #[test]
    fn test_jwt_claims() {
        let claims = JwtClaims::new(
            1,
            "testuser".to_string(),
            false,
            vec!["use_openai".to_string()],
            3600,
        );

        assert_eq!(claims.user_id().unwrap(), 1);
        assert_eq!(claims.username, "testuser");
        assert!(!claims.is_admin);
        assert!(!claims.is_expired());
    }

    #[test]
    fn test_auth_type_display() {
        assert_eq!(AuthType::ApiKey.to_string(), "api_key");
        assert_eq!(AuthType::OAuth.to_string(), "oauth");
    }

    #[test]
    fn test_auth_type_from() {
        assert_eq!(AuthType::from("api_key"), Some(AuthType::ApiKey));
        assert_eq!(AuthType::from("oauth"), Some(AuthType::OAuth));
        assert_eq!(AuthType::from("API_KEY"), Some(AuthType::ApiKey)); // 测试大小写
        assert_eq!(AuthType::from("unknown"), None); // 未知类型返回 None
    }

    #[test]
    fn test_auth_status_display() {
        assert_eq!(AuthStatus::Pending.to_string(), "pending");
        assert_eq!(AuthStatus::Authorized.to_string(), "authorized");
        assert_eq!(AuthStatus::Expired.to_string(), "expired");
        assert_eq!(AuthStatus::Error.to_string(), "error");
        assert_eq!(AuthStatus::Revoked.to_string(), "revoked");
    }
}
