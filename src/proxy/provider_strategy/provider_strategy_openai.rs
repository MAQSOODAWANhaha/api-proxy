//! `OpenAI` 提供商策略
//!
//! 处理OpenAI特有的逻辑，包括429错误处理、JWT解析等

use crate::auth::openai::OpenAI;
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
use serde_json::Value;
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

const CODEX_INSTRUCTIONS: &str = "You are Codex, based on GPT-5. You are running as a coding agent in the Codex CLI on a user's computer.\n\n## General\n\n- When searching for text or files, prefer using `rg` or `rg --files` respectively because `rg` is much faster than alternatives like `grep`. (If the `rg` command is not found, then use alternatives.)\n\n## Editing constraints\n\n- Default to ASCII when editing or creating files. Only introduce non-ASCII or other Unicode characters when there is a clear justification and the file already uses them.\n- Add succinct code comments that explain what is going on if code is not self-explanatory. You should not add comments like \"Assigns the value to the variable\", but a brief comment might be useful ahead of a complex code block that the user would otherwise have to spend time parsing out. Usage of these comments should be rare.\n- Try to use apply_patch for single file edits, but it is fine to explore other options to make the edit if it does not work well. Do not use apply_patch for changes that are auto";

fn is_codex_responses_path(path: &str) -> bool {
    path.trim_end_matches('/') == "/backend-api/codex/responses"
}

impl OpenAIStrategy {
    #[must_use]
    pub const fn new(health_checker: Option<Arc<ApiKeyHealthService>>) -> Self {
        Self { health_checker }
    }

    /// `从OpenAI` access_token中解析chatgpt-account-id
    fn extract_chatgpt_account_id(access_token: &str) -> Option<String> {
        let parse = OpenAI;
        parse.extract_chatgpt_account_id(access_token).ok()?
    }

