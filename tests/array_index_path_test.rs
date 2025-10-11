//! 测试数组索引路径功能
//!
//! 测试新的数组索引路径解析功能，如 usage.0.usageMetadata.promptTokenCount

use api_proxy::statistics::field_extractor::{TokenFieldExtractor, TokenMappingConfig};
use serde_json::json;

#[test]
fn test_array_index_path_extraction() {
    let config_json = r#"{
        "tokens_prompt": {"type": "direct", "path": "usage.0.usageMetadata.promptTokenCount"},
        "tokens_completion": {"type": "direct", "path": "usage.0.usageMetadata.candidatesTokenCount"},
        "tokens_total": {"type": "expression", "formula": "usage.0.usageMetadata.promptTokenCount + usage.0.usageMetadata.candidatesTokenCount"}
    }"#;

    let config = TokenMappingConfig::from_json(config_json).unwrap();
    let extractor = TokenFieldExtractor::new(config);

    let response = json!({
        "usage": [
            {
                "usageMetadata": {
                    "promptTokenCount": 15,
                    "candidatesTokenCount": 25,
                    "totalTokenCount": 40
                }
            }
        ]
    });

    // Debug: Check individual field extraction
    let prompt_result = extractor.extract_token_u32(&response, "tokens_prompt");
    let completion_result = extractor.extract_token_u32(&response, "tokens_completion");
    let total_result = extractor.extract_token_u32(&response, "tokens_total");

    println!("Prompt result: {prompt_result:?}");
    println!("Completion result: {completion_result:?}");
    println!("Total result: {total_result:?}");

    assert_eq!(
        extractor.extract_token_u32(&response, "tokens_prompt"),
        Some(15)
    );
    assert_eq!(
        extractor.extract_token_u32(&response, "tokens_completion"),
        Some(25)
    );
    assert_eq!(
        extractor.extract_token_u32(&response, "tokens_total"),
        Some(40) // 15 + 25
    );
}

#[test]
fn test_mixed_array_index_path() {
    let config_json = r#"{
        "model": {"type": "direct", "path": "data.0.model"},
        "content": {"type": "direct", "path": "choices.0.message.content"}
    }"#;

    let config = TokenMappingConfig::from_json(config_json).unwrap();
    let extractor = TokenFieldExtractor::new(config);

    let response = json!({
        "data": [
            {
                "model": "gpt-4"
            }
        ],
        "choices": [
            {
                "message": {
                    "content": "Hello, world!"
                }
            }
        ]
    });

    assert_eq!(
        extractor.extract_token_field(&response, "model"),
        Some(json!("gpt-4"))
    );
    assert_eq!(
        extractor.extract_token_field(&response, "content"),
        Some(json!("Hello, world!"))
    );
}

#[test]
fn test_array_index_out_of_bounds() {
    let config_json = r#"{
        "tokens_prompt": {"type": "direct", "path": "usage.1.usageMetadata.promptTokenCount"}
    }"#;

    let config = TokenMappingConfig::from_json(config_json).unwrap();
    let extractor = TokenFieldExtractor::new(config);

    let response = json!({
        "usage": [
            {
                "usageMetadata": {
                    "promptTokenCount": 15
                }
            }
        ]
    });

    // 数组索引越界，应该返回None
    assert_eq!(
        extractor.extract_token_u32(&response, "tokens_prompt"),
        None
    );
}

#[test]
fn test_direct_array_access() {
    let config_json = r#"{
        "first_model": {"type": "direct", "path": "0.model"},
        "second_choice": {"type": "direct", "path": "1.choices.1.content"}
    }"#;

    let config = TokenMappingConfig::from_json(config_json).unwrap();
    let extractor = TokenFieldExtractor::new(config);

    let response = json!([
        {
            "model": "claude-3.5-sonnet"
        },
        {
            "choices": [
                {"content": "First choice"},
                {"content": "Second choice"}
            ]
        }
    ]);

    assert_eq!(
        extractor.extract_token_field(&response, "first_model"),
        Some(json!("claude-3.5-sonnet"))
    );
    assert_eq!(
        extractor.extract_token_field(&response, "second_choice"),
        Some(json!("Second choice"))
    );
}
