//! OAuth2错误处理
//!
//! 定义OAuth2认证过程中可能出现的各种错误类型

use thiserror::Error;

/// OAuth2专用错误类型
#[derive(Debug, Error)]
pub enum OAuth2Error {
    /// 配置错误
    #[error("配置错误: {0}")]
    ConfigError(String),
    
    /// 网络请求错误
    #[error("网络错误: {0}")]
    NetworkError(String),
    
    /// 令牌交换失败
    #[error("令牌交换失败: {0}")]
    TokenExchangeError(String),
    
    /// 令牌刷新失败
    #[error("令牌刷新失败: {0}")]
    TokenRefreshError(String),
    
    /// 令牌撤销失败
    #[error("令牌撤销失败: {0}")]
    TokenRevokeError(String),
    
    /// 数据库错误
    #[error("数据库错误: {0}")]
    DatabaseError(#[from] sea_orm::DbErr),
    
    /// JSON解析错误
    #[error("JSON解析错误: {0}")]
    JsonError(#[from] serde_json::Error),
    
    /// URL解析错误
    #[error("URL解析错误: {0}")]
    UrlError(#[from] url::ParseError),
    
    /// HTTP请求错误
    #[error("HTTP请求错误: {0}")]
    HttpError(#[from] reqwest::Error),
    
    /// OAuth2库错误
    #[error("OAuth2错误: {0}")]
    OAuth2LibError(String),
    
    /// 状态验证失败
    #[error("状态验证失败: {0}")]
    StateValidationError(String),
    
    /// PKCE验证失败
    #[error("PKCE验证失败: {0}")]
    PkceValidationError(String),
    
    /// 会话错误
    #[error("会话错误: {0}")]
    SessionError(String),
    
    /// 认证错误
    #[error("认证错误: {0}")]
    AuthenticationError(String),
    
    /// 授权错误
    #[error("授权错误: {0}")]
    AuthorizationError(String),
    
    /// 提供商不支持
    #[error("不支持的提供商: {0}")]
    UnsupportedProvider(String),
    
    /// 认证类型不支持
    #[error("不支持的认证类型: {0}")]
    UnsupportedAuthType(String),
}

impl OAuth2Error {
    /// 创建配置错误
    pub fn config_error<S: Into<String>>(msg: S) -> Self {
        Self::ConfigError(msg.into())
    }
    
    /// 创建网络错误
    pub fn network_error<S: Into<String>>(msg: S) -> Self {
        Self::NetworkError(msg.into())
    }
    
    /// 创建令牌交换错误
    pub fn token_exchange_error<S: Into<String>>(msg: S) -> Self {
        Self::TokenExchangeError(msg.into())
    }
    
    /// 创建OAuth2库错误
    pub fn oauth2_lib_error<S: Into<String>>(msg: S) -> Self {
        Self::OAuth2LibError(msg.into())
    }
    
    /// 创建状态验证错误
    pub fn state_validation_error<S: Into<String>>(msg: S) -> Self {
        Self::StateValidationError(msg.into())
    }
    
    /// 创建会话错误
    pub fn session_error<S: Into<String>>(msg: S) -> Self {
        Self::SessionError(msg.into())
    }
    
    /// 创建认证错误
    pub fn authentication_error<S: Into<String>>(msg: S) -> Self {
        Self::AuthenticationError(msg.into())
    }
    
    /// 创建不支持的提供商错误
    pub fn unsupported_provider<S: Into<String>>(provider: S) -> Self {
        Self::UnsupportedProvider(provider.into())
    }
    
    /// 创建不支持的认证类型错误
    pub fn unsupported_auth_type<S: Into<String>>(auth_type: S) -> Self {
        Self::UnsupportedAuthType(auth_type.into())
    }
}

/// OAuth2结果类型别名
pub type OAuth2Result<T> = Result<T, OAuth2Error>;

// OAuth2库错误转换将在实际使用时按需实现，避免复杂的泛型类型声明

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let config_err = OAuth2Error::config_error("测试配置错误");
        assert_eq!(config_err.to_string(), "配置错误: 测试配置错误");

        let network_err = OAuth2Error::network_error("网络连接失败");
        assert_eq!(network_err.to_string(), "网络错误: 网络连接失败");

        let token_err = OAuth2Error::token_exchange_error("授权码无效");
        assert_eq!(token_err.to_string(), "令牌交换失败: 授权码无效");
    }

    #[test]
    fn test_unsupported_provider_error() {
        let provider_err = OAuth2Error::unsupported_provider("unknown_provider");
        assert_eq!(provider_err.to_string(), "不支持的提供商: unknown_provider");

        let auth_type_err = OAuth2Error::unsupported_auth_type("unknown_auth");
        assert_eq!(auth_type_err.to_string(), "不支持的认证类型: unknown_auth");
    }

    #[test]
    fn test_error_from_conversions() {
        // 测试从serde_json::Error转换
        let json_str = r#"{"invalid": json,}"#;
        let json_err: serde_json::Error = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let oauth_err: OAuth2Error = json_err.into();
        assert!(matches!(oauth_err, OAuth2Error::JsonError(_)));

        // 测试从url::ParseError转换
        let url_err = url::Url::parse("invalid://url with spaces").unwrap_err();
        let oauth_err: OAuth2Error = url_err.into();
        assert!(matches!(oauth_err, OAuth2Error::UrlError(_)));
    }

    #[test]
    fn test_oauth2_result_type() {
        fn test_function() -> OAuth2Result<String> {
            Ok("success".to_string())
        }

        let result = test_function();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[test]
    fn test_state_and_pkce_errors() {
        let state_err = OAuth2Error::state_validation_error("状态不匹配");
        assert_eq!(state_err.to_string(), "状态验证失败: 状态不匹配");

        let pkce_err = OAuth2Error::PkceValidationError("PKCE验证失败".to_string());
        assert_eq!(pkce_err.to_string(), "PKCE验证失败: PKCE验证失败");
    }

    #[test]
    fn test_session_and_auth_errors() {
        let session_err = OAuth2Error::session_error("会话已过期");
        assert_eq!(session_err.to_string(), "会话错误: 会话已过期");

        let auth_err = OAuth2Error::authentication_error("认证失败");
        assert_eq!(auth_err.to_string(), "认证错误: 认证失败");

        let authz_err = OAuth2Error::AuthorizationError("无权限访问".to_string());
        assert_eq!(authz_err.to_string(), "授权错误: 无权限访问");
    }
}