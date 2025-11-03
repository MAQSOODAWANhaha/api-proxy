//! 数据采集阶段统一使用的类型定义

use serde::Serialize;
use serde_json::Value;

use crate::types::{ProviderTypeId, TokenCount};

// === 请求/响应概览类型（采集层） ===
#[derive(Debug, Clone)]
pub struct RequestStats {
    pub method: String,
    pub path: String,
    pub client_ip: String,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResponseStats {
    pub status_code: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub content_type: Option<String>,
    pub content_length: Option<i64>,
}

// === 请求/响应详情类型（上下文持久化） ===
#[derive(Clone, Debug, Default, Serialize)]
pub struct RequestDetails {
    pub headers: std::collections::HashMap<String, String>,
    pub body_size: Option<u64>,
    pub content_type: Option<String>,
    pub client_ip: String,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub method: String,
    pub path: String,
    pub protocol_version: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ResponseDetails {
    pub headers: std::collections::HashMap<String, String>,
    pub content_type: Option<String>,
    pub content_encoding: Option<String>,
    pub status_code: Option<u16>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct TokenUsageMetrics {
    pub prompt_tokens: Option<TokenCount>,
    pub completion_tokens: Option<TokenCount>,
    pub total_tokens: Option<TokenCount>,
    pub cache_create_tokens: Option<TokenCount>,
    pub cache_read_tokens: Option<TokenCount>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ComputedStats {
    pub usage: TokenUsageMetrics,
    pub model_name: Option<String>,
    pub cost: Option<f64>,
    pub cost_currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sse_json: Option<Value>,
}

/// 成本快照
#[derive(Clone, Debug, Default, Serialize)]
pub struct CollectedCost {
    pub value: Option<f64>,
    pub currency: Option<String>,
}

/// 采集完成后的整体指标
#[derive(Clone, Debug, Serialize)]
pub struct CollectedMetrics {
    pub request_id: String,
    pub user_id: Option<i32>,
    pub user_service_api_id: Option<i32>,
    pub provider_type_id: Option<ProviderTypeId>,
    pub model: Option<String>,
    pub usage: TokenUsageMetrics,
    pub cost: CollectedCost,
    pub duration_ms: u128,
    pub status_code: u16,
}
