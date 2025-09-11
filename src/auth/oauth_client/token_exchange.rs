//! # Token交换逻辑
//!
//! 实现OAuth 2.0授权码到访问令牌的交换流程
//! 支持PKCE验证、刷新令牌、多提供商兼容等功能

use super::providers::OAuthProviderManager;
use super::session_manager::SessionManager;
use super::{OAuthError, OAuthResult, OAuthTokenResponse};
use entity::oauth_client_sessions;
use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// 令牌响应结构（来自OAuth服务器的原始响应）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub scope: Option<String>,
    // 错误响应字段
    pub error: Option<String>,
    pub error_description: Option<String>,
    pub error_uri: Option<String>,
}

/// 令牌交换请求参数
#[derive(Debug, Clone)]
pub struct TokenExchangeRequest {
    pub session_id: String,
    pub authorization_code: String,
    pub code_verifier: String,
    pub redirect_uri: String,
}

/// Token交换客户端
#[derive(Debug, Clone)]
pub struct TokenExchangeClient {
    http_client: reqwest::Client,
}

impl TokenExchangeClient {
    /// 创建新的Token交换客户端
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("OAuth-TokenExchange/1.0")
            .build()
            .unwrap_or_default();

        Self {
            http_client: client,
        }
    }

    /// 交换授权码获取访问令牌
    pub async fn exchange_token(
        &self,
        provider_manager: &OAuthProviderManager,
        session_manager: &SessionManager,
        session_id: &str,
        authorization_code: &str,
    ) -> OAuthResult<OAuthTokenResponse> {
        // 获取会话信息
        let session = session_manager.get_session(session_id).await?;

        // 验证会话状态
        if session.status != "pending" {
            return Err(OAuthError::InvalidSession(format!(
                "Session {} is not in pending state",
                session_id
            )));
        }

        if session.is_expired() {
            return Err(OAuthError::SessionExpired(session_id.to_string()));
        }

        // 获取提供商配置
        let config = provider_manager.get_config(&session.provider_name).await?;

        // 提取真正的authorization code（移除fragment部分）
        let actual_code = if authorization_code.contains('#') {
            let parts: Vec<&str> = authorization_code.split('#').collect();
            tracing::debug!(
                "Authorization code contains fragment, using code part: {} -> {}",
                authorization_code,
                parts[0]
            );
            parts[0].to_string()
        } else {
            authorization_code.to_string()
        };

        // 构建Token交换请求
        let mut form_params = HashMap::new();
        form_params.insert("grant_type".to_string(), "authorization_code".to_string());
        form_params.insert("client_id".to_string(), config.client_id.clone());
        form_params.insert("code".to_string(), actual_code);
        form_params.insert("redirect_uri".to_string(), config.redirect_uri.clone());

        // 添加客户端密钥（如果有）
        if let Some(client_secret) = &config.client_secret {
            form_params.insert("client_secret".to_string(), client_secret.clone());
        }

        // 添加PKCE验证器
        if config.pkce_required {
            form_params.insert("code_verifier".to_string(), session.code_verifier.clone());
        }

        // 添加提供商特定参数
        self.add_provider_specific_params(&mut form_params, &session.provider_name, &session);

        // 添加OAuth配置中的额外参数
        self.add_config_based_params(&mut form_params, provider_manager, &session.provider_name)
            .await?;

        // 发送Token交换请求
        let response = self
            .send_token_request(&config.token_url, form_params)
            .await?;

        // 处理响应
        let token_response = self.process_token_response(response, session_id).await?;

        // 更新会话状态
        session_manager
            .update_session_with_tokens(session_id, &token_response)
            .await?;

        Ok(token_response)
    }

    /// 刷新访问令牌
    pub async fn refresh_token(
        &self,
        provider_manager: &OAuthProviderManager,
        session_manager: &SessionManager,
        session_id: &str,
    ) -> OAuthResult<OAuthTokenResponse> {
        // 获取会话信息
        let session = session_manager.get_session(session_id).await?;

        // 检查是否有刷新令牌
        let refresh_token = session.refresh_token.as_ref().ok_or_else(|| {
            OAuthError::TokenExchangeFailed("No refresh token available".to_string())
        })?;

        // 获取提供商配置
        let config = provider_manager.get_config(&session.provider_name).await?;

        // 构建刷新请求
        let mut form_params = HashMap::new();
        form_params.insert("grant_type".to_string(), "refresh_token".to_string());
        form_params.insert("client_id".to_string(), config.client_id.clone());
        form_params.insert("refresh_token".to_string(), refresh_token.clone());

        // 添加客户端密钥（如果有）
        if let Some(client_secret) = &config.client_secret {
            form_params.insert("client_secret".to_string(), client_secret.clone());
        }

        // 发送刷新请求
        let response = self
            .send_token_request(&config.token_url, form_params)
            .await?;

        // 处理响应
        let token_response = self.process_token_response(response, session_id).await?;

        // 更新会话状态
        session_manager
            .update_session_with_tokens(session_id, &token_response)
            .await?;

        Ok(token_response)
    }

    /// 撤销令牌
    pub async fn revoke_token(
        &self,
        provider_manager: &OAuthProviderManager,
        session_manager: &SessionManager,
        session_id: &str,
        token: &str,
        token_type_hint: Option<&str>,
    ) -> OAuthResult<()> {
        // 获取会话信息
        let session = session_manager.get_session(session_id).await?;
        let config = provider_manager.get_config(&session.provider_name).await?;

        // 解析基础提供商名称
        let base_provider = if session.provider_name.contains(':') {
            session
                .provider_name
                .split(':')
                .next()
                .unwrap_or(&session.provider_name)
        } else {
            &session.provider_name
        };

        // 构建撤销请求URL（不是所有提供商都支持）
        let revoke_url = match base_provider {
            "google" | "gemini" => "https://oauth2.googleapis.com/revoke",
            "openai" => "https://auth.openai.com/oauth/revoke",
            _ => {
                // 对于不支持撤销的提供商，只是在本地标记为失效
                tracing::debug!(
                    "Provider {} does not support token revocation",
                    base_provider
                );
                return Ok(());
            }
        };

        let mut form_params = HashMap::new();
        form_params.insert("token".to_string(), token.to_string());
        form_params.insert("client_id".to_string(), config.client_id.clone());

        if let Some(hint) = token_type_hint {
            form_params.insert("token_type_hint".to_string(), hint.to_string());
        }

        // 发送撤销请求
        let response = self
            .http_client
            .post(revoke_url)
            .form(&form_params)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(OAuthError::TokenExchangeFailed(format!(
                "Token revocation failed: {}",
                response.status()
            )));
        }

        Ok(())
    }

    /// 验证访问令牌有效性
    pub async fn validate_token(
        &self,
        provider_name: &str,
        access_token: &str,
    ) -> OAuthResult<bool> {
        // 解析基础提供商名称
        let base_provider = if provider_name.contains(':') {
            provider_name.split(':').next().unwrap_or(provider_name)
        } else {
            provider_name
        };

        match base_provider {
            "google" | "gemini" => self.validate_google_token(access_token).await,
            "openai" => self.validate_openai_token(access_token).await,
            "claude" => self.validate_claude_token(access_token).await,
            _ => {
                // 对于未知提供商，执行基础HTTP验证
                self.validate_generic_token(base_provider, access_token)
                    .await
            }
        }
    }

    /// 验证Google/Gemini令牌
    async fn validate_google_token(&self, access_token: &str) -> OAuthResult<bool> {
        let validation_url = format!(
            "https://oauth2.googleapis.com/tokeninfo?access_token={}",
            access_token
        );
        let response = self.http_client.get(&validation_url).send().await?;

        Ok(response.status().is_success())
    }

    /// 通用令牌验证
    async fn validate_generic_token(
        &self,
        provider_name: &str,
        _access_token: &str,
    ) -> OAuthResult<bool> {
        // 对于没有特定验证端点的提供商，默认认为令牌有效
        // 实际应用中可以根据需要实现更复杂的验证逻辑
        tracing::debug!("Generic token validation for provider: {}", provider_name);
        Ok(true)
    }

    // 私有方法

    /// 发送Token请求
    async fn send_token_request(
        &self,
        token_url: &str,
        form_params: HashMap<String, String>,
    ) -> OAuthResult<TokenResponse> {
        // Claude需要使用JSON格式，其他提供商使用form格式
        let is_claude_token_url = token_url.contains("console.anthropic.com");

        let response = if is_claude_token_url {
            // Claude使用JSON格式 - 根据Wei-Shaw项目实现
            tracing::debug!(
                "🌟 发送Claude token exchange请求: url={}, params={:?}",
                token_url,
                form_params
            );

            self.http_client
                .post(token_url)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/plain, */*")
                .header("User-Agent", "claude-cli/1.0.56 (external, cli)")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Referer", "https://claude.ai/")
                .header("Origin", "https://claude.ai")
                .json(&form_params)
                .send()
                .await?
        } else {
            // 其他提供商使用标准form格式
            self.http_client
                .post(token_url)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .header("Accept", "application/json")
                .form(&form_params)
                .send()
                .await?
        };

        let status = response.status();

        if !status.is_success() {
            // 对于错误响应，先尝试解析为JSON，如果失败则获取文本内容
            let error_text = response.text().await?;
            tracing::debug!(
                "🌟 Token exchange error response: status={}, body={}",
                status,
                error_text
            );

            // 尝试解析错误响应
            if let Ok(error_response) = serde_json::from_str::<TokenResponse>(&error_text) {
                if let Some(error) = error_response.error {
                    return Err(OAuthError::TokenExchangeFailed(format!(
                        "{}: {}",
                        error,
                        error_response.error_description.unwrap_or_default()
                    )));
                }
            }
            return Err(OAuthError::TokenExchangeFailed(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        // 先获取原始响应文本以便打印所有数据
        let data = response
            .text()
            .await
            .map_err(|e| OAuthError::SerdeError(format!("Failed to read response text: {}", e)))?;

        // 打印完整的原始JSON响应（注意：生产环境中应该小心处理敏感信息）
        tracing::info!(
            "🌟 Token exchange complete raw response: status={}, body={}",
            status,
            data
        );

        // 解析为我们定义的TokenResponse结构体
        let response = serde_json::from_str::<TokenResponse>(&data).map_err(|e| {
            OAuthError::SerdeError(format!("Failed to parse token response: {}", e))
        })?;

        // 也尝试解析为通用的JSON Value以捕获所有字段
        if let Ok(raw_json) = serde_json::from_str::<serde_json::Value>(&data) {
            tracing::info!("🌟 Token response parsed as JSON Value: {:#}", raw_json);
        }

        // 打印结构化的关键信息
        tracing::info!(
            "🌟 Token exchange structured response: status={}, token_type={}, expires_in={:?}, has_refresh_token={}, has_id_token={}, scope={:?}",
            status,
            response.token_type,
            response.expires_in,
            response.refresh_token.is_some(),
            response.id_token.is_some(),
            response.scope
        );

        Ok(response)
    }

    /// 处理Token响应
    async fn process_token_response(
        &self,
        response: TokenResponse,
        session_id: &str,
    ) -> OAuthResult<OAuthTokenResponse> {
        // 检查是否有错误
        if let Some(error) = response.error {
            return Err(OAuthError::TokenExchangeFailed(format!(
                "{}: {}",
                error,
                response.error_description.unwrap_or_default()
            )));
        }

        // 解析作用域
        let scopes = response
            .scope
            .map(|s| s.split_whitespace().map(|s| s.to_string()).collect())
            .unwrap_or_default();

        Ok(OAuthTokenResponse {
            session_id: session_id.to_string(),
            access_token: response.access_token,
            refresh_token: response.refresh_token,
            id_token: response.id_token,
            token_type: response.token_type,
            expires_in: response.expires_in.map(|e| e as i32),
            scopes,
        })
    }

    /// 添加提供商特定参数（从OAuth配置中读取）
    fn add_provider_specific_params(
        &self,
        form_params: &mut HashMap<String, String>,
        provider_name: &str,
        session: &oauth_client_sessions::Model,
    ) {
        // 基于provider_name解析基础提供商名称
        let base_provider = if provider_name.contains(':') {
            provider_name.split(':').next().unwrap_or(provider_name)
        } else {
            provider_name
        };

        // 为不同提供商添加特定参数
        match base_provider {
            "google" => {
                form_params.insert("access_type".to_string(), "offline".to_string());
            }
            "gemini" => {
                form_params.insert("access_type".to_string(), "offline".to_string());
            }
            "openai" => {
                form_params.insert("scope".to_string(), "openid profile email".to_string());
            }
            "claude" => {
                // Claude需要特殊的参数
                // form_params.insert("scope".to_string(), "user:inference".to_string());
                form_params.insert("state".to_string(), session.state.clone());
                form_params.insert("expires_in".to_string(), "31536000".to_string()); // 1年有效期
            }
            _ => {}
        }
    }

    /// 从OAuth配置中添加额外参数
    async fn add_config_based_params(
        &self,
        form_params: &mut HashMap<String, String>,
        provider_manager: &OAuthProviderManager,
        provider_name: &str,
    ) -> OAuthResult<()> {
        // 获取提供商配置
        let config = provider_manager.get_config(provider_name).await?;

        // 添加配置中的额外参数
        for (key, value) in &config.extra_params {
            // 只添加Token交换时需要的参数
            if matches!(key.as_str(), "access_type" | "grant_type" | "scope") {
                form_params.insert(key.clone(), value.clone());
            }
        }

        Ok(())
    }

    /// 验证OpenAI令牌
    async fn validate_openai_token(&self, access_token: &str) -> OAuthResult<bool> {
        let response = self
            .http_client
            .get("https://api.openai.com/v1/me")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    /// 验证Claude令牌
    async fn validate_claude_token(&self, access_token: &str) -> OAuthResult<bool> {
        let response = self
            .http_client
            .get("https://api.anthropic.com/v1/me")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        Ok(response.status().is_success())
    }
}

impl Default for TokenExchangeClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Token交换统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenExchangeStats {
    /// 成功交换次数
    pub successful_exchanges: u64,
    /// 失败交换次数
    pub failed_exchanges: u64,
    /// 刷新令牌次数
    pub token_refreshes: u64,
    /// 令牌撤销次数
    pub token_revocations: u64,
    /// 平均交换时间（毫秒）
    pub average_exchange_time_ms: u64,
    /// 各提供商成功率
    pub provider_success_rates: HashMap<String, f64>,
    /// 最后更新时间
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl Default for TokenExchangeStats {
    fn default() -> Self {
        Self {
            successful_exchanges: 0,
            failed_exchanges: 0,
            token_refreshes: 0,
            token_revocations: 0,
            average_exchange_time_ms: 0,
            provider_success_rates: HashMap::new(),
            last_updated: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_response_parsing() {
        let json = r#"{
            "access_token": "test_token",
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": "refresh_token",
            "scope": "read write"
        }"#;

        let response: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.access_token, "test_token");
        assert_eq!(response.token_type, "Bearer");
        assert_eq!(response.expires_in, Some(3600));
        assert_eq!(response.scope, Some("read write".to_string()));
    }

    #[test]
    fn test_error_response_parsing() {
        let json = r#"{
            "error": "invalid_grant",
            "error_description": "The authorization code is invalid"
        }"#;

        let response: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.error, Some("invalid_grant".to_string()));
        assert_eq!(
            response.error_description,
            Some("The authorization code is invalid".to_string())
        );
    }

    #[test]
    fn test_token_exchange_client_creation() {
        let client = TokenExchangeClient::new();
        assert!(format!("{:?}", client).contains("TokenExchangeClient"));
    }
}
