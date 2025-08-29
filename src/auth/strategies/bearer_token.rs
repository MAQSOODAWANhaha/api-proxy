//! # Bearer Token认证策略
//!
//! 实现简单的Bearer Token认证，用于已有访问令牌的场景

use super::traits::{AuthStrategy, OAuthTokenResult};
use crate::auth::types::{AuthType, AuthError};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

/// Bearer Token认证策略
pub struct BearerTokenStrategy {
    /// HTTP客户端
    http_client: Client,
    /// 令牌验证端点（可选）
    pub validation_endpoint: Option<String>,
    /// 用户信息端点（可选）
    pub userinfo_endpoint: Option<String>,
    /// 是否跳过令牌验证
    pub skip_validation: bool,
}

impl Default for BearerTokenStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl BearerTokenStrategy {
    /// 创建新的Bearer Token认证策略
    pub fn new() -> Self {
        Self {
            http_client: Client::new(),
            validation_endpoint: None,
            userinfo_endpoint: None,
            skip_validation: true, // 默认跳过验证，直接接受令牌
        }
    }

    /// 从配置创建策略
    pub fn from_config(config: &Value) -> Result<Self, AuthError> {
        let mut strategy = Self::new();

        // 可选：设置令牌验证端点
        if let Some(validation_url) = config.get("validation_endpoint").and_then(|v| v.as_str()) {
            strategy.validation_endpoint = Some(validation_url.to_string());
            strategy.skip_validation = false; // 如果提供了验证端点，默认启用验证
        }

        // 可选：设置用户信息端点
        if let Some(userinfo_url) = config.get("userinfo_endpoint").and_then(|v| v.as_str()) {
            strategy.userinfo_endpoint = Some(userinfo_url.to_string());
        }

        // 可选：设置是否跳过验证
        if let Some(skip) = config.get("skip_validation").and_then(|v| v.as_bool()) {
            strategy.skip_validation = skip;
        }

        Ok(strategy)
    }

    /// 验证Bearer Token
    pub async fn validate_token(&self, token: &str) -> Result<Option<Value>, AuthError> {
        if self.skip_validation {
            return Ok(None); // 跳过验证，直接返回None表示没有额外信息
        }

        if let Some(validation_url) = &self.validation_endpoint {
            let response = self.http_client
                .get(validation_url)
                .bearer_auth(token)
                .send()
                .await
                .map_err(|e| AuthError::NetworkError(format!("令牌验证请求失败: {}", e)))?;

            let status = response.status();
            if status.is_success() {
                let validation_info: Value = response.json().await
                    .map_err(|e| AuthError::NetworkError(format!("JSON解析失败: {}", e)))?;
                Ok(Some(validation_info))
            } else if status == reqwest::StatusCode::UNAUTHORIZED {
                Err(AuthError::Expired)
            } else {
                let error_text = response.text().await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(AuthError::ConfigError(format!(
                    "令牌验证失败 ({}): {}", status, error_text
                )))
            }
        } else {
            // 没有验证端点，进行基本格式检查
            if token.is_empty() {
                Err(AuthError::ConfigError("Bearer Token不能为空".to_string()))
            } else if token.len() < 10 {
                Err(AuthError::ConfigError("Bearer Token格式无效".to_string()))
            } else {
                Ok(None) // 基本检查通过
            }
        }
    }

    /// 获取用户信息
    pub async fn get_user_info(&self, token: &str) -> Result<Option<Value>, AuthError> {
        if let Some(userinfo_url) = &self.userinfo_endpoint {
            let response = self.http_client
                .get(userinfo_url)
                .bearer_auth(token)
                .send()
                .await
                .map_err(|e| AuthError::NetworkError(format!("用户信息请求失败: {}", e)))?;

            let status = response.status();
            if status.is_success() {
                let user_info: Value = response.json().await
                    .map_err(|e| AuthError::NetworkError(format!("JSON解析失败: {}", e)))?;
                Ok(Some(user_info))
            } else if status == reqwest::StatusCode::UNAUTHORIZED {
                Err(AuthError::Expired)
            } else {
                // 用户信息获取失败不阻断认证流程，只记录警告
                tracing::warn!("获取用户信息失败 ({})", status);
                Ok(None)
            }
        } else {
            Ok(None) // 没有用户信息端点
        }
    }

