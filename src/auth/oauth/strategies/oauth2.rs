//! # OAuth2认证策略
//!
//! 实现标准OAuth2认证流程，支持授权码模式和客户端凭据模式

use crate::auth::strategies::traits::{AuthStrategy, OAuthTokenResult};
use crate::auth::types::{AuthType, AuthError, OAuth2GrantType, PkceMethod};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use url::Url;

/// OAuth2认证策略
pub struct OAuth2Strategy {
    /// HTTP客户端
    http_client: Client,
    /// 客户端ID
    pub client_id: String,
    /// 客户端密钥
    pub client_secret: String,
    /// 授权URL
    pub auth_url: String,
    /// 令牌URL
    pub token_url: String,
    /// 默认作用域
    pub default_scope: Option<String>,
    /// 支持的授权类型
    pub supported_grant_types: Vec<OAuth2GrantType>,
    /// 是否支持PKCE
    pub pkce_enabled: bool,
}

impl OAuth2Strategy {
    /// 创建新的OAuth2认证策略
    pub fn new(
        client_id: String,
        client_secret: String,
        auth_url: String,
        token_url: String,
    ) -> Self {
        Self {
            http_client: Client::new(),
            client_id,
            client_secret,
            auth_url,
            token_url,
            default_scope: None,
            supported_grant_types: vec![
                OAuth2GrantType::AuthorizationCode,
                OAuth2GrantType::ClientCredentials,
                OAuth2GrantType::RefreshToken,
            ],
            pkce_enabled: true,
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

        let auth_url = config.get("auth_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::ConfigError("缺少auth_url配置".to_string()))?
            .to_string();

        let token_url = config.get("token_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::ConfigError("缺少token_url配置".to_string()))?
            .to_string();

        let mut strategy = Self::new(client_id, client_secret, auth_url, token_url);

        // 可选配置
        if let Some(scope) = config.get("default_scope").and_then(|v| v.as_str()) {
            strategy.default_scope = Some(scope.to_string());
        }

        if let Some(pkce) = config.get("pkce_enabled").and_then(|v| v.as_bool()) {
            strategy.pkce_enabled = pkce;
        }

        Ok(strategy)
    }

    /// 生成授权URL
    pub async fn build_auth_url(
        &self,
        state: &str,
        redirect_uri: &str,
        scope: Option<&str>,
        code_challenge: Option<&str>,
        code_challenge_method: Option<PkceMethod>,
    ) -> Result<String, AuthError> {
        let mut auth_url = Url::parse(&self.auth_url)
            .map_err(|e| AuthError::ConfigError(format!("无效的授权URL: {}", e)))?;

        let mut params = vec![
            ("response_type", "code"),
            ("client_id", &self.client_id),
            ("redirect_uri", redirect_uri),
            ("state", state),
        ];

        // 添加作用域
        let scope_value;
        if let Some(s) = scope {
            scope_value = s.to_string();
            params.push(("scope", &scope_value));
        } else if let Some(default_scope) = &self.default_scope {
            params.push(("scope", default_scope));
        }

        // 添加PKCE参数
        if let Some(challenge) = code_challenge {
            params.push(("code_challenge", challenge));
            if let Some(method) = code_challenge_method {
                match method {
                    PkceMethod::Plain => params.push(("code_challenge_method", "plain")),
                    PkceMethod::S256 => params.push(("code_challenge_method", "S256")),
                }
            }
        }

        auth_url.query_pairs_mut().extend_pairs(params);
        Ok(auth_url.to_string())
    }

    /// 使用授权码获取访问令牌
    pub async fn exchange_code_for_token(
        &self,
        code: &str,
        redirect_uri: &str,
        code_verifier: Option<&str>,
    ) -> Result<OAuthTokenResult, AuthError> {
        let mut params = HashMap::new();
        params.insert("grant_type", "authorization_code");
        params.insert("code", code);
        params.insert("redirect_uri", redirect_uri);
        params.insert("client_id", &self.client_id);
        params.insert("client_secret", &self.client_secret);

        if let Some(verifier) = code_verifier {
            params.insert("code_verifier", verifier);
        }

        let response = self.http_client
            .post(&self.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("令牌请求失败: {}", e)))?;

        let status = response.status();
        let body = response.text().await
            .map_err(|e| AuthError::NetworkError(format!("响应读取失败: {}", e)))?;

        if !status.is_success() {
            return Err(AuthError::OAuth2Error(format!(
                "令牌交换失败 ({}): {}", status, body
            )));
        }

        let token_response: Value = serde_json::from_str(&body)
            .map_err(|e| AuthError::JsonError(e))?;

        self.parse_token_response(&token_response)
    }

    /// 使用客户端凭据获取访问令牌
    pub async fn client_credentials_grant(
        &self,
        scope: Option<&str>,
    ) -> Result<OAuthTokenResult, AuthError> {
        let mut params = HashMap::new();
        params.insert("grant_type", "client_credentials");
        params.insert("client_id", &self.client_id);
        params.insert("client_secret", &self.client_secret);

        if let Some(s) = scope {
            params.insert("scope", s);
        } else if let Some(default_scope) = &self.default_scope {
            params.insert("scope", default_scope);
        }

        let response = self.http_client
            .post(&self.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("令牌请求失败: {}", e)))?;

        let status = response.status();
        let body = response.text().await
            .map_err(|e| AuthError::NetworkError(format!("响应读取失败: {}", e)))?;

        if !status.is_success() {
            return Err(AuthError::OAuth2Error(format!(
                "客户端凭据授权失败 ({}): {}", status, body
            )));
        }

        let token_response: Value = serde_json::from_str(&body)
            .map_err(|e| AuthError::JsonError(e))?;

        self.parse_token_response(&token_response)
    }

    /// 刷新访问令牌
    pub async fn refresh_access_token(&self, refresh_token: &str) -> Result<OAuthTokenResult, AuthError> {
        let mut params = HashMap::new();
        params.insert("grant_type", "refresh_token");
        params.insert("refresh_token", refresh_token);
        params.insert("client_id", &self.client_id);
        params.insert("client_secret", &self.client_secret);

        let response = self.http_client
            .post(&self.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("令牌刷新请求失败: {}", e)))?;

        let status = response.status();
        let body = response.text().await
            .map_err(|e| AuthError::NetworkError(format!("响应读取失败: {}", e)))?;

        if !status.is_success() {
            return Err(AuthError::OAuth2Error(format!(
                "令牌刷新失败 ({}): {}", status, body
            )));
        }

        let token_response: Value = serde_json::from_str(&body)
            .map_err(|e| AuthError::JsonError(e))?;

        self.parse_token_response(&token_response)
    }

