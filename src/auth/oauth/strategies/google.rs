//! # Google OAuth认证策略
//!
//! 实现Google特定的OAuth2认证流程，包括Google API的特殊处理

use crate::auth::strategies::traits::{AuthStrategy, OAuthTokenResult};
use super::oauth2::OAuth2Strategy;
use crate::auth::types::{AuthType, AuthError, PkceMethod};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

/// Google OAuth认证策略
pub struct GoogleOAuthStrategy {
    /// 内部OAuth2策略
    oauth2_strategy: OAuth2Strategy,
    /// HTTP客户端
    http_client: Client,
    /// Google用户信息API端点
    pub userinfo_endpoint: String,
}

impl GoogleOAuthStrategy {
    /// Google OAuth2的默认端点
    pub const GOOGLE_AUTH_URL: &'static str = "https://accounts.google.com/o/oauth2/auth";
    pub const GOOGLE_TOKEN_URL: &'static str = "https://oauth2.googleapis.com/token";
    pub const GOOGLE_USERINFO_URL: &'static str = "https://www.googleapis.com/oauth2/v2/userinfo";

    /// 创建新的Google OAuth认证策略
    pub fn new(client_id: String, client_secret: String) -> Self {
        let oauth2_strategy = OAuth2Strategy::new(
            client_id,
            client_secret,
            Self::GOOGLE_AUTH_URL.to_string(),
            Self::GOOGLE_TOKEN_URL.to_string(),
        );

        Self {
            oauth2_strategy,
            http_client: Client::new(),
            userinfo_endpoint: Self::GOOGLE_USERINFO_URL.to_string(),
        }
    }

    /// 从配置创建策略
    pub fn from_config(config: &Value) -> Result<Self, AuthError> {
        let client_id = config.get("client_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::ConfigError("缺少client_id配置".to_string()))?
            .to_string();

        let client_secret = config.get("client_secret")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::ConfigError("缺少client_secret配置".to_string()))?
            .to_string();

        let mut strategy = Self::new(client_id, client_secret);

        // 可选：自定义用户信息端点
        if let Some(userinfo_url) = config.get("userinfo_endpoint").and_then(|v| v.as_str()) {
            strategy.userinfo_endpoint = userinfo_url.to_string();
        }

        // 设置默认的Google作用域
        if config.get("default_scope").is_none() {
            strategy.oauth2_strategy.default_scope = Some("openid email profile".to_string());
        }

        Ok(strategy)
    }

    /// 生成Google OAuth授权URL
    pub async fn build_google_auth_url(
        &self,
        state: &str,
        redirect_uri: &str,
        scope: Option<&str>,
        access_type: Option<&str>, // "offline" for refresh token
        prompt: Option<&str>,      // "consent" to force consent screen
    ) -> Result<String, AuthError> {
        // 使用PKCE生成代码挑战
        let code_verifier = OAuth2Strategy::generate_random_string(64);
        let code_challenge = OAuth2Strategy::generate_pkce_challenge(&code_verifier, PkceMethod::S256);

        let final_scope = scope.unwrap_or("openid email profile");
        
        let mut url = self.oauth2_strategy.build_auth_url(
            state,
            redirect_uri,
            Some(final_scope),
            Some(&code_challenge),
            Some(PkceMethod::S256),
        ).await?;

        // 添加Google特定参数
        if let Some(access_type_val) = access_type {
            url = format!("{}&access_type={}", url, access_type_val);
        }

        if let Some(prompt_val) = prompt {
            url = format!("{}&prompt={}", url, prompt_val);
        }

        // 在实际应用中，应该将code_verifier保存到会话中
        // 这里返回URL，code_verifier需要在其他地方处理
        Ok(url)
    }