    /// 从JWT解析基本信息（如果是JWT格式的Bearer Token）
    pub fn parse_jwt_claims(&self, token: &str) -> Option<Value> {
        // 简单的JWT解析（不验证签名，仅解析payload）
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return None; // 不是JWT格式
        }

        let payload_part = parts[1];
        
        // JWT使用base64url编码，需要特殊处理
        let payload_bytes = match self.decode_base64url(payload_part) {
            Ok(bytes) => bytes,
            Err(_) => return None,
        };

        let payload_str = match String::from_utf8(payload_bytes) {
            Ok(s) => s,
            Err(_) => return None,
        };

        serde_json::from_str(&payload_str).ok()
    }

    /// Base64URL解码
    fn decode_base64url(&self, input: &str) -> Result<Vec<u8>, AuthError> {
        use base64::{Engine as _, engine::general_purpose};
        
        // Base64URL转换：替换字符并添加填充
        let mut padded = input.replace('-', "+").replace('_', "/");
        while padded.len() % 4 != 0 {
            padded.push('=');
        }

        general_purpose::STANDARD.decode(padded)
            .map_err(|e| AuthError::ConfigError(format!("Base64URL解码失败: {}", e)))
    }

    /// 检查令牌是否已过期（针对JWT）
    pub fn check_token_expiry(&self, claims: &Value) -> bool {
        if let Some(exp) = claims.get("exp").and_then(|v| v.as_i64()) {
            let now = chrono::Utc::now().timestamp();
            exp > now
        } else {
            true // 没有过期信息，假设有效
        }
    }
}

#[async_trait]
impl AuthStrategy for BearerTokenStrategy {
    fn auth_type(&self) -> AuthType {
        AuthType::BearerToken
    }