    /// 解析令牌响应
    fn parse_token_response(&self, response: &Value) -> Result<OAuthTokenResult, AuthError> {
        let access_token = response.get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::OAuth2Error("响应中缺少access_token".to_string()))?
            .to_string();

        let refresh_token = response.get("refresh_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let token_type = response.get("token_type")
            .and_then(|v| v.as_str())
            .unwrap_or("Bearer")
            .to_string();

        let expires_in = response.get("expires_in")
            .and_then(|v| v.as_i64());

        let scope = response.get("scope")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(OAuthTokenResult {
            access_token,
            refresh_token,
            token_type,
            expires_in,
            scope,
            user_info: None,
        })
    }

    /// 生成PKCE代码挑战
    pub fn generate_pkce_challenge(verifier: &str, method: PkceMethod) -> String {
        match method {
            PkceMethod::Plain => verifier.to_string(),
            PkceMethod::S256 => {
                use sha2::{Sha256, Digest};
                use base64::{Engine as _, engine::general_purpose};
                
                let mut hasher = Sha256::new();
                hasher.update(verifier.as_bytes());
                let hash = hasher.finalize();
                general_purpose::URL_SAFE_NO_PAD.encode(hash)
            }
        }
    }

    /// 生成随机字符串（用于状态和验证码）
    pub fn generate_random_string(length: usize) -> String {
        use rand::{Rng, thread_rng};
        use rand::distributions::Alphanumeric;

        thread_rng()
            .sample_iter(&Alphanumeric)
            .take(length)
            .map(char::from)
            .collect()
    }
}

#[async_trait]
impl AuthStrategy for OAuth2Strategy {
    fn auth_type(&self) -> AuthType {
        AuthType::OAuth2
    }

