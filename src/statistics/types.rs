//! 统计类型定义：统一流式增量与最终统计

use serde::Serialize;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    #[serde(skip_serializing)]
    pub body_chunks: Vec<u8>,
    #[serde(skip_serializing)]
    pub tail_window: Vec<u8>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct TokenUsageMetrics {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub cache_create_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PartialUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub cache_create_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
}

impl PartialUsage {
    pub fn merge_latest(&mut self, other: &PartialUsage) {
        if other.prompt_tokens.is_some() {
            self.prompt_tokens = other.prompt_tokens;
        }
        if other.completion_tokens.is_some() {
            self.completion_tokens = other.completion_tokens;
        }
        if other.total_tokens.is_some() {
            self.total_tokens = other.total_tokens;
        }
        if other.cache_create_tokens.is_some() {
            self.cache_create_tokens = other.cache_create_tokens;
        }
        if other.cache_read_tokens.is_some() {
            self.cache_read_tokens = other.cache_read_tokens;
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ComputedStats {
    pub usage: TokenUsageMetrics,
    pub model_name: Option<String>,
    pub cost: Option<f64>,
    pub cost_currency: Option<String>,
}
