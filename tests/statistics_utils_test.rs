//! 采集工具测试：保持与精简后的实现一致

use api_proxy::collect::util::{
    content_type_is_json, decompress_for_stats, find_last_balanced_json,
};
use flate2::{Compression, write::GzEncoder};
use std::io::Write;

#[test]
fn test_content_type_guard() {
    assert!(content_type_is_json("application/json"));
    assert!(content_type_is_json("application/vnd.api+json"));
    assert!(!content_type_is_json("text/plain"));
    assert!(!content_type_is_json("text/html"));
}

#[test]
fn test_find_last_balanced_json() {
    let s = "prefix\n data: {\"a\":1}\n data: {\"model\":\"gpt-4o\"}\n tail";
    let v = find_last_balanced_json(s).expect("should extract json");
    assert_eq!(v.get("model").and_then(|x| x.as_str()), Some("gpt-4o"));
}

#[test]
fn test_decompress_gzip_roundtrip() {
    let raw = b"{\"k\":\"v\"}";
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(raw).unwrap();
    let gz = enc.finish().unwrap();

    let cow = decompress_for_stats(Some("gzip"), &gz, 4096);
    assert_eq!(std::str::from_utf8(&cow).unwrap(), "{\"k\":\"v\"}");
}
