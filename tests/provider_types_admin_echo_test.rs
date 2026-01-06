//! Provider Types（管理端）回显与更新行为测试
//!
//! 关注点：
//! 1. 管理端需要能够回显原始 `auth_configs_json`（包含 `client_secret`）
//! 2. JSON 字段不做限制：提交什么就存什么（更新时不做合并）

use api_proxy::management::middleware::AuthContext;
use api_proxy::management::services::provider_types;
use api_proxy::management::services::{ProviderTypesCrudService, UpdateProviderTypeRequest};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use std::sync::Arc;

use entity::provider_types as provider_types_entity;

async fn create_test_db() -> DatabaseConnection {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    db
}

const fn make_admin() -> AuthContext {
    AuthContext {
        user_id: 1,
        is_admin: true,
    }
}

#[tokio::test]
async fn test_admin_echo_and_update_stores_exact_payload() {
    let db = Arc::new(create_test_db().await);
    let service = ProviderTypesCrudService::new(db.clone());
    let now = chrono::Utc::now().naive_utc();

    let original_auth_configs = serde_json::json!({
        "client_id": "test_client_id",
        "client_secret": "test_client_secret",
        "authorize_url": "https://example.com/oauth/authorize",
        "token_url": "https://example.com/oauth/token",
        "redirect_uri": "https://example.com/callback",
        "scopes": "scope_a scope_b",
        "pkce_required": true,
        "extra_params": { "response_type": "code" }
    });

    let inserted = provider_types_entity::ActiveModel {
        name: Set("provider_types_admin_echo_test".to_string()),
        display_name: Set("Provider Types Admin Echo Test".to_string()),
        auth_type: Set("oauth".to_string()),
        base_url: Set("cloudcode-pa.googleapis.com".to_string()),
        is_active: Set(true),
        config_json: Set(None),
        token_mappings_json: Set(None),
        model_extraction_json: Set(None),
        auth_configs_json: Set(Some(original_auth_configs.to_string())),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db.as_ref())
    .await
    .unwrap();

    // 回显应包含完整配置（包含 `client_secret`）
    let item_before = provider_types::convert_model_to_dto(&inserted, chrono_tz::UTC).unwrap();
    let raw_before = item_before.auth_configs_json.as_ref().unwrap();
    assert_eq!(
        raw_before.get("client_secret").and_then(|v| v.as_str()),
        Some("test_client_secret")
    );
    assert_eq!(
        raw_before.get("client_id").and_then(|v| v.as_str()),
        Some("test_client_id")
    );

    // 更新时未提交 client_secret：应按提交内容覆盖存储（不会自动保留/合并）
    let update_auth_configs = serde_json::json!({
        "client_id": "test_client_id",
        "authorize_url": "https://example.com/oauth/authorize",
        "token_url": "https://example.com/oauth/token",
        "redirect_uri": "https://example.com/callback",
        "scopes": "scope_a scope_b",
        "pkce_required": true,
        "extra_params": { "response_type": "code" }
    });

    let update_request = UpdateProviderTypeRequest {
        auth_configs_json: Some(update_auth_configs),
        ..Default::default()
    };

    let updated = service
        .update(&make_admin(), inserted.id, &update_request)
        .await
        .unwrap();

    let stored = updated.auth_configs_json.as_ref().unwrap();
    let stored_val: serde_json::Value = serde_json::from_str(stored).unwrap();

    assert!(
        stored_val.get("client_secret").is_none(),
        "更新时未提交 client_secret，应按提交内容存储，不应自动保留"
    );

    // 回显应保持与数据库一致
    let item_admin = provider_types::convert_model_to_dto(&updated, chrono_tz::UTC).unwrap();
    let raw = item_admin.auth_configs_json.unwrap();
    assert_eq!(raw.get("client_secret"), None, "更新后回显应与数据库一致");
}
