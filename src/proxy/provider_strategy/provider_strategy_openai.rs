//! `OpenAI` 提供商策略
//!
//! 处理OpenAI特有的逻辑，包括429错误处理、JWT解析等

use crate::auth::oauth_client::JWTParser;
use crate::error::{Context, Result};
use crate::key_pool::ApiKeyHealthChecker;
use crate::logging::{LogComponent, LogStage};
use crate::proxy::ProxyContext;
use crate::proxy::context::ResolvedCredential;
use crate::proxy::prelude::ProviderStrategy;
use crate::{linfo, lwarn};
use chrono::Utc;
use entity::user_provider_keys;
use pingora_http::RequestHeader;
use pingora_proxy::Session;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    health_checker: Option<Arc<ApiKeyHealthChecker>>,
}

impl OpenAIStrategy {
    #[must_use]
    pub const fn new(health_checker: Option<Arc<ApiKeyHealthChecker>>) -> Self {
        Self { health_checker }
    }

    /// `从OpenAI` access_token中解析chatgpt-account-id
    fn extract_chatgpt_account_id(access_token: &str) -> Option<String> {
        let jwt_parser = JWTParser;
        jwt_parser.extract_chatgpt_account_id(access_token).ok()?
    }

    /// 异步处理429错误
    async fn handle_429_error(&self, ctx: &ProxyContext, body: &[u8]) -> Result<()> {
        let Some(health_checker) = self.health_checker.as_ref() else {
            return Ok(());
        };
        let Some(key_id) = ctx.selected_backend.as_ref().map(|k| k.id) else {
            return Ok(());
        };

        if let Ok(error_info) = serde_json::from_slice::<OpenAI429Error>(body) {
            linfo!(
                &ctx.request_id,
                LogStage::Internal,
                LogComponent::OpenAIStrategy,
                "parse_429_error",
                "成功解析OpenAI 429错误，准备更新密钥状态",
                error_type = %error_info.error.r#type
            );
            let resets_at = error_info
                .error
                .resets_in_seconds
                .map(|seconds| (Utc::now() + chrono::Duration::seconds(seconds)).naive_utc());
            let details = serde_json::to_string(&error_info.error).unwrap_or_default();
            health_checker
                .mark_key_as_rate_limited(key_id, resets_at, &details)
                .await?;
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
}

#[async_trait::async_trait]
impl ProviderStrategy for OpenAIStrategy {
    fn name(&self) -> &'static str {
        "openai"
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
                .context(format!(
                    "设置OpenAI host头失败, request_id: {}",
                    ctx.request_id
                ))?;

            if let Some(ResolvedCredential::OAuthAccessToken(token)) = &ctx.resolved_credential
                && let Some(account_id) = Self::extract_chatgpt_account_id(token)
            {
                ctx.account_id = Some(account_id.clone());
                upstream_request
                    .insert_header("chatgpt-account-id", &account_id)
                    .context(format!(
                        "设置OpenAI chatgpt-account-id头失败, request_id: {}",
                        ctx.request_id
                    ))?;
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
