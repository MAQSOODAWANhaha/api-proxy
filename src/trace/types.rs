//! 追踪类型（与 providers 解耦）

use crate::types::TokenCount;

#[derive(Debug, Clone, Default)]
pub struct TraceStats {
    pub input_tokens: Option<TokenCount>,
    pub output_tokens: Option<TokenCount>,
    pub total_tokens: Option<TokenCount>,
    pub cache_create_tokens: Option<TokenCount>,
    pub cache_read_tokens: Option<TokenCount>,
    pub cost: Option<f64>,
    pub cost_currency: Option<String>,
    pub model_name: Option<String>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
}
