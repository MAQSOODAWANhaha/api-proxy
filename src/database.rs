//! # 数据库模块
//!
//! 数据库连接和迁移管理

use sea_orm::{Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;
use tracing::{info, warn};

/// 初始化数据库连接
pub async fn init_database(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    info!("正在连接数据库: {}", 
          if database_url.starts_with("sqlite:") {
              &database_url[..20]
          } else {
              database_url
          });
    
    let db = Database::connect(database_url).await?;
    
    info!("数据库连接成功");
    Ok(db)
}

/// 运行数据库迁移
pub async fn run_migrations(db: &DatabaseConnection) -> Result<(), DbErr> {
    info!("开始运行数据库迁移...");
    
    migration::Migrator::up(db, None).await?;
    
    info!("数据库迁移完成");
    Ok(())
}

/// 检查数据库状态
pub async fn check_database_status(db: &DatabaseConnection) -> Result<(), DbErr> {
    info!("检查数据库状态...");
    
    let status = migration::Migrator::get_pending_migrations(db).await?;
    
    if status.is_empty() {
        info!("所有迁移都已应用");
    } else {
        warn!("有 {} 个待应用的迁移", status.len());
    }
    
    Ok(())
}