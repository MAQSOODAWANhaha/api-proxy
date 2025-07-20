//! # 集成测试
//!
//! 测试数据库迁移和实体定义的集成

use api_proxy::database::{init_database, run_migrations};
use entity::{provider_types, users};
use sea_orm::{EntityTrait, Set};
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_database_migration_and_entities() {
    // 创建临时数据库文件
    let temp_db = NamedTempFile::new().unwrap();
    let db_url = format!("sqlite:{}", temp_db.path().display());
    
    // 初始化数据库连接
    let db = init_database(&db_url).await.expect("数据库连接失败");
    
    // 运行迁移
    run_migrations(&db).await.expect("数据库迁移失败");
    
    // 测试查询初始化数据
    let provider_types = provider_types::Entity::find()
        .all(&db)
        .await
        .expect("查询 provider_types 失败");
        
    assert_eq!(provider_types.len(), 3);
    assert_eq!(provider_types[0].name, "openai");
    assert_eq!(provider_types[1].name, "gemini");
    assert_eq!(provider_types[2].name, "claude");
    
    // 测试插入用户数据
    let new_user = users::ActiveModel {
        username: Set("test_user".to_string()),
        email: Set("test@example.com".to_string()),
        password_hash: Set("hash123".to_string()),
        salt: Set("salt123".to_string()),
        is_active: Set(true),
        is_admin: Set(false),
        ..Default::default()
    };
    
    let user = users::Entity::insert(new_user)
        .exec(&db)
        .await
        .expect("插入用户失败");
        
    assert_eq!(user.last_insert_id, 1);
    
    // 测试查询用户数据
    let created_user = users::Entity::find_by_id(1)
        .one(&db)
        .await
        .expect("查询用户失败")
        .expect("用户不存在");
        
    assert_eq!(created_user.username, "test_user");
    assert_eq!(created_user.email, "test@example.com");
    assert_eq!(created_user.is_active, true);
    assert_eq!(created_user.is_admin, false);
    
    println!("✅ 数据库迁移和实体定义集成测试通过");
}