//! `OpenAI` 提供商策略
//!
//! 处理OpenAI特有的逻辑，包括429错误处理、JWT解析等

use crate::auth::oauth_client::JWTParser;
use crate::error::{ErrorContext, Result};
use crate::proxy::ProxyContext;
use crate::proxy::context::ResolvedCredential;
use crate::proxy_err;
use crate::{
    linfo,
    logging::{LogComponent, LogStage},
    lwarn,
};
use chrono::Utc;
use entity::user_provider_keys;
use pingora_http::RequestHeader;
use pingora_proxy::Session;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::ProviderStrategy;

/// `OpenAI` 429错误响应体结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAI429Error {
    pub error: OpenAIErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIErrorDetail {
    pub r#type: String,
    pub message: String,
    pub plan_type: Option<String>,
    pub resets_in_seconds: Option<i64>,
}

#[derive(Default)]
pub struct OpenAIStrategy {
    db: Option<Arc<DatabaseConnection>>,
}

impl OpenAIStrategy {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `从OpenAI` access_token中解析chatgpt-account-id
    fn extract_chatgpt_account_id(access_token: &str) -> Option<String> {
        let jwt_parser = JWTParser::new().ok()?;
        jwt_parser.extract_chatgpt_account_id(access_token).ok()?
    }

    /// 异步处理429错误
    async fn handle_429_error(&self, ctx: &ProxyContext, body: &[u8]) -> Result<()> {
        let Some(db) = self.db.as_ref() else {
            return Ok(());
        };
        let Some(key_id) = ctx.selected_backend.as_ref().map(|k| k.id) else {
            return Ok(());
        };

        if let Some(error_info) = Self::parse_429_response(body) {
            linfo!(
                &ctx.request_id,
                LogStage::Internal,
                LogComponent::OpenAIStrategy,
                "parse_429_error",
                "成功解析OpenAI 429错误，准备更新密钥状态",
                error_type = %error_info.error.r#type
            );
            Self::update_key_health_status_async(db.clone(), key_id, &error_info.error).await?;
        } else {
            lwarn!(
                &ctx.request_id,
                LogStage::Internal,
                LogComponent::OpenAIStrategy,
                "parse_429_error_fail",
                "无法解析OpenAI 429错误响应体"
            );
        }
        Ok(())
    }

    /// 解析429错误响应体
    fn parse_429_response(body: &[u8]) -> Option<OpenAI429Error> {
        serde_json::from_slice(body).ok()
    }

    /// 异步更新API密钥健康状态
    async fn update_key_health_status_async(
        db: Arc<DatabaseConnection>,
        key_id: i32,
        error_detail: &OpenAIErrorDetail,
    ) -> Result<()> {
        let now = Utc::now().naive_utc();
        let rate_limit_resets_at = error_detail
            .resets_in_seconds
            .map(|seconds| now + chrono::Duration::seconds(seconds));

        let mut key: user_provider_keys::ActiveModel =
            user_provider_keys::Entity::find_by_id(key_id)
                .one(db.as_ref())
                .await
                .with_database_context(|| format!("查询API密钥失败，ID: {key_id}"))?
                .ok_or_else(|| proxy_err!(database, "API密钥不存在: {}", key_id))?
                .into();

        key.health_status = Set("rate_limited".to_string());
        key.health_status_detail = Set(Some(
            serde_json::to_string(error_detail)
                .with_database_context(|| "序列化OpenAI错误详情失败".to_string())?,
        ));
        key.rate_limit_resets_at = Set(rate_limit_resets_at);
        key.last_error_time = Set(Some(now));
        key.updated_at = Set(now);

        key.update(db.as_ref())
            .await
            .with_database_context(|| format!("更新API密钥健康状态失败，ID: {key_id}"))?;

        linfo!("system", LogStage::Internal, LogComponent::OpenAIStrategy, "update_key_status", "OpenAI API密钥已更新为详细限流状态", key_id = key_id, error_type = %error_detail.r#type);
        Ok(())
    }
}

#[async_trait::async_trait]
impl ProviderStrategy for OpenAIStrategy {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn set_db_connection(&mut self, db: Option<Arc<DatabaseConnection>>) {
        self.db = db;
    }

    async fn modify_request(
        &self,
        _session: &Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut ProxyContext,
    ) -> Result<()> {
        if let Some(backend) = &ctx.selected_backend
            && backend.auth_type == "oauth"
        {
            upstream_request
                .insert_header("host", "chatgpt.com")
                .with_network_context(|| {
                    format!("设置OpenAI host头失败, request_id: {}", ctx.request_id)
                })?;

            if let Some(ResolvedCredential::OAuthAccessToken(token)) = &ctx.resolved_credential
                && let Some(account_id) = Self::extract_chatgpt_account_id(token)
            {
                ctx.account_id = Some(account_id.clone());
                upstream_request
                    .insert_header("chatgpt-account-id", &account_id)
                    .with_network_context(|| {
                        format!(
                            "设置OpenAI chatgpt-account-id头失败, request_id: {}",
                            ctx.request_id
                        )
                    })?;
            }
        }
        Ok(())
    }

    async fn handle_response_body(
        &self,
        _session: &Session,
        ctx: &ProxyContext,
        status_code: u16,
        body: &[u8],
    ) -> Result<()> {
        if status_code == 429 {
            self.handle_429_error(ctx, body).await?;
        }
        Ok(())
    }

    async fn should_retry_key(&self, key: &user_provider_keys::Model) -> Result<bool> {
        if key.health_status == "rate_limited"
            && let Some(resets_at) = key.rate_limit_resets_at
            && Utc::now().naive_utc() > resets_at
        {
            linfo!(
                "system",
                LogStage::Internal,
                LogComponent::OpenAIStrategy,
                "rate_limit_lifted",
                "OpenAI API密钥限流已解除，恢复使用",
                key_id = key.id
            );
            return Ok(true);
        }
        Ok(key.is_active && key.health_status == "healthy")
    }

    fn build_auth_headers(&self, api_key: &str) -> Vec<(String, String)> {
        vec![("Authorization".to_string(), format!("Bearer {api_key}"))]
    }
}
