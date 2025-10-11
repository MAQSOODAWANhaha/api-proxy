//! 分层更新策略测试
//!
//! 测试智能分层更新策略的正确性和有效性

use api_proxy::proxy::tracing_service::TracingService;
use api_proxy::trace::immediate::{ImmediateProxyTracer, ImmediateTracerConfig};
use chrono::Utc;
use entity::{provider_types, proxy_tracing, user_provider_keys, user_service_apis, users};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ColumnTrait, Database, EntityTrait, PaginatorTrait, QueryFilter, Set};
use serial_test::serial;
use std::sync::Arc;

async fn setup_test_db() -> Arc<sea_orm::DatabaseConnection> {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test database");
    Migrator::up(&db, None)
        .await
        .expect("Failed to run migrations");

    // 创建测试用户
    let user = users::ActiveModel {
        id: Set(1000),
        username: Set("testuser_layered".to_string()),
        password_hash: Set("...".to_string()),
        email: Set("layered@test.com".to_string()),
        salt: Set("salt".to_string()),
        is_admin: Set(false),
        is_active: Set(true),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };
    users::Entity::insert(user)
        .exec(&db)
        .await
        .expect("Failed to create test user");

    // 创建测试提供商类型
    let provider_type = provider_types::ActiveModel {
        id: Set(100),
        name: Set("test_provider_layered".to_string()),
        display_name: Set("Test Provider Layered".to_string()),
        base_url: Set("https://api.test.com".to_string()),
        api_format: Set("openai".to_string()),
        is_active: Set(true),
        supported_auth_types: Set("[\"api_key\"]".to_string()),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };
    provider_types::Entity::insert(provider_type)
        .exec(&db)
        .await
        .expect("Failed to create test provider type");

    // 创建测试用户服务API
    let user_service_api = user_service_apis::ActiveModel {
        id: Set(1000),
        user_id: Set(1000),
        provider_type_id: Set(100),
        api_key: Set("test-api-key-layered".to_string()),
        name: Set(Some("Test API Layered".to_string())),
        is_active: Set(true),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };
    user_service_apis::Entity::insert(user_service_api)
        .exec(&db)
        .await
        .expect("Failed to create test user service API");

    // 创建测试用户提供商密钥
    let user_provider_key = user_provider_keys::ActiveModel {
        id: Set(1000),
        user_id: Set(1000),
        provider_type_id: Set(100),
        api_key: Set("test-provider-key-layered".to_string()),
        auth_type: Set("api_key".to_string()),
        name: Set("Test Provider Key Layered".to_string()),
        is_active: Set(true),
        health_status: Set("healthy".to_string()),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };
    user_provider_keys::Entity::insert(user_provider_key)
        .exec(&db)
        .await
        .expect("Failed to create test user provider key");

    Arc::new(db)
}

/// 测试第一层：立即更新模型信息
#[tokio::test]
#[serial]
async fn test_layer1_immediate_model_info_update() {
    let db = setup_test_db().await;
    let config = ImmediateTracerConfig::default();
    let tracer = ImmediateProxyTracer::new(db.clone(), config);
    let tracing_service = TracingService::new(Some(Arc::new(tracer.clone())));
    let request_id = "test_layer1_immediate_model_info_update_1";

    let start_params = api_proxy::trace::immediate::StartTraceParams {
        request_id: request_id.to_string(),
        user_service_api_id: 1000,
        user_id: Some(1000),
        provider_type_id: Some(100),
        user_provider_key_id: None,
        method: "POST".to_string(),
        path: Some("/v1/chat/completions".to_string()),
        client_ip: Some("127.0.0.1".to_string()),
        user_agent: Some("test-client/1.0".to_string()),
    };
    tracer
        .start_trace(start_params)
        .await
        .expect("Failed to start trace");

    // 验证基础记录已创建
    let count = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.eq(request_id))
        .count(&*db)
        .await
        .expect("Failed to count records");
    assert_eq!(count, 1, "Should have exactly one trace record");

    // 阶段1：立即更新模型信息
    tracing_service
        .update_trace_model_info(request_id, Some(100), Some("gpt-4".to_string()), Some(1000))
        .await
        .expect("Failed to update model info");

    // 验证模型信息已更新
    let record = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.eq(request_id))
        .one(&*db)
        .await
        .expect("Failed to find record")
        .expect("Record should exist");

    assert_eq!(record.provider_type_id, Some(100));
    assert_eq!(record.model_used, Some("gpt-4".to_string()));
    assert_eq!(record.user_provider_key_id, Some(1000));
    assert!(record.start_time.is_some());
    assert!(record.end_time.is_none()); // 还未完成
}

