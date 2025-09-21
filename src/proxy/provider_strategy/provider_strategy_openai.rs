//! OpenAI 提供商策略
//!
//! 处理OpenAI特有的逻辑，包括429错误处理、JWT解析等

use crate::error::{ErrorContext, Result};
use crate::proxy::ProxyContext;
use crate::proxy_err;
use chrono::Utc;
use entity::user_provider_keys;
use pingora_http::RequestHeader;
use pingora_proxy::Session;
use bytes::Bytes;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

use super::ProviderStrategy;
use crate::proxy::service::ResponseBodyService;

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

    /// 统一解析429错误响应
    fn parse_429_error(&self, body: &[u8]) -> Option<OpenAI429Error> {
        info!("开始解析OpenAI 429错误响应体，响应体大小: {} 字节", body.len());

        let json_str = match std::str::from_utf8(body) {
            Ok(s) => s,
            Err(_) => {
                error!("429响应体UTF-8解析失败");
                return None;
            }
        };

        info!("429响应体JSON字符串: {}", json_str);

        // 首先尝试标准格式解析
        if let Ok(error) = serde_json::from_str::<OpenAI429Error>(json_str) {
            info!("成功解析429错误: type={}, resets_in_seconds={:?}",
                   error.error.r#type, error.error.resets_in_seconds);
            return Some(error);
        }

        // 标准格式失败，尝试解析替代格式
        info!("尝试解析替代的429响应格式");

        if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
            info!("响应体JSON结构: {:?}", value);

            if let Some(error_obj) = value.get("error").and_then(|v| v.as_object()) {
                info!("找到error对象: {:?}", error_obj);

                let error_type = error_obj.get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let message = error_obj.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string();

                let plan_type = error_obj.get("plan_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let resets_in_seconds = error_obj.get("resets_in_seconds")
                    .and_then(|v| v.as_i64())
                    .or_else(|| {
                        error_obj.get("reset_in_seconds")
                            .and_then(|v| v.as_i64())
                            .or_else(|| {
                                error_obj.get("retry_after")
                                    .and_then(|v| v.as_i64())
                            })
                    });

                info!("提取到的字段: type={}, message={}, plan_type={:?}, resets_in_seconds={:?}",
                       error_type, message, plan_type, resets_in_seconds);

                return Some(OpenAI429Error {
                    error: OpenAIErrorDetail {
                        r#type: error_type,
                        message,
                        plan_type,
                        resets_in_seconds,
                    },
                });
            }
        }

        error!("429响应体JSON解析失败");
        None
    }

    /// 更新API密钥健康状态
    async fn update_key_health_status(
        &self,
        key_id: i32,
        error_detail: &OpenAIErrorDetail,
    ) -> Result<()> {
        info!("开始更新API密钥健康状态: key_id={}, error_detail={:?}", key_id, error_detail);

        let db = self
            .db
            .as_ref()
            .ok_or_else(|| proxy_err!(database, "数据库连接未配置"))?;

        let now = Utc::now().naive_utc();
        let rate_limit_resets_at = error_detail
            .resets_in_seconds
            .map(|seconds| {
                info!("计算限流重置时间: resets_in_seconds={} seconds, now={}", seconds, now);
                now + chrono::Duration::seconds(seconds)
            });

        info!("计算得到的限流重置时间: {:?}", rate_limit_resets_at);

        let mut key: user_provider_keys::ActiveModel =
            user_provider_keys::Entity::find_by_id(key_id)
                .one(db.as_ref())
                .await
                .with_database_context(|| format!("查询API密钥失败，ID: {}", key_id))?
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

    /// 统一处理429错误
    async fn process_429_error(&self, ctx: &ProxyContext, body: &[u8]) -> Result<()> {
        info!(
            request_id = %ctx.request_id,
            body_size = body.len(),
            "处理OpenAI 429错误响应"
        );

        // 记录原始响应体用于调试
        if let Ok(body_str) = std::str::from_utf8(body) {
            info!(
                request_id = %ctx.request_id,
                raw_body = %body_str,
                "429响应体原始内容"
            );
        } else {
            info!(
                request_id = %ctx.request_id,
                "429响应体不是有效的UTF-8字符串，大小: {} 字节",
                body.len()
            );
        }

        // 标记密钥为限流状态并尝试解析详细信息
        if let Some(backend_key) = &ctx.selected_backend {
            let db = self
                .db
                .as_ref()
                .ok_or_else(|| proxy_err!(database, "数据库连接未配置"))?;

            let now = Utc::now().naive_utc();

            // 先标记为基本限流状态
            let mut key: user_provider_keys::ActiveModel =
                user_provider_keys::Entity::find_by_id(backend_key.id)
                    .one(db.as_ref())
                    .await
                    .with_database_context(|| format!("查询API密钥失败，ID: {}", backend_key.id))?
                    .ok_or_else(|| proxy_err!(database, "API密钥不存在: {}", backend_key.id))?
                    .into();

            key.health_status = Set("rate_limited".to_string());
            key.health_status_detail = Set(Some(r#"{"type": "rate_limit", "message": "429 status code detected"}"#.to_string()));
            key.rate_limit_resets_at = Set(None);
            key.last_error_time = Set(Some(now));
            key.updated_at = Set(now);

            key.update(db.as_ref())
                .await
                .with_database_context(|| format!("更新API密钥健康状态失败，ID: {}", backend_key.id))?;

            info!(
                request_id = %ctx.request_id,
                key_id = backend_key.id,
                "OpenAI API密钥已标记为限流状态（基于429状态码）"
            );

            // 尝试解析响应体获取更详细的信息
            if let Some(error_info) = self.parse_429_error(body) {
                info!(
                    request_id = %ctx.request_id,
                    error_type = %error_info.error.r#type,
                    error_message = %error_info.error.message,
                    plan_type = ?error_info.error.plan_type,
                    resets_in_seconds = ?error_info.error.resets_in_seconds,
                    "成功解析OpenAI 429错误详情"
                );

                if let Err(e) = self
                    .update_key_health_status(backend_key.id, &error_info.error)
                    .await
                {
                    error!(
                        request_id = %ctx.request_id,
                        key_id = backend_key.id,
                        error = %e,
                        "更新API密钥详细限流信息失败"
                    );
                } else {
                    info!(
                        request_id = %ctx.request_id,
                        key_id = backend_key.id,
                        "成功更新API密钥详细限流信息"
                    );
                }
            } else {
                warn!(
                    request_id = %ctx.request_id,
                    "无法解析OpenAI 429错误响应体，使用基础限流状态"
                );
            }
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
            // 优先使用 body_chunks 中收集的数据（通过 response_body_filter），否则使用传入的 body
            let actual_body = if !ctx.response_details.body_chunks.is_empty() {
                info!(
                    request_id = %ctx.request_id,
                    chunks_size = ctx.response_details.body_chunks.len(),
                    provided_body_size = body.len(),
                    "使用body_chunks中的429响应数据"
                );
                &ctx.response_details.body_chunks
            } else {
                info!(
                    request_id = %ctx.request_id,
                    provided_body_size = body.len(),
                    "使用传入的429响应数据"
                );
                body
            };

            if let Err(e) = self.process_429_error(ctx, actual_body).await {
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

/// 为 OpenAIStrategy 实现 ResponseBodyService 以便在 response_body_filter 中处理429错误
impl ResponseBodyService for OpenAIStrategy {
    fn exec(
        &self,
        body: &mut Option<Bytes>,
        end_of_stream: bool,
        ctx: &mut ProxyContext,
    ) -> pingora_core::Result<Option<Duration>> {
        // 只在流结束且有429状态码时处理
        if !end_of_stream || ctx.response_details.status_code != Some(429) {
            return Ok(None);
        }

        // 在这里收集响应体数据，但不直接处理async逻辑
        // 我们将响应体数据存储到ctx中，后续在handle_response_body中处理
        if let Some(body_bytes) = body {
            // 将响应体数据存储到ctx的body_chunks中，供后续处理
            ctx.response_details.add_body_chunk(body_bytes);

            info!(
                request_id = %ctx.request_id,
                body_size = body_bytes.len(),
                total_chunks_size = ctx.response_details.body_chunks.len(),
                "在response_body_filter中收集到OpenAI 429响应体"
            );

            // 立即尝试解析响应体用于日志记录（但不更新数据库）
            if let Some(error_info) = self.parse_429_error(body_bytes.as_ref()) {
                info!(
                    request_id = %ctx.request_id,
                    error_type = %error_info.error.r#type,
                    resets_in_seconds = ?error_info.error.resets_in_seconds,
                    "在response_body_filter中成功解析OpenAI 429错误详情"
                );
            }
        }

        Ok(None)
    }
}
