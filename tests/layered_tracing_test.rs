//! `TraceManager` 分层更新测试
//!
//! 覆盖模型信息更新、成功/失败写入等关键路径，确保新的 Collect → Trace
//! 流程在数据库层面的表现符合预期。

use api_proxy::auth::api_key_usage_limit_service::ApiKeyUsageLimitService;
use api_proxy::cache::CacheManager;
use api_proxy::collect::types::{CollectedCost, CollectedMetrics, TokenUsageMetrics};
use api_proxy::proxy::ProxyContext;
use api_proxy::trace::{TraceManager, immediate::ImmediateProxyTracer};
use api_proxy::types::ProviderTypeId;
use chrono::Utc;
use entity::{provider_types, proxy_tracing, user_provider_keys, user_service_apis, users};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter, Set};
use serial_test::serial;
use std::sync::Arc;

async fn setup_test_db() -> Arc<sea_orm::DatabaseConnection> {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect test db");
    Migrator::up(&db, None).await.expect("run migrations");
    Arc::new(db)
}

async fn seed_user(db: &Arc<sea_orm::DatabaseConnection>) -> i32 {
    let user = users::ActiveModel {
        id: Set(2000),
        username: Set("trace_user".to_string()),
        password_hash: Set("hashed".to_string()),
        email: Set("trace@test.com".to_string()),
        salt: Set("salt".to_string()),
        is_admin: Set(false),
        is_active: Set(true),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };
    let result = users::Entity::insert(user)
        .exec(db.as_ref())
        .await
        .expect("insert user");
    result.last_insert_id
}

async fn seed_provider_type(db: &Arc<sea_orm::DatabaseConnection>) -> ProviderTypeId {
    let provider = provider_types::ActiveModel {
        id: Set(300),
        name: Set("trace_provider".to_string()),
        display_name: Set("Trace Provider".to_string()),
        base_url: Set("https://api.trace.test".to_string()),
        api_format: Set("openai".to_string()),
        is_active: Set(true),
        supported_auth_types: Set("[\"api_key\"]".to_string()),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };
    let result = provider_types::Entity::insert(provider)
        .exec(db.as_ref())
        .await
        .expect("insert provider");
    result.last_insert_id
}

async fn seed_service_api(
    db: &Arc<sea_orm::DatabaseConnection>,
    user_id: i32,
    provider_type_id: ProviderTypeId,
) -> i32 {
    let api = user_service_apis::ActiveModel {
        id: Set(4000),
        user_id: Set(user_id),
        provider_type_id: Set(provider_type_id),
        api_key: Set("trace-api-key".to_string()),
        name: Set(Some("Trace Service API".to_string())),
        is_active: Set(true),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };
    let result = user_service_apis::Entity::insert(api)
        .exec(db.as_ref())
        .await
        .expect("insert service api");
    result.last_insert_id
}

async fn seed_provider_key(
    db: &Arc<sea_orm::DatabaseConnection>,
    user_id: i32,
    provider_type_id: ProviderTypeId,
) -> i32 {
    let key = user_provider_keys::ActiveModel {
        id: Set(5000),
        user_id: Set(user_id),
        provider_type_id: Set(provider_type_id),
        api_key: Set("trace-provider-key".to_string()),
        auth_type: Set("api_key".to_string()),
        name: Set("Trace Key".to_string()),
        is_active: Set(true),
        health_status: Set("healthy".to_string()),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };
    let result = user_provider_keys::Entity::insert(key)
        .exec(db.as_ref())
        .await
        .expect("insert provider key");
    result.last_insert_id
}

fn build_trace_manager(db: Arc<sea_orm::DatabaseConnection>) -> Arc<TraceManager> {
    let tracer = Arc::new(ImmediateProxyTracer::new(db.clone()));
    let cache = Arc::new(CacheManager::memory_only());
    let rate_limiter = Arc::new(ApiKeyUsageLimitService::new(cache, db));
    Arc::new(TraceManager::new(Some(tracer), rate_limiter))
}

fn build_context(request_id: &str) -> ProxyContext {
    ProxyContext {
        request_id: request_id.to_string(),
        ..Default::default()
    }
}

