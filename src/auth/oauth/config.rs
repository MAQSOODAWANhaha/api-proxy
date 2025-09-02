//! OAuth2配置管理
//!
//! 定义OAuth2认证所需的配置结构体和相关功能

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// OAuth2配置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2Config {
    /// OAuth2客户端ID
    pub client_id: String,
    /// OAuth2客户端密钥
    pub client_secret: String,
    /// 授权端点URL
    pub authorize_url: String,
    /// 令牌端点URL
    pub token_url: String,
    /// OAuth2作用域
    pub scopes: String,
    /// 是否启用PKCE
    pub pkce_required: bool,
    /// 额外参数(用于特定提供商的自定义参数)
    pub extra_params: HashMap<String, String>,
    /// 令牌撤销端点URL(可选)
    pub revoke_url: Option<String>,
}

impl OAuth2Config {
    /// 创建新的OAuth2配置
    pub fn new(
        client_id: String,
        client_secret: String,
        authorize_url: String,
        token_url: String,
        scopes: String,
        pkce_required: bool,
    ) -> Self {
        Self {
            client_id,
            client_secret,
            authorize_url,
            token_url,
            scopes,
            pkce_required,
            extra_params: HashMap::new(),
            revoke_url: None,
        }
    }

    /// 添加额外参数
    pub fn with_extra_param(mut self, key: String, value: String) -> Self {
        self.extra_params.insert(key, value);
        self
    }

    /// 设置撤销端点
    pub fn with_revoke_url(mut self, revoke_url: String) -> Self {
        self.revoke_url = Some(revoke_url);
        self
    }

