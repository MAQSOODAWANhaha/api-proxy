//! # 认证类型定义
//!
//! 定义认证相关的数据结构和常量

use chrono::{DateTime, Utc};
use entity::user_service_apis;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

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
    pub provider_type_id: i32,
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
        self.sub.parse().map_err(|err| {
            crate::error::ProxyError::authentication_with_source("JWT sub 字段解析失败", err)
        })
    }
}

/// 代理端认证结果（原位于 auth/proxy.rs）
#[derive(Debug, Clone)]
pub struct ProxyAuthResult {
    /// 用户服务API信息
    pub user_api: user_service_apis::Model,
    /// 用户ID
    pub user_id: i32,
    /// 服务商类型ID
    pub provider_type_id: i32,
}

/// 认证令牌类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenType {
    /// Bearer 令牌
    Bearer(String),
    /// API 密钥
    ApiKey(String),
    /// 基础认证
    Basic { username: String, password: String },
}

impl TokenType {
    /// 从 Authorization 头解析令牌
    #[must_use]
    pub fn from_auth_header(auth_header: &str) -> Option<Self> {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            Some(Self::Bearer(token.to_string()))
        } else if let Some(encoded) = auth_header.strip_prefix("Basic ") {
            // 解析基础认证
            use base64::{Engine as _, engine::general_purpose};
            if let Ok(decoded) = general_purpose::STANDARD.decode(encoded)
                && let Ok(credentials) = String::from_utf8(decoded)
                && let Some((username, password)) = credentials.split_once(':') {
                    return Some(Self::Basic {
                        username: username.to_string(),
                        password: password.to_string(),
                    });
                }
            None
        } else if auth_header.starts_with("sk-") {
            // 直接的 API 密钥
            Some(Self::ApiKey(auth_header.to_string()))
        } else {
            None
        }
    }

    /// 获取令牌的字符串表示
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Bearer(token) | Self::ApiKey(token) => token,
            Self::Basic { username, .. } => username,
        }
    }
}

/// 速率限制信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitInfo {
    /// 限制类型
    pub limit_type: RateLimitType,
    /// 限制值
    pub limit: u32,
    /// 时间窗口（秒）
    pub window_seconds: u32,
    /// 当前使用量
    pub current_usage: u32,
    /// 重置时间
    pub reset_time: DateTime<Utc>,
}

/// 速率限制类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitType {
    /// 请求数限制
    Requests,
    /// Token 数限制
    Tokens,
    /// 带宽限制
    Bandwidth,
}

/// 认证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// JWT 密钥
    pub jwt_secret: String,
    /// JWT 过期时间（秒）
    pub jwt_expires_in: i64,
    /// 刷新令牌过期时间（秒）
    pub refresh_expires_in: i64,
}

impl AuthConfig {
    /// 开发环境配置
    #[must_use]
    pub fn development() -> Self {
        Self::default()
    }

    /// 测试配置
    #[must_use]
    pub fn test() -> Self {
        Self {
            jwt_secret: "test-secret-key".to_string(),
            jwt_expires_in: 86400,
            refresh_expires_in: 604_800,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "your-secret-key".to_string(),
            jwt_expires_in: 86400,       // 1 天
            refresh_expires_in: 604_800, // 7 天
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

/// `OAuth2授权类型`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuth2GrantType {
    /// 授权码模式
    AuthorizationCode,
    /// 客户端凭据模式
    ClientCredentials,
    /// 刷新令牌
    RefreshToken,
}

impl fmt::Display for OAuth2GrantType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthorizationCode => write!(f, "authorization_code"),
            Self::ClientCredentials => write!(f, "client_credentials"),
            Self::RefreshToken => write!(f, "refresh_token"),
        }
    }
}

/// PKCE代码挑战方法
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PkceMethod {
    /// 纯文本
    Plain,
    /// SHA256哈希
    S256,
}

impl fmt::Display for PkceMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plain => write!(f, "plain"),
            Self::S256 => write!(f, "S256"),
        }
    }
}

/// 认证头格式
/// 多认证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiAuthConfig {
    /// 认证类型
    pub auth_type: AuthType,
    /// 额外配置（JSON格式）
    #[serde(default)]
    pub extra_config: Option<serde_json::Value>,
}

impl MultiAuthConfig {
    /// 创建API密钥认证配置
    #[must_use]
    pub const fn api_key() -> Self {
        Self {
            auth_type: AuthType::ApiKey,
            extra_config: None,
        }
    }

    /// `创建OAuth认证配置`
    #[must_use]
    pub fn oauth(client_id: &str, client_secret: &str, auth_url: &str, token_url: &str) -> Self {
        let mut config = serde_json::Map::new();
        config.insert(
            "client_id".to_string(),
            serde_json::Value::String(client_id.to_string()),
        );
        config.insert(
            "client_secret".to_string(),
            serde_json::Value::String(client_secret.to_string()),
        );
        config.insert(
            "auth_url".to_string(),
            serde_json::Value::String(auth_url.to_string()),
        );
        config.insert(
            "token_url".to_string(),
            serde_json::Value::String(token_url.to_string()),
        );

        Self {
            auth_type: AuthType::OAuth,
            extra_config: Some(serde_json::Value::Object(config)),
        }
    }
}

/// 审计日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// 事件ID
    pub id: String,
    /// 用户ID
    pub user_id: Option<i32>,
    /// 用户名
    pub username: Option<String>,
    /// 事件类型
    pub event_type: AuditEventType,
    /// 资源路径
    pub resource_path: String,
    /// HTTP 方法
    pub method: String,
    /// 客户端 IP
    pub client_ip: Option<String>,
    /// 用户代理
    pub user_agent: Option<String>,
    /// 事件结果
    pub result: AuditResult,
    /// 错误信息
    pub error_message: Option<String>,
    /// 附加元数据
    pub metadata: HashMap<String, serde_json::Value>,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
}

/// 审计事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    /// 登录
    Login,
    /// 登出
    Logout,
    /// API 调用
    ApiCall,
    /// 权限检查
    PermissionCheck,
    /// 配置更改
    ConfigChange,
    /// 用户管理
    UserManagement,
    /// 密钥管理
    KeyManagement,
}

/// 审计结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditResult {
    /// 成功
    Success,
    /// 失败
    Failure,
    /// 权限拒绝
    PermissionDenied,
    /// 速率限制
    RateLimited,
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

        // Basic auth
        use base64::{Engine as _, engine::general_purpose};
        let credentials = general_purpose::STANDARD.encode("user:pass");
        let auth_header = format!("Basic {}", credentials);
        assert_eq!(
            TokenType::from_auth_header(&auth_header),
            Some(TokenType::Basic {
                username: "user".to_string(),
                password: "pass".to_string(),
            })
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

    #[test]
    fn test_multi_auth_config_creation() {
        let api_config = MultiAuthConfig::api_key();
        assert_eq!(api_config.auth_type, AuthType::ApiKey);
        assert_eq!(api_config.extra_config, None);

        let oauth_config = MultiAuthConfig::oauth(
            "client123",
            "secret456",
            "https://auth.example.com",
            "https://token.example.com",
        );
        assert_eq!(oauth_config.auth_type, AuthType::OAuth);
        assert!(oauth_config.extra_config.is_some());
    }
}
