//! # 数据库模块
//!
//! 数据库连接和迁移管理

use sea_orm::{Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;
use std::path::Path;
use tracing::{debug, error, info, warn};

/// 初始化数据库连接
pub async fn init_database(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    info!(
        "正在连接数据库: {}",
        if database_url.starts_with("sqlite:") {
            &database_url[..std::cmp::min(database_url.len(), 50)]
        } else {
            database_url
        }
    );

    // 对于SQLite数据库，确保数据库文件的目录和文件存在
    if database_url.starts_with("sqlite:") {
        let db_path = database_url
            .strip_prefix("sqlite://")
            .unwrap_or(database_url.strip_prefix("sqlite:").unwrap_or(database_url));
        let db_file_path = Path::new(db_path);

        // 确保父目录存在
        if let Some(parent_dir) = db_file_path.parent() {
            if !parent_dir.exists() {
                debug!("创建数据库目录: {}", parent_dir.display());
                std::fs::create_dir_all(parent_dir).map_err(|e| {
                    DbErr::Custom(format!(
                        "无法创建数据库目录 {}: {}",
                        parent_dir.display(),
                        e
                    ))
                })?;
                info!("数据库目录创建成功: {}", parent_dir.display());
            } else {
                debug!("数据库目录已存在: {}", parent_dir.display());
            }
        }

        // 确保数据库文件存在（如果不存在则创建空文件）
        if !db_file_path.exists() {
            debug!("创建数据库文件: {}", db_file_path.display());
            std::fs::File::create(db_file_path).map_err(|e| {
                DbErr::Custom(format!(
                    "无法创建数据库文件 {}: {}",
                    db_file_path.display(),
                    e
                ))
            })?;
            info!("数据库文件创建成功: {}", db_file_path.display());
        } else {
            debug!("数据库文件已存在: {}", db_file_path.display());
        }
    }

    let db = Database::connect(database_url).await?;

    info!("数据库连接成功");
    Ok(db)
}

/// 运行数据库迁移
pub async fn run_migrations(db: &DatabaseConnection) -> Result<(), DbErr> {
    info!("开始运行数据库迁移...");

    match ::migration::Migrator::up(db, None).await {
        Ok(_) => {
            info!("数据库迁移完成");
            Ok(())
        }
        Err(e) => {
            error!("数据库迁移失败: {}", e);
            Err(e)
        }
    }
}

/// 检查数据库状态
pub async fn check_database_status(db: &DatabaseConnection) -> Result<(), DbErr> {
    info!("检查数据库状态...");

    let status = ::migration::Migrator::get_pending_migrations(db).await?;

    if status.is_empty() {
        info!("所有迁移都已应用");
    } else {
        warn!("有 {} 个待应用的迁移", status.len());
    }

    Ok(())
}