    async fn authenticate(&self, credentials: &Value) -> Result<OAuthTokenResult, AuthError> {
        let token = credentials.get("access_token")
            .or_else(|| credentials.get("token"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::ConfigError("缺少access_token或token参数".to_string()))?;

        // 验证令牌
        let validation_info = self.validate_token(token).await?;

        // 获取用户信息
        let user_info = match self.get_user_info(token).await {
            Ok(info) => info,
            Err(e) => {
                tracing::warn!("获取用户信息失败: {}", e);
                None
            }
        };

        // 尝试解析JWT Claims
        let jwt_claims = self.parse_jwt_claims(token);

        // 如果是JWT，检查过期时间
        if let Some(claims) = &jwt_claims {
            if !self.check_token_expiry(claims) {
                return Err(AuthError::Expired);
            }
        }

        // 确定令牌类型
        let token_type = if jwt_claims.is_some() {
            "JWT"
        } else {
            "Bearer"
        }.to_string();

        // 从JWT Claims中提取过期时间
        let expires_in = if let Some(claims) = &jwt_claims {
            claims.get("exp").and_then(|v| v.as_i64()).map(|exp| {
                let now = chrono::Utc::now().timestamp();
                (exp - now).max(0)
            })
        } else {
            None
        };

        // 合并用户信息
        let final_user_info = match (user_info, jwt_claims) {
            (Some(api_info), Some(jwt_info)) => {
                // 合并API用户信息和JWT Claims
                let mut merged = api_info;
                if let (Some(api_obj), Some(jwt_obj)) = (merged.as_object_mut(), jwt_info.as_object()) {
                    for (key, value) in jwt_obj {
                        api_obj.entry(key.clone()).or_insert(value.clone());
                    }
                }
                Some(merged)
            }
            (Some(info), None) | (None, Some(info)) => Some(info),
            (None, None) => validation_info,
        };

        Ok(OAuthTokenResult {
            access_token: token.to_string(),
            refresh_token: None, // Bearer Token通常不提供刷新令牌
            token_type,
            expires_in,
            scope: final_user_info.as_ref()
                .and_then(|info| info.get("scope"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            user_info: final_user_info,
        })
    }

    fn validate_config(&self, config: &Value) -> Result<(), AuthError> {
        // 验证可选的URL格式
        for url_field in &["validation_endpoint", "userinfo_endpoint"] {
            if let Some(url_str) = config.get(url_field).and_then(|v| v.as_str()) {
                url::Url::parse(url_str)
                    .map_err(|e| AuthError::ConfigError(format!(
                        "无效的URL格式 {}: {}", url_field, e
                    )))?;
            }
        }

        // 验证布尔类型字段
        if let Some(skip_val) = config.get("skip_validation") {
            if !skip_val.is_boolean() {
                return Err(AuthError::ConfigError(
                    "skip_validation必须是布尔值".to_string()
                ));
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
    fn test_bearer_token_strategy_creation() {
        let strategy = BearerTokenStrategy::new();
        assert!(strategy.skip_validation);
        assert!(strategy.validation_endpoint.is_none());
        assert!(strategy.userinfo_endpoint.is_none());
    }

    #[test]
    fn test_from_config() {
        let config = json!({
            "validation_endpoint": "https://api.example.com/validate",
            "userinfo_endpoint": "https://api.example.com/userinfo",
            "skip_validation": false
        });

        let strategy = BearerTokenStrategy::from_config(&config).unwrap();
        assert_eq!(strategy.validation_endpoint, Some("https://api.example.com/validate".to_string()));
        assert_eq!(strategy.userinfo_endpoint, Some("https://api.example.com/userinfo".to_string()));
        assert!(!strategy.skip_validation);
    }

    #[test]
    fn test_parse_jwt_claims() {
        let strategy = BearerTokenStrategy::new();
        
        // 创建一个简单的JWT（不验证签名）
        let header = r#"{"alg":"HS256","typ":"JWT"}"#;
        let payload = r#"{"sub":"1234567890","name":"John Doe","iat":1516239022,"exp":9999999999}"#;
        let signature = "fake_signature";
        
        use base64::{Engine as _, engine::general_purpose};
        let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header.as_bytes());
        let payload_b64 = general_purpose::URL_SAFE_NO_PAD.encode(payload.as_bytes());
        
        let jwt = format!("{}.{}.{}", header_b64, payload_b64, signature);
        
        let claims = strategy.parse_jwt_claims(&jwt);
        assert!(claims.is_some());
        
        let claims = claims.unwrap();
        assert_eq!(claims.get("name").unwrap().as_str().unwrap(), "John Doe");
        assert_eq!(claims.get("sub").unwrap().as_str().unwrap(), "1234567890");
    }

    #[test]
    fn test_check_token_expiry() {
        let strategy = BearerTokenStrategy::new();
        
        // 未过期的令牌
        let future_exp = chrono::Utc::now().timestamp() + 3600;
        let valid_claims = json!({"exp": future_exp});
        assert!(strategy.check_token_expiry(&valid_claims));
        
        // 已过期的令牌
        let past_exp = chrono::Utc::now().timestamp() - 3600;
        let expired_claims = json!({"exp": past_exp});
        assert!(!strategy.check_token_expiry(&expired_claims));
        
        // 没有过期时间的令牌
        let no_exp_claims = json!({"sub": "user123"});
        assert!(strategy.check_token_expiry(&no_exp_claims));
    }

    #[tokio::test]
    async fn test_authenticate() {
        let strategy = BearerTokenStrategy::new();
        
        let credentials = json!({
            "access_token": "valid_bearer_token_12345"
        });

        let result = strategy.authenticate(&credentials).await.unwrap();
        assert_eq!(result.access_token, "valid_bearer_token_12345");
        assert_eq!(result.token_type, "Bearer");
        assert!(result.refresh_token.is_none());
    }

    #[tokio::test]
    async fn test_authenticate_missing_token() {
        let strategy = BearerTokenStrategy::new();
        
        let credentials = json!({});

        let result = strategy.authenticate(&credentials).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config() {
        let strategy = BearerTokenStrategy::new();

        // 有效配置
        let valid_config = json!({
            "validation_endpoint": "https://api.example.com/validate",
            "skip_validation": true
        });
        assert!(strategy.validate_config(&valid_config).is_ok());

        // 无效URL
        let invalid_url_config = json!({
            "validation_endpoint": "invalid-url"
        });
        assert!(strategy.validate_config(&invalid_url_config).is_err());

        // 无效布尔值
        let invalid_bool_config = json!({
            "skip_validation": "true"
        });
        assert!(strategy.validate_config(&invalid_bool_config).is_err());
    }

    #[test]
    fn test_decode_base64url() {
        let strategy = BearerTokenStrategy::new();
        
        // 测试标准base64url编码
        let input = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
        let result = strategy.decode_base64url(input);
        assert!(result.is_ok());
        
        let decoded = String::from_utf8(result.unwrap()).unwrap();
        assert_eq!(decoded, r#"{"alg":"HS256","typ":"JWT"}"#);
    }
}