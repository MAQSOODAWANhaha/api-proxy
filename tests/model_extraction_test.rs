//! 测试模型提取功能
//!
//! 测试新的多格式模型提取功能，验证对各种AI API响应格式的支持

use api_proxy::statistics::service::StatisticsService;
use api_proxy::pricing::PricingCalculatorService;
use serde_json::json;
use std::sync::Arc;

/// 创建测试用的统计服务
async fn create_test_statistics_service() -> StatisticsService {
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test database");
    let pricing_calculator = Arc::new(PricingCalculatorService::new(Arc::new(db)));
    StatisticsService::new(pricing_calculator)
}

#[tokio::test]
async fn test_extract_model_openai_format() {
    let stats_service = create_test_statistics_service().await;

    // OpenAI格式响应
    let response = json!({
        "id": "chatcmpl-B9MHDbslfkBeAs8l4bebGdFOJ6PeG",
        "object": "chat.completion",
        "created": 1741570283,
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

    let model = stats_service.extract_model_from_response_body(&response);
    assert_eq!(model, Some("gpt-4o-2024-08-06".to_string()));
}

#[tokio::test]
async fn test_extract_model_data_array_format() {
    let stats_service = create_test_statistics_service().await;

    // data.0.model格式
    let response = json!({
        "data": [
            {
                "id": "req_123",
                "model": "claude-3.5-sonnet",
                "object": "model"
            }
        ]
    });

    let model = stats_service.extract_model_from_response_body(&response);
    assert_eq!(model, Some("claude-3.5-sonnet".to_string()));
}

#[tokio::test]
async fn test_extract_model_choices_array_format() {
    let stats_service = create_test_statistics_service().await;

    // choices.0.model格式
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

    let model = stats_service.extract_model_from_response_body(&response);
    assert_eq!(model, Some("gemini-2.5-flash".to_string()));
}

#[tokio::test]
async fn test_extract_model_nested_format() {
    let stats_service = create_test_statistics_service().await;

    // response.model格式
    let response = json!({
        "response": {
            "model": "custom-model-v1",
            "content": "Response content"
        }
    });

    let model = stats_service.extract_model_from_response_body(&response);
    assert_eq!(model, Some("custom-model-v1".to_string()));
}

#[tokio::test]
async fn test_extract_model_model_name_format() {
    let stats_service = create_test_statistics_service().await;

    // modelName格式
    let response = json!({
        "modelName": "llama-3.1-70b",
        "parameters": {
            "temperature": 0.7
        }
    });

    let model = stats_service.extract_model_from_response_body(&response);
    assert_eq!(model, Some("llama-3.1-70b".to_string()));
}

#[tokio::test]
async fn test_extract_model_priority_order() {
    let stats_service = create_test_statistics_service().await;

    // 测试优先级：model > modelName > data.0.model
    let response = json!({
        "model": "gpt-4o",
        "modelName": "claude-3.5-sonnet",
        "data": [
            {
                "model": "gemini-2.5-flash"
            }
        ]
    });

    // 应该返回优先级最高的model字段
    let model = stats_service.extract_model_from_response_body(&response);
    assert_eq!(model, Some("gpt-4o".to_string()));
}

#[tokio::test]
async fn test_extract_model_array_out_of_bounds() {
    let stats_service = create_test_statistics_service().await;

    // 数组越界情况 - data只有1个元素，但访问索引1
    let response = json!({
        "data": [
            {
                "model": "first-model"
            }
        ]
    });

    let model = stats_service.extract_model_from_response_body(&response);
    // 应该找到 data.0.model = "first-model"，而不是None
    assert_eq!(model, Some("first-model".to_string()));
}

#[tokio::test]
async fn test_extract_model_real_out_of_bounds() {
    let stats_service = create_test_statistics_service().await;

    // 真正的数组越界情况 - 测试访问不存在的索引
    let response = json!({
        "data": [
            {
                "id": "req_123",
                "object": "model"
            }
        ]
    });

    let model = stats_service.extract_model_from_response_body(&response);
    // data.0.model 不存在，应该返回None
    assert_eq!(model, None);
}

#[tokio::test]
async fn test_extract_model_empty_value() {
    let stats_service = create_test_statistics_service().await;

    // 空值情况
    let response = json!({
        "model": "",
        "choices": [
            {
                "model": "   "
            }
        ]
    });

    let model = stats_service.extract_model_from_response_body(&response);
    // 空值应该被过滤掉
    assert_eq!(model, None);
}

#[tokio::test]
async fn test_extract_model_non_string_value() {
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test database");
    let pricing_service = Arc::new(PricingCalculatorService::new(Arc::new(db)));
    let stats_service = StatisticsService::new(pricing_service);

    // 非字符串值
    let response = json!({
        "model": 123,
        "modelName": true,
        "data": [
            {
                "model": ["array", "value"]
            }
        ]
    });

    let model = stats_service.extract_model_from_response_body(&response);
    // 非字符串值应该被过滤掉
    assert_eq!(model, None);
}

#[tokio::test]
async fn test_extract_model_no_valid_paths() {
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test database");
    let pricing_service = Arc::new(PricingCalculatorService::new(Arc::new(db)));
    let stats_service = StatisticsService::new(pricing_service);

    // 没有有效路径
    let response = json!({
        "id": "test",
        "content": "Hello world",
        "metadata": {
            "version": "1.0"
        }
    });

    let model = stats_service.extract_model_from_response_body(&response);
    // 没有有效的模型字段
    assert_eq!(model, None);
}

#[tokio::test]
async fn test_extract_model_complex_nested_structure() {
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test database");
    let pricing_service = Arc::new(PricingCalculatorService::new(Arc::new(db)));
    let stats_service = StatisticsService::new(pricing_service);

    // 复杂嵌套结构
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

    let model = stats_service.extract_model_from_response_body(&response);
    // 应该找到 candidates.0.model
    assert_eq!(model, Some("gemini-1.5-pro".to_string()));
}