//! 费用计算服务（轻量封装）

use std::sync::Arc;

use crate::error::ProxyError;
use crate::pricing::PricingCalculatorService;
use crate::statistics::types::TokenUsageMetrics;

/// 根据模型、提供商与用量计算成本
pub async fn calculate(
    pricing: &Arc<PricingCalculatorService>,
    model: &str,
    provider_type_id: i32,
    usage: &TokenUsageMetrics,
    request_id: &str,
) -> Result<(Option<f64>, Option<String>), ProxyError> {
    match pricing
        .calculate_cost(
            model,
            provider_type_id,
            &crate::pricing::TokenUsage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                cache_create_tokens: usage.cache_create_tokens,
                cache_read_tokens: usage.cache_read_tokens,
            },
            request_id,
        )
        .await
    {
        Ok(cost) => Ok((Some(cost.total_cost), Some(cost.currency))),
        Err(e) => {
            tracing::warn!(component = "statistics.price", request_id = %request_id, error = %e, "Failed to calculate cost");
            Ok((None, None))
        }
    }
}
