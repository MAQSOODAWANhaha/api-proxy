//! Provider Types CRUD 集成测试

use api_proxy::management::middleware::AuthContext;
use api_proxy::management::services::{
    CreateProviderTypeRequest, ProviderTypesCrudService, UpdateProviderTypeRequest,
};
use migration::{Migrator, MigratorTrait};
use sea_orm::Database;
use std::sync::Arc;

async fn setup_test_db() -> Arc<sea_orm::DatabaseConnection> {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect test db");
    Migrator::up(&db, None).await.expect("run migrations");
    Arc::new(db)
}

const fn admin() -> AuthContext {
    AuthContext {
        user_id: 1,
        is_admin: true,
    }
}

#[tokio::test]
async fn create_update_delete_provider_type() {
    let db = setup_test_db().await;
    let service = ProviderTypesCrudService::new(db);

    let created = service
        .create(
            &admin(),
            &CreateProviderTypeRequest {
                name: "test".to_string(),
                display_name: "Test Provider".to_string(),
                auth_type: "api_key".to_string(),
                base_url: "example.com".to_string(),
                is_active: Some(true),
                config_json: None,
                token_mappings_json: None,
                model_extraction_json: None,
                auth_configs_json: Some(serde_json::json!({})),
            },
        )
        .await
        .expect("create provider type");

    assert_eq!(created.name, "test");
    assert_eq!(created.auth_type, "api_key");

    // 同 (name, auth_type) 重复应失败
    let dup = service
        .create(
            &admin(),
            &CreateProviderTypeRequest {
                name: "test".to_string(),
                display_name: "Test Provider".to_string(),
                auth_type: "api_key".to_string(),
                base_url: "example.com".to_string(),
                is_active: Some(true),
                config_json: None,
                token_mappings_json: None,
                model_extraction_json: None,
                auth_configs_json: Some(serde_json::json!({})),
            },
        )
        .await;
    assert!(dup.is_err());

    // 同 name 不同 auth_type 允许
    let created_oauth = service
        .create(
            &admin(),
            &CreateProviderTypeRequest {
                name: "test".to_string(),
                display_name: "Test Provider".to_string(),
                auth_type: "oauth".to_string(),
                base_url: "oauth.example.com".to_string(),
                is_active: Some(true),
                config_json: None,
                token_mappings_json: None,
                model_extraction_json: None,
                auth_configs_json: Some(serde_json::json!({
                    "client_id":"x",
                    "redirect_uri":"https://example.com/callback",
                    "scopes":"s",
                    "pkce_required":true,
                    "authorize":{"url":"https://example.com/oauth/authorize","method":"GET","query":{}},
                    "exchange":{"url":"https://example.com/oauth/token","method":"POST","body":{}},
                    "refresh":{"url":"https://example.com/oauth/token","method":"POST","body":{}}
                })),
            },
        )
        .await
        .expect("create oauth row");
    assert_eq!(created_oauth.name, "test");
    assert_eq!(created_oauth.auth_type, "oauth");

    let updated = service
        .update(
            &admin(),
            created.id,
            &UpdateProviderTypeRequest {
                base_url: Some("changed.example.com".to_string()),
                ..Default::default()
            },
        )
        .await
        .expect("update provider type");
    assert_eq!(updated.base_url, "changed.example.com");

    service
        .delete(&admin(), created.id)
        .await
        .expect("delete provider type");
}
