//! # 状态码一致性测试
//!
//! 测试追踪系统中的状态码一致性验证功能

use crate::trace::immediate::{
    ImmediateProxyTracer, ImmediateTracerConfig, SimpleCompleteTraceParams,
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{Database, EntityTrait, Set};
use serial_test::serial;
use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_db() -> Arc<sea_orm::DatabaseConnection> {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to test database");
        Migrator::up(&db, None)
            .await
            .expect("Failed to run migrations");
        Arc::new(db)
    }

    async fn setup_test_user(db: &Arc<sea_orm::DatabaseConnection>) -> i32 {
        use entity::users;

        let user = users::ActiveModel {
            id: Set(1001),
            username: Set("test_user".to_string()),
            password_hash: Set("hashed_password".to_string()),
            email: Set("test@example.com".to_string()),
            salt: Set("salt".to_string()),
            is_admin: Set(false),
            is_active: Set(true),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        };

        let result = users::Entity::insert(user)
            .exec(db.as_ref())
            .await
            .expect("Failed to insert test user");

        result.last_insert_id
    }

    async fn setup_test_service_api(db: &Arc<sea_orm::DatabaseConnection>, user_id: i32) -> i32 {
        use entity::user_service_apis;

        let service_api = user_service_apis::ActiveModel {
            id: Set(2001),
            user_id: Set(user_id),
            provider_type_id: Set(1),
            api_key: Set("test-api-key".to_string()),
            name: Set(Some("Test API".to_string())),
            is_active: Set(true),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
            ..Default::default()
        };

        let result = user_service_apis::Entity::insert(service_api)
            .exec(db.as_ref())
            .await
            .expect("Failed to insert test service API");

        result.last_insert_id
    }

    #[tokio::test]
    #[serial]
    async fn test_status_code_consistency_validation_success() {
        let db = setup_test_db().await;
        let tracer = ImmediateProxyTracer::new(db.clone(), ImmediateTracerConfig::default());

        // 启动追踪
        let user_id = setup_test_user(&db).await;
        let service_api_id = setup_test_service_api(&db, user_id).await;

        crate::trace::immediate::StartTraceParams {
            request_id: "consistency-test-1".to_string(),
            user_service_api_id: service_api_id,
            user_id: Some(user_id),
            provider_type_id: Some(1),
            user_provider_key_id: None,
            method: "POST".to_string(),
            path: Some("/v1/chat/completions".to_string()),
            client_ip: Some("127.0.0.1".to_string()),
            user_agent: Some("test-client".to_string()),
        };

        // 测试一致的状态码（成功）
        let params = SimpleCompleteTraceParams {
            request_id: "consistency-test-1".to_string(),
            status_code: 200,
            is_success: true, // 与状态码一致
            tokens_prompt: Some(10),
            tokens_completion: Some(5),
            error_type: None,
            error_message: None,
        };

        // 这应该成功，不会产生警告
        let result = tracer.complete_trace(params).await;
        assert!(result.is_ok(), "一致的状态码应该成功完成追踪");
    }

    #[tokio::test]
    #[serial]
    async fn test_status_code_consistency_validation_mismatch() {
        let db = setup_test_db().await;
        let tracer = ImmediateProxyTracer::new(db.clone(), ImmediateTracerConfig::default());

        // 启动追踪
        let user_id = setup_test_user(&db).await;
        let service_api_id = setup_test_service_api(&db, user_id).await;

        crate::trace::immediate::StartTraceParams {
            request_id: "consistency-test-2".to_string(),
            user_service_api_id: service_api_id,
            user_id: Some(user_id),
            provider_type_id: Some(1),
            user_provider_key_id: None,
            method: "POST".to_string(),
            path: Some("/v1/chat/completions".to_string()),
            client_ip: Some("127.0.0.1".to_string()),
            user_agent: Some("test-client".to_string()),
        };

        // 测试不一致的状态码
        let params = SimpleCompleteTraceParams {
            request_id: "consistency-test-2".to_string(),
            status_code: 500,
            is_success: true, // 与状态码不一致！
            tokens_prompt: Some(10),
            tokens_completion: Some(5),
            error_type: Some("internal_error".to_string()),
            error_message: Some("Internal server error".to_string()),
        };

        // 这应该仍然成功，但会产生一致性警告日志
        let result = tracer.complete_trace(params).await;
        assert!(result.is_ok(), "不一致的状态码应该仍然能完成追踪，但会有警告");
    }

    #[tokio::test]
    #[serial]
    async fn test_connection_failure_status_codes() {
        let db = setup_test_db().await;
        let tracer = ImmediateProxyTracer::new(db.clone(), ImmediateTracerConfig::default());

        // 启动追踪
        let user_id = setup_test_user(&db).await;
        let service_api_id = setup_test_service_api(&db, user_id).await;

        crate::trace::immediate::StartTraceParams {
            request_id: "connection-failure-test".to_string(),
            user_service_api_id: service_api_id,
            user_id: Some(user_id),
            provider_type_id: Some(1),
            user_provider_key_id: None,
            method: "POST".to_string(),
            path: Some("/v1/chat/completions".to_string()),
            client_ip: Some("127.0.0.1".to_string()),
            user_agent: Some("test-client".to_string()),
        };

        // 测试502状态码（连接失败）
        let params_502 = SimpleCompleteTraceParams {
            request_id: "connection-failure-test".to_string(),
            status_code: 502,
            is_success: false,
            tokens_prompt: None,
            tokens_completion: None,
            error_type: Some("connection_failure".to_string()),
            error_message: Some("Connection to upstream failed".to_string()),
        };

        let result_502 = tracer.complete_trace(params_502).await;
        assert!(result_502.is_ok(), "502状态码应该成功完成追踪");

        // 测试504状态码（超时）
        let params_504 = SimpleCompleteTraceParams {
            request_id: "connection-failure-test-2".to_string(),
            status_code: 504,
            is_success: false,
            tokens_prompt: None,
            tokens_completion: None,
            error_type: Some("timeout".to_string()),
            error_message: Some("Gateway timeout".to_string()),
        };

        let result_504 = tracer.complete_trace(params_504).await;
        assert!(result_504.is_ok(), "504状态码应该成功完成追踪");
    }

    #[tokio::test]
    #[serial]
    async fn test_error_status_codes() {
        let db = setup_test_db().await;
        let tracer = ImmediateProxyTracer::new(db.clone(), ImmediateTracerConfig::default());

        // 启动追踪
        let user_id = setup_test_user(&db).await;
        let service_api_id = setup_test_service_api(&db, user_id).await;

        crate::trace::immediate::StartTraceParams {
            request_id: "error-status-test".to_string(),
            user_service_api_id: service_api_id,
            user_id: Some(user_id),
            provider_type_id: Some(1),
            user_provider_key_id: None,
            method: "POST".to_string(),
            path: Some("/v1/chat/completions".to_string()),
            client_ip: Some("127.0.0.1".to_string()),
            user_agent: Some("test-client".to_string()),
        };

        // 测试4xx客户端错误
        let params_4xx = SimpleCompleteTraceParams {
            request_id: "error-status-test".to_string(),
            status_code: 429,
            is_success: false,
            tokens_prompt: None,
            tokens_completion: None,
            error_type: Some("rate_limited".to_string()),
            error_message: Some("Too many requests".to_string()),
        };

        let result_4xx = tracer.complete_trace(params_4xx).await;
        assert!(result_4xx.is_ok(), "4xx错误状态码应该成功完成追踪");

        // 测试5xx服务器错误
        let params_5xx = SimpleCompleteTraceParams {
            request_id: "error-status-test-2".to_string(),
            status_code: 500,
            is_success: false,
            tokens_prompt: None,
            tokens_completion: None,
            error_type: Some("internal_error".to_string()),
            error_message: Some("Internal server error".to_string()),
        };

        let result_5xx = tracer.complete_trace(params_5xx).await;
        assert!(result_5xx.is_ok(), "5xx错误状态码应该成功完成追踪");
    }

    #[tokio::test]
    #[serial]
    async fn test_database_record_accuracy() {
        let db = setup_test_db().await;
        let tracer = ImmediateProxyTracer::new(db.clone(), ImmediateTracerConfig::default());

        // 启动追踪
        let user_id = setup_test_user(&db).await;
        let service_api_id = setup_test_service_api(&db, user_id).await;

        let request_id = "accuracy-test".to_string();

        crate::trace::immediate::StartTraceParams {
            request_id: request_id.clone(),
            user_service_api_id: service_api_id,
            user_id: Some(user_id),
            provider_type_id: Some(1),
            user_provider_key_id: Some(123),
            method: "POST".to_string(),
            path: Some("/v1/chat/completions".to_string()),
            client_ip: Some("127.0.0.1".to_string()),
            user_agent: Some("test-client".to_string()),
        };

        // 完成追踪
        let params = SimpleCompleteTraceParams {
            request_id: request_id.clone(),
            status_code: 502,
            is_success: false,
            tokens_prompt: Some(100),
            tokens_completion: Some(50),
            error_type: Some("connection_failure".to_string()),
            error_message: Some("Downstream connection closed".to_string()),
        };

        let result = tracer.complete_trace(params).await;
        assert!(result.is_ok(), "追踪应该成功完成");

        // 验证数据库记录的准确性
        use entity::proxy_tracing;

        let record = proxy_tracing::Entity::find()
            .filter(proxy_tracing::Column::RequestId.eq(&request_id))
            .one(db.as_ref())
            .await
            .expect("Failed to query trace record");

        assert!(record.is_some(), "应该找到追踪记录");
        let record = record.unwrap();

        // 验证关键字段
        assert_eq!(record.status_code, Some(502), "状态码应该正确记录");
        assert_eq!(record.is_success, false, "成功标志应该为false");
        assert_eq!(record.tokens_prompt, Some(100), "提示token应该正确记录");
        assert_eq!(record.tokens_completion, Some(50), "完成token应该正确记录");
        assert_eq!(record.error_type, Some("connection_failure".to_string()), "错误类型应该正确记录");
        assert!(record.error_message.as_ref().unwrap().contains("Downstream connection closed"),
                "错误消息应该正确记录");
        assert_eq!(record.user_provider_key_id, Some(123), "提供商密钥ID应该正确记录");
    }
}