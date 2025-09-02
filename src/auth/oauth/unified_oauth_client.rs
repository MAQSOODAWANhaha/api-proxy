//! 统一OAuth客户端实现
//!
//! 提供统一的OAuth认证接口，封装数据库驱动的OAuth管理器

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use super::{OAuth2Config, OAuth2Result, SimpleOAuthManager};
use crate::auth::strategies::traits::{AuthStrategy, OAuthTokenResult};
use crate::auth::types::AuthType;

/// 统一OAuth客户端
pub struct UnifiedOAuthClient {
    /// OAuth管理器
    oauth_manager: Arc<SimpleOAuthManager>,
    /// 提供商名称
    provider_name: String,
    /// 认证类型
    auth_type: String,
}

impl UnifiedOAuthClient {
    /// 创建新的统一OAuth客户端
    pub async fn new(
        db: DatabaseConnection,
        provider_name: String,
        auth_type: String,
    ) -> OAuth2Result<Self> {
        let oauth_manager = Arc::new(SimpleOAuthManager::new(db).await?);

        // 验证配置存在
        oauth_manager
            .load_oauth_config(&provider_name, &auth_type)
            .await?;

        Ok(Self {
            oauth_manager,
            provider_name,
            auth_type,
        })
    }

    /// 获取授权URL
    pub async fn get_authorization_url(
        &self,
        state: &str,
        redirect_uri: &str,
    ) -> OAuth2Result<(String, Option<String>)> {
        self.oauth_manager
            .get_authorization_url(&self.provider_name, &self.auth_type, state, redirect_uri)
            .await
    }

    /// 交换授权码获取令牌
    pub async fn exchange_code_for_token(
        &self,
        code: &str,
        redirect_uri: &str,
        code_verifier: Option<&str>,
    ) -> OAuth2Result<OAuthTokenResult> {
        self.oauth_manager
            .exchange_code_for_token(
                &self.provider_name,
                &self.auth_type,
                code,
                redirect_uri,
                code_verifier,
            )
            .await
    }

    /// 刷新访问令牌
    pub async fn refresh_token(&self, refresh_token: &str) -> OAuth2Result<OAuthTokenResult> {
        self.oauth_manager
            .refresh_access_token(&self.provider_name, &self.auth_type, refresh_token)
            .await
    }

    /// 撤销令牌
    pub async fn revoke_token(&self, token: &str) -> OAuth2Result<()> {
        self.oauth_manager
            .revoke_token(&self.provider_name, &self.auth_type, token)
            .await
    }

    /// 获取OAuth配置
    pub async fn get_config(&self) -> OAuth2Result<OAuth2Config> {
        self.oauth_manager
            .load_oauth_config(&self.provider_name, &self.auth_type)
            .await
    }

    /// 获取提供商名称
    pub fn provider_name(&self) -> &str {
        &self.provider_name
    }

    /// 获取认证类型
    pub fn auth_type(&self) -> &str {
        &self.auth_type
    }

