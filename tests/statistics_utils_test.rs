//! 统计服务测试用例
//!
//! 测试统计服务在各种边界情况下的处理能力，包括：
//! - 空响应体处理
//! - 压缩数据处理
//! - JSON解析容错
//! - 解压错误处理
//! - 混合内容提取

use api_proxy::statistics::service::{
    is_valid_json_format, preprocess_json_string,
    try_extract_json_from_mixed_content, detect_compression_format
};

#[test]
fn test_json_format_validation() {
    // 测试有效的JSON格式
    assert!(is_valid_json_format("{}"));
    assert!(is_valid_json_format("[]"));
    assert!(is_valid_json_format("\"string\""));
    assert!(is_valid_json_format("true"));
    assert!(is_valid_json_format("null"));

    // 测试无效的JSON格式
    assert!(!is_valid_json_format(""));
    assert!(!is_valid_json_format("not json"));
    assert!(!is_valid_json_format("{"));
    assert!(!is_valid_json_format("["));
    assert!(!is_valid_json_format("42")); // 数字不被认为有效的JSON格式
}

#[test]
fn test_json_preprocessing() {
    // 测试BOM处理
    let with_bom = "\u{FEFF}{\"key\": \"value\"}";
    let processed = preprocess_json_string(with_bom);
    assert!(processed.starts_with('{'));
    assert!(!processed.starts_with('\u{FEFF}'));

    // 测试空格处理
    let with_spaces = "  {\"key\": \"value\"}  ";
    let processed = preprocess_json_string(with_spaces);
    assert!(processed.starts_with('{'));
    assert!(processed.ends_with('}'));
}

#[test]
fn test_mixed_content_extraction() {
    // 测试混合内容提取
    let mixed = "prefix {\"key\": \"value\"} suffix";
    let extracted = try_extract_json_from_mixed_content(mixed);
    assert!(extracted.is_some());

    // 测试纯JSON
    let pure_json = "{\"key\": \"value\"}";
    let extracted = try_extract_json_from_mixed_content(pure_json);
    assert!(extracted.is_some());

    // 测试无JSON内容
    let no_json = "just plain text";
    let extracted = try_extract_json_from_mixed_content(no_json);
    assert!(extracted.is_none());
}

#[test]
fn test_compression_detection() {
    // 测试gzip魔术字检测
    let gzip_data = b"\x1f\x8b\x08\x00\x00\x00\x00\x00\x03";
    let detected = detect_compression_format(gzip_data);
    assert_eq!(detected, Some("gzip".to_string()));

    // 测试zlib魔术字检测
    let zlib_data = b"\x78\x9c\x03\x00\x00\x00\x00\x01";
    let detected = detect_compression_format(zlib_data);
    assert_eq!(detected, Some("deflate".to_string()));

    // 测试非压缩数据
    let normal_data = b"normal text data";
    let detected = detect_compression_format(normal_data);
    assert!(detected.is_none());

    // 测试空数据
    let empty_data = b"";
    let detected = detect_compression_format(empty_data);
    assert!(detected.is_none());

    // 测试短数据
    let short_data = b"x";
    let detected = detect_compression_format(short_data);
    assert!(detected.is_none());
}

#[test]
fn test_gzip_magic_bytes() {
    // 测试各种gzip魔术字组合
    let test_cases = vec![
        (b"\x1f\x8b", "gzip"),  // 标准gzip魔术字
    ];

    for (data, expected) in test_cases {
        let detected = detect_compression_format(data);
        assert_eq!(detected, Some(expected.to_string()));
    }
}

#[test]
fn test_invalid_compression_data() {
    // 测试无效的压缩数据
    let invalid_cases = vec![
        b"\x1f\x00",  // 无效的gzip魔术字
        b"\x78\x00",  // 无效的zlib魔术字
        b"\xff\xff",  // 完全无效的数据
        b"no",  // 纯文本（2字节）
    ];

    for data in invalid_cases {
        let detected = detect_compression_format(data);
        assert!(detected.is_none(), "Should not detect compression for: {:?}", data);
    }
}

#[tokio::test]
async fn test_json_with_comments() {
    // 测试包含JavaScript注释的JSON预处理
    let json_with_comments = r#"{
        // 这是模型信息
        "model": "gpt-3.5-turbo",
        /* 这是usage块 */
        "usage": {
            "prompt_tokens": 8,    // 提示tokens
            "completion_tokens": 12, // 完成tokens
            "total_tokens": 20     // 总tokens
        }
    }"#;

    // 预处理应该保留注释（标准JSON解析器会处理）
    let processed = preprocess_json_string(json_with_comments);
    assert!(processed.contains("model"));
    assert!(processed.contains("usage"));
}

#[test]
fn test_unicode_and_bom_handling() {
    // 测试Unicode BOM处理
    let utf8_bom = "\u{FEFF}{\"test\": \"value\"}";
    let processed = preprocess_json_string(utf8_bom);
    assert!(!processed.starts_with('\u{FEFF}'));
    assert!(processed.starts_with('{'));
}

#[test]
fn test_whitespace_handling() {
    // 测试各种空白字符处理
    let test_cases = vec![
        "  \t\n\r{\"test\": \"value\"}\t\n\r  ",
        "\u{2003}\u{2002}{\"test\": \"value\"}\u{2003}", // Unicode空格
    ];

    for input in test_cases {
        let processed = preprocess_json_string(input);
        assert!(processed.starts_with('{'));
        assert!(processed.ends_with('}'));
    }
}