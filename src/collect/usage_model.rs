//! 模型与用量解析服务（合并非流式与流式处理）

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde_json::Value;
use std::sync::LazyLock;

use crate::collect::types::{ComputedStats, TokenUsageMetrics};
use crate::proxy::ProxyContext;
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
        "response.model",        // openai
        "response.modelVersion", // gemini
    ]
});

// 提取器缓存：provider_id -> TokenFieldExtractor
static EXTRACTOR_CACHE: LazyLock<
    RwLock<HashMap<i32, Arc<crate::collect::field_extractor::TokenFieldExtractor>>>,
> = LazyLock::new(|| RwLock::new(HashMap::new()));

/// 清理指定 provider 的 Token 提取器缓存（配置更新后生效）
pub fn invalidate_token_extractor_cache(provider_id: i32) {
    let _ = EXTRACTOR_CACHE
        .write()
        .map(|mut cache| cache.remove(&provider_id));
}

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
) -> Option<Arc<crate::collect::field_extractor::TokenFieldExtractor>> {
    let id = provider.id;
    let value = EXTRACTOR_CACHE.read().unwrap().get(&id).cloned();
    if let Some(extractor) = value {
        return Some(extractor);
    }
    let mapping_json = provider.token_mappings_json.as_ref()?;
    let Ok(cfg) = crate::collect::field_extractor::TokenMappingConfig::from_json(mapping_json)
    else {
        return None;
    };
    let extractor = Arc::new(crate::collect::field_extractor::TokenFieldExtractor::new(
        cfg,
    ));
    EXTRACTOR_CACHE
        .write()
        .unwrap()
        .insert(id, extractor.clone());
    Some(extractor)
}

#[must_use]
pub fn extract_tokens_from_json(
    provider: Option<&entity::provider_types::Model>,
    json: &Value,
) -> TokenUsageMetrics {
    let mut usage = TokenUsageMetrics::default();
    if let Some(p) = provider
        && let Some(extractor) = get_or_build_extractor(p)
    {
        usage.prompt_tokens = extractor.extract_token_count(json, "tokens_prompt");
        usage.completion_tokens = extractor.extract_token_count(json, "tokens_completion");
        usage.total_tokens = extractor.extract_token_count(json, "tokens_total");
        usage.cache_create_tokens = extractor.extract_token_count(json, "cache_create_tokens");
        usage.cache_read_tokens = extractor.extract_token_count(json, "cache_read_tokens");
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

/// 统一在 `EOS（end_of_stream）时进行解析与统计`。
///
/// 逻辑：
/// - 使用完整的 `ctx.body` 进行解压与解析；
/// - Content-Type 决定解析方式：SSE（按事件）、NDJSON（按行）、普通 JSON（整体/窗口）。
/// - 用量字段采用“累加”策略；模型名称取最后一次出现或整体 JSON 中的字段。
#[allow(clippy::too_many_lines)]
pub fn finalize_eos(ctx: &mut ProxyContext) -> ComputedStats {
    use crate::collect::util::{decompress_for_stats, find_last_balanced_json};
    use bytes::BytesMut;

    let mut stats = ComputedStats::default();

    let content_type = ctx
        .response
        .details
        .content_type
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let encoding = ctx.response.details.content_encoding.as_deref();
    let raw = ctx.response.body.clone();

    // 无正文：置零并回退模型
    if raw.is_empty() {
        stats.usage.prompt_tokens = Some(0);
        stats.usage.completion_tokens = Some(0);
        stats.usage.total_tokens = Some(0);
        stats.model_name.clone_from(&ctx.request.requested_model);
        return stats;
    }

    // 解压+限流读取（默认 2MB 上限）
    let decoded = decompress_for_stats(encoding, &raw, 2 * 1024 * 1024);
    let Ok(body_str) = std::str::from_utf8(&decoded) else {
        // UTF-8 解码失败，无法进行 JSON 解析
        stats.usage.prompt_tokens = Some(0);
        stats.usage.completion_tokens = Some(0);
        stats.usage.total_tokens = Some(0);
        stats.model_name.clone_from(&ctx.request.requested_model);
        return stats;
    };

    // SSE：text/event-stream
    if content_type.contains("text/event-stream") {
        let mut event_stream_decoder = crate::utils::event_stream::EventStreamData::new();
        let mut buf = BytesMut::new();
        buf.extend_from_slice(body_str.as_bytes());
        let mut last_json: Option<serde_json::Value> = None;
        loop {
            match event_stream_decoder.decode(&mut buf) {
                Ok(Some(ev)) => {
                    let json = ev.data;
                    if !json.is_null() {
                        let usage =
                            extract_tokens_from_json(ctx.routing.provider_type.as_ref(), &json);
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
                    if let Ok(Some(ev)) = event_stream_decoder.decode_eof(&mut buf) {
                        let json = ev.data;
                        if !json.is_null() {
                            let usage =
                                extract_tokens_from_json(ctx.routing.provider_type.as_ref(), &json);
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
            stats.model_name.clone_from(&ctx.request.requested_model);
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
                    let usage = extract_tokens_from_json(ctx.routing.provider_type.as_ref(), &json);
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
            stats.model_name.clone_from(&ctx.request.requested_model);
        }
        return stats;
    }

    // 普通 JSON：整体/窗口解析
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_str) {
        let usage = extract_tokens_from_json(ctx.routing.provider_type.as_ref(), &json);
        stats.usage = usage;
        stats.model_name =
            extract_model_from_json(&json).or_else(|| ctx.request.requested_model.clone());
        return stats;
    }
    // 尝试窗口：逐行扫描最后一段 JSON 或查找最后一个平衡的 JSON
    let mut last_json: Option<serde_json::Value> = None;
    for line in body_str.lines() {
        let t = line.trim_start_matches("data:").trim();
        if let Some(pos) = t.find('{')
            && let Ok(j) = serde_json::from_str::<serde_json::Value>(&t[pos..])
        {
            last_json = Some(j);
        }
    }
    if last_json.is_none() {
        last_json = find_last_balanced_json(body_str);
    }
    if let Some(j) = last_json {
        let usage = extract_tokens_from_json(ctx.routing.provider_type.as_ref(), &j);
        stats.usage = usage;
        stats.model_name = extract_model_from_json(&j);
    }
    if stats.usage.total_tokens.is_none() {
        stats.usage.prompt_tokens = Some(0);
        stats.usage.completion_tokens = Some(0);
        stats.usage.total_tokens = Some(0);
    }
    if stats.model_name.is_none() {
        stats.model_name.clone_from(&ctx.request.requested_model);
    }
    stats
}

// 注意：不再提供 finalize_streaming 别名，统一使用 finalize_eos。
