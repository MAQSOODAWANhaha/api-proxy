//! # 认证策略特质
//!
//! 定义所有认证策略必须实现的基础接口

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::auth::types::AuthType;
use crate::error::Result;

/// `OAuth认证返回结果`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenResult {
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌（可选）
    pub refresh_token: Option<String>,
    /// 令牌类型
    pub token_type: String,
    /// 过期时间（秒）
    pub expires_in: Option<i64>,
    /// 作用域
    pub scope: Option<String>,
    /// 用户信息（可选）
    pub user_info: Option<serde_json::Value>,
}

/// 认证策略接口
#[async_trait]
pub trait AuthStrategy: Send + Sync {
    /// 认证策略的类型
    fn auth_type(&self) -> AuthType;

    /// 验证认证凭据
    async fn authenticate(&self, credentials: &serde_json::Value) -> Result<OAuthTokenResult>;

    /// 刷新认证凭据（如果支持）
    async fn refresh(&self, _refresh_token: &str) -> Result<OAuthTokenResult> {
        Err(crate::error!(Config, "刷新操作不支持"))
    }

    /// 撤销认证（如果支持）
    async fn revoke(&self, _token: &str) -> Result<()> {
        Err(crate::error!(Config, "撤销操作不支持"))
    }

    /// 验证配置是否有效
    fn validate_config(&self, config: &serde_json::Value) -> Result<()>;

    /// 获取认证URL（用于OAuth流程）
    async fn get_auth_url(&self, _state: &str, _redirect_uri: &str) -> Result<String> {
        Err(crate::error!(Config, "不支持授权URL生成"))
    }

    /// 处理回调（用于OAuth流程）
    async fn handle_callback(&self, _code: &str, _state: &str) -> Result<OAuthTokenResult> {
        Err(crate::error!(Config, "不支持回调处理"))
    }
}