    /// 异步处理429限流错误
    async fn handle_rate_limit(&self, ctx: &ProxyContext, body: &[u8]) -> Result<()> {
        let Some(health_checker) = self.health_checker.as_ref() else {
            return Ok(());
        };
        let Some(key_id) = ctx.routing.selected_backend.as_ref().map(|k| k.id) else {
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
                .mark_key_rate_limited(key_id, resets_at, &details)
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

    /// `更新OpenAI速率限制信息到健康状态详情`
    async fn update_health_status_detail(&self, ctx: &ProxyContext) -> Result<()> {
        let headers = &ctx.response.details.headers;
        if headers.is_empty() {
            return Ok(());
        }

        let Some(snapshot) = Self::extract_rate_limit_snapshot_from_headers(headers) else {
            return Ok(());
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

        // 更新数据库中的健康状态详情
        if let Some(key_id) = ctx.routing.selected_backend.as_ref().map(|k| k.id)
            && let Some(health_checker) = &self.health_checker
        {
            health_checker
                .update_health_status_detail(key_id, &snapshot)
                .await?;
        }

        Ok(())
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
        session: &Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut ProxyContext,
    ) -> Result<()> {
        let path = session.req_header().uri.path();
        if is_codex_responses_path(path) {
            ctx.request.will_modify_body = true;
            linfo!(
                &ctx.request_id,
                LogStage::RequestModify,
                LogComponent::OpenAIStrategy,
                "enable_instructions_injection",
                "OpenAI请求启用instructions注入",
                route_path = path,
                will_modify_body = ctx.request.will_modify_body
            );
        }
        if let Some(backend) = &ctx.routing.selected_backend
            && backend.auth_type == "oauth"
        {
            upstream_request
                .insert_header("host", "chatgpt.com")
                .context(format!(
                    "设置OpenAI host头失败, request_id: {}",
                    ctx.request_id
                ))?;

            if let Some(ResolvedCredential::OAuthAccessToken(token)) =
                &ctx.routing.resolved_credential
                && let Some(account_id) = Self::extract_chatgpt_account_id(token)
            {
                ctx.routing.account_id = Some(account_id.clone());
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

    async fn modify_request_body_json(
        &self,
        session: &Session,
        ctx: &ProxyContext,
        json_value: &mut Value,
    ) -> Result<bool> {
        let path = session.req_header().uri.path();
        if !is_codex_responses_path(path) {
            return Ok(false);
        }

        let Some(object) = json_value.as_object_mut() else {
            return Ok(false);
        };

        if object.contains_key("instructions") {
            return Ok(false);
        }

        object.insert(
            "instructions".to_string(),
            Value::String(CODEX_INSTRUCTIONS.to_string()),
        );

        linfo!(
            &ctx.request_id,
            LogStage::RequestModify,
            LogComponent::OpenAIStrategy,
            "inject_instructions",
            "OpenAI请求补充instructions字段",
            route_path = path
        );

        Ok(true)
    }

    async fn handle_response_body(
        &self,
        _session: &Session,
        ctx: &ProxyContext,
        status_code: u16,
        body: &[u8],
    ) -> Result<()> {
        match status_code {
            200..=299 => {
                // 成功响应：更新健康状态详情（限流窗口信息）
                self.update_health_status_detail(ctx).await?;
            }
            429 => {
                // 429 限流错误：处理限流状态
                self.handle_rate_limit(ctx, body).await?;
            }
            _ => {
                // 其他状态码：暂不处理，直接返回
                linfo!(
                    &ctx.request_id,
                    LogStage::Internal,
                    LogComponent::OpenAIStrategy,
                    "unhandled_status_code",
                    "收到未处理的状态码，暂不处理",
                    status_code = status_code
                );
                return Ok(());
            }
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

#[derive(Debug, Serialize)]
struct RateLimitSnapshotLog {
    primary: Option<RateLimitWindowLog>,
    secondary: Option<RateLimitWindowLog>,
}

#[derive(Debug, Serialize)]
struct RateLimitWindowLog {
    used_percent: f64,
    window_seconds: Option<i64>,
    resets_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::OpenAIStrategy;
    use crate::proxy::ProxyContext;
    use crate::proxy::provider_strategy::ProviderStrategy;
    use pingora_core::protocols::l4::stream::Stream;
    use pingora_http::RequestHeader;
    use pingora_proxy::Session;
    use serde_json::json;
    use tokio::io::AsyncWriteExt;
    use tokio::net::{TcpListener, TcpStream};

    async fn make_test_session(request: &str) -> Session {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");

        let mut client = TcpStream::connect(addr).await.expect("connect");
        let (server, _) = listener.accept().await.expect("accept");

        client
            .write_all(request.as_bytes())
            .await
            .expect("write request");

        let stream = Stream::from(server);
        let mut session = Session::new_h1(Box::new(stream));

        session
            .downstream_session
            .read_request()
            .await
            .expect("read request");

        session
    }

    #[tokio::test]
    async fn test_openai_sets_will_modify_body_for_codex_responses() {
        let request = "POST /backend-api/codex/responses HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let session = make_test_session(request).await;
        let mut ctx = ProxyContext::default();
        let mut upstream_request =
            RequestHeader::build("POST", b"/backend-api/codex/responses", None)
                .expect("build request header");
        let strategy = OpenAIStrategy::new(None);

        strategy
            .modify_request(&session, &mut upstream_request, &mut ctx)
            .await
            .expect("modify request");

        assert!(ctx.request.will_modify_body);
    }

    #[tokio::test]
    async fn test_openai_injects_instructions_when_missing() {
        let request = "POST /backend-api/codex/responses HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let session = make_test_session(request).await;
        let ctx = ProxyContext::default();
        let mut json_value = json!({
            "model": "gpt-5"
        });
        let strategy = OpenAIStrategy::new(None);

        let modified = strategy
            .modify_request_body_json(&session, &ctx, &mut json_value)
            .await
            .expect("modify request body");

        let expected = "You are Codex, based on GPT-5. You are running as a coding agent in the Codex CLI on a user's computer.\n\n## General\n\n- When searching for text or files, prefer using `rg` or `rg --files` respectively because `rg` is much faster than alternatives like `grep`. (If the `rg` command is not found, then use alternatives.)\n\n## Editing constraints\n\n- Default to ASCII when editing or creating files. Only introduce non-ASCII or other Unicode characters when there is a clear justification and the file already uses them.\n- Add succinct code comments that explain what is going on if code is not self-explanatory. You should not add comments like \"Assigns the value to the variable\", but a brief comment might be useful ahead of a complex code block that the user would otherwise have to spend time parsing out. Usage of these comments should be rare.\n- Try to use apply_patch for single file edits, but it is fine to explore other options to make the edit if it does not work well. Do not use apply_patch for changes that are auto";
        assert!(modified);
        assert_eq!(json_value["instructions"], expected);
    }
}
