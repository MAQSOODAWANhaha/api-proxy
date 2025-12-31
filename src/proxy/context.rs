//! # 代理上下文模块
//!
//! 包含代理请求处理过程中使用的上下文类型定义

use crate::proxy::provider_strategy::ProviderStrategy;
use bytes::BytesMut;
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

/// 请求上下文
// #[derive(Debug, Clone)]
pub struct ProxyContext {
    /// 请求ID
    pub request_id: String,
    /// 开始时间
    pub start_time: Instant,
    /// 重试次数
    pub retry_count: u32,
    /// 请求详情
    pub request_details: RequestDetails,
    /// 响应详情
    pub response_details: ResponseDetails,
    /// 连接超时时间(秒)
    pub timeout_seconds: Option<i32>,
    /// 请求体缓冲区 (`用于request_body_filter中的数据收集`)
    pub request_body: BytesMut,
    /// 响应体缓冲区 (`用于response_body_filter中的数据收集`)
    pub response_body: BytesMut,
    /// 是否计划修改请求体（供上游头部处理决策使用）
    pub will_modify_body: bool,
    /// 解析得到的最终上游凭证（由 `CredentialResolutionStep` 设置）
    pub resolved_credential: Option<ResolvedCredential>,
    /// `ChatGPT` Account ID（用于OpenAI `ChatGPT` API）
    pub account_id: Option<String>,
    /// 用户请求的模型名称
    pub requested_model: Option<String>,
    /// 最终使用量（统一出口）
    pub usage_final: Option<TokenUsageMetrics>,
    /// 追踪记录是否已成功写入数据库
    pub trace_started: bool,

    // === 认证相关字段（逐步填充） ===
    /// 用户对外API配置
    pub user_service_api: Option<user_service_apis::Model>,
    /// 选择的后端API密钥
    pub selected_backend: Option<user_provider_keys::Model>,
    /// 提供商类型配置
    pub provider_type: Option<provider_types::Model>,
    /// 选定的服务商策略
    pub strategy: Option<Arc<dyn ProviderStrategy>>,

    // === 日志模式相关字段（仅在 user_service_api.log_mode=true 时填充） ===
    /// 最终上游请求头（包含注入/清理后的结果）
    pub upstream_request_headers: Option<BTreeMap<String, String>>,
    /// 最终上游请求 URI（可能被策略改写）
    pub upstream_request_uri: Option<String>,
}

impl Default for ProxyContext {
    fn default() -> Self {
        Self {
            request_id: String::new(),
            start_time: Instant::now(),
            retry_count: 0,
            request_details: RequestDetails::default(),
            response_details: ResponseDetails::default(),
            timeout_seconds: None,
            request_body: BytesMut::new(),
            response_body: BytesMut::new(),
            will_modify_body: false,
            resolved_credential: None,
            account_id: None,
            requested_model: None,
            usage_final: None,
            trace_started: false,
            // 认证相关字段
            user_service_api: None,
            selected_backend: None,
            provider_type: None,
            strategy: None,
            upstream_request_headers: None,
            upstream_request_uri: None,
        }
    }
}

impl ProxyContext {}

impl ProxyContext {
    /// 标记追踪已成功启动
    pub const fn mark_trace_started(&mut self) {
        self.trace_started = true;
    }

    /// 判断是否已成功启动追踪
    #[must_use]
    pub const fn is_trace_started(&self) -> bool {
        self.trace_started
    }
}
