//! 统计类型定义：统一流式增量与最终统计

use serde::Serialize;

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
