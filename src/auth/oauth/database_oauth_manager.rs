//! 数据库驱动的OAuth管理器
//!
//! 从数据库动态加载OAuth配置，创建OAuth客户端，管理OAuth流程

use std::collections::HashMap;
use std::sync::Arc;
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, QuerySelect};
use tokio::sync::RwLock;
use reqwest::Client as HttpClient;
use oauth2::{
    basic::{BasicClient, BasicTokenResponse},
    AuthUrl, TokenUrl, ClientId, ClientSecret,
    RedirectUrl, CsrfToken, PkceCodeChallenge, PkceCodeVerifier,
    AuthorizationCode, Scope, RefreshToken,
    AuthorizationRequest, TokenRequest, RequestTokenError,
};

use super::{OAuth2Config, OAuth2Error, OAuth2Result};
use crate::auth::strategies::traits::OAuthTokenResult;

/// 数据库驱动的OAuth管理器
pub struct DatabaseOAuthManager {
    /// 数据库连接
    db: DatabaseConnection,
    /// HTTP客户端用于API调用
    http_client: HttpClient,
    /// OAuth配置缓存 (provider_name -> auth_type -> config)
    config_cache: Arc<RwLock<HashMap<String, HashMap<String, OAuth2Config>>>>,
    /// OAuth客户端缓存
    client_cache: Arc<RwLock<HashMap<String, BasicClient>>>,
}

