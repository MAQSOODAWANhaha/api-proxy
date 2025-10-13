//! # Trace 数据模型
//!
//! 定义所有 trace 相关的数据结构

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::{ratio_as_f64, ProviderKeyId, ProviderTypeId, RequestCount, TokenCount};

/// 请求追踪数据 - 完整的请求生命周期记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestTrace {
    /// 请求 ID
    pub request_id: String,
    /// 用户 ID
    pub user_id: i32,
    /// 提供商类型 ID
    pub provider_type_id: ProviderTypeId,
    /// 提供商名称
    pub provider_name: String,
    /// 后端 API 密钥 ID
    pub backend_key_id: ProviderKeyId,
    /// 请求路径
    pub request_path: String,
    /// HTTP 方法
    pub http_method: String,
    /// 模型名称
    pub model_name: Option<String>,
    /// 请求开始时间
    pub start_time: DateTime<Utc>,
    /// 请求结束时间
    pub end_time: Option<DateTime<Utc>>,
    /// 总响应时间（毫秒）
    pub duration_ms: Option<u64>,
    /// HTTP 状态码
    pub status_code: Option<u16>,
    /// 是否成功
    pub is_success: bool,
    /// 错误信息
    pub error_message: Option<String>,
    /// 错误类型
    pub error_type: Option<String>,
    /// Token 使用统计
    pub token_usage: TokenUsage,
    /// 请求阶段追踪
    pub phases: Vec<TracePhase>,
    /// 自定义标签
    pub labels: HashMap<String, String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

/// Token 使用统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// 输入 token 数
    pub prompt_tokens: TokenCount,
    /// 输出 token 数
    pub completion_tokens: TokenCount,
    /// 总 token 数
    pub total_tokens: TokenCount,
    /// Token 使用效率（输出/输入比率）
    pub efficiency_ratio: Option<f64>,
}

/// 请求处理阶段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracePhase {
    /// 阶段名称
    pub phase: RequestPhase,
    /// 开始时间
    pub start_time: DateTime<Utc>,
    /// 结束时间
    pub end_time: Option<DateTime<Utc>>,
    /// 阶段耗时（毫秒）
    pub duration_ms: Option<u64>,
    /// 阶段状态
    pub status: PhaseStatus,
    /// 阶段详细信息
    pub details: Option<String>,
}

/// 请求处理阶段枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RequestPhase {
    /// 认证阶段
    Authentication,
    /// 速率限制检查
    RateLimit,
    /// 负载均衡选择
    LoadBalancing,
    /// 上游连接
    UpstreamConnection,
    /// 请求发送
    RequestSending,
    /// 等待响应
    AwaitingResponse,
    /// 响应处理
    ResponseProcessing,
    /// 完成
    Completed,
}

/// 阶段状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PhaseStatus {
    /// 进行中
    InProgress,
    /// 成功
    Success,
    /// 失败
    Failed,
    /// 跳过
    Skipped,
}

/// 健康状态指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    /// 提供商类型 ID
    pub provider_type_id: ProviderTypeId,
    /// 提供商名称
    pub provider_name: String,
    /// 时间窗口开始
    pub window_start: DateTime<Utc>,
    /// 时间窗口结束
    pub window_end: DateTime<Utc>,
    /// 时间窗口大小（分钟）
    pub window_minutes: u32,
    /// 总请求数
    pub total_requests: RequestCount,
    /// 成功请求数
    pub successful_requests: RequestCount,
    /// 失败请求数
    pub failed_requests: RequestCount,
    /// 成功率
    pub success_rate: f64,
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: f64,
    /// P50 响应时间（毫秒）
    pub p50_response_time_ms: f64,
    /// P95 响应时间（毫秒）
    pub p95_response_time_ms: f64,
    /// P99 响应时间（毫秒）
    pub p99_response_time_ms: f64,
    /// 错误分布
    pub error_distribution: HashMap<String, u64>,
    /// Token 使用统计
    pub token_stats: TokenStats,
    /// 健康评分 (0-100)
    pub health_score: f64,
    /// 健康状态
    pub health_status: HealthStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

/// Token 统计信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenStats {
    /// 总输入 token 数
    pub total_prompt_tokens: TokenCount,
    /// 总输出 token 数
    pub total_completion_tokens: TokenCount,
    /// 总 token 数
    pub total_tokens: TokenCount,
    /// 平均每请求 token 数
    pub avg_tokens_per_request: f64,
    /// 平均 token 使用效率
    pub avg_efficiency_ratio: f64,
}

/// 健康状态枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    /// 健康
    Healthy,
    /// 警告
    Warning,
    /// 严重
    Critical,
    /// 不可用
    Unavailable,
}

/// Trace 事件 - 用于实时收集
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    /// 请求 ID
    pub request_id: String,
    /// 事件类型
    pub event_type: TraceEventType,
    /// 事件时间
    pub timestamp: DateTime<Utc>,
    /// 事件数据
    pub data: serde_json::Value,
}

