//! # 代理上下文模块
//!
//! 包含代理请求处理过程中使用的上下文类型定义

use crate::proxy::provider_strategy::ProviderStrategy;
use crate::{ldebug, logging::LogComponent, logging::LogStage};
use bytes::BytesMut;
use rand::Rng;
use std::sync::Arc;
use std::time::Instant;

use crate::collect::types::TokenUsageMetrics;
use crate::collect::types::{RequestDetails, ResponseDetails};
use entity::{provider_types, user_provider_keys, user_service_apis};
use std::collections::BTreeMap;

/// 解析后的最终上游凭证
#[derive(Debug, Clone)]
pub enum ResolvedCredential {
    /// 直接上游 API Key
    ApiKey(String),
    /// OAuth 访问令牌
    OAuthAccessToken(String),
}

/// 重试相关的运行时状态
///
/// 说明：
/// - `retry_count` 表示**额外重试次数**（不包含首次尝试）
/// - `next_retry_delay_ms` 用于在下一次尝试开始前（`upstream_peer` 阶段）做退避等待
#[derive(Debug, Default)]
pub struct RetryState {
    /// 已发生的重试次数（不包含首次尝试）
    pub retry_count: u32,
    /// 下一次重试前的等待时间（毫秒）
    pub next_retry_delay_ms: Option<u64>,
    /// 本轮失败是否已应用过重试策略（防止同一次失败被多次 hook 重入导致重复计数）
    pub retry_policy_applied: bool,
    /// 上一次触发重试的 HTTP 状态码（用于观测与调试）
    pub last_retry_status_code: Option<u16>,
    /// 上游建议的 Retry-After（毫秒），仅在解析到对应响应头时设置
    pub retry_after_ms: Option<u64>,
}

impl RetryState {
    pub const fn reset_for_new_attempt(&mut self) {
        self.next_retry_delay_ms = None;
        self.retry_policy_applied = false;
        self.last_retry_status_code = None;
        self.retry_after_ms = None;
    }

    pub const fn try_mark_policy_applied(&mut self) -> bool {
        if self.retry_policy_applied {
            false
        } else {
            self.retry_policy_applied = true;
            true
        }
    }

    pub const fn clear_policy_after_no_retry(&mut self) {
        self.retry_policy_applied = false;
        self.retry_after_ms = None;
    }

    pub fn set_retry_after_from_header_value(&mut self, request_id: &str, header_value: &str) {
        let trimmed = header_value.trim();

        // Retry-After: <delay-seconds> 或 HTTP-date（常见：Sun, 06 Nov 1994 08:49:37 GMT）
        if let Ok(seconds) = trimmed.parse::<u64>() {
            self.retry_after_ms = Some(seconds.saturating_mul(1000));
            return;
        }

        match chrono::DateTime::parse_from_rfc2822(trimmed) {
            Ok(dt) => {
                let now = chrono::Utc::now();
                let target = dt.with_timezone(&chrono::Utc);
                let delta = target.signed_duration_since(now);
                let ms = delta.num_milliseconds();

                // RFC 9110: 若时间已过，视为“立即可重试”
                self.retry_after_ms = if ms <= 0 {
                    Some(0)
                } else {
                    u64::try_from(ms).ok()
                };
            }
            Err(e) => {
                ldebug!(
                    request_id,
                    LogStage::ResponseFailure,
                    LogComponent::Proxy,
                    "parse_retry_after_failed",
                    "解析 Retry-After 失败，忽略该头",
                    value = trimmed,
                    error = %e
                );
                self.retry_after_ms = None;
            }
        }
    }

    pub fn schedule_next_retry(
        &mut self,
        status_code: Option<u16>,
        base_delay_ms: u64,
        max_delay_ms: u64,
    ) -> u64 {
        let attempt = self.retry_count;
        let mut delay_ms = Self::calculate_backoff_delay_ms(attempt, base_delay_ms, max_delay_ms);

        if status_code == Some(429)
            && let Some(retry_after_ms) = self.retry_after_ms.take()
        {
            delay_ms = delay_ms.max(retry_after_ms).min(max_delay_ms);
        }

        self.next_retry_delay_ms = Some(delay_ms);
        self.last_retry_status_code = status_code;
        delay_ms
    }

    pub fn consume_budget_and_schedule(
        &mut self,
        max_retry_budget: u32,
        status_code: Option<u16>,
        base_delay_ms: u64,
        max_delay_ms: u64,
    ) -> u64 {
        if self.retry_count >= max_retry_budget {
            self.next_retry_delay_ms = None;
            return 0;
        }

        self.retry_count = self.retry_count.saturating_add(1);
        self.schedule_next_retry(status_code, base_delay_ms, max_delay_ms)
    }

    /// 计算指数退避 + full jitter 的等待时间（毫秒）
    fn calculate_backoff_delay_ms(attempt: u32, base_delay_ms: u64, max_delay_ms: u64) -> u64 {
        // attempt 从 1 开始；为避免溢出，指数上限限制到 30。
        let exp = attempt.saturating_sub(1).min(30);
        let multiplier = 1u64.checked_shl(exp).unwrap_or(u64::MAX);
        let cap = base_delay_ms
            .checked_mul(multiplier)
            .unwrap_or(max_delay_ms)
            .min(max_delay_ms);

        if cap == 0 {
            return 0;
        }

        // full jitter: [0, cap]
        rand::thread_rng().gen_range(0..=cap).min(max_delay_ms)
    }
}