    /// 从JSON值创建OAuth2配置
    pub fn from_json(auth_type: &str, json_value: &serde_json::Value) -> Result<Self, super::error::OAuth2Error> {
        let client_id = json_value
            .get("client_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| super::error::OAuth2Error::ConfigError(format!("{}配置中缺少client_id", auth_type)))?
            .to_string();

        let client_secret = json_value
            .get("client_secret")
            .and_then(|v| v.as_str())
            .ok_or_else(|| super::error::OAuth2Error::ConfigError(format!("{}配置中缺少client_secret", auth_type)))?
            .to_string();

        let authorize_url = json_value
            .get("authorize_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| super::error::OAuth2Error::ConfigError(format!("{}配置中缺少authorize_url", auth_type)))?
            .to_string();

        let token_url = json_value
            .get("token_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| super::error::OAuth2Error::ConfigError(format!("{}配置中缺少token_url", auth_type)))?
            .to_string();

        let scopes = json_value
            .get("scopes")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let pkce_required = json_value
            .get("pkce_required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let extra_params = json_value
            .get("extra_params")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let revoke_url = json_value
            .get("revoke_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(OAuth2Config {
            client_id,
            client_secret,
            authorize_url,
            token_url,
            scopes,
            pkce_required,
            extra_params,
            revoke_url,
        })
    }

    /// 验证配置有效性
    pub fn validate(&self) -> Result<(), super::error::OAuth2Error> {
        if self.client_id.trim().is_empty() {
            return Err(super::error::OAuth2Error::ConfigError("client_id不能为空".to_string()));
        }
        
        if self.client_secret.trim().is_empty() {
            return Err(super::error::OAuth2Error::ConfigError("client_secret不能为空".to_string()));
        }

        if self.authorize_url.trim().is_empty() {
            return Err(super::error::OAuth2Error::ConfigError("authorize_url不能为空".to_string()));
        }

        if self.token_url.trim().is_empty() {
            return Err(super::error::OAuth2Error::ConfigError("token_url不能为空".to_string()));
        }

        // 验证URL格式
        url::Url::parse(&self.authorize_url)
            .map_err(|_| super::error::OAuth2Error::ConfigError(format!("无效的authorize_url: {}", self.authorize_url)))?;

        url::Url::parse(&self.token_url)
            .map_err(|_| super::error::OAuth2Error::ConfigError(format!("无效的token_url: {}", self.token_url)))?;

        if let Some(revoke_url) = &self.revoke_url {
            url::Url::parse(revoke_url)
                .map_err(|_| super::error::OAuth2Error::ConfigError(format!("无效的revoke_url: {}", revoke_url)))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_oauth2_config_new() {
        let config = OAuth2Config::new(
            "test_client".to_string(),
            "test_secret".to_string(),
            "https://auth.example.com/authorize".to_string(),
            "https://auth.example.com/token".to_string(),
            "openid email".to_string(),
            true,
        );

        assert_eq!(config.client_id, "test_client");
        assert_eq!(config.client_secret, "test_secret");
        assert_eq!(config.authorize_url, "https://auth.example.com/authorize");
        assert_eq!(config.token_url, "https://auth.example.com/token");
        assert_eq!(config.scopes, "openid email");
        assert!(config.pkce_required);
        assert!(config.extra_params.is_empty());
        assert!(config.revoke_url.is_none());
    }

    #[test]
    fn test_oauth2_config_with_extra_params() {
        let config = OAuth2Config::new(
            "test_client".to_string(),
            "test_secret".to_string(),
            "https://auth.example.com/authorize".to_string(),
            "https://auth.example.com/token".to_string(),
            "openid email".to_string(),
            true,
        )
        .with_extra_param("prompt".to_string(), "select_account".to_string())
        .with_extra_param("access_type".to_string(), "offline".to_string());

        assert_eq!(config.extra_params.len(), 2);
        assert_eq!(config.extra_params.get("prompt"), Some(&"select_account".to_string()));
        assert_eq!(config.extra_params.get("access_type"), Some(&"offline".to_string()));
    }

    #[test]
    fn test_oauth2_config_from_json() {
        let json_config = json!({
            "client_id": "test_client_123",
            "client_secret": "test_secret_456",
            "authorize_url": "https://accounts.google.com/o/oauth2/auth",
            "token_url": "https://oauth2.googleapis.com/token",
            "scopes": "openid email profile",
            "pkce_required": true,
            "extra_params": {
                "prompt": "select_account",
                "access_type": "offline"
            },
            "revoke_url": "https://oauth2.googleapis.com/revoke"
        });

        let config = OAuth2Config::from_json("google_oauth", &json_config).unwrap();
        
        assert_eq!(config.client_id, "test_client_123");
        assert_eq!(config.client_secret, "test_secret_456");
        assert_eq!(config.authorize_url, "https://accounts.google.com/o/oauth2/auth");
        assert_eq!(config.token_url, "https://oauth2.googleapis.com/token");
        assert_eq!(config.scopes, "openid email profile");
        assert!(config.pkce_required);
        assert_eq!(config.extra_params.len(), 2);
        assert_eq!(config.extra_params.get("prompt"), Some(&"select_account".to_string()));
        assert_eq!(config.extra_params.get("access_type"), Some(&"offline".to_string()));
        assert_eq!(config.revoke_url, Some("https://oauth2.googleapis.com/revoke".to_string()));
    }

    #[test]
    fn test_oauth2_config_validation() {
        // 有效配置
        let valid_config = OAuth2Config::new(
            "test_client".to_string(),
            "test_secret".to_string(),
            "https://auth.example.com/authorize".to_string(),
            "https://auth.example.com/token".to_string(),
            "openid email".to_string(),
            true,
        );
        assert!(valid_config.validate().is_ok());

        // 无效client_id
        let invalid_config = OAuth2Config::new(
            "".to_string(),
            "test_secret".to_string(),
            "https://auth.example.com/authorize".to_string(),
            "https://auth.example.com/token".to_string(),
            "openid email".to_string(),
            true,
        );
        assert!(invalid_config.validate().is_err());

        // 无效URL
        let invalid_url_config = OAuth2Config::new(
            "test_client".to_string(),
            "test_secret".to_string(),
            "invalid_url".to_string(),
            "https://auth.example.com/token".to_string(),
            "openid email".to_string(),
            true,
        );
        assert!(invalid_url_config.validate().is_err());
    }

    #[test]
    fn test_oauth2_config_from_json_missing_required_fields() {
        // 缺少client_id
        let incomplete_json = json!({
            "client_secret": "test_secret",
            "authorize_url": "https://auth.example.com/authorize",
            "token_url": "https://auth.example.com/token"
        });

        let result = OAuth2Config::from_json("test", &incomplete_json);
        assert!(result.is_err());
        
        if let Err(super::super::error::OAuth2Error::ConfigError(msg)) = result {
            assert!(msg.contains("缺少client_id"));
        }
    }
}