/// 测试第二层：批量更新统计信息
#[tokio::test]
#[serial]
async fn test_layer2_batch_statistics_update() {
    let db = setup_test_db().await;
    let config = ImmediateTracerConfig::default();
    let tracer = ImmediateProxyTracer::new(db.clone(), config);
    let tracing_service = TracingService::new(Some(Arc::new(tracer.clone())));
    let request_id = "test_layer2_batch_statistics_update_1";

    let start_params = api_proxy::trace::immediate::StartTraceParams {
        request_id: request_id.to_string(),
        user_service_api_id: 1000,
        user_id: Some(1000),
        provider_type_id: Some(100),
        user_provider_key_id: None,
        method: "POST".to_string(),
        path: Some("/v1/chat/completions".to_string()),
        client_ip: Some("127.0.0.1".to_string()),
        user_agent: Some("test-client/1.0".to_string()),
    };
    tracer
        .start_trace(start_params)
        .await
        .expect("Failed to start trace");

    // 阶段1：更新模型信息
    tracing_service
        .update_trace_model_info(
            request_id,
            Some(100),
            Some("claude-3".to_string()),
            Some(1000),
        )
        .await
        .expect("Failed to update model info");

    // 阶段2：批量更新统计信息
    tracing_service
        .complete_trace_success(
            request_id,
            200,
            Some(150),
            Some(75),
            Some(225),
            Some("claude-3".to_string()),
            None, // cache_create_tokens
            None, // cache_read_tokens
            None, // cost
            None, // cost_currency
        )
        .await
        .expect("Failed to complete trace success");

    // 验证所有信息都已正确更新
    let record = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.eq(request_id))
        .one(&*db)
        .await
        .expect("Failed to find record")
        .expect("Record should exist");

    assert_eq!(record.status_code, Some(200));
    assert!(record.is_success);
    assert_eq!(record.tokens_prompt, Some(150));
    assert_eq!(record.tokens_completion, Some(75));
    assert_eq!(record.tokens_total, Some(225));
    assert_eq!(record.provider_type_id, Some(100));
    assert_eq!(record.model_used, Some("claude-3".to_string()));
    assert_eq!(record.user_provider_key_id, Some(1000));
    assert!(record.end_time.is_some());
    assert!(record.duration_ms.is_some());
}

/// 测试错误情况的分层更新
#[tokio::test]
#[serial]
async fn test_layered_update_with_error() {
    let db = setup_test_db().await;
    let config = ImmediateTracerConfig::default();
    let tracer = ImmediateProxyTracer::new(db.clone(), config);
    let tracing_service = TracingService::new(Some(Arc::new(tracer.clone())));
    let request_id = "test_layered_update_with_error_1";

    let start_params = api_proxy::trace::immediate::StartTraceParams {
        request_id: request_id.to_string(),
        user_service_api_id: 1000,
        user_id: Some(1000),
        provider_type_id: Some(100),
        user_provider_key_id: None,
        method: "POST".to_string(),
        path: Some("/v1/chat/completions".to_string()),
        client_ip: Some("127.0.0.1".to_string()),
        user_agent: Some("test-client/1.0".to_string()),
    };
    tracer
        .start_trace(start_params)
        .await
        .expect("Failed to start trace");

    // 阶段1：更新模型信息
    tracing_service
        .update_trace_model_info(
            request_id,
            Some(100),
            Some("gemini-pro".to_string()),
            Some(1000),
        )
        .await
        .expect("Failed to update model info");

    // 阶段2：错误完成追踪
    tracing_service
        .complete_trace_failure(
            request_id,
            500,
            Some("upstream_error".to_string()),
            Some("Upstream service unavailable".to_string()),
        )
        .await
        .expect("Failed to complete trace failure");

    // 验证错误信息已正确更新，但模型信息已保存
    let record = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.eq(request_id))
        .one(&*db)
        .await
        .expect("Failed to find record")
        .expect("Record should exist");

    assert_eq!(record.status_code, Some(500));
    assert!(!record.is_success);
    assert_eq!(record.error_type, Some("upstream_error".to_string()));
    assert_eq!(
        record.error_message,
        Some("Upstream service unavailable".to_string())
    );
    // 关键：即使出错，模型信息也应该已经保存
    assert_eq!(record.provider_type_id, Some(100));
    assert_eq!(record.model_used, Some("gemini-pro".to_string()));
    assert_eq!(record.user_provider_key_id, Some(1000));
    assert!(record.end_time.is_some());
}