/// 请求相关上下文
pub struct ProxyRequestContext {
    /// 请求详情
    pub details: RequestDetails,
    /// 请求体缓冲区（用于 `request_body_filter` 中的数据收集）
    pub body: BytesMut,
    /// 请求体总接收字节数（用于统计与日志）
    pub body_received_size: usize,
    /// 请求体是否被截断（避免无限增长）
    pub body_truncated: bool,
    /// 是否计划修改请求体（供上游头部处理决策使用）
    pub will_modify_body: bool,
    /// 用户请求的模型名称
    pub requested_model: Option<String>,
}

/// 响应相关上下文
pub struct ProxyResponseContext {
    /// 响应详情
    pub details: ResponseDetails,
    /// 响应体缓冲区（用于 `response_body_filter` 中的数据收集）
    pub body: BytesMut,
    /// 响应体总接收字节数（用于统计与日志）
    pub body_received_size: usize,
    /// 响应体是否被截断（避免无限增长）
    pub body_truncated: bool,
    /// 是否为 SSE 响应（在 `response_filter` 时缓存）
    pub is_sse: bool,
    /// SSE 首包心跳是否已注入（用于保持下游连接活跃）
    pub sse_keepalive_sent: bool,
    /// 最终使用量（统一出口）
    pub usage_final: Option<TokenUsageMetrics>,
}

/// 路由与认证相关上下文
pub struct ProxyRoutingContext {
    /// 解析得到的最终上游凭证（由 `CredentialResolutionStep` 设置）
    pub resolved_credential: Option<ResolvedCredential>,
    /// `ChatGPT` Account ID（用于OpenAI `ChatGPT` API）
    pub account_id: Option<String>,
    /// 用户对外 API 配置
    pub user_service_api: Option<user_service_apis::Model>,
    /// 选择的后端 API 密钥
    pub selected_backend: Option<user_provider_keys::Model>,
    /// 提供商类型配置
    pub provider_type: Option<provider_types::Model>,
    /// 选定的服务商策略
    pub strategy: Option<Arc<dyn ProviderStrategy>>,
}

/// 请求控制相关上下文
pub struct ProxyControlContext {
    /// 重试相关运行时状态
    pub retry: RetryState,
    /// 连接超时时间(秒)
    pub timeout_seconds: Option<i32>,
}

/// 追踪与日志相关上下文
pub struct ProxyTraceContext {
    /// 追踪记录是否已成功写入数据库
    pub trace_started: bool,
    /// 最终上游请求头（包含注入/清理后的结果）
    pub upstream_request_headers: Option<BTreeMap<String, String>>,
    /// 最终上游请求 URI（可能被策略改写）
    pub upstream_request_uri: Option<String>,
}

/// 请求上下文
// #[derive(Debug, Clone)]
pub struct ProxyContext {
    /// 请求ID
    pub request_id: String,
    /// 开始时间
    pub start_time: Instant,
    /// 控制域上下文
    pub control: ProxyControlContext,
    /// 请求域上下文
    pub request: ProxyRequestContext,
    /// 响应域上下文
    pub response: ProxyResponseContext,
    /// 路由域上下文
    pub routing: ProxyRoutingContext,
    /// 追踪域上下文
    pub trace: ProxyTraceContext,
}

impl Default for ProxyContext {
    fn default() -> Self {
        Self {
            request_id: String::new(),
            start_time: Instant::now(),
            control: ProxyControlContext {
                retry: RetryState::default(),
                timeout_seconds: None,
            },
            request: ProxyRequestContext {
                details: RequestDetails::default(),
                body: BytesMut::new(),
                body_received_size: 0,
                body_truncated: false,
                will_modify_body: false,
                requested_model: None,
            },
            response: ProxyResponseContext {
                details: ResponseDetails::default(),
                body: BytesMut::new(),
                body_received_size: 0,
                body_truncated: false,
                is_sse: false,
                sse_keepalive_sent: false,
                usage_final: None,
            },
            routing: ProxyRoutingContext {
                resolved_credential: None,
                account_id: None,
                user_service_api: None,
                selected_backend: None,
                provider_type: None,
                strategy: None,
            },
            trace: ProxyTraceContext {
                trace_started: false,
                upstream_request_headers: None,
                upstream_request_uri: None,
            },
        }
    }
}

impl ProxyContext {}

impl ProxyContext {
    /// 标记追踪已成功启动
    pub const fn mark_trace_started(&mut self) {
        self.trace.trace_started = true;
    }

    /// 判断是否已成功启动追踪
    #[must_use]
    pub const fn is_trace_started(&self) -> bool {
        self.trace.trace_started
    }
}

#[cfg(test)]
mod tests {
    use super::RetryState;

    #[test]
    fn test_retry_after_http_date_parsing_future_is_some() {
        let mut retry = RetryState::default();
        let target = chrono::Utc::now() + chrono::Duration::seconds(2);
        let header_value = target.to_rfc2822();

        retry.set_retry_after_from_header_value("test-request", &header_value);

        let ms = retry.retry_after_ms.expect("retry_after_ms");
        assert!(ms <= 5_000);
    }

    #[test]
    fn test_retry_after_http_date_parsing_past_is_zero() {
        let mut retry = RetryState::default();
        let target = chrono::Utc::now() - chrono::Duration::seconds(2);
        let header_value = target.to_rfc2822();

        retry.set_retry_after_from_header_value("test-request", &header_value);

        assert_eq!(retry.retry_after_ms, Some(0));
    }
}
