//! 测试模块：工具函数的尾窗处理与解压缩功能
//!
//! 此模块包含对 `crate::statistics::util` 中 `find_last_balanced_json` 等函数的集成测试，
//! 确保流式响应解析的正确性。

use api_proxy::statistics::usage_model::extract_model_from_json;
use api_proxy::statistics::util::{decompress_for_stats, find_last_balanced_json};
use flate2::{Compression, write::GzEncoder};
use std::io::Write;

#[test]
fn find_last_balanced_json_extracts_model() {
    let s = "noise\n data: {\"foo\":1}\n data: {\"model\":\"gpt-4o-mini\", \"usage\":{}}\n";
    let json = find_last_balanced_json(s).expect("should find json");
    let model = extract_model_from_json(&json);
    assert_eq!(model.as_deref(), Some("gpt-4o-mini"));
}

#[test]
fn decompress_for_stats_gzip_ok() {
    let payload = b"{\"hello\":\"world\"}";
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(payload).unwrap();
    let gz = encoder.finish().unwrap();

    let cow = decompress_for_stats(Some("gzip"), &gz, 1024);
    let s = std::str::from_utf8(&cow).unwrap();
    assert_eq!(s, "{\"hello\":\"world\"}");
}
