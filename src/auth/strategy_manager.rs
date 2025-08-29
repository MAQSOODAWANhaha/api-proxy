//! # 认证策略管理器
//!
//! 专门负责管理和执行各种认证策略

use std::collections::HashMap;
use serde_json::Value;
use tracing::info;

use crate::auth::{
    types::{AuthType, MultiAuthConfig},
    strategies::{traits::{AuthStrategy, OAuthTokenResult}, ApiKeyStrategy, BearerTokenStrategy},
    AuthError,
};
use crate::error::Result;

/// 认证策略管理器
/// 
/// 负责注册、管理和执行各种认证策略
pub struct AuthStrategyManager {
    /// 认证策略注册表
    strategies: HashMap<AuthType, Box<dyn AuthStrategy>>,
    /// 默认配置
    default_configs: HashMap<AuthType, MultiAuthConfig>,
}

impl AuthStrategyManager {
    /// 创建新的认证策略管理器
    pub fn new() -> Self {
        let mut manager = Self {
            strategies: HashMap::new(),
            default_configs: HashMap::new(),
        };
        
        // 注册默认认证策略
        manager.register_default_strategies();
        manager
    }

    /// 注册默认认证策略
    fn register_default_strategies(&mut self) {
        // 注册API密钥策略
        let api_key_strategy = Box::new(ApiKeyStrategy::new("Authorization", "Bearer {key}"));
        self.register_strategy(api_key_strategy);
        
        // 注册Bearer Token策略
        let bearer_token_strategy = Box::new(BearerTokenStrategy::new());
        self.register_strategy(bearer_token_strategy);
        
        info!("Default auth strategies registered in AuthStrategyManager");
    }

    /// 注册认证策略
    pub fn register_strategy(&mut self, strategy: Box<dyn AuthStrategy>) {
        let auth_type = strategy.auth_type();
        self.strategies.insert(auth_type, strategy);
    }

    /// 获取支持的认证类型列表
    pub fn supported_auth_types(&self) -> Vec<AuthType> {
        self.strategies.keys().cloned().collect()
    }

    /// 设置默认配置
    pub fn set_default_config(&mut self, auth_type: AuthType, config: MultiAuthConfig) {
        self.default_configs.insert(auth_type, config);
    }

    /// 获取默认配置
    pub fn get_default_config(&self, auth_type: &AuthType) -> Option<&MultiAuthConfig> {
        self.default_configs.get(auth_type)
    }

    /// 验证配置
    pub fn validate_config(&self, auth_type: &AuthType, config: &Value) -> Result<()> {
        let strategy = self.strategies.get(auth_type)
            .ok_or_else(|| AuthError::InvalidAuthType(format!("不支持的认证类型: {:?}", auth_type)))?;

        strategy.validate_config(config).map_err(Into::into)
    }

    /// 多认证接口 - 使用集成的认证策略系统
    pub async fn multi_authenticate(
        &self,
        auth_type: &AuthType,
        credentials: &Value,
    ) -> Result<OAuthTokenResult> {
        let strategy = self.strategies.get(auth_type)
            .ok_or_else(|| AuthError::InvalidAuthType(format!("不支持的认证类型: {:?}", auth_type)))?;

        strategy.authenticate(credentials).await.map_err(Into::into)
    }
    
    /// 刷新认证凭据
    pub async fn refresh_multi_auth(
        &self,
        auth_type: &AuthType,
        refresh_token: &str,
    ) -> Result<OAuthTokenResult> {
        let strategy = self.strategies.get(auth_type)
            .ok_or_else(|| AuthError::InvalidAuthType(format!("不支持的认证类型: {:?}", auth_type)))?;

        strategy.refresh(refresh_token).await.map_err(Into::into)
    }
    
    /// 撤销认证凭据
    pub async fn revoke_multi_auth(
        &self,
        auth_type: &AuthType,
        token: &str,
    ) -> Result<()> {
        let strategy = self.strategies.get(auth_type)
            .ok_or_else(|| AuthError::InvalidAuthType(format!("不支持的认证类型: {:?}", auth_type)))?;

        strategy.revoke(token).await.map_err(Into::into)
    }
    
    /// 获取OAuth认证URL
    pub async fn get_oauth_auth_url(
        &self,
        auth_type: &AuthType,
        state: &str,
        redirect_uri: &str,
    ) -> Result<String> {
        let strategy = self.strategies.get(auth_type)
            .ok_or_else(|| AuthError::InvalidAuthType(format!("不支持的认证类型: {:?}", auth_type)))?;

        strategy.get_auth_url(state, redirect_uri).await.map_err(Into::into)
    }
    
    /// 处理OAuth回调
    pub async fn handle_oauth_callback(
        &self,
        auth_type: &AuthType,
        code: &str,
        state: &str,
    ) -> Result<OAuthTokenResult> {
        let strategy = self.strategies.get(auth_type)
            .ok_or_else(|| AuthError::InvalidAuthType(format!("不支持的认证类型: {:?}", auth_type)))?;

        strategy.handle_callback(code, state).await.map_err(Into::into)
    }
    
    /// 动态注册OAuth2策略
    pub async fn register_oauth2_strategy(
        &mut self,
        client_id: String,
        client_secret: String,
        auth_url: String,
        token_url: String,
    ) -> Result<()> {
        use crate::auth::oauth::strategies::OAuth2Strategy;
        let strategy = Box::new(OAuth2Strategy::new(
            client_id,
            client_secret,
            auth_url,
            token_url,
        ));
        self.register_strategy(strategy);
        Ok(())
    }
}

impl Default for AuthStrategyManager {
    fn default() -> Self {
        Self::new()
    }
}