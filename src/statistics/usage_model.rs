//! 模型与用量解析服务（合并非流式与流式处理）

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde_json::Value;
use std::sync::LazyLock;

use crate::proxy::ProxyContext;
use crate::statistics::types::{ComputedStats, TokenUsageMetrics};
use tokio_util::codec::Decoder as _; // for EventStreamData decode

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
    let cfg = match crate::statistics::field_extractor::TokenMappingConfig::from_json(mapping_json)
    {
        Ok(c) => c,
        Err(_) => return None,
    };
    let extractor = Arc::new(crate::statistics::field_extractor::TokenFieldExtractor::new(cfg));
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

// 流式请求一律采用“累加”策略（单事件/单行提取的用量按字段相加）。

// 已统一仅使用 extract_tokens_from_json（数据库驱动 + 归一化），
// 流式路径在合并阶段采用“累加”策略，不再需要 partial 版本。

/// 统一在 EOS（end_of_stream）时进行解析与统计。
///
/// 逻辑：
/// - 使用完整的 `ctx.body` 进行解压与解析；
/// - Content-Type 决定解析方式：SSE（按事件）、NDJSON（按行）、普通 JSON（整体/窗口）。
/// - 用量字段采用“累加”策略；模型名称取最后一次出现或整体 JSON 中的字段。
pub fn finalize_eos(ctx: &mut ProxyContext) -> ComputedStats {
    use crate::logging::log_complete_response;
    use crate::statistics::util::{decompress_for_stats, find_last_balanced_json};
    use bytes::BytesMut;

    let mut stats = ComputedStats::default();

    let content_type = ctx
        .response_details
        .content_type
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let encoding = ctx.response_details.content_encoding.as_deref();
    let raw = ctx.response_body.clone();

    // 使用 log_complete_response 记录完整的响应信息
    log_complete_response(
        &ctx.request_id,
        &ctx.request_details.path, // 从 request_details 获取 path
        &ctx.response_details.headers,
        &raw,
    );

    // 无正文：置零并回退模型
    if raw.is_empty() {
        stats.usage.prompt_tokens = Some(0);
        stats.usage.completion_tokens = Some(0);
        stats.usage.total_tokens = Some(0);
        stats.model_name = ctx.requested_model.clone();
        return stats;
    }

    // 解压+限流读取（默认 2MB 上限）
    let decoded = decompress_for_stats(encoding, &raw, 2 * 1024 * 1024);
    let body_str = match std::str::from_utf8(&decoded) {
        Ok(s) => s,
        Err(_) => {
            // UTF-8 解码失败，无法进行 JSON 解析
            stats.usage.prompt_tokens = Some(0);
            stats.usage.completion_tokens = Some(0);
            stats.usage.total_tokens = Some(0);
            stats.model_name = ctx.requested_model.clone();
            return stats;
        }
    };

    // SSE：text/event-stream
    if content_type.contains("text/event-stream") {
        let mut decoder = crate::utils::event_stream::EventStreamData::new();
        let mut buf = BytesMut::new();
        buf.extend_from_slice(body_str.as_bytes());
        let mut last_json: Option<serde_json::Value> = None;
        loop {
            match decoder.decode(&mut buf) {
                Ok(Some(ev)) => {
                    let json = ev.data;
                    if !json.is_null() {
                        let usage = extract_tokens_from_json(ctx.provider_type.as_ref(), &json);
                        // 累加策略
                        stats.usage.prompt_tokens = Some(
                            stats.usage.prompt_tokens.unwrap_or(0)
                                + usage.prompt_tokens.unwrap_or(0),
                        );
                        stats.usage.completion_tokens = Some(
                            stats.usage.completion_tokens.unwrap_or(0)
                                + usage.completion_tokens.unwrap_or(0),
                        );
                        stats.usage.total_tokens = Some(
                            stats.usage.total_tokens.unwrap_or(0) + usage.total_tokens.unwrap_or(0),
                        );
                        stats.usage.cache_create_tokens = Some(
                            stats.usage.cache_create_tokens.unwrap_or(0)
                                + usage.cache_create_tokens.unwrap_or(0),
                        );
                        stats.usage.cache_read_tokens = Some(
                            stats.usage.cache_read_tokens.unwrap_or(0)
                                + usage.cache_read_tokens.unwrap_or(0),
                        );
                        last_json = Some(json);
                    }
                }
                Ok(None) => {
                    // flush EOF
                    if let Ok(Some(ev)) = decoder.decode_eof(&mut buf) {
                        let json = ev.data;
                        if !json.is_null() {
                            let usage = extract_tokens_from_json(ctx.provider_type.as_ref(), &json);
                            // 累加策略
                            stats.usage.prompt_tokens = Some(
                                stats.usage.prompt_tokens.unwrap_or(0)
                                    + usage.prompt_tokens.unwrap_or(0),
                            );
                            stats.usage.completion_tokens = Some(
                                stats.usage.completion_tokens.unwrap_or(0)
                                    + usage.completion_tokens.unwrap_or(0),
                            );
                            stats.usage.total_tokens = Some(
                                stats.usage.total_tokens.unwrap_or(0)
                                    + usage.total_tokens.unwrap_or(0),
                            );
                            stats.usage.cache_create_tokens = Some(
                                stats.usage.cache_create_tokens.unwrap_or(0)
                                    + usage.cache_create_tokens.unwrap_or(0),
                            );
                            stats.usage.cache_read_tokens = Some(
                                stats.usage.cache_read_tokens.unwrap_or(0)
                                    + usage.cache_read_tokens.unwrap_or(0),
                            );
                            last_json = Some(json);
                        }
                    }
                    break;
                }
                Err(_) => break,
            }
        }
        if stats.usage.total_tokens.is_none() {
            stats.usage.prompt_tokens = Some(0);
            stats.usage.completion_tokens = Some(0);
            stats.usage.total_tokens = Some(0);
        }
        if let Some(j) = last_json {
            stats.model_name = extract_model_from_json(&j);
        }
        if stats.model_name.is_none() {
            stats.model_name = ctx.requested_model.clone();
        }
        return stats;
    }

    // NDJSON：application/stream+json 或行式 JSON 退化
    if content_type.contains("application/stream+json") {
        let mut last_json: Option<serde_json::Value> = None;
        for raw in body_str.lines() {
            let mut line = raw.trim();
            if line.is_empty() || line.starts_with(':') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("data:") {
                line = rest.trim_start();
            }
            if let Some(pos) = line.find('{') {
                let json_str = &line[pos..];
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                    let usage = extract_tokens_from_json(ctx.provider_type.as_ref(), &json);
                    stats.usage.prompt_tokens = Some(
                        stats.usage.prompt_tokens.unwrap_or(0) + usage.prompt_tokens.unwrap_or(0),
                    );
                    stats.usage.completion_tokens = Some(
                        stats.usage.completion_tokens.unwrap_or(0)
                            + usage.completion_tokens.unwrap_or(0),
                    );
                    stats.usage.total_tokens = Some(
                        stats.usage.total_tokens.unwrap_or(0) + usage.total_tokens.unwrap_or(0),
                    );
                    stats.usage.cache_create_tokens = Some(
                        stats.usage.cache_create_tokens.unwrap_or(0)
                            + usage.cache_create_tokens.unwrap_or(0),
                    );
                    stats.usage.cache_read_tokens = Some(
                        stats.usage.cache_read_tokens.unwrap_or(0)
                            + usage.cache_read_tokens.unwrap_or(0),
                    );
                    last_json = Some(json);
                }
            }
        }
        if stats.usage.total_tokens.is_none() {
            stats.usage.prompt_tokens = Some(0);
            stats.usage.completion_tokens = Some(0);
            stats.usage.total_tokens = Some(0);
        }
        if let Some(j) = last_json {
            stats.model_name = extract_model_from_json(&j);
        }
        if stats.model_name.is_none() {
            stats.model_name = ctx.requested_model.clone();
        }
        return stats;
    }

    // 普通 JSON：整体/窗口解析
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_str) {
        let usage = extract_tokens_from_json(ctx.provider_type.as_ref(), &json);
        stats.usage = usage;
        stats.model_name = extract_model_from_json(&json).or_else(|| ctx.requested_model.clone());
        return stats;
    } else {
        // 尝试窗口：逐行扫描最后一段 JSON 或查找最后一个平衡的 JSON
        let mut last_json: Option<serde_json::Value> = None;
        for line in body_str.lines() {
            let t = line.trim_start_matches("data:").trim();
            if let Some(pos) = t.find('{') {
                if let Ok(j) = serde_json::from_str::<serde_json::Value>(&t[pos..]) {
                    last_json = Some(j);
                }
            }
        }
        if last_json.is_none() {
            last_json = find_last_balanced_json(body_str);
        }
        if let Some(j) = last_json {
            let usage = extract_tokens_from_json(ctx.provider_type.as_ref(), &j);
            stats.usage = usage;
            stats.model_name = extract_model_from_json(&j);
        }
        if stats.usage.total_tokens.is_none() {
            stats.usage.prompt_tokens = Some(0);
            stats.usage.completion_tokens = Some(0);
            stats.usage.total_tokens = Some(0);
        }
        if stats.model_name.is_none() {
            stats.model_name = ctx.requested_model.clone();
        }
        return stats;
    }
}

// 注意：不再提供 finalize_streaming 别名，统一使用 finalize_eos。
