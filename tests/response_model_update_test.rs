//! 响应时模型信息更新测试
//!
//! 测试在响应处理阶段从响应体中提取并更新模型信息的功能

use api_proxy::proxy::ProxyContext;
use api_proxy::statistics::service::StatisticsService;
use api_proxy::pricing::PricingCalculatorService;
use serde_json::json;

/// 创建测试用的统计服务
async fn create_test_statistics_service() -> StatisticsService {
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test database");
    let pricing_calculator = std::sync::Arc::new(PricingCalculatorService::new(std::sync::Arc::new(db)));
    StatisticsService::new(pricing_calculator)
}

/// 测试从响应体中提取模型信息 - 基本model字段
#[tokio::test]
async fn test_extract_model_from_response_basic() {
    let service = create_test_statistics_service().await;

    // 测试包含model字段的响应
    let response = json!({
        "model": "gpt-4-turbo",
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 20,
            "total_tokens": 30
        }
    });

    let model = service.extract_model_from_response_body(&response);
    assert_eq!(model, Some("gpt-4-turbo".to_string()));
}

/// 测试从响应体中提取模型信息 - 没有model字段
#[tokio::test]
async fn test_extract_model_from_response_no_model() {
    let service = create_test_statistics_service().await;

    // 测试不包含model字段的响应
    let response = json!({
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 20,
            "total_tokens": 30
        }
    });

    let model = service.extract_model_from_response_body(&response);
    assert_eq!(model, None);
}

/// 测试从响应体中提取模型信息 - 空的model字段
#[tokio::test]
async fn test_extract_model_from_response_empty_model() {
    let service = create_test_statistics_service().await;

    // 测试包含空model字段的响应
    let response = json!({
        "model": "",
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 20,
            "total_tokens": 30
        }
    });

    let model = service.extract_model_from_response_body(&response);
    assert_eq!(model, None);
}

/// 测试从响应体中提取模型信息 - 非字符串model字段
#[tokio::test]
async fn test_extract_model_from_response_non_string_model() {
    let service = create_test_statistics_service().await;

    // 测试model字段为非字符串类型
    let response = json!({
        "model": 123,
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 20,
            "total_tokens": 30
        }
    });

    let model = service.extract_model_from_response_body(&response);
    assert_eq!(model, None);
}

/// 测试响应时模型信息更新逻辑 - 响应中有更准确的模型信息
#[tokio::test]
async fn test_response_model_update_with_better_info() {
    let service = create_test_statistics_service().await;

    let mut ctx = ProxyContext::default();
    ctx.request_id = "test-response-update-123".to_string();

    // 设置请求时的模型信息（可能不够准确）
    ctx.requested_model = Some("gpt-4".to_string());

    // 模拟响应体包含更准确的模型信息
    let response_body = r#"
    {
        "model": "gpt-4-turbo-preview",
        "usage": {
            "prompt_tokens": 15,
            "completion_tokens": 25,
            "total_tokens": 40
        }
    }
    "#;

    ctx.response_details.body = Some(response_body.to_string());

    // 调用统计信息提取
    let stats = service.extract_detailed_stats_from_response(&mut ctx).await
        .expect("Failed to extract stats from response body");

    // 验证模型信息已更新为响应中的更准确信息
    assert_eq!(stats.model_name, Some("gpt-4-turbo-preview".to_string()));

    // 验证上下文中的模型信息也已更新
    assert_eq!(ctx.requested_model, Some("gpt-4-turbo-preview".to_string()));
}

/// 测试响应时模型信息更新逻辑 - 响应中没有模型信息
#[tokio::test]
async fn test_response_model_update_no_response_model() {
    let service = create_test_statistics_service().await;

    let mut ctx = ProxyContext::default();
    ctx.request_id = "test-no-response-model-456".to_string();

    // 设置请求时的模型信息
    ctx.requested_model = Some("claude-3-sonnet".to_string());

    // 模拟响应体不包含模型信息
    let response_body = r#"
    {
        "usage": {
            "prompt_tokens": 15,
            "completion_tokens": 25,
            "total_tokens": 40
        }
    }
    "#;

    ctx.response_details.body = Some(response_body.to_string());

    // 调用统计信息提取
    let stats = service.extract_detailed_stats_from_response(&mut ctx).await
        .expect("Failed to extract stats from response body");

    // 验证仍然使用请求时的模型信息
    assert_eq!(stats.model_name, Some("claude-3-sonnet".to_string()));
    assert_eq!(ctx.requested_model, Some("claude-3-sonnet".to_string()));
}

