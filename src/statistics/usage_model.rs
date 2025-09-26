//! 模型与用量解析服务（合并非流式与流式处理）

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde_json::Value;
use std::sync::LazyLock;

use crate::proxy::ProxyContext;
use crate::statistics::types::{ComputedStats, TokenUsageMetrics};

// 预编译模型路径（按优先级）
static MODEL_PATHS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    vec![
        "model",
        "modelName",
        "model_id",
        "data.0.model",
        "choices.0.model",
        "candidates.0.model",
        "response.model",
        "result.model",
    ]
});

// 提取器缓存：provider_id -> TokenFieldExtractor
static EXTRACTOR_CACHE: LazyLock<
    RwLock<HashMap<i32, Arc<crate::statistics::field_extractor::TokenFieldExtractor>>>,
> = LazyLock::new(|| RwLock::new(HashMap::new()));

fn extract_model_by_path(json: &Value, path: &str) -> Option<String> {
    let mut cur = json;
    for seg in path.split('.') {
        if let Ok(idx) = seg.parse::<usize>() {
            cur = cur.get(idx)?;
        } else {
            cur = cur.get(seg)?;
        }
    }
    if let Some(s) = cur.as_str() {
        let t = s.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    None
}

pub fn extract_model_from_json(json: &Value) -> Option<String> {
    for p in MODEL_PATHS.iter() {
        if let Some(m) = extract_model_by_path(json, p) {
            return Some(m);
        }
    }
    None
}

fn get_or_build_extractor(
    provider: &entity::provider_types::Model,
) -> Option<Arc<crate::statistics::field_extractor::TokenFieldExtractor>> {
    let id = provider.id;
    if let Some(extractor) = EXTRACTOR_CACHE.read().unwrap().get(&id).cloned() {
        return Some(extractor);
    }
    let mapping_json = provider.token_mappings_json.as_ref()?;
    let cfg = match crate::statistics::field_extractor::TokenMappingConfig::from_json(mapping_json) {
        Ok(c) => c,
        Err(_) => return None,
    };
    let extractor = Arc::new(crate::statistics::field_extractor::TokenFieldExtractor::new(
        cfg,
    ));
    EXTRACTOR_CACHE
        .write()
        .unwrap()
        .insert(id, extractor.clone());
    Some(extractor)
}

pub fn extract_tokens_from_json(
    provider: Option<&entity::provider_types::Model>,
    json: &Value,
) -> TokenUsageMetrics {
    let mut usage = TokenUsageMetrics::default();
    if let Some(p) = provider {
        if let Some(extractor) = get_or_build_extractor(p) {
            usage.prompt_tokens = extractor.extract_token_u32(json, "tokens_prompt");
            usage.completion_tokens = extractor.extract_token_u32(json, "tokens_completion");
            usage.total_tokens = extractor.extract_token_u32(json, "tokens_total");
            usage.cache_create_tokens = extractor.extract_token_u32(json, "cache_create_tokens");
            usage.cache_read_tokens = extractor.extract_token_u32(json, "cache_read_tokens");
        }
    }
    normalize(&mut usage);
    usage
}

pub fn normalize(usage: &mut TokenUsageMetrics) {
    if usage.prompt_tokens.is_none() {
        usage.prompt_tokens = Some(0);
    }
    if usage.completion_tokens.is_none() {
        usage.completion_tokens = Some(0);
    }
    if usage.total_tokens.is_none() {
        usage.total_tokens =
            Some(usage.prompt_tokens.unwrap_or(0) + usage.completion_tokens.unwrap_or(0));
    }
}

/// 流式收口：归总 usage_partial 并尝试从尾窗解析模型
pub fn finalize_streaming(ctx: &mut ProxyContext) -> ComputedStats {
    let mut stats = ComputedStats::default();

    // 使用增量聚合的 usage（latest-wins）
    if ctx.usage_partial.prompt_tokens.is_some()
        || ctx.usage_partial.completion_tokens.is_some()
        || ctx.usage_partial.total_tokens.is_some()
        || ctx.usage_partial.cache_create_tokens.is_some()
        || ctx.usage_partial.cache_read_tokens.is_some()
    {
        stats.usage.prompt_tokens = ctx.usage_partial.prompt_tokens;
        stats.usage.completion_tokens = ctx.usage_partial.completion_tokens;
        stats.usage.total_tokens = ctx
            .usage_partial
            .total_tokens
            .or_else(|| {
                match (
                    ctx.usage_partial.prompt_tokens,
                    ctx.usage_partial.completion_tokens,
                ) {
                    (Some(p), Some(c)) => Some(p + c),
                    (Some(p), None) => Some(p),
                    (None, Some(c)) => Some(c),
                    _ => Some(0),
                }
            })
            .or(Some(0));
        stats.usage.cache_create_tokens = ctx.usage_partial.cache_create_tokens;
        stats.usage.cache_read_tokens = ctx.usage_partial.cache_read_tokens;
    } else {
        stats.usage.prompt_tokens = Some(0);
        stats.usage.completion_tokens = Some(0);
        stats.usage.total_tokens = Some(0);
    }

    // 从尾窗尝试提取模型兜底
    if stats.model_name.is_none() && !ctx.response_details.tail_window.is_empty() {
        if let Ok(s) = String::from_utf8(ctx.response_details.tail_window.clone()) {
            if let Some(last_json) = crate::statistics::util::find_last_balanced_json(&s) {
                stats.model_name = extract_model_from_json(&last_json);
            }
        }
    }

    // 最终兜底：仍无模型则使用请求阶段模型
    if stats.model_name.is_none() {
        stats.model_name = ctx.requested_model.clone();
    }

    stats
}
