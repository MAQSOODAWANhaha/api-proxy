//! 统计通用工具

use std::borrow::Cow;

/// 判断 content-type 是否为 JSON（application/json 或 application/*+json）
#[must_use]
pub fn content_type_is_json(content_type: &str) -> bool {
    let ct = content_type.to_ascii_lowercase();
    ct.starts_with("application/json") || ct.contains("+json")
}

/// 仅用于统计侧的限量解压（不影响下游透传）
/// 支持 gzip/deflate/br；对于逗号分隔的多编码，选择首个可识别的编码处理
#[must_use]
pub fn decompress_for_stats<'a>(
    encoding: Option<&'a str>,
    input: &'a [u8],
    max_out: usize,
) -> Cow<'a, [u8]> {
    use flate2::read::{GzDecoder, ZlibDecoder};
    use std::io::Read;

    let normalize = |e: &str| {
        e.split(',')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase()
    };
    match encoding.map(normalize) {
        Some(enc) if enc.contains("gzip") => {
            let mut dec = GzDecoder::new(input);
            let mut out = Vec::with_capacity(input.len().min(max_out));
            let mut buf = [0u8; 8192];
            while out.len() < max_out {
                match dec.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let take = n.min(max_out - out.len());
                        out.extend_from_slice(&buf[..take]);
                        if take < n {
                            break;
                        }
                    }
                }
            }
            Cow::Owned(out)
        }
        Some(enc) if enc.contains("deflate") => {
            let mut dec = ZlibDecoder::new(input);
            let mut out = Vec::with_capacity(input.len().min(max_out));
            let mut buf = [0u8; 8192];
            while out.len() < max_out {
                match dec.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let take = n.min(max_out - out.len());
                        out.extend_from_slice(&buf[..take]);
                        if take < n {
                            break;
                        }
                    }
                }
            }
            Cow::Owned(out)
        }
        Some(enc) if enc.contains("br") || enc.contains("brotli") => {
            let mut out = Vec::with_capacity(input.len().min(max_out));
            let mut reader = brotli_decompressor::Decompressor::new(input, 4096);
            let mut buf = [0u8; 8192];
            while out.len() < max_out {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let take = n.min(max_out - out.len());
                        out.extend_from_slice(&buf[..take]);
                        if take < n {
                            break;
                        }
                    }
                }
            }
            Cow::Owned(out)
        }
        _ => Cow::Borrowed(input),
    }
}

/// 从字符串中提取最后一个 JSON 对象（容错：优先逐行 data:{...}，其次括号平衡回溯）
#[must_use]
pub fn find_last_balanced_json(s: &str) -> Option<serde_json::Value> {
    // 先尝试逐行 data: {...}
    for line in s.lines().rev() {
        let t = line.trim_start_matches("data: ").trim();
        if let Some(pos) = t.find('{')
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&t[pos..])
        {
            return Some(v);
        }
    }

    // 括号平衡回溯
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut start_idx: Option<usize> = None;
    for (i, b) in bytes.iter().enumerate().rev() {
        match *b {
            b'}' => depth += 1,
            b'{' => {
                depth -= 1;
                if depth == 0 {
                    start_idx = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    if let Some(idx) = start_idx
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&s[idx..])
    {
        return Some(v);
    }
    None
}
