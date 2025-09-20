//! OpenAI 提供商策略
//!
//! 处理OpenAI特有的逻辑，包括429错误处理、JWT解析等

use crate::error::{ErrorContext, Result};
use crate::proxy::ProxyContext;
use crate::{proxy_bail, proxy_err};
use chrono::Utc;
use entity::user_provider_keys;
use pingora_http::RequestHeader;
use pingora_proxy::Session;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use super::ProviderStrategy;

/// OpenAI 429错误响应体结构
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

pub struct OpenAIStrategy {
    db: Option<Arc<DatabaseConnection>>,
}

impl Default for OpenAIStrategy {
    fn default() -> Self {
        Self { db: None }
    }
}

impl OpenAIStrategy {
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置数据库连接
    pub fn with_db(mut self, db: Arc<DatabaseConnection>) -> Self {
        self.db = Some(db);
        self
    }

    /// 解析429错误响应
    fn parse_429_error(&self, body: &[u8]) -> Option<OpenAI429Error> {
        if let Ok(json_str) = std::str::from_utf8(body) {
            if let Ok(error) = serde_json::from_str::<OpenAI429Error>(json_str) {
                return Some(error);
            }
        }
        None
    }

    /// 更新API密钥健康状态
    async fn update_key_health_status(
        &self,
        key_id: i32,
        error_detail: &OpenAIErrorDetail,
    ) -> Result<()> {
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| proxy_err!(database, "数据库连接未配置"))?;

        let now = Utc::now().naive_utc();
        let rate_limit_resets_at = error_detail
            .resets_in_seconds
            .map(|seconds| now + chrono::Duration::seconds(seconds));

        // 更新健康状态为 "rate_limited"
        let mut key: user_provider_keys::ActiveModel =
            user_provider_keys::Entity::find_by_id(key_id)
                .one(db.as_ref())
                .await
                .with_database_context(|| format!("查询API密钥失败，ID: {}", key_id))?
                .ok_or_else(|| proxy_err!(database, "API密钥不存在: {}", key_id))?
                .into();

        // 更新健康状态
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
            .with_database_context(|| format!("更新API密钥健康状态失败，ID: {}", key_id))?;

        info!(
            key_id = key_id,
            error_type = %error_detail.r#type,
            plan_type = ?error_detail.plan_type,
            resets_in_seconds = ?error_detail.resets_in_seconds,
            rate_limit_resets_at = ?rate_limit_resets_at,
            "OpenAI API密钥已标记为限流状态"
        );

        Ok(())
    }

    /// 处理429错误响应
    async fn handle_429_response(
        &self,
        _session: &Session,
        ctx: &ProxyContext,
        body: &[u8],
    ) -> Result<()> {
        debug!(
            request_id = %ctx.request_id,
            "处理OpenAI 429错误响应"
        );

        // 解析429错误
        if let Some(error_info) = self.parse_429_error(body) {
            debug!(
                request_id = %ctx.request_id,
                error_type = %error_info.error.r#type,
                resets_in_seconds = ?error_info.error.resets_in_seconds,
                "检测到OpenAI 429错误"
            );

            // 更新API密钥健康状态
            if let Some(backend_key) = &ctx.selected_backend {
                if let Err(e) = self
                    .update_key_health_status(backend_key.id, &error_info.error)
                    .await
                {
                    error!(
                        request_id = %ctx.request_id,
                        key_id = backend_key.id,
                        error = %e,
                        "更新API密钥健康状态失败"
                    );
                    // 返回速率限制错误，但不要中断处理流程
                    proxy_bail!(
                        rate_limit,
                        "OpenAI API速率限制: {}",
                        error_info.error.message
                    );
                }
            } else {
                warn!(
                    request_id = %ctx.request_id,
                    "未找到后端密钥信息，无法更新健康状态"
                );
            }
        } else {
            debug!(
                request_id = %ctx.request_id,
                "无法解析OpenAI 429错误响应"
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

    fn set_db_connection(&mut self, db: Option<Arc<DatabaseConnection>>) {
        self.db = db;
    }

    async fn modify_request(
        &self,
        _session: &Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut ProxyContext,
    ) -> Result<()> {
        // 对于OpenAI，可以在这里添加特定的请求头
        // 例如设置特定的User-Agent或其他OpenAI需要的头

        // 如果是ChatGPT API，确保host正确
        if let Some(backend) = &ctx.selected_backend {
            if backend.auth_type == "oauth" {
                // ChatGPT API使用chatgpt.com作为host
                upstream_request
                    .insert_header("host", "chatgpt.com")
                    .with_network_context(|| {
                        format!("设置OpenAI host头失败, request_id: {}", ctx.request_id)
                    })?;
            }
        }

        Ok(())
    }

    /// 处理响应体，包括429错误
    async fn handle_response_body(
        &self,
        _session: &Session,
        ctx: &ProxyContext,
        status_code: u16,
        body: &[u8],
    ) -> Result<()> {
        // 处理429错误
        if status_code == 429 {
            if let Err(e) = self.handle_429_response(_session, ctx, body).await {
                error!(
                    request_id = %ctx.request_id,
                    error = %e,
                    "处理OpenAI 429错误失败"
                );
                // 返回错误但不中断处理流程，让原始429响应返回给客户端
                return Err(e).with_network_context(|| {
                    format!("处理OpenAI 429响应失败, request_id: {}", ctx.request_id)
                });
            }
        }

        Ok(())
    }

    /// 检查密钥是否应该恢复使用
    async fn should_retry_key(&self, key: &user_provider_keys::Model) -> Result<bool> {
        // 如果密钥是限流状态，检查是否已经过了重置时间
        if key.health_status == "rate_limited" {
            if let Some(resets_at) = key.rate_limit_resets_at {
                let now = Utc::now().naive_utc();
                if now > resets_at {
                    info!(key_id = key.id, "OpenAI API密钥限流已解除，恢复使用");
                    return Ok(true);
                }
            }
        }

        // 其他情况使用默认逻辑
        Ok(key.is_active && key.health_status == "healthy")
    }
}
