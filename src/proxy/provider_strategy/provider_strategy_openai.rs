//! `OpenAI` 提供商策略
//!
//! 处理OpenAI特有的逻辑，包括429错误处理、JWT解析等

use crate::auth::oauth_client::JWTParser;
use crate::error::{Context, Result};
use crate::key_pool::ApiKeyHealthService;
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
use std::collections::HashMap;
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
    health_checker: Option<Arc<ApiKeyHealthService>>,
}

impl OpenAIStrategy {
    #[must_use]
    pub const fn new(health_checker: Option<Arc<ApiKeyHealthService>>) -> Self {
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

    /// 记录OpenAI返回的主/次限流窗口信息
    fn log_openai_rate_limits(ctx: &ProxyContext) {
        let headers = &ctx.response_details.headers;
        if headers.is_empty() {
            return;
        }

        let Some(snapshot) = Self::extract_rate_limit_snapshot_from_headers(headers) else {
            return;
        };

        let primary_used_percent = snapshot.primary.as_ref().map(|w| w.used_percent);
        let primary_window_seconds = snapshot.primary.as_ref().and_then(|w| w.window_seconds);
        let primary_resets_at = snapshot.primary.as_ref().and_then(|w| w.resets_at);

        let secondary_used_percent = snapshot.secondary.as_ref().map(|w| w.used_percent);
        let secondary_window_seconds = snapshot.secondary.as_ref().and_then(|w| w.window_seconds);
        let secondary_resets_at = snapshot.secondary.as_ref().and_then(|w| w.resets_at);

        linfo!(
            &ctx.request_id,
            LogStage::Response,
            LogComponent::OpenAIStrategy,
            "openai_rate_limit_snapshot",
            "记录OpenAI返回的限流窗口信息",
            primary_used_percent = primary_used_percent,
            primary_window_seconds = primary_window_seconds,
            primary_resets_at = primary_resets_at,
            secondary_used_percent = secondary_used_percent,
            secondary_window_seconds = secondary_window_seconds,
            secondary_resets_at = secondary_resets_at
        );
    }

    fn extract_rate_limit_snapshot_from_headers(
        headers: &HashMap<String, String>,
    ) -> Option<RateLimitSnapshotLog> {
        let primary = Self::parse_rate_limit_window_from_headers(headers, "x-codex-primary");
        let secondary = Self::parse_rate_limit_window_from_headers(headers, "x-codex-secondary");

        if primary.is_none() && secondary.is_none() {
            return None;
        }

        Some(RateLimitSnapshotLog { primary, secondary })
    }

    fn parse_rate_limit_window_from_headers(
        headers: &HashMap<String, String>,
        prefix: &str,
    ) -> Option<RateLimitWindowLog> {
        let used_percent =
            Self::parse_header_value_f64(headers, &format!("{prefix}-used-percent"))?;
        let window_seconds =
            Self::parse_header_value_i64(headers, &format!("{prefix}-window-minutes"))
                .map(|minutes| minutes.saturating_mul(60));
        let resets_at = Self::parse_header_value_i64(headers, &format!("{prefix}-reset-at"));

        Some(RateLimitWindowLog {
            used_percent,
            window_seconds,
            resets_at,
        })
    }

    fn parse_header_value_f64(headers: &HashMap<String, String>, name: &str) -> Option<f64> {
        Self::find_header_value_case_insensitive(headers, name)?
            .parse::<f64>()
            .ok()
    }

    fn parse_header_value_i64(headers: &HashMap<String, String>, name: &str) -> Option<i64> {
        Self::find_header_value_case_insensitive(headers, name)?
            .parse::<i64>()
            .ok()
    }

    fn find_header_value_case_insensitive<'a>(
        headers: &'a HashMap<String, String>,
        name: &str,
    ) -> Option<&'a str> {
        headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
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
        Self::log_openai_rate_limits(ctx);

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

#[derive(Debug)]
struct RateLimitSnapshotLog {
    primary: Option<RateLimitWindowLog>,
    secondary: Option<RateLimitWindowLog>,
}

#[derive(Debug)]
struct RateLimitWindowLog {
    used_percent: f64,
    window_seconds: Option<i64>,
    resets_at: Option<i64>,
}
