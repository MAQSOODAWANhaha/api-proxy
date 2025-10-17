//! 模型字段提取测试
//!
//! 验证 `collect::usage_model::extract_model_from_json` 在不同响应格式下的行为。

use api_proxy::collect::usage_model::extract_model_from_json;
use serde_json::json;

#[test]
fn test_extract_model_openai_format() {
    let response = json!({
        "id": "chatcmpl-B9MHDbslfkBeAs8l4bebGdFOJ6PeG",
        "object": "chat.completion",
        "created": 1_741_570_283,
        "model": "gpt-4o-2024-08-06",
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello, world!"
                }
            }
        ],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    });

    let model = extract_model_from_json(&response);
    assert_eq!(model, Some("gpt-4o-2024-08-06".to_string()));
}

#[test]
fn test_extract_model_data_array_format() {
    let response = json!({
        "data": [
            {
                "id": "req_123",
                "model": "claude-3.5-sonnet",
                "object": "model"
            }
        ]
    });

    let model = extract_model_from_json(&response);
    assert_eq!(model, Some("claude-3.5-sonnet".to_string()));
}

#[test]
fn test_extract_model_choices_array_format() {
    let response = json!({
        "choices": [
            {
                "index": 0,
                "model": "gemini-2.5-flash",
                "message": {
                    "role": "assistant",
                    "content": "Hello!"
                }
            }
        ]
    });

    let model = extract_model_from_json(&response);
    assert_eq!(model, Some("gemini-2.5-flash".to_string()));
}

#[test]
fn test_extract_model_nested_format() {
    let response = json!({
        "response": {
            "model": "custom-model-v1",
            "content": "Response content"
        }
    });

    let model = extract_model_from_json(&response);
    assert_eq!(model, Some("custom-model-v1".to_string()));
}

#[test]
fn test_extract_model_model_name_format() {
    let response = json!({
        "modelName": "llama-3.1-70b",
        "parameters": {
            "temperature": 0.7
        }
    });

    let model = extract_model_from_json(&response);
    assert_eq!(model, Some("llama-3.1-70b".to_string()));
}

#[test]
fn test_extract_model_priority_order() {
    let response = json!({
        "model": "gpt-4o",
        "modelName": "claude-3.5-sonnet",
        "data": [
            {
                "model": "gemini-2.5-flash"
            }
        ]
    });

    let model = extract_model_from_json(&response);
    assert_eq!(model, Some("gpt-4o".to_string()));
}

#[test]
fn test_extract_model_array_out_of_bounds() {
    let response = json!({
        "data": [
            {
                "model": "first-model"
            }
        ]
    });

    let model = extract_model_from_json(&response);
    assert_eq!(model, Some("first-model".to_string()));
}

#[test]
fn test_extract_model_real_out_of_bounds() {
    let response = json!({
        "data": [
            {
                "id": "req_123",
                "object": "model"
            }
        ]
    });

    let model = extract_model_from_json(&response);
    assert_eq!(model, None);
}

#[test]
fn test_extract_model_empty_value() {
    let response = json!({
        "model": "",
        "choices": [
            {
                "model": "   "
            }
        ]
    });

    let model = extract_model_from_json(&response);
    assert_eq!(model, None);
}

#[test]
fn test_extract_model_non_string_value() {
    let response = json!({
        "model": 123,
        "modelName": true,
        "data": [
            {
                "model": ["array", "value"]
            }
        ]
    });

    let model = extract_model_from_json(&response);
    assert_eq!(model, None);
}

#[test]
fn test_extract_model_no_valid_paths() {
    let response = json!({
        "id": "test",
        "content": "Hello world",
        "metadata": {
            "version": "1.0"
        }
    });

    let model = extract_model_from_json(&response);
    assert_eq!(model, None);
}

#[test]
fn test_extract_model_complex_nested_structure() {
    let response = json!({
        "candidates": [
            {
                "content": {
                    "parts": [
                        {
                            "text": "Hello!"
                        }
                    ]
                },
                "model": "gemini-1.5-pro"
            }
        ],
        "usageMetadata": {
            "promptTokenCount": 10,
            "candidatesTokenCount": 5
        }
    });

    let model = extract_model_from_json(&response);
    assert_eq!(model, Some("gemini-1.5-pro".to_string()));
}
