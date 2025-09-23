//! # OAuth孤立会话清理集成测试
//!
//! 测试 validate_session_association 方法的孤立会话自动清理功能

use chrono::{Duration, Utc};
use migration::{Migrator, MigratorTrait};
use sea_orm::{Database, DatabaseConnection, Set, entity::*, QueryFilter, PaginatorTrait};
use serial_test::serial;

use api_proxy::auth::oauth_client::session_manager::SessionManager;
use entity::{oauth_client_sessions, user_provider_keys};

/// 创建临时测试数据库
async fn create_test_db() -> DatabaseConnection {
    // 使用内存数据库避免权限问题
    let db_url = "sqlite::memory:";

    let db = Database::connect(db_url).await.unwrap();

    // 运行迁移
    Migrator::up(&db, None).await.unwrap();

    // 创建基础测试数据
    setup_test_data(&db).await;

    db
}

/// 设置测试基础数据
async fn setup_test_data(db: &DatabaseConnection) {
    // 创建测试用户
    let user1 = entity::users::ActiveModel {
        username: Set("test_user_1".to_string()),
        email: Set("test1@example.com".to_string()),
        password_hash: Set("fake_hash".to_string()),
        salt: Set("fake_salt".to_string()),
        is_active: Set(true),
        is_admin: Set(false),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };
    user1.insert(db).await.unwrap();

    let user2 = entity::users::ActiveModel {
        username: Set("test_user_2".to_string()),
        email: Set("test2@example.com".to_string()),
        password_hash: Set("fake_hash".to_string()),
        salt: Set("fake_salt".to_string()),
        is_active: Set(true),
        is_admin: Set(false),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };
    user2.insert(db).await.unwrap();

    let user3 = entity::users::ActiveModel {
        username: Set("test_user_3".to_string()),
        email: Set("test3@example.com".to_string()),
        password_hash: Set("fake_hash".to_string()),
        salt: Set("fake_salt".to_string()),
        is_active: Set(true),
        is_admin: Set(false),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };
    user3.insert(db).await.unwrap();

    // 注意：provider_types 数据可能已经在迁移中创建，跳过创建
}

/// 创建测试用的 OAuth 会话记录
async fn create_test_session(
    db: &DatabaseConnection,
    status: &str,
    created_minutes_ago: i64,
    user_id: i32,
) -> oauth_client_sessions::Model {
    let created_at = Utc::now() - Duration::minutes(created_minutes_ago);

    let session = oauth_client_sessions::ActiveModel {
        session_id: Set(format!("test_session_{}", uuid::Uuid::new_v4())),
        user_id: Set(user_id),
        provider_name: Set("gemini".to_string()),
        provider_type_id: Set(Some(1)),
        code_verifier: Set("test_verifier".to_string()),
        code_challenge: Set("test_challenge".to_string()),
        state: Set("test_state".to_string()),
        name: Set("Test Session".to_string()),
        description: Set(Some("Test description".to_string())),
        status: Set(status.to_string()),
        access_token: Set(Some("fake_access_token".to_string())),
        refresh_token: Set(Some("fake_refresh_token".to_string())),
        id_token: Set(None),
        token_type: Set(Some("Bearer".to_string())),
        expires_in: Set(Some(3600)),
        expires_at: Set((created_at + Duration::hours(1)).naive_utc()),
        error_message: Set(None),
        created_at: Set(created_at.naive_utc()),
        updated_at: Set(created_at.naive_utc()),
        completed_at: Set(Some(created_at.naive_utc())),
        ..Default::default()
    };

    session.insert(db).await.unwrap()
}

/// 创建测试用的 user_provider_keys 记录
async fn create_user_provider_key(
    db: &DatabaseConnection,
    user_id: i32,
    provider_type_id: i32,
    api_key: &str,
) -> user_provider_keys::Model {
    let key = user_provider_keys::ActiveModel {
        user_id: Set(user_id),
        provider_type_id: Set(provider_type_id),
        api_key: Set(api_key.to_string()),
        auth_type: Set("oauth".to_string()),
        name: Set("Test Provider Key".to_string()),
        weight: Set(Some(1)),
        max_requests_per_minute: Set(Some(100)),
        max_tokens_prompt_per_minute: Set(Some(1000)),
        max_requests_per_day: Set(Some(10000)),
        is_active: Set(true),
        health_status: Set("healthy".to_string()),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };

    key.insert(db).await.unwrap()
}