    /// 验证配置
    pub async fn validate_config(&self) -> OAuth2Result<bool> {
        match self.get_config().await {
            Ok(config) => {
                config.validate()?;
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }
}

#[async_trait]
impl AuthStrategy for UnifiedOAuthClient {
    fn auth_type(&self) -> AuthType {
        match self.auth_type.as_str() {
            "google_oauth" => AuthType::GoogleOAuth,
            "oauth2" => AuthType::OAuth2,
            _ => AuthType::OAuth2, // 默认类型
        }
    }

    async fn authenticate(
        &self,
        credentials: &serde_json::Value,
    ) -> Result<OAuthTokenResult, crate::auth::types::AuthError> {
        let grant_type = credentials
            .get("grant_type")
            .and_then(|v| v.as_str())
            .unwrap_or("authorization_code");

        match grant_type {
            "authorization_code" => {
                let code = credentials
                    .get("code")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::auth::types::AuthError::ConfigError("缺少授权码".to_string())
                    })?;

                let redirect_uri = credentials
                    .get("redirect_uri")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::auth::types::AuthError::ConfigError("缺少回调URI".to_string())
                    })?;

                let code_verifier = credentials.get("code_verifier").and_then(|v| v.as_str());

                self.exchange_code_for_token(code, redirect_uri, code_verifier)
                    .await
                    .map_err(|e| crate::auth::types::AuthError::OAuth2Error(e.to_string()))
            }
            _ => Err(crate::auth::types::AuthError::ConfigError(format!(
                "不支持的授权类型: {}",
                grant_type
            ))),
        }
    }

    async fn refresh(
        &self,
        refresh_token: &str,
    ) -> Result<OAuthTokenResult, crate::auth::types::AuthError> {
        self.refresh_token(refresh_token)
            .await
            .map_err(|e| crate::auth::types::AuthError::OAuth2Error(e.to_string()))
    }

    async fn revoke(&self, token: &str) -> Result<(), crate::auth::types::AuthError> {
        self.revoke_token(token)
            .await
            .map_err(|e| crate::auth::types::AuthError::OAuth2Error(e.to_string()))
    }

    async fn get_auth_url(
        &self,
        state: &str,
        redirect_uri: &str,
    ) -> Result<String, crate::auth::types::AuthError> {
        let (url, _code_verifier) = self
            .get_authorization_url(state, redirect_uri)
            .await
            .map_err(|e| crate::auth::types::AuthError::OAuth2Error(e.to_string()))?;
        Ok(url)
    }

    async fn handle_callback(
        &self,
        _code: &str,
        _state: &str,
    ) -> Result<OAuthTokenResult, crate::auth::types::AuthError> {
        // 简化实现，实际应用需要从会话获取redirect_uri和code_verifier
        Err(crate::auth::types::AuthError::ConfigError(
            "需要完整的OAuth会话支持".to_string(),
        ))
    }

    fn validate_config(
        &self,
        _config: &serde_json::Value,
    ) -> Result<(), crate::auth::types::AuthError> {
        // 配置验证通过OAuth管理器实现
        Ok(())
    }
}

/// OAuth客户端工厂
pub struct UnifiedOAuthClientFactory {
    db: DatabaseConnection,
}

impl UnifiedOAuthClientFactory {
    /// 创建新的工厂实例
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// 创建Google OAuth客户端
    pub async fn create_google_oauth_client(&self) -> OAuth2Result<UnifiedOAuthClient> {
        UnifiedOAuthClient::new(
            self.db.clone(),
            "gemini".to_string(),
            "google_oauth".to_string(),
        )
        .await
    }

    /// 创建Claude OAuth客户端
    pub async fn create_claude_oauth_client(&self) -> OAuth2Result<UnifiedOAuthClient> {
        UnifiedOAuthClient::new(self.db.clone(), "claude".to_string(), "oauth2".to_string()).await
    }

    /// 根据提供商和认证类型创建客户端
    pub async fn create_oauth_client(
        &self,
        provider_name: &str,
        auth_type: &str,
    ) -> OAuth2Result<UnifiedOAuthClient> {
        UnifiedOAuthClient::new(
            self.db.clone(),
            provider_name.to_string(),
            auth_type.to_string(),
        )
        .await
    }

    /// 获取所有可用的OAuth客户端配置
    pub async fn list_available_oauth_configs(&self) -> OAuth2Result<Vec<(String, String)>> {
        let oauth_manager = SimpleOAuthManager::new(self.db.clone()).await?;
        let all_configs = oauth_manager.load_all_oauth_configs().await?;

        let mut configs = Vec::new();
        for (provider_name, auth_configs) in all_configs {
            for auth_type in auth_configs.keys() {
                configs.push((provider_name.clone(), auth_type.clone()));
            }
        }

        Ok(configs)
    }
}