impl DatabaseOAuthManager {
    /// 创建新的数据库OAuth管理器
    pub async fn new(db: DatabaseConnection) -> OAuth2Result<Self> {
        Ok(Self {
            db,
            http_client: HttpClient::new(),
            config_cache: Arc::new(RwLock::new(HashMap::new())),
            client_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 从数据库加载特定提供商的OAuth配置
    pub async fn load_oauth_config(&self, provider_name: &str, auth_type: &str) -> OAuth2Result<OAuth2Config> {
        // 首先检查缓存
        {
            let cache = self.config_cache.read().await;
            if let Some(provider_configs) = cache.get(provider_name) {
                if let Some(config) = provider_configs.get(auth_type) {
                    return Ok(config.clone());
                }
            }
        }

        // 从数据库查询
        let provider_type = entity::provider_types::Entity::find()
            .filter(entity::provider_types::Column::Name.eq(provider_name))
            .filter(entity::provider_types::Column::IsActive.eq(true))
            .one(&self.db)
            .await
            .map_err(OAuth2Error::DatabaseError)?
            .ok_or_else(|| OAuth2Error::unsupported_provider(provider_name))?;

        // 解析认证配置
        let auth_configs_json = provider_type.auth_configs_json
            .ok_or_else(|| OAuth2Error::config_error(format!("{}没有配置认证信息", provider_name)))?;

        let auth_configs: serde_json::Value = serde_json::from_str(&auth_configs_json)?;
        
        let auth_config = auth_configs.get(auth_type)
            .ok_or_else(|| OAuth2Error::unsupported_auth_type(format!("{}不支持{}认证类型", provider_name, auth_type)))?;

        // 创建OAuth2配置
        let oauth_config = OAuth2Config::from_json(auth_type, auth_config)?;
        oauth_config.validate()?;

        // 更新缓存
        {
            let mut cache = self.config_cache.write().await;
            cache.entry(provider_name.to_string())
                .or_insert_with(HashMap::new)
                .insert(auth_type.to_string(), oauth_config.clone());
        }

        Ok(oauth_config)
    }

    /// 创建OAuth2客户端
    pub async fn create_oauth_client(&self, provider_name: &str, auth_type: &str) -> OAuth2Result<BasicClient> {
        let cache_key = format!("{}:{}", provider_name, auth_type);
        
        // 检查客户端缓存
        {
            let cache = self.client_cache.read().await;
            if let Some(client) = cache.get(&cache_key) {
                return Ok(client.clone());
            }
        }

        // 加载配置并创建客户端
        let config = self.load_oauth_config(provider_name, auth_type).await?;
        let client = self.build_oauth_client(&config)?;

        // 更新客户端缓存
        {
            let mut cache = self.client_cache.write().await;
            cache.insert(cache_key, client.clone());
        }

        Ok(client)
    }

    /// 构建OAuth2客户端 
    fn build_oauth_client(&self, config: &OAuth2Config) -> OAuth2Result<BasicClient> {
        let auth_url = AuthUrl::new(config.authorize_url.clone())
            .map_err(|e| OAuth2Error::config_error(format!("无效的授权URL: {}", e)))?;

        let token_url = Some(TokenUrl::new(config.token_url.clone())
            .map_err(|e| OAuth2Error::config_error(format!("无效的令牌URL: {}", e)))?);

        let client = BasicClient::new(
            ClientId::new(config.client_id.clone()),
            Some(ClientSecret::new(config.client_secret.clone())),
            auth_url,
            token_url,
        );

        Ok(client)
    }

    /// 生成授权URL
    pub async fn get_authorization_url(
        &self,
        provider_name: &str,
        auth_type: &str,
        state: &str,
        redirect_uri: &str,
    ) -> OAuth2Result<(String, Option<String>)> {
        let client = self.create_oauth_client(provider_name, auth_type).await?;
        let config = self.load_oauth_config(provider_name, auth_type).await?;

        let redirect_url = RedirectUrl::new(redirect_uri.to_string())
            .map_err(|e| OAuth2Error::config_error(format!("无效的重定向URI: {}", e)))?;

        // 构建授权请求
        let mut auth_request = client
            .authorize_url(CsrfToken::new_random)
            .set_redirect_uri(std::borrow::Cow::Owned(redirect_url));

        // 添加作用域
        if !config.scopes.is_empty() {
            for scope in config.scopes.split_whitespace() {
                auth_request = auth_request.add_scope(Scope::new(scope.to_string()));
            }
        }

        // 设置状态
        auth_request = auth_request.set_state(CsrfToken::new(state.to_string()));

        // PKCE支持
        let mut code_verifier = None;
        if config.pkce_required {
            let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
            auth_request = auth_request.set_pkce_challenge(pkce_challenge);
            code_verifier = Some(pkce_verifier.secret().clone());
        }

        // 生成授权URL
        let (mut auth_url, _state) = auth_request.url();

        // 添加额外参数
        if !config.extra_params.is_empty() {
            let mut url = url::Url::parse(auth_url.as_str())
                .map_err(|e| OAuth2Error::config_error(format!("URL解析失败: {}", e)))?;
            
            {
                let mut query_pairs = url.query_pairs_mut();
                for (key, value) in &config.extra_params {
                    query_pairs.append_pair(key, value);
                }
            }
            
            auth_url = url;
        }

        Ok((auth_url.to_string(), code_verifier))
    }

    /// 交换授权码获取令牌
    pub async fn exchange_code_for_token(
        &self,
        provider_name: &str,
        auth_type: &str,
        code: &str,
        redirect_uri: &str,
        code_verifier: Option<&str>,
    ) -> OAuth2Result<OAuthTokenResult> {
        let client = self.create_oauth_client(provider_name, auth_type).await?;
        
        let redirect_url = RedirectUrl::new(redirect_uri.to_string())
            .map_err(|e| OAuth2Error::config_error(format!("无效的重定向URI: {}", e)))?;

        // 构建令牌交换请求
        let mut token_request = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_redirect_uri(std::borrow::Cow::Owned(redirect_url));

        // 添加PKCE验证码
        if let Some(verifier) = code_verifier {
            let pkce_verifier = PkceCodeVerifier::new(verifier.to_string());
            token_request = token_request.set_pkce_verifier(pkce_verifier);
        }

        // 执行令牌交换
        let token_response = token_request
            .request_async(&self.http_client)
            .await
            .map_err(|e| OAuth2Error::token_exchange_error(format!("令牌交换失败: {}", e)))?;

        // 转换为我们的结果格式
        let mut result = OAuthTokenResult {
            access_token: token_response.access_token().secret().clone(),
            token_type: token_response.token_type().as_ref().to_string(),
            expires_in: token_response.expires_in().map(|d| d.as_secs() as i64),
            refresh_token: token_response.refresh_token().map(|t| t.secret().clone()),
            scope: token_response.scopes().map(|scopes| {
                scopes.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ")
            }),
            user_info: None,
        };

        // 如果支持，获取用户信息
        if auth_type == "google_oauth" {
            result.user_info = self.fetch_google_user_info(&result.access_token).await.ok();
        }

        Ok(result)
    }

    /// 刷新访问令牌
    pub async fn refresh_access_token(
        &self,
        provider_name: &str,
        auth_type: &str,
        refresh_token: &str,
    ) -> OAuth2Result<OAuthTokenResult> {
        let client = self.create_oauth_client(provider_name, auth_type).await?;

        let refresh_token_obj = oauth2::RefreshToken::new(refresh_token.to_string());
        
        let token_response = client
            .exchange_refresh_token(&refresh_token_obj)
            .request_async(&self.http_client)
            .await
            .map_err(|e| OAuth2Error::TokenRefreshError(format!("令牌刷新失败: {}", e)))?;

        // 转换为我们的结果格式
        Ok(OAuthTokenResult {
            access_token: token_response.access_token().secret().clone(),
            token_type: token_response.token_type().as_ref().to_string(),
            expires_in: token_response.expires_in().map(|d| d.as_secs() as i64),
            refresh_token: token_response.refresh_token().map(|t| t.secret().clone())
                .or(Some(refresh_token.to_string())), // 保持原刷新令牌
            scope: token_response.scopes().map(|scopes| {
                scopes.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ")
            }),
            user_info: None,
        })
    }

    /// 撤销令牌
    pub async fn revoke_token(
        &self,
        provider_name: &str,
        auth_type: &str,
        token: &str,
    ) -> OAuth2Result<()> {
        let config = self.load_oauth_config(provider_name, auth_type).await?;
        
        if let Some(revoke_url) = &config.revoke_url {
            let response = self.http_client
                .post(revoke_url)
                .form(&[("token", token)])
                .send()
                .await
                .map_err(|e| OAuth2Error::network_error(format!("撤销请求失败: {}", e)))?;

            if !response.status().is_success() {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                return Err(OAuth2Error::TokenRevokeError(format!("令牌撤销失败: {}", error_text)));
            }
        } else {
            // 如果没有撤销端点，记录警告但不报错
            tracing::warn!("{}的{}认证类型没有配置撤销端点", provider_name, auth_type);
        }

        Ok(())
    }

    /// 加载所有OAuth配置（用于初始化）
    pub async fn load_all_oauth_configs(&self) -> OAuth2Result<HashMap<String, HashMap<String, OAuth2Config>>> {
        let provider_types = entity::provider_types::Entity::find()
            .filter(entity::provider_types::Column::IsActive.eq(true))
            .all(&self.db)
            .await
            .map_err(OAuth2Error::DatabaseError)?;

        let mut all_configs = HashMap::new();

        for provider_type in provider_types {
            if let Some(auth_configs_json) = provider_type.auth_configs_json {
                let auth_configs: serde_json::Value = serde_json::from_str(&auth_configs_json)
                    .map_err(|e| OAuth2Error::config_error(format!("解析{}的认证配置失败: {}", provider_type.name, e)))?;

                let mut provider_configs = HashMap::new();

                if let Some(configs_obj) = auth_configs.as_object() {
                    for (auth_type, config_value) in configs_obj {
                        // 只处理OAuth类型的认证
                        if auth_type.contains("oauth") {
                            match OAuth2Config::from_json(auth_type, config_value) {
                                Ok(oauth_config) => {
                                    if oauth_config.validate().is_ok() {
                                        provider_configs.insert(auth_type.clone(), oauth_config);
                                    } else {
                                        tracing::warn!("跳过无效的OAuth配置: {}:{}", provider_type.name, auth_type);
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("解析OAuth配置失败: {}:{} - {}", provider_type.name, auth_type, e);
                                }
                            }
                        }
                    }
                }

                if !provider_configs.is_empty() {
                    all_configs.insert(provider_type.name.clone(), provider_configs);
                }
            }
        }

        // 更新缓存
        {
            let mut cache = self.config_cache.write().await;
            *cache = all_configs.clone();
        }

        Ok(all_configs)
    }

    /// 清除配置缓存
    pub async fn clear_cache(&self) {
        let mut config_cache = self.config_cache.write().await;
        let mut client_cache = self.client_cache.write().await;
        config_cache.clear();
        client_cache.clear();
        tracing::info!("OAuth配置缓存已清除");
    }

    /// 获取Google用户信息（特殊处理）
    async fn fetch_google_user_info(&self, access_token: &str) -> OAuth2Result<serde_json::Value> {
        let userinfo_url = "https://www.googleapis.com/oauth2/v2/userinfo";
        
        let response = self.http_client
            .get(userinfo_url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| OAuth2Error::network_error(format!("用户信息请求失败: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(OAuth2Error::authentication_error(format!(
                "获取Google用户信息失败 ({}): {}", status, error_text
            )));
        }

        let user_info: serde_json::Value = response.json().await
            .map_err(|e| OAuth2Error::network_error(format!("JSON解析失败: {}", e)))?;

        Ok(user_info)
    }

    /// 验证OAuth配置是否有效
    pub async fn validate_oauth_config(&self, provider_name: &str, auth_type: &str) -> OAuth2Result<bool> {
        match self.load_oauth_config(provider_name, auth_type).await {
            Ok(config) => {
                config.validate()?;
                // 尝试创建客户端以进一步验证
                self.build_oauth_client(&config)?;
                Ok(true)
            }
            Err(_) => Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    async fn create_test_db() -> DatabaseConnection {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database_url = format!("sqlite://{}", db_path.display());

        let db = sea_orm::Database::connect(&database_url).await.unwrap();
        
        // 运行迁移创建表结构
        migration::Migrator::up(&db, None).await.unwrap();

        db
    }

    #[tokio::test]
    async fn test_database_oauth_manager_creation() {
        let db = create_test_db().await;
        let manager = DatabaseOAuthManager::new(db).await;
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_load_all_oauth_configs() {
        let db = create_test_db().await;
        let manager = DatabaseOAuthManager::new(db).await.unwrap();
        
        // 加载所有OAuth配置
        let configs = manager.load_all_oauth_configs().await.unwrap();
        
        // 验证加载了预期的提供商
        assert!(configs.contains_key("gemini"));
        assert!(configs.contains_key("claude"));
        
        // 验证Google OAuth配置
        if let Some(gemini_configs) = configs.get("gemini") {
            assert!(gemini_configs.contains_key("google_oauth"));
        }
        
        // 验证Claude OAuth配置
        if let Some(claude_configs) = configs.get("claude") {
            assert!(claude_configs.contains_key("oauth2"));
        }
    }

    #[tokio::test]
    async fn test_create_oauth_client() {
        let db = create_test_db().await;
        let manager = DatabaseOAuthManager::new(db).await.unwrap();
        
        // 测试创建Google OAuth客户端
        let client_result = manager.create_oauth_client("gemini", "google_oauth").await;
        assert!(client_result.is_ok());
    }

    #[tokio::test]
    async fn test_cache_functionality() {
        let db = create_test_db().await;
        let manager = DatabaseOAuthManager::new(db).await.unwrap();
        
        // 第一次加载配置
        let config1 = manager.load_oauth_config("gemini", "google_oauth").await.unwrap();
        
        // 第二次加载应该从缓存获取
        let config2 = manager.load_oauth_config("gemini", "google_oauth").await.unwrap();
        
        assert_eq!(config1.client_id, config2.client_id);
        assert_eq!(config1.authorize_url, config2.authorize_url);
    }

    #[tokio::test]
    async fn test_unsupported_provider() {
        let db = create_test_db().await;
        let manager = DatabaseOAuthManager::new(db).await.unwrap();
        
        // 测试不支持的提供商
        let result = manager.load_oauth_config("unknown_provider", "oauth2").await;
        assert!(result.is_err());
        
        if let Err(OAuth2Error::UnsupportedProvider(provider)) = result {
            assert_eq!(provider, "unknown_provider");
        }
    }

    #[tokio::test]
    async fn test_validate_oauth_config() {
        let db = create_test_db().await;
        let manager = DatabaseOAuthManager::new(db).await.unwrap();
        
        // 验证有效配置
        let is_valid = manager.validate_oauth_config("gemini", "google_oauth").await.unwrap();
        assert!(is_valid);
        
        // 验证无效配置
        let is_invalid = manager.validate_oauth_config("unknown", "oauth2").await.unwrap();
        assert!(!is_invalid);
    }
}