    async fn authenticate(&self, credentials: &Value) -> Result<OAuthTokenResult, AuthError> {
        let grant_type = credentials.get("grant_type")
            .and_then(|v| v.as_str())
            .unwrap_or("client_credentials");

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

                self.exchange_code_for_token(code, redirect_uri, code_verifier).await
            }
            "client_credentials" => {
                let scope = credentials.get("scope")
                    .and_then(|v| v.as_str());

                self.client_credentials_grant(scope).await
            }
            _ => Err(AuthError::ConfigError(format!("不支持的授权类型: {}", grant_type)))
        }
    }

    async fn refresh(&self, refresh_token: &str) -> Result<OAuthTokenResult, AuthError> {
        self.refresh_access_token(refresh_token).await
    }

    async fn get_auth_url(&self, state: &str, redirect_uri: &str) -> Result<String, AuthError> {
        self.build_auth_url(state, redirect_uri, None, None, None).await
    }

    async fn handle_callback(&self, _code: &str, _state: &str) -> Result<OAuthTokenResult, AuthError> {
        // 这里需要从会话中获取redirect_uri和code_verifier
        // 简化实现，实际应用中应该从OAuth会话存储中获取
        Err(AuthError::ConfigError("需要完整的OAuth会话支持".to_string()))
    }

    fn validate_config(&self, config: &Value) -> Result<(), AuthError> {
        let required_fields = ["client_id", "client_secret", "auth_url", "token_url"];
        
        for field in &required_fields {
            if !config.get(field).and_then(|v| v.as_str()).is_some() {
                return Err(AuthError::ConfigError(format!("缺少必需字段: {}", field)));
            }
        }

        // 验证URL格式
        for url_field in &["auth_url", "token_url"] {
            if let Some(url_str) = config.get(url_field).and_then(|v| v.as_str()) {
                Url::parse(url_str)
                    .map_err(|e| AuthError::ConfigError(format!("无效的URL格式 {}: {}", url_field, e)))?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_oauth2_strategy_creation() {
        let strategy = OAuth2Strategy::new(
            "client123".to_string(),
            "secret456".to_string(),
            "https://auth.example.com/oauth2/authorize".to_string(),
            "https://auth.example.com/oauth2/token".to_string(),
        );

        assert_eq!(strategy.client_id, "client123");
        assert_eq!(strategy.client_secret, "secret456");
        assert!(strategy.pkce_enabled);
        assert_eq!(strategy.supported_grant_types.len(), 3);
    }

    #[test]
    fn test_from_config() {
        let config = json!({
            "client_id": "test_client",
            "client_secret": "test_secret",
            "auth_url": "https://auth.example.com/authorize",
            "token_url": "https://auth.example.com/token",
            "default_scope": "read write",
            "pkce_enabled": false
        });

        let strategy = OAuth2Strategy::from_config(&config).unwrap();
        assert_eq!(strategy.client_id, "test_client");
        assert_eq!(strategy.default_scope, Some("read write".to_string()));
        assert!(!strategy.pkce_enabled);
    }

    #[test]
    fn test_from_config_missing_required_field() {
        let config = json!({
            "client_id": "test_client"
            // 缺少其他必需字段
        });

        let result = OAuth2Strategy::from_config(&config);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_build_auth_url() {
        let strategy = OAuth2Strategy::new(
            "client123".to_string(),
            "secret456".to_string(),
            "https://auth.example.com/authorize".to_string(),
            "https://auth.example.com/token".to_string(),
        );

        let url = strategy.build_auth_url(
            "random_state",
            "https://app.example.com/callback",
            Some("read write"),
            None,
            None,
        ).await.unwrap();

        assert!(url.contains("client_id=client123"));
        assert!(url.contains("redirect_uri=https%3A//app.example.com/callback"));
        assert!(url.contains("state=random_state"));
        assert!(url.contains("scope=read+write"));
        assert!(url.contains("response_type=code"));
    }

    #[test]
    fn test_generate_pkce_challenge() {
        let verifier = "test_verifier_123456789";
        
        // Plain方法
        let plain_challenge = OAuth2Strategy::generate_pkce_challenge(verifier, PkceMethod::Plain);
        assert_eq!(plain_challenge, verifier);

        // S256方法
        let s256_challenge = OAuth2Strategy::generate_pkce_challenge(verifier, PkceMethod::S256);
        assert_ne!(s256_challenge, verifier);
        assert!(!s256_challenge.is_empty());
    }

    #[test]
    fn test_generate_random_string() {
        let random1 = OAuth2Strategy::generate_random_string(32);
        let random2 = OAuth2Strategy::generate_random_string(32);
        
        assert_eq!(random1.len(), 32);
        assert_eq!(random2.len(), 32);
        assert_ne!(random1, random2); // 应该生成不同的随机字符串
    }

    #[test]
    fn test_validate_config() {
        let strategy = OAuth2Strategy::new(
            "client".to_string(),
            "secret".to_string(),
            "https://auth.example.com".to_string(),
            "https://token.example.com".to_string(),
        );

        // 有效配置
        let valid_config = json!({
            "client_id": "test_client",
            "client_secret": "test_secret",
            "auth_url": "https://auth.example.com/authorize",
            "token_url": "https://auth.example.com/token"
        });
        assert!(strategy.validate_config(&valid_config).is_ok());

        // 缺少必需字段
        let invalid_config = json!({
            "client_id": "test_client"
        });
        assert!(strategy.validate_config(&invalid_config).is_err());

        // 无效URL
        let invalid_url_config = json!({
            "client_id": "test_client",
            "client_secret": "test_secret",
            "auth_url": "invalid-url",
            "token_url": "https://auth.example.com/token"
        });
        assert!(strategy.validate_config(&invalid_url_config).is_err());
    }
}