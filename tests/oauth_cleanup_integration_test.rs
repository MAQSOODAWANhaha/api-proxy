use chrono::{Utc, Duration};
use serial_test::serial;
use sea_orm::{Database, DatabaseConnection, entity::*, query::*, Set};
use migration::{Migrator, MigratorTrait};

use api_proxy::auth::OAuthCleanupTask;
use api_proxy::config::OAuthCleanupConfig;
use entity::oauth_client_sessions;

/// 创建临时测试数据库
async fn create_test_db() -> DatabaseConnection {
    // 使用内存数据库避免权限问题
    let db_url = "sqlite::memory:";
    
    let db = Database::connect(db_url).await.unwrap();
    
    // 运行迁移
    Migrator::up(&db, None).await.unwrap();
    
    db
}

/// 创建测试用的 OAuth 会话记录
async fn create_test_session(
    db: &DatabaseConnection, 
    status: &str, 
    created_minutes_ago: i64
) -> oauth_client_sessions::Model {
    let created_at = Utc::now() - Duration::minutes(created_minutes_ago);
    
    let session = oauth_client_sessions::ActiveModel {
        session_id: Set(format!("test_session_{}", uuid::Uuid::new_v4())),
        user_id: Set(1),
        provider_name: Set("openai".to_string()),
        provider_type_id: Set(Some(1)),
        code_verifier: Set("test_verifier".to_string()),
        code_challenge: Set("test_challenge".to_string()),
        state: Set("test_state".to_string()),
        name: Set("Test Session".to_string()),
        description: Set(Some("Test description".to_string())),
        status: Set(status.to_string()),
        access_token: Set(None),
        refresh_token: Set(None),
        id_token: Set(None),
        token_type: Set(Some("Bearer".to_string())),
        expires_in: Set(None),
        expires_at: Set((created_at + Duration::hours(1)).naive_utc()),
        error_message: Set(None),
        created_at: Set(created_at.naive_utc()),
        updated_at: Set(created_at.naive_utc()),
        completed_at: Set(None),
        ..Default::default()
    };
    
    session.insert(db).await.unwrap()
}

#[tokio::test]
#[serial]
async fn test_oauth_cleanup_basic_functionality() {
    let db = create_test_db().await;
    
    // 创建测试数据：
    // - 2 个超过 30 分钟的 pending 会话（应该被清理）
    // - 1 个 20 分钟的 pending 会话（不应该被清理）
    // - 1 个已完成的会话（不应该被清理）
    create_test_session(&db, "pending", 35).await; // 应该被清理
    create_test_session(&db, "pending", 40).await; // 应该被清理
    create_test_session(&db, "pending", 20).await; // 不应该被清理
    create_test_session(&db, "completed", 50).await; // 不应该被清理
    
    let config = OAuthCleanupConfig {
        enabled: true,
        pending_expire_minutes: 30,
        cleanup_interval_seconds: 300,
        max_cleanup_records: 1000,
        expired_records_retention_days: 7,
    };
    
    let cleanup_task = OAuthCleanupTask::new(db.clone(), config);
    
    // 获取清理前的统计信息
    let stats_before = cleanup_task.get_cleanup_stats().await.unwrap();
    assert_eq!(stats_before.total_pending, 3);
    assert_eq!(stats_before.expired_pending, 2);
    
    // 执行清理
    cleanup_task.cleanup_expired_sessions().await.unwrap();
    
    // 获取清理后的统计信息
    let stats_after = cleanup_task.get_cleanup_stats().await.unwrap();
    assert_eq!(stats_after.total_pending, 1); // 只剩下 20 分钟的那个
    assert_eq!(stats_after.total_expired, 2); // 应该有 2 个被标记为 expired
    assert_eq!(stats_after.expired_pending, 0); // 不应该有过期的 pending 了
    
    // 验证数据库中的记录状态
    let expired_sessions = oauth_client_sessions::Entity::find()
        .filter(oauth_client_sessions::Column::Status.eq("expired"))
        .all(&db)
        .await
        .unwrap();
    
    assert_eq!(expired_sessions.len(), 2);
    
    // 验证错误消息被正确设置
    for session in expired_sessions {
        assert!(session.error_message.is_some());
        assert!(session.error_message.unwrap().contains("Session expired after 30 minutes"));
    }
}