#[tokio::test]
#[serial]
async fn test_update_model_info() {
    let db = setup_test_db().await;
    let user_id = seed_user(&db).await;
    let provider_type_id = seed_provider_type(&db).await;
    let service_api_id = seed_service_api(&db, user_id, provider_type_id).await;
    let provider_key_id = seed_provider_key(&db, user_id, provider_type_id).await;
    let trace_manager = build_trace_manager(db.clone());
    let request_id = "trace-update-model";

    trace_manager
        .start_trace(
            request_id,
            service_api_id,
            Some(user_id),
            Some(provider_type_id),
            None,
            "POST",
            Some("/v1/chat/completions".to_string()),
            Some("127.0.0.1".to_string()),
            Some("trace-client".to_string()),
        )
        .await
        .expect("start trace");

    trace_manager
        .update_model(
            request_id,
            Some(provider_type_id),
            Some("gpt-trace".to_string()),
            Some(provider_key_id),
        )
        .await;

    let record = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.eq(request_id))
        .one(db.as_ref())
        .await
        .expect("query trace")
        .expect("trace exists");

    assert_eq!(record.provider_type_id, Some(provider_type_id));
    assert_eq!(record.model_used, Some("gpt-trace".to_string()));
    assert_eq!(record.user_provider_key_id, Some(provider_key_id));
}

#[tokio::test]
#[serial]
async fn test_record_success_updates_trace() {
    let db = setup_test_db().await;
    let user_id = seed_user(&db).await;
    let provider_type_id = seed_provider_type(&db).await;
    let service_api_id = seed_service_api(&db, user_id, provider_type_id).await;
    seed_provider_key(&db, user_id, provider_type_id).await;

    let trace_manager = build_trace_manager(db.clone());
    let request_id = "trace-success";

    trace_manager
        .start_trace(
            request_id,
            service_api_id,
            Some(user_id),
            Some(provider_type_id),
            None,
            "POST",
            Some("/v1/chat/completions".to_string()),
            Some("127.0.0.1".to_string()),
            Some("trace-client".to_string()),
        )
        .await
        .expect("start trace");

    let mut ctx = build_context(request_id);
    ctx.user_service_api = user_service_apis::Entity::find_by_id(service_api_id)
        .one(db.as_ref())
        .await
        .expect("fetch service api");

    let metrics = CollectedMetrics {
        request_id: request_id.to_string(),
        user_id: Some(user_id),
        user_service_api_id: Some(service_api_id),
        provider_type_id: Some(provider_type_id),
        model: Some("gpt-success".to_string()),
        usage: TokenUsageMetrics {
            prompt_tokens: Some(120),
            completion_tokens: Some(60),
            total_tokens: Some(180),
            cache_create_tokens: Some(0),
            cache_read_tokens: Some(0),
        },
        cost: CollectedCost {
            value: Some(2.5),
            currency: Some("USD".to_string()),
        },
        duration_ms: 345,
        status_code: 200,
    };

    trace_manager
        .record_success(&metrics, &ctx)
        .await
        .expect("record success");

    let record = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.eq(request_id))
        .one(db.as_ref())
        .await
        .expect("query trace")
        .expect("trace exists");

    assert_eq!(record.status_code, Some(200));
    assert!(record.is_success);
    assert_eq!(record.tokens_prompt, Some(120));
    assert_eq!(record.tokens_completion, Some(60));
    assert_eq!(record.tokens_total, Some(180));
    assert_eq!(record.cost, Some(2.5));
    assert_eq!(record.cost_currency, Some("USD".to_string()));
    assert!(record.end_time.is_some());
    assert!(record.duration_ms.is_some());
}

#[tokio::test]
#[serial]
async fn test_record_failure_updates_trace() {
    let db = setup_test_db().await;
    let user_id = seed_user(&db).await;
    let provider_type_id = seed_provider_type(&db).await;
    let service_api_id = seed_service_api(&db, user_id, provider_type_id).await;
    let trace_manager = build_trace_manager(db.clone());
    let request_id = "trace-failure";

    trace_manager
        .start_trace(
            request_id,
            service_api_id,
            Some(user_id),
            Some(provider_type_id),
            None,
            "POST",
            Some("/v1/chat/completions".to_string()),
            Some("127.0.0.1".to_string()),
            Some("trace-client".to_string()),
        )
        .await
        .expect("start trace");

    let ctx = build_context(request_id);

    trace_manager.record_failure(None, 502, None, &ctx).await;

    let record = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.eq(request_id))
        .one(db.as_ref())
        .await
        .expect("query trace")
        .expect("trace exists");

    assert_eq!(record.status_code, Some(502));
    assert!(!record.is_success);
    assert!(record.error_type.is_some());
    assert!(record.end_time.is_some());
}