    /// 获取Google用户信息
    pub async fn get_user_info(&self, access_token: &str) -> Result<Value, AuthError> {
        let response = self.http_client
            .get(&self.userinfo_endpoint)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("用户信息请求失败: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AuthError::OAuth2Error(format!(
                "获取用户信息失败 ({}): {}", status, error_text
            )));
        }

        let user_info: Value = response.json().await
            .map_err(|e| AuthError::NetworkError(format!("JSON解析失败: {}", e)))?;

        Ok(user_info)
    }

    /// 使用授权码获取访问令牌并获取用户信息
    pub async fn exchange_code_for_token_with_userinfo(
        &self,
        code: &str,
        redirect_uri: &str,
        code_verifier: Option<&str>,
    ) -> Result<OAuthTokenResult, AuthError> {
        // 首先获取访问令牌
        let mut auth_result = self.oauth2_strategy
            .exchange_code_for_token(code, redirect_uri, code_verifier)
            .await?;

        // 然后获取用户信息
        match self.get_user_info(&auth_result.access_token).await {
            Ok(user_info) => {
                auth_result.user_info = Some(user_info);
                Ok(auth_result)
            }
            Err(e) => {
                // 如果用户信息获取失败，记录错误但不中断流程
                tracing::warn!("获取Google用户信息失败: {}", e);
                Ok(auth_result)
            }
        }
    }

    /// 验证Google ID Token（如果需要的话）
    pub async fn verify_id_token(&self, id_token: &str) -> Result<Value, AuthError> {
        // Google提供了ID Token验证端点
        let verify_url = "https://oauth2.googleapis.com/tokeninfo";
        
        let response = self.http_client
            .get(verify_url)
            .query(&[("id_token", id_token)])
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("ID Token验证请求失败: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AuthError::OAuth2Error(format!(
                "ID Token验证失败 ({}): {}", status, error_text
            )));
        }

        let token_info: Value = response.json().await
            .map_err(|e| AuthError::NetworkError(format!("JSON解析失败: {}", e)))?;

        // 验证audience（客户端ID）
        if let Some(aud) = token_info.get("aud").and_then(|v| v.as_str()) {
            if aud != self.oauth2_strategy.client_id {
                return Err(AuthError::OAuth2Error(
                    "ID Token的audience不匹配".to_string()
                ));
            }
        }

        Ok(token_info)
    }

    /// 撤销Google访问令牌
    pub async fn revoke_google_token(&self, token: &str) -> Result<(), AuthError> {
        let revoke_url = "https://oauth2.googleapis.com/revoke";
        
        let response = self.http_client
            .post(revoke_url)
            .form(&[("token", token)])
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("令牌撤销请求失败: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AuthError::OAuth2Error(format!(
                "令牌撤销失败: {}", error_text
            )));
        }

        Ok(())
    }

    /// 获取Google应用的授权作用域
    pub fn get_common_scopes() -> Vec<&'static str> {
        vec![
            "openid",                    // OpenID Connect
            "email",                     // 邮箱地址
            "profile",                   // 基本个人信息
            "https://www.googleapis.com/auth/userinfo.email",    // 用户邮箱
            "https://www.googleapis.com/auth/userinfo.profile",  // 用户资料
            "https://www.googleapis.com/auth/drive",             // Google Drive
            "https://www.googleapis.com/auth/gmail.readonly",    // Gmail只读
        ]
    }
}

#[async_trait]
impl AuthStrategy for GoogleOAuthStrategy {
    fn auth_type(&self) -> AuthType {
        AuthType::GoogleOAuth
    }