#[tokio::test]
#[serial]
async fn test_cleanup_old_expired_sessions() {
    let db = create_test_db().await;
    
    // 创建一个 8 天前被标记为 expired 的会话（应该被删除）
    let old_expired_time = Utc::now() - Duration::days(8);
    let old_session = oauth_client_sessions::ActiveModel {
        session_id: Set("old_expired_session".to_string()),
        user_id: Set(1),
        provider_name: Set("openai".to_string()),
        provider_type_id: Set(Some(1)),
        code_verifier: Set("test_verifier".to_string()),
        code_challenge: Set("test_challenge".to_string()),
        state: Set("test_state".to_string()),
        name: Set("Old Session".to_string()),
        description: Set(Some("Old expired session".to_string())),
        status: Set("expired".to_string()),
        access_token: Set(None),
        refresh_token: Set(None),
        id_token: Set(None),
        token_type: Set(Some("Bearer".to_string())),
        expires_in: Set(None),
        expires_at: Set((old_expired_time + Duration::hours(1)).naive_utc()),
        error_message: Set(Some("Session expired".to_string())),
        created_at: Set(old_expired_time.naive_utc()),
        updated_at: Set(old_expired_time.naive_utc()),
        completed_at: Set(None),
        ..Default::default()
    };
    old_session.insert(&db).await.unwrap();
    
    // 创建一个 2 天前被标记为 expired 的会话（不应该被删除）
    let recent_expired_time = Utc::now() - Duration::days(2);
    let recent_session = oauth_client_sessions::ActiveModel {
        session_id: Set("recent_expired_session".to_string()),
        user_id: Set(1),
        provider_name: Set("openai".to_string()),
        provider_type_id: Set(Some(1)),
        code_verifier: Set("test_verifier".to_string()),
        code_challenge: Set("test_challenge".to_string()),
        state: Set("test_state".to_string()),
        name: Set("Recent Session".to_string()),
        description: Set(Some("Recent expired session".to_string())),
        status: Set("expired".to_string()),
        access_token: Set(None),
        refresh_token: Set(None),
        id_token: Set(None),
        token_type: Set(Some("Bearer".to_string())),
        expires_in: Set(None),
        expires_at: Set((recent_expired_time + Duration::hours(1)).naive_utc()),
        error_message: Set(Some("Session expired".to_string())),
        created_at: Set(recent_expired_time.naive_utc()),
        updated_at: Set(recent_expired_time.naive_utc()),
        completed_at: Set(None),
        ..Default::default()
    };
    recent_session.insert(&db).await.unwrap();
    
    let config = OAuthCleanupConfig {
        enabled: true,
        pending_expire_minutes: 30,
        cleanup_interval_seconds: 300,
        max_cleanup_records: 1000,
        expired_records_retention_days: 7,
    };
    
    let cleanup_task = OAuthCleanupTask::new(db.clone(), config);
    
    // 验证清理前有 2 个 expired 记录
    let expired_before = oauth_client_sessions::Entity::find()
        .filter(oauth_client_sessions::Column::Status.eq("expired"))
        .count(&db)
        .await
        .unwrap();
    assert_eq!(expired_before, 2);
    
    // 执行清理（这会同时清理过期的 pending 会话和老的 expired 记录）
    cleanup_task.cleanup_expired_sessions().await.unwrap();
    
    // 验证清理后只有 1 个 expired 记录（8天前的被删除了）
    let expired_after = oauth_client_sessions::Entity::find()
        .filter(oauth_client_sessions::Column::Status.eq("expired"))
        .count(&db)
        .await
        .unwrap();
    assert_eq!(expired_after, 1);
    
    // 验证剩下的是2天前的那个
    let remaining_session = oauth_client_sessions::Entity::find()
        .filter(oauth_client_sessions::Column::Status.eq("expired"))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(remaining_session.session_id, "recent_expired_session");
}

#[tokio::test]
#[serial]
async fn test_cleanup_with_max_records_limit() {
    let db = create_test_db().await;
    
    // 创建 5 个超过 30 分钟的 pending 会话
    for i in 0..5 {
        create_test_session(&db, "pending", 35 + i).await;
    }
    
    let config = OAuthCleanupConfig {
        enabled: true,
        pending_expire_minutes: 30,
        cleanup_interval_seconds: 300,
        max_cleanup_records: 3, // 限制每次最多清理 3 个
        expired_records_retention_days: 7,
    };
    
    let cleanup_task = OAuthCleanupTask::new(db.clone(), config);
    
    // 执行清理
    cleanup_task.cleanup_expired_sessions().await.unwrap();
    
    // 验证只有 3 个被清理（由于 limit 限制）
    let expired_count = oauth_client_sessions::Entity::find()
        .filter(oauth_client_sessions::Column::Status.eq("expired"))
        .count(&db)
        .await
        .unwrap();
    assert_eq!(expired_count, 3);
    
    let pending_count = oauth_client_sessions::Entity::find()
        .filter(oauth_client_sessions::Column::Status.eq("pending"))
        .count(&db)
        .await
        .unwrap();
    assert_eq!(pending_count, 2);
}