//! # ADC (Application Default Credentials) 认证策略
//!
//! 实现Google Application Default Credentials认证策略

use super::traits::{AuthStrategy, OAuthTokenResult};
use crate::auth::types::{AuthError, AuthType};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::env;
use std::path::PathBuf;
use tokio::fs;

/// ADC认证策略
pub struct AdcStrategy {
    /// HTTP客户端
    http_client: Client,
    /// 凭据文件路径（可选）
    pub credential_path: Option<PathBuf>,
    /// 默认作用域
    pub default_scopes: Vec<String>,
    /// Token有效期（秒）
    pub token_expiry: i64,
}

impl AdcStrategy {
    /// Google默认元数据服务端点
    pub const GOOGLE_METADATA_TOKEN_URL: &'static str = "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

    /// Google OAuth2 Token端点
    pub const GOOGLE_TOKEN_URL: &'static str = "https://oauth2.googleapis.com/token";

    /// 创建新的ADC认证策略
    pub fn new() -> Self {
        Self {
            http_client: Client::new(),
            credential_path: None,
            default_scopes: Vec::new(),
            token_expiry: 3600, // 默认1小时
        }
    }

    /// 设置凭据文件路径
    pub fn with_credential_path(mut self, path: PathBuf) -> Self {
        self.credential_path = Some(path);
        self
    }

    /// 设置默认作用域
    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.default_scopes = scopes;
        self
    }

    /// 从配置创建策略
    pub fn from_config(config: &Value) -> Result<Self, AuthError> {
        let mut strategy = Self::new();

        // 设置作用域
        if let Some(scopes_value) = config.get("scopes") {
            if let Some(scopes_str) = scopes_value.as_str() {
                strategy.default_scopes = scopes_str
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
            } else if let Some(scopes_array) = scopes_value.as_array() {
                strategy.default_scopes = scopes_array
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect();
            }
        }

        // 设置凭据文件路径
        if let Some(path_str) = config.get("credential_path").and_then(|v| v.as_str()) {
            strategy.credential_path = Some(PathBuf::from(path_str));
        }

        Ok(strategy)
    }

    /// 尝试从环境变量获取凭据路径
    fn get_credential_path_from_env(&self) -> Option<PathBuf> {
        // 1. GOOGLE_APPLICATION_CREDENTIALS 环境变量
        if let Ok(path) = env::var("GOOGLE_APPLICATION_CREDENTIALS") {
            return Some(PathBuf::from(path));
        }

        // 2. 默认路径检查
        if let Ok(home) = env::var("HOME") {
            let default_path = PathBuf::from(home)
                .join(".config")
                .join("gcloud")
                .join("application_default_credentials.json");

            if default_path.exists() {
                return Some(default_path);
            }
        }

        None
    }

    /// 从文件加载凭据
    async fn load_credentials_from_file(&self, path: &PathBuf) -> Result<Value, AuthError> {
        let content = fs::read_to_string(path).await.map_err(|e| {
            AuthError::ConfigError(format!("Failed to read credentials file: {}", e))
        })?;

        let credentials: Value = serde_json::from_str(&content).map_err(|e| {
            AuthError::ConfigError(format!("Invalid credentials file format: {}", e))
        })?;

        Ok(credentials)
    }

    /// 从元数据服务获取token（GCE/GKE环境）
    async fn get_token_from_metadata_service(
        &self,
        scopes: &[String],
    ) -> Result<OAuthTokenResult, AuthError> {
        let mut url = Self::GOOGLE_METADATA_TOKEN_URL.to_string();

        if !scopes.is_empty() {
            url.push_str(&format!("?scopes={}", scopes.join(",")));
        }

        let response = self
            .http_client
            .get(&url)
            .header("Metadata-Flavor", "Google")
            .send()
            .await
            .map_err(|e| {
                AuthError::NetworkError(format!("Failed to call metadata service: {}", e))
            })?;

        if !response.status().is_success() {
            return Err(AuthError::NetworkError(format!(
                "Metadata service returned status: {}",
                response.status()
            )));
        }

        let token_response: Value = response.json().await.map_err(|e| {
            AuthError::NetworkError(format!("Failed to parse metadata response: {}", e))
        })?;

        let access_token = token_response
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AuthError::NetworkError("No access token in metadata response".to_string())
            })?
            .to_string();

        let expires_in = token_response
            .get("expires_in")
            .and_then(|v| v.as_i64())
            .unwrap_or(self.token_expiry);

        Ok(OAuthTokenResult {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: Some(expires_in),
            refresh_token: None,
            scope: Some(scopes.join(" ")),
            user_info: None,
        })
    }

    /// 使用服务账户凭据获取token
    async fn get_token_from_service_account(
        &self,
        credentials: &Value,
        _scopes: &[String],
    ) -> Result<OAuthTokenResult, AuthError> {
        // 从凭据中提取必要信息
        let _client_email = credentials
            .get("client_email")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AuthError::ConfigError("Missing client_email in credentials".to_string())
            })?;

        let _private_key = credentials
            .get("private_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AuthError::ConfigError("Missing private_key in credentials".to_string())
            })?;

        let _token_uri = credentials
            .get("token_uri")
            .and_then(|v| v.as_str())
            .unwrap_or(Self::GOOGLE_TOKEN_URL);

        // 创建JWT断言 - 这里需要使用JWT库，为了简化这里返回错误
        // 在实际实现中需要使用jsonwebtoken或similar库来生成JWT
        Err(AuthError::ConfigError(
            "Service account JWT generation not implemented in ADC strategy".to_string(),
        ))
    }
}

