//! 追踪类型（与 providers 解耦）

#[derive(Debug, Clone, Default)]
pub struct TraceStats {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub cache_create_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    pub cost: Option<f64>,
    pub cost_currency: Option<String>,
    pub model_name: Option<String>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
}