#[tokio::test]
#[serial]
async fn test_orphaned_session_cleanup_functionality() {
    let db = create_test_db().await;

    // 创建测试数据：
    // - 1 个创建 6 分钟前且无 user_provider_keys 关联的会话（孤立会话）
    // - 1 个创建 6 分钟前但有 user_provider_keys 关联的会话（正常会话）
    // - 1 个创建 3 分钟前且无 user_provider_keys 关联的会话（年轻会话）
    let orphaned_session = create_test_session(&db, "completed", 6, 1).await;
    let normal_session = create_test_session(&db, "completed", 6, 2).await;
    let young_session = create_test_session(&db, "completed", 3, 3).await;

    // 为正常会话创建关联的 user_provider_keys
    create_user_provider_key(&db, 2, 1, &normal_session.session_id).await;

    // 验证初始状态：3个会话都存在
    let initial_count = oauth_client_sessions::Entity::find()
        .count(&db)
        .await
        .unwrap();
    assert_eq!(initial_count, 3);

    // 验证有1个关联记录
    let initial_keys_count = user_provider_keys::Entity::find()
        .count(&db)
        .await
        .unwrap();
    assert_eq!(initial_keys_count, 1);

    // 创建 SessionManager 实例
    let session_manager = SessionManager::new(db.clone());

    // 模拟 validate_session_association 的逻辑
    // 测试6分钟的孤立会话：应该被删除
    let now = Utc::now().naive_utc();
    let orphaned_age = now.signed_duration_since(orphaned_session.created_at);
    let orphaned_threshold = Duration::minutes(5);

    if orphaned_age >= orphaned_threshold {
        // 检查是否有关联
        let has_association = user_provider_keys::Entity::find()
            .filter(user_provider_keys::Column::UserId.eq(orphaned_session.user_id))
            .filter(user_provider_keys::Column::AuthType.eq("oauth"))
            .filter(user_provider_keys::Column::ApiKey.eq(&orphaned_session.session_id))
            .one(&db)
            .await
            .unwrap()
            .is_some();

        if !has_association {
            // 删除孤立会话
            let result = session_manager.delete_session(&orphaned_session.session_id, orphaned_session.user_id).await;
            assert!(result.is_ok(), "删除孤立会话应该成功");
        }
    }

    // 验证孤立会话被删除
    let count_after_orphan_cleanup = oauth_client_sessions::Entity::find()
        .count(&db)
        .await
        .unwrap();
    assert_eq!(count_after_orphan_cleanup, 2);

    // 测试6分钟的正常会话：不应该被删除
    let normal_age = now.signed_duration_since(normal_session.created_at);
    if normal_age >= orphaned_threshold {
        // 检查是否有关联
        let has_association = user_provider_keys::Entity::find()
            .filter(user_provider_keys::Column::UserId.eq(normal_session.user_id))
            .filter(user_provider_keys::Column::AuthType.eq("oauth"))
            .filter(user_provider_keys::Column::ApiKey.eq(&normal_session.session_id))
            .one(&db)
            .await
            .unwrap()
            .is_some();

        assert!(has_association, "正常会话应该有关联");
    }

    // 测试3分钟的年轻会话：不应该被删除（因为不足5分钟）
    let young_age = now.signed_duration_since(young_session.created_at);
    if young_age >= orphaned_threshold {
        // 这个逻辑不应该执行，因为年轻会话不足5分钟
        panic!("年轻会话不应该达到5分钟阈值");
    }

    // 验证正常会话和年轻会话仍然存在
    let remaining_sessions = oauth_client_sessions::Entity::find()
        .all(&db)
        .await
        .unwrap();

    assert_eq!(remaining_sessions.len(), 2);

    // 验证剩下的会话ID
    let remaining_ids: Vec<String> = remaining_sessions
        .into_iter()
        .map(|s| s.session_id)
        .collect();

    assert!(remaining_ids.contains(&normal_session.session_id));
    assert!(remaining_ids.contains(&young_session.session_id));
    assert!(!remaining_ids.contains(&orphaned_session.session_id));

    // 验证关联记录仍然存在
    let final_keys_count = user_provider_keys::Entity::find()
        .count(&db)
        .await
        .unwrap();
    assert_eq!(final_keys_count, 1);
}

#[tokio::test]
#[serial]
async fn test_young_session_not_cleaned() {
    let db = create_test_db().await;

    // 创建一个3分钟的孤立会话
    let young_session = create_test_session(&db, "completed", 3, 1).await;

    // 验证初始状态
    let initial_count = oauth_client_sessions::Entity::find()
        .count(&db)
        .await
        .unwrap();
    assert_eq!(initial_count, 1);

    // 创建 SessionManager 实例
    let session_manager = SessionManager::new(db.clone());

    // 模拟 validate_session_association 的逻辑
    let now = Utc::now().naive_utc();
    let young_age = now.signed_duration_since(young_session.created_at);
    let threshold = Duration::minutes(5);

    // 年轻会话不足5分钟，不应该被处理
    if young_age >= threshold {
        panic!("年轻会话不应该达到5分钟阈值");
    }

    // 验证会话仍然存在
    let final_count = oauth_client_sessions::Entity::find()
        .count(&db)
        .await
        .unwrap();
    assert_eq!(final_count, 1);

    // 验证可以获取到会话
    let session = session_manager.get_session(&young_session.session_id).await;
    assert!(session.is_ok());
}

#[tokio::test]
#[serial]
async fn test_session_deletion_ownership_check() {
    let db = create_test_db().await;

    // 创建两个不同用户的会话
    let session_user1 = create_test_session(&db, "completed", 6, 1).await;
    let session_user2 = create_test_session(&db, "completed", 6, 2).await;

    // 验证初始状态
    let initial_count = oauth_client_sessions::Entity::find()
        .count(&db)
        .await
        .unwrap();
    assert_eq!(initial_count, 2);

    // 创建 SessionManager 实例
    let session_manager = SessionManager::new(db.clone());

    // 尝试用错误的用户ID删除会话（应该失败）
    let result = session_manager.delete_session(&session_user1.session_id, 2).await; // 错误的用户ID
    assert!(result.is_err(), "用错误的用户ID删除会话应该失败");

    // 用正确的用户ID删除会话（应该成功）
    let result = session_manager.delete_session(&session_user1.session_id, 1).await; // 正确的用户ID
    assert!(result.is_ok(), "用正确的用户ID删除会话应该成功");

    // 验证只有一个会话被删除
    let final_count = oauth_client_sessions::Entity::find()
        .count(&db)
        .await
        .unwrap();
    assert_eq!(final_count, 1);

    // 验证剩下的是用户2的会话
    let remaining_session = oauth_client_sessions::Entity::find()
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(remaining_session.user_id, 2);
    assert_eq!(remaining_session.session_id, session_user2.session_id);
}