impl Default for AdcStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuthStrategy for AdcStrategy {
    fn auth_type(&self) -> AuthType {
        AuthType::Adc
    }

    async fn authenticate(&self, _credentials: &Value) -> Result<OAuthTokenResult, AuthError> {
        let scopes = if self.default_scopes.is_empty() {
            vec!["https://www.googleapis.com/auth/cloud-platform".to_string()]
        } else {
            self.default_scopes.clone()
        };

        // 1. 尝试从指定的凭据文件路径
        if let Some(ref path) = self.credential_path {
            match self.load_credentials_from_file(path).await {
                Ok(credentials) => {
                    return self
                        .get_token_from_service_account(&credentials, &scopes)
                        .await;
                }
                Err(e) => {
                    tracing::warn!("Failed to load credentials from specified path: {}", e);
                }
            }
        }

        // 2. 尝试从环境变量获取凭据路径
        if let Some(env_path) = self.get_credential_path_from_env() {
            match self.load_credentials_from_file(&env_path).await {
                Ok(credentials) => {
                    return self
                        .get_token_from_service_account(&credentials, &scopes)
                        .await;
                }
                Err(e) => {
                    tracing::warn!("Failed to load credentials from environment path: {}", e);
                }
            }
        }

        // 3. 尝试从元数据服务获取（GCE/GKE环境）
        match self.get_token_from_metadata_service(&scopes).await {
            Ok(token) => Ok(token),
            Err(e) => {
                tracing::warn!("Failed to get token from metadata service: {}", e);
                Err(AuthError::ConfigError(
                    "No valid ADC credentials found. Please set GOOGLE_APPLICATION_CREDENTIALS or run on GCE/GKE".to_string()
                ))
            }
        }
    }

    fn validate_config(&self, _config: &Value) -> Result<(), AuthError> {
        // ADC strategy doesn't require specific config validation
        // as it relies on environment and metadata service discovery
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_adc_strategy_creation() {
        let strategy = AdcStrategy::new();
        assert_eq!(strategy.auth_type(), AuthType::Adc);
        assert!(strategy.default_scopes.is_empty());
        assert!(strategy.credential_path.is_none());
    }

    #[test]
    fn test_adc_strategy_with_scopes() {
        let scopes = vec![
            "https://www.googleapis.com/auth/generative-language".to_string(),
            "https://www.googleapis.com/auth/cloud-platform".to_string(),
        ];
        let strategy = AdcStrategy::new().with_scopes(scopes.clone());
        assert_eq!(strategy.default_scopes, scopes);
    }

    #[test]
    fn test_adc_strategy_from_config() {
        let config = json!({
            "scopes": "https://www.googleapis.com/auth/generative-language https://www.googleapis.com/auth/cloud-platform",
            "credential_path": "/path/to/credentials.json"
        });

        let strategy = AdcStrategy::from_config(&config).unwrap();
        assert_eq!(strategy.default_scopes.len(), 2);
        assert!(strategy.credential_path.is_some());
    }

    #[test]
    fn test_adc_strategy_from_config_with_array_scopes() {
        let config = json!({
            "scopes": [
                "https://www.googleapis.com/auth/generative-language",
                "https://www.googleapis.com/auth/cloud-platform"
            ]
        });

        let strategy = AdcStrategy::from_config(&config).unwrap();
        assert_eq!(strategy.default_scopes.len(), 2);
        assert_eq!(
            strategy.default_scopes[0],
            "https://www.googleapis.com/auth/generative-language"
        );
    }
}
