//! # 认证类型定义
//!
//! 定义认证相关的数据结构和常量

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    /// 权限列表
    pub permissions: Vec<crate::auth::permissions::Permission>,
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
    /// 密钥名称
    pub name: String,
    /// API 密钥 (通常是脱敏的)
    pub api_key: String,
    /// 权重
    pub weight: Option<i32>,
    /// 每分钟最大请求数
    pub max_requests_per_minute: Option<i32>,
    /// 每日最大 Token 数
    pub max_tokens_per_day: Option<i32>,
    /// 今日已使用 Token 数
    pub used_tokens_today: Option<i32>,
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
    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp() > self.exp
    }

    /// 获取用户ID
    pub fn user_id(&self) -> Result<i32, std::num::ParseIntError> {
        self.sub.parse()
    }
}

/// 认证令牌类型
#[derive(Debug, Clone, PartialEq)]
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
    pub fn from_auth_header(auth_header: &str) -> Option<Self> {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            Some(TokenType::Bearer(token.to_string()))
        } else if let Some(encoded) = auth_header.strip_prefix("Basic ") {
            // 解析基础认证
            use base64::{Engine as _, engine::general_purpose};
            if let Ok(decoded) = general_purpose::STANDARD.decode(encoded) {
                if let Ok(credentials) = String::from_utf8(decoded) {
                    if let Some((username, password)) = credentials.split_once(':') {
                        return Some(TokenType::Basic {
                            username: username.to_string(),
                            password: password.to_string(),
                        });
                    }
                }
            }
            None
        } else if auth_header.starts_with("sk-") {
            // 直接的 API 密钥
            Some(TokenType::ApiKey(auth_header.to_string()))
        } else {
            None
        }
    }

    /// 获取令牌的字符串表示
    pub fn as_str(&self) -> &str {
        match self {
            TokenType::Bearer(token) | TokenType::ApiKey(token) => token,
            TokenType::Basic { username, .. } => username,
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
    /// 是否启用速率限制
    pub enable_rate_limiting: bool,
    /// 默认请求速率限制
    pub default_request_rate_limit: u32,
    /// 默认 Token 速率限制
    pub default_token_rate_limit: u32,
    /// 密码最小长度
    pub min_password_length: usize,
    /// 是否要求密码包含数字
    pub require_password_numbers: bool,
    /// 是否要求密码包含特殊字符
    pub require_password_special_chars: bool,
    /// 会话超时时间（秒）
    pub session_timeout: i64,
    /// 最大登录尝试次数
    pub max_login_attempts: u32,
    /// 登录锁定时间（秒）
    pub login_lockout_duration: i64,
    /// 认证缓存TTL（分钟）
    pub cache_ttl_minutes: u32,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "your-secret-key".to_string(),
            jwt_expires_in: 3600,       // 1 小时
            refresh_expires_in: 604800, // 7 天
            enable_rate_limiting: true,
            default_request_rate_limit: 100,  // 每分钟 100 请求
            default_token_rate_limit: 100000, // 每日 100k tokens
            min_password_length: 8,
            require_password_numbers: true,
            require_password_special_chars: true,
            session_timeout: 3600, // 1 小时
            max_login_attempts: 5,
            login_lockout_duration: 900, // 15 分钟
            cache_ttl_minutes: 10, // 10 分钟缓存
        }
    }
}

/// 认证错误类型
#[derive(Debug, Clone, PartialEq)]
pub enum AuthError {
    /// 无效令牌
    InvalidToken,
    /// 令牌过期
    TokenExpired,
    /// 权限不足
    InsufficientPermissions,
    /// 用户未找到
    UserNotFound,
    /// 密码错误
    InvalidPassword,
    /// 账户被锁定
    AccountLocked,
    /// 账户未激活
    AccountInactive,
    /// 速率限制超出
    RateLimitExceeded,
    /// 缺少认证凭据
    MissingCredentials,
    /// 无效的认证凭据
    InvalidCredentials,
    /// 令牌已被加入黑名单
    TokenBlacklisted,
    /// 内部错误
    InternalError(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidToken => write!(f, "无效的认证令牌"),
            AuthError::TokenExpired => write!(f, "认证令牌已过期"),
            AuthError::InsufficientPermissions => write!(f, "权限不足"),
            AuthError::UserNotFound => write!(f, "用户不存在"),
            AuthError::InvalidPassword => write!(f, "密码错误"),
            AuthError::AccountLocked => write!(f, "账户已被锁定"),
            AuthError::AccountInactive => write!(f, "账户未激活"),
            AuthError::RateLimitExceeded => write!(f, "请求频率超出限制"),
            AuthError::MissingCredentials => write!(f, "缺少认证凭据"),
            AuthError::InvalidCredentials => write!(f, "无效的认证凭据"),
            AuthError::TokenBlacklisted => write!(f, "令牌已被加入黑名单"),
            AuthError::InternalError(msg) => write!(f, "内部错误: {}", msg),
        }
    }
}

impl std::error::Error for AuthError {}

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
}