/// 测试响应时模型信息更新逻辑 - 响应中模型信息与请求时相同
#[tokio::test]
async fn test_response_model_update_same_model() {
    let service = create_test_statistics_service().await;

    let mut ctx = ProxyContext::default();
    ctx.request_id = "test-same-model-789".to_string();

    // 设置请求时的模型信息
    let requested_model = "gemini-pro".to_string();
    ctx.requested_model = Some(requested_model.clone());

    // 模拟响应体包含相同的模型信息
    let response_body = r#"
    {
        "model": "gemini-pro",
        "usage": {
            "prompt_tokens": 20,
            "completion_tokens": 30,
            "total_tokens": 50
        }
    }
    "#;

    ctx.response_details.body = Some(response_body.to_string());

    // 调用统计信息提取
    let stats = service.extract_detailed_stats_from_response(&mut ctx).await
        .expect("Failed to extract stats from response body");

    // 验证模型信息保持一致
    assert_eq!(stats.model_name, Some(requested_model.clone()));
    assert_eq!(ctx.requested_model, Some(requested_model.clone()));
}

/// 测试响应时模型信息更新逻辑 - 请求时没有模型信息
#[tokio::test]
async fn test_response_model_update_no_requested_model() {
    let service = create_test_statistics_service().await;

    let mut ctx = ProxyContext::default();
    ctx.request_id = "test-no-requested-model-012".to_string();

    // 请求时没有模型信息
    ctx.requested_model = None;

    // 模拟响应体包含模型信息
    let response_body = r#"
    {
        "model": "claude-3-haiku",
        "usage": {
            "prompt_tokens": 5,
            "completion_tokens": 10,
            "total_tokens": 15
        }
    }
    "#;

    ctx.response_details.body = Some(response_body.to_string());

    // 调用统计信息提取
    let stats = service.extract_detailed_stats_from_response(&mut ctx).await
        .expect("Failed to extract stats from response body");

    // 验证使用了响应中的模型信息
    assert_eq!(stats.model_name, Some("claude-3-haiku".to_string()));
    assert_eq!(ctx.requested_model, Some("claude-3-haiku".to_string()));
}

/// 测试响应时模型信息更新逻辑 - 无效JSON响应
#[tokio::test]
async fn test_response_model_update_invalid_json() {
    let service = create_test_statistics_service().await;

    let mut ctx = ProxyContext::default();
    ctx.request_id = "test-invalid-json-345".to_string();

    // 设置请求时的模型信息
    let requested_model = "gpt-4".to_string();
    ctx.requested_model = Some(requested_model.clone());

    // 模拟无效的JSON响应
    ctx.response_details.body = Some("invalid json response".to_string());

    // 调用统计信息提取，应该返回默认统计信息
    let stats = service.extract_detailed_stats_from_response(&mut ctx).await
        .expect("Failed to extract stats from response body");

    // 验证使用了请求时的模型信息
    assert_eq!(stats.model_name, Some(requested_model.clone()));
    assert_eq!(ctx.requested_model, Some(requested_model.clone()));
}

/// 测试响应时模型信息更新逻辑 - 没有响应体
#[tokio::test]
async fn test_response_model_update_no_response_body() {
    let service = create_test_statistics_service().await;

    let mut ctx = ProxyContext::default();
    ctx.request_id = "test-no-response-body-678".to_string();

    // 设置请求时的模型信息
    let requested_model = "gemini-pro".to_string();
    ctx.requested_model = Some(requested_model.clone());

    // 没有响应体
    ctx.response_details.body = None;

    // 调用统计信息提取，应该返回默认统计信息
    let stats = service.extract_detailed_stats_from_response(&mut ctx).await
        .expect("Failed to extract stats from response body");

    // 验证使用了请求时的模型信息
    assert_eq!(stats.model_name, Some(requested_model.clone()));
    assert_eq!(ctx.requested_model, Some(requested_model.clone()));
}