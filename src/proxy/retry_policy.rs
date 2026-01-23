//! # 重试策略评估
//!
//! 提供清晰的重试决策逻辑，将复杂的重试策略从主服务中分离。

use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, linfo};
use pingora_proxy::Session;

use crate::proxy::context::ProxyContext;

/// 重试决策结果
#[derive(Debug, Clone, Copy)]
pub struct RetryDecision {
    /// 是否应该重试
    pub should_retry: bool,
    /// 延迟毫秒数
    pub delay_ms: u64,
    /// 原因
    pub reason: RetryReason,
}

/// 重试决策原因
#[derive(Debug, Clone, Copy)]
pub enum RetryReason {
    /// 达到重试上限
    MaxRetryExceeded,
    /// 未配置重试预算
    NoRetryBudget,
    /// 已收到部分响应
    PartialResponse,
    /// 请求体不可重放
    NotSafeToRetry,
    /// 退避计算为 0
    ZeroBackoff,
    /// 可以重试
    Retryable,
}

impl RetryDecision {
    /// 创建不重试的决策
    #[must_use]
    pub const fn no_retry(reason: RetryReason) -> Self {
        Self {
            should_retry: false,
            delay_ms: 0,
            reason,
        }
    }

    /// 创建重试的决策
    #[must_use]
    pub const fn retry(delay_ms: u64) -> Self {
        Self {
            should_retry: true,
            delay_ms,
            reason: RetryReason::Retryable,
        }
    }
}

/// 重试策略评估器
pub struct RetryPolicyEvaluator<'a> {
    session: &'a mut Session,
    max_retry_budget: u32,
    retry_count: u32,
}

impl<'a> RetryPolicyEvaluator<'a> {
    /// 创建新的评估器
    pub const fn new(session: &'a mut Session, max_retry_budget: u32, retry_count: u32) -> Self {
        Self {
            session,
            max_retry_budget,
            retry_count,
        }
    }

    /// 评估是否应该重试（不包含部分响应检查）
    pub fn evaluate(&mut self) -> RetryDecision {
        // 检查重试预算
        if self.max_retry_budget == 0 {
            return RetryDecision::no_retry(RetryReason::NoRetryBudget);
        }

        // 检查是否达到上限
        if self.retry_count >= self.max_retry_budget {
            return RetryDecision::no_retry(RetryReason::MaxRetryExceeded);
        }

        // 检查是否安全重试
        if !Self::is_safe_to_retry(self.session) {
            return RetryDecision::no_retry(RetryReason::NotSafeToRetry);
        }

        // 可以重试（延迟计算将在外部完成）
        RetryDecision::retry(0)
    }

    /// 检查是否安全重试
    fn is_safe_to_retry(session: &mut Session) -> bool {
        // 不具备可安全重试的前提（retry buffer 缺失/截断）
        if session.retry_buffer_truncated() {
            return false;
        }

        // 无 body 的请求天然可重试；有 body 时需要确保 retry buffer 存在
        if session.is_body_empty() {
            return true;
        }

        // 检查是否有可重放的请求体
        session.get_retry_buffer().is_some()
    }
}

/// 应用重试策略
///
/// 这是简化的主入口函数，协调重试决策的各个步骤
pub fn apply_retry_policy(
    session: &mut Session,
    ctx: &mut ProxyContext,
    err: &mut pingora_core::Error,
    reason: &'static str,
    status_code: Option<u16>,
    default_base_delay_ms: u64,
    max_delay_ms: u32,
) {
    // 防止同一次失败被多次 hook 重入导致重复计数与预算提前耗尽
    if !ctx.control.retry.try_mark_policy_applied() {
        return;
    }

    let max_retry_budget = calculate_max_retry_budget(ctx);

    // 创建评估器并评估
    let mut evaluator =
        RetryPolicyEvaluator::new(session, max_retry_budget, ctx.control.retry.retry_count);

    let decision = evaluator.evaluate();

    // 根据决策处理
    if !decision.should_retry {
        log_retry_skipped(
            &ctx.request_id,
            reason,
            status_code,
            &decision,
            ctx.control.retry.retry_count,
            max_retry_budget,
        );
        err.set_retry(false);
        return;
    }

    // 计算退避延迟
    let base_delay_ms = default_base_delay_ms;
    let max_delay_ms_u64 = u64::from(max_delay_ms);
    let delay_ms = ctx.control.retry.consume_budget_and_schedule(
        max_retry_budget,
        status_code,
        base_delay_ms,
        max_delay_ms_u64,
    );

    if delay_ms == 0 {
        log_retry_skipped(
            &ctx.request_id,
            reason,
            status_code,
            &RetryDecision::no_retry(RetryReason::ZeroBackoff),
            ctx.control.retry.retry_count,
            max_retry_budget,
        );
        err.set_retry(false);
        return;
    }

    // 应用重试决策
    err.set_retry(true);
    log_retry_decision(
        &ctx.request_id,
        reason,
        status_code,
        delay_ms,
        ctx.control.retry.retry_count,
        max_retry_budget,
    );
}

/// 记录重试决策
fn log_retry_decision(
    request_id: &str,
    reason: &'static str,
    status_code: Option<u16>,
    delay_ms: u64,
    attempt: u32,
    max_retry_budget: u32,
) {
    linfo!(
        request_id,
        LogStage::ResponseFailure,
        LogComponent::Proxy,
        "retry_scheduled",
        "满足重试条件，计划重试同一上游请求",
        reason = reason,
        status_code = status_code,
        attempt = attempt,
        max_retry_budget = max_retry_budget,
        delay_ms = delay_ms
    );
}

/// 记录跳过重试
fn log_retry_skipped(
    request_id: &str,
    reason: &'static str,
    status_code: Option<u16>,
    decision: &RetryDecision,
    attempt: u32,
    max_retry_budget: u32,
) {
    let (skip_reason, message) = match decision.reason {
        RetryReason::NoRetryBudget => ("no_budget", "未触发重试（未配置重试预算）"),
        RetryReason::MaxRetryExceeded => ("max_exceeded", "未触发重试（已达重试上限）"),
        RetryReason::NotSafeToRetry => ("not_safe", "未触发重试（请求体不可重放）"),
        RetryReason::ZeroBackoff => ("zero_backoff", "未触发重试（退避计算为 0）"),
        RetryReason::PartialResponse => ("partial_response", "未触发重试（已收到部分响应）"),
        RetryReason::Retryable => unreachable!(),
    };

    ldebug!(
        request_id,
        LogStage::ResponseFailure,
        LogComponent::Proxy,
        "retry_skipped",
        message,
        reason = reason,
        status_code = status_code,
        attempt = attempt,
        max_retry_budget = max_retry_budget,
        skip_reason = skip_reason
    );
}

/// 计算最大重试预算
///
/// 从用户服务 API 配置中获取重试次数限制，确保返回非负值
fn calculate_max_retry_budget(ctx: &ProxyContext) -> u32 {
    let retry_count = ctx
        .routing
        .user_service_api
        .as_ref()
        .and_then(|api| api.retry_count)
        .unwrap_or(0);

    // 确保非负并安全转换为 u32
    let retry_count = retry_count.max(0);
    u32::try_from(retry_count).unwrap_or(u32::MAX)
}
