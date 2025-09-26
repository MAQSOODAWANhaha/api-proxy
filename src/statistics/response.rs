//! 响应侧统计采集

use pingora_http::ResponseHeader;

use crate::proxy::ProxyContext;
use crate::statistics::types::ResponseStats;

/// 收集响应详情并同步关键字段到上下文
pub fn collect_details(
    upstream_response: &ResponseHeader,
    ctx: &mut ProxyContext,
) -> ResponseStats {
    // 收集响应头
    let mut headers = std::collections::HashMap::new();
    for (name, value) in upstream_response.headers.iter() {
        if let Ok(value_str) = std::str::from_utf8(value.as_bytes()) {
            headers.insert(name.as_str().to_string(), value_str.to_string());
        }
    }

    // 获取Content-Type和Content-Length
    let content_type = upstream_response
        .headers
        .get("content-type")
        .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
        .map(|s| s.to_string());

    let content_length = upstream_response
        .headers
        .get("content-length")
        .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
        .and_then(|s| s.parse::<i64>().ok());

    // 同步关键字段到上下文
    if let Some(ct) = &content_type {
        ctx.response_details.content_type = Some(ct.clone());
    }
    let content_encoding = upstream_response
        .headers
        .get("content-encoding")
        .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
        .map(|s| s.to_lowercase());
    ctx.response_details.content_encoding = content_encoding;
    ctx.response_details.status_code = Some(upstream_response.status.as_u16());

    ResponseStats {
        status_code: upstream_response.status.as_u16(),
        headers,
        content_type,
        content_length,
    }
}

// 将 ResponseDetails 相关方法迁移到统计服务侧，便于集中维护
impl crate::statistics::types::ResponseDetails {

    pub fn add_body_chunk(&mut self, chunk: &[u8]) {
        let prev_size = self.body_chunks.len();
        self.body_chunks.extend_from_slice(chunk);
        let new_size = self.body_chunks.len();
        if new_size % 8192 == 0 || (prev_size < 1024 && new_size >= 1024) {
            tracing::debug!(
                component = "statistics.collector",
                chunk_size = chunk.len(),
                total_size = new_size,
                "Response body chunk added (milestone reached)"
            );
        }
    }

    pub fn is_sse_format(&self) -> bool {
        if let Some(content_type) = &self.content_type {
            if content_type.contains("text/event-stream") {
                return true;
            }
        }
        false
    }

    pub fn finalize_body(&mut self) {
        let original_chunks_len = self.body_chunks.len();
        if !self.body_chunks.is_empty() {
            tracing::debug!(
                component = "statistics.collector",
                raw_body_size = original_chunks_len,
                "Starting response body finalization"
            );
            match String::from_utf8(self.body_chunks.clone()) {
                Ok(body_str) => {
                    let original_str_len = body_str.len();
                    if body_str.len() > 65536 {
                        self.body = Some(format!(
                            "{}...[truncated {} bytes]",
                            &body_str[..65536],
                            body_str.len() - 65536
                        ));
                        tracing::info!(
                            component = "statistics.collector",
                            original_size = original_str_len,
                            stored_size = 65536,
                            truncated_bytes = original_str_len - 65536,
                            "Response body finalized as UTF-8 string (truncated)"
                        );
                    } else {
                        self.body = Some(body_str.clone());
                        let is_sse = body_str
                            .lines()
                            .any(|line| line.trim().starts_with("data: "));
                        if is_sse {
                            let data_line_count = body_str
                                .lines()
                                .filter(|line| {
                                    line.trim().starts_with("data: ") && !line.contains("[DONE]")
                                })
                                .count();
                            tracing::info!(
                                component = "statistics.collector",
                                body_size = original_str_len,
                                is_sse_format = true,
                                sse_data_lines = data_line_count,
                                "Response body finalized as UTF-8 string (complete, SSE format detected)"
                            );
                        } else {
                            tracing::info!(
                                component = "statistics.collector",
                                body_size = original_str_len,
                                is_sse_format = false,
                                "Response body finalized as UTF-8 string (complete)"
                            );
                        }
                    }
                }
                Err(utf8_error) => {
                    let truncated_chunks = if self.body_chunks.len() > 1024 {
                        &self.body_chunks[..1024]
                    } else {
                        &self.body_chunks
                    };
                    self.body = Some(format!("binary-data:{}", hex::encode(truncated_chunks)));
                    tracing::info!(
                        component = "statistics.collector",
                        raw_size = original_chunks_len,
                        encoded_size = truncated_chunks.len(),
                        utf8_error = format!("{:?}", utf8_error),
                        "Response body finalized as hex-encoded binary data"
                    );
                }
            }
            self.body_size = Some(self.body_chunks.len() as u64);
        } else {
            tracing::debug!(component = "statistics.collector", "No response body chunks to finalize (empty response)");
        }
    }

    pub fn clear_body_chunks(&mut self) {
        if !self.body_chunks.is_empty() {
            tracing::debug!(
                component = "statistics.collector",
                cleared_bytes = self.body_chunks.len(),
                "Clearing collected body chunks to reduce memory"
            );
            self.body_chunks.clear();
        }
    }

    pub fn make_preview(&self, limit: usize) -> String {
        if let Some(b) = &self.body {
            if b.len() > limit {
                format!("{}...[truncated {} bytes]", &b[..limit], b.len() - limit)
            } else {
                b.clone()
            }
        } else {
            "<empty>".to_string()
        }
    }
}