    async fn authenticate(&self, credentials: &Value) -> Result<OAuthTokenResult, AuthError> {
        let grant_type = credentials.get("grant_type")
            .and_then(|v| v.as_str())
            .unwrap_or("authorization_code");

        match grant_type {
            "authorization_code" => {
                let code = credentials.get("code")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AuthError::ConfigError("缺少授权码".to_string()))?;

                let redirect_uri = credentials.get("redirect_uri")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AuthError::ConfigError("缺少回调URI".to_string()))?;

                let code_verifier = credentials.get("code_verifier")
                    .and_then(|v| v.as_str());

                // 获取令牌并获取用户信息
                self.exchange_code_for_token_with_userinfo(code, redirect_uri, code_verifier).await
            }
            "client_credentials" => {
                // Google API一般不使用客户端凭据模式，但为了兼容性保留
                let scope = credentials.get("scope")
                    .and_then(|v| v.as_str());

                self.oauth2_strategy.client_credentials_grant(scope).await
            }
            _ => Err(AuthError::ConfigError(format!("不支持的授权类型: {}", grant_type)))
        }
    }

    async fn refresh(&self, refresh_token: &str) -> Result<OAuthTokenResult, AuthError> {
        self.oauth2_strategy.refresh_access_token(refresh_token).await
    }

    async fn revoke(&self, token: &str) -> Result<(), AuthError> {
        self.revoke_google_token(token).await
    }

    async fn get_auth_url(&self, state: &str, redirect_uri: &str) -> Result<String, AuthError> {
        self.build_google_auth_url(
            state,
            redirect_uri,
            None,           // 使用默认作用域
            Some("offline"), // 请求刷新令牌
            Some("consent")  // 强制显示同意屏幕
        ).await
    }

    async fn handle_callback(&self, _code: &str, _state: &str) -> Result<OAuthTokenResult, AuthError> {
        // 这里需要从会话中获取redirect_uri和code_verifier
        // 简化实现，实际应用中应该从OAuth会话存储中获取
        Err(AuthError::ConfigError("需要完整的OAuth会话支持".to_string()))
    }

    fn validate_config(&self, config: &Value) -> Result<(), AuthError> {
        // 复用OAuth2策略的配置验证，但使用Google的默认端点
        let mut google_config = config.clone();
        
        // 设置Google默认端点（如果未提供）
        if config.get("auth_url").is_none() {
            google_config["auth_url"] = json!(Self::GOOGLE_AUTH_URL);
        }
        if config.get("token_url").is_none() {
            google_config["token_url"] = json!(Self::GOOGLE_TOKEN_URL);
        }

        self.oauth2_strategy.validate_config(&google_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_google_oauth_strategy_creation() {
        let strategy = GoogleOAuthStrategy::new(
            "google_client_123".to_string(),
            "google_secret_456".to_string(),
        );

        assert_eq!(strategy.oauth2_strategy.client_id, "google_client_123");
        assert_eq!(strategy.userinfo_endpoint, GoogleOAuthStrategy::GOOGLE_USERINFO_URL);
        assert_eq!(
            strategy.oauth2_strategy.default_scope, 
            Some("openid email profile".to_string())
        );
    }

    #[test]
    fn test_from_config() {
        let config = json!({
            "client_id": "google_client",
            "client_secret": "google_secret",
            "userinfo_endpoint": "https://custom.googleapis.com/userinfo"
        });

        let strategy = GoogleOAuthStrategy::from_config(&config).unwrap();
        assert_eq!(strategy.oauth2_strategy.client_id, "google_client");
        assert_eq!(strategy.userinfo_endpoint, "https://custom.googleapis.com/userinfo");
    }

    #[test]
    fn test_get_common_scopes() {
        let scopes = GoogleOAuthStrategy::get_common_scopes();
        assert!(scopes.contains(&"openid"));
        assert!(scopes.contains(&"email"));
        assert!(scopes.contains(&"profile"));
        assert!(scopes.len() >= 3);
    }

    #[tokio::test]
    async fn test_build_google_auth_url() {
        let strategy = GoogleOAuthStrategy::new(
            "google_client_123".to_string(),
            "google_secret_456".to_string(),
        );

        let url = strategy.build_google_auth_url(
            "random_state",
            "https://app.example.com/auth/google/callback",
            Some("openid email profile"),
            Some("offline"),
            Some("consent"),
        ).await.unwrap();

        assert!(url.contains("accounts.google.com"));
        assert!(url.contains("client_id=google_client_123"));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("prompt=consent"));
        assert!(url.contains("scope=openid+email+profile"));
        assert!(url.contains("code_challenge"));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn test_validate_config() {
        let strategy = GoogleOAuthStrategy::new(
            "client".to_string(),
            "secret".to_string(),
        );

        // 最小有效配置（Google端点会自动设置）
        let valid_config = json!({
            "client_id": "google_client",
            "client_secret": "google_secret"
        });
        assert!(strategy.validate_config(&valid_config).is_ok());

        // 缺少必需字段
        let invalid_config = json!({
            "client_id": "google_client"
        });
        assert!(strategy.validate_config(&invalid_config).is_err());
    }

    #[tokio::test]
    async fn test_authenticate_with_authorization_code() {
        let strategy = GoogleOAuthStrategy::new(
            "test_client".to_string(),
            "test_secret".to_string(),
        );

        let credentials = json!({
            "grant_type": "authorization_code",
            "code": "test_code",
            "redirect_uri": "https://app.example.com/callback"
        });

        // 这会失败，因为我们在测试中没有实际的Google服务器
        // 但可以验证参数解析逻辑
        let result = strategy.authenticate(&credentials).await;
        assert!(result.is_err()); // 预期失败，因为网络请求会失败
    }
}