/// Trace 事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraceEventType {
    /// 请求开始
    RequestStarted,
    /// 认证完成
    AuthenticationCompleted,
    /// 速率限制检查完成
    RateLimitChecked,
    /// 负载均衡选择完成
    LoadBalancingCompleted,
    /// 上游连接建立
    UpstreamConnected,
    /// 请求发送完成
    RequestSent,
    /// 响应接收开始
    ResponseReceived,
    /// 响应处理完成
    ResponseProcessed,
    /// 请求完成
    RequestCompleted,
    /// 请求失败
    RequestFailed,
    /// Token 使用统计
    TokenUsage,
}

impl RequestTrace {
    /// 创建新的请求追踪
    #[must_use]
    pub fn new(
        request_id: String,
        user_id: i32,
        provider_type_id: i32,
        provider_name: String,
        backend_key_id: i32,
        request_path: String,
        http_method: String,
    ) -> Self {
        Self {
            request_id,
            user_id,
            provider_type_id,
            provider_name,
            backend_key_id,
            request_path,
            http_method,
            model_name: None,
            start_time: Utc::now(),
            end_time: None,
            duration_ms: None,
            status_code: None,
            is_success: false,
            error_message: None,
            error_type: None,
            token_usage: TokenUsage::default(),
            phases: Vec::new(),
            labels: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    /// 开始新阶段
    pub fn start_phase(&mut self, phase: &RequestPhase) {
        self.phases.push(TracePhase {
            phase: phase.clone(),
            start_time: Utc::now(),
            end_time: None,
            duration_ms: None,
            status: PhaseStatus::InProgress,
            details: None,
        });
    }

    /// 完成当前阶段
    pub fn complete_phase(
        &mut self,
        phase: &RequestPhase,
        status: PhaseStatus,
        details: Option<String>,
    ) {
        if let Some(current_phase) = self
            .phases
            .iter_mut()
            .rev()
            .find(|p| p.phase == *phase && p.status == PhaseStatus::InProgress)
        {
            current_phase.end_time = Some(Utc::now());
            current_phase.status = status;
            current_phase.details = details;

            if let Some(end_time) = current_phase.end_time {
                let duration_ms = end_time
                    .signed_duration_since(current_phase.start_time)
                    .num_milliseconds();
                if duration_ms >= 0 {
                    current_phase.duration_ms = Some(duration_ms.try_into().unwrap_or(u64::MAX));
                }
            }
        }
    }

    /// 设置请求完成
    pub fn complete(&mut self, status_code: u16, is_success: bool) {
        self.end_time = Some(Utc::now());
        self.status_code = Some(status_code);
        self.is_success = is_success;

        if let Some(end_time) = self.end_time {
            let duration_ms = end_time
                .signed_duration_since(self.start_time)
                .num_milliseconds();
            if duration_ms >= 0 {
                self.duration_ms = Some(duration_ms.try_into().unwrap_or(u64::MAX));
            }
        }
    }

    /// 设置错误信息
    pub fn set_error(&mut self, error_type: String, error_message: String) {
        self.error_type = Some(error_type);
        self.error_message = Some(error_message);
        self.is_success = false;
    }

    /// 设置 token 使用
    pub fn set_token_usage(&mut self, prompt_tokens: TokenCount, completion_tokens: TokenCount) {
        self.token_usage.prompt_tokens = prompt_tokens;
        self.token_usage.completion_tokens = completion_tokens;
        self.token_usage.total_tokens = prompt_tokens + completion_tokens;

        self.token_usage.efficiency_ratio = ratio_as_f64(completion_tokens, prompt_tokens);
    }

    /// 添加标签
    pub fn add_label(&mut self, key: String, value: String) {
        self.labels.insert(key, value);
    }
}

impl TokenUsage {
    /// 计算使用效率
    pub fn calculate_efficiency(&mut self) {
        self.efficiency_ratio = ratio_as_f64(self.completion_tokens, self.prompt_tokens);
    }
}

impl HealthMetrics {
    /// 计算健康评分
    pub fn calculate_health_score(&mut self) {
        let mut score = 100.0;

        // 成功率权重 40%
        if self.success_rate < 0.95 {
            score -= (0.95 - self.success_rate) * 400.0;
        }

        // 平均响应时间权重 30%
        if self.avg_response_time_ms > 1000.0 {
            score -= ((self.avg_response_time_ms - 1000.0) / 1000.0) * 30.0;
        }

        // P95 响应时间权重 20%
        if self.p95_response_time_ms > 5000.0 {
            score -= ((self.p95_response_time_ms - 5000.0) / 5000.0) * 20.0;
        }

        // 错误多样性权重 10%
        if self.error_distribution.len() > 3
            && let Ok(len_u32) = u32::try_from(self.error_distribution.len())
        {
            score += (f64::from(len_u32) - 3.0) * -2.5;
        }

        self.health_score = score.clamp(0.0, 100.0);

        // 确定健康状态
        self.health_status = match self.health_score {
            s if s >= 80.0 => HealthStatus::Healthy,
            s if s >= 60.0 => HealthStatus::Warning,
            s if s >= 30.0 => HealthStatus::Critical,
            _ => HealthStatus::Unavailable,
        };
    }
}