/// 测试分层更新的性能优势
#[tokio::test]
#[serial]
async fn test_layered_update_performance() {
    let db = setup_test_db().await;
    let config = ImmediateTracerConfig::default();
    let tracer = ImmediateProxyTracer::new(db.clone(), config);
    let tracing_service = TracingService::new(Some(Arc::new(tracer.clone())));
    let request_id = "test_layered_update_performance_1";
    let start_time = std::time::Instant::now();

    let start_params = api_proxy::trace::immediate::StartTraceParams {
        request_id: request_id.to_string(),
        user_service_api_id: 1000,
        user_id: Some(1000),
        provider_type_id: Some(100),
        user_provider_key_id: None,
        method: "POST".to_string(),
        path: Some("/v1/chat/completions".to_string()),
        client_ip: Some("127.0.0.1".to_string()),
        user_agent: Some("test-client/1.0".to_string()),
    };
    tracer
        .start_trace(start_params)
        .await
        .expect("Failed to start trace");

    // 阶段1：立即更新模型信息（模拟实时获取到信息）
    tracing_service
        .update_trace_model_info(
            request_id,
            Some(100),
            Some("gpt-3.5-turbo".to_string()),
            Some(1000),
        )
        .await
        .expect("Failed to update model info");

    // 模拟请求处理时间
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // 阶段2：批量更新统计信息
    tracing_service
        .complete_trace_success(
            request_id,
            200,
            Some(100),
            Some(50),
            Some(150),
            Some("gpt-3.5-turbo".to_string()),
            None, // cache_create_tokens
            None, // cache_read_tokens
            None, // cost
            None, // cost_currency
        )
        .await
        .expect("Failed to complete trace success");

    let total_time = start_time.elapsed();

    // 验证数据完整性
    let record = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.eq(request_id))
        .one(&*db)
        .await
        .expect("Failed to find record")
        .expect("Record should exist");

    assert_eq!(record.provider_type_id, Some(100));
    assert_eq!(record.model_used, Some("gpt-3.5-turbo".to_string()));
    assert_eq!(record.tokens_prompt, Some(100));
    assert_eq!(record.tokens_completion, Some(50));
    assert_eq!(record.tokens_total, Some(150));

    // 性能测试：总时间应该合理（主要用于回归测试）
    println!("分层更新总耗时: {total_time:?}");
    assert!(
        total_time < std::time::Duration::from_secs(1),
        "分层更新应该在合理时间内完成"
    );
}

/// 测试向后兼容性
#[tokio::test]
#[serial]
async fn test_backward_compatibility() {
    let db = setup_test_db().await;
    let config = ImmediateTracerConfig::default();
    let tracer = ImmediateProxyTracer::new(db.clone(), config);

    let request_id = "test_compatibility_13579";

    // 使用旧的API路径（直接调用complete_trace）
    let start_params = api_proxy::trace::immediate::StartTraceParams {
        request_id: request_id.to_string(),
        user_service_api_id: 1000,
        user_id: Some(1000),
        provider_type_id: Some(100),
        user_provider_key_id: None,
        method: "POST".to_string(),
        path: Some("/v1/chat/completions".to_string()),
        client_ip: Some("127.0.0.1".to_string()),
        user_agent: Some("test-client/1.0".to_string()),
    };
    tracer
        .start_trace(start_params)
        .await
        .expect("Failed to start trace");

    // 直接使用旧的complete_trace方法
    let complete_params = api_proxy::trace::immediate::SimpleCompleteTraceParams {
        request_id: request_id.to_string(),
        status_code: 200,
        is_success: true,
        tokens_prompt: Some(200),
        tokens_completion: Some(100),
        error_type: None,
        error_message: None,
    };
    tracer
        .complete_trace(complete_params)
        .await
        .expect("Failed to complete trace");

    // 验证仍然可以正常工作
    let record = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.eq(request_id))
        .one(&*db)
        .await
        .expect("Failed to find record")
        .expect("Record should exist");

    assert_eq!(record.status_code, Some(200));
    assert!(record.is_success);
    assert_eq!(record.tokens_prompt, Some(200));
    assert_eq!(record.tokens_completion, Some(100));
}
