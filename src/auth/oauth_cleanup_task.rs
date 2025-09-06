use std::time::Duration;
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set, QuerySelect, PaginatorTrait};
use tokio::time;
use tracing::{info, error};
use chrono::Utc;
use entity::oauth_client_sessions;
use crate::config::OAuthCleanupConfig;

/// OAuth 会话清理任务
pub struct OAuthCleanupTask {
    db: DatabaseConnection,
    config: OAuthCleanupConfig,
}

impl OAuthCleanupTask {
    pub fn new(db: DatabaseConnection, config: OAuthCleanupConfig) -> Self {
        Self { db, config }
    }

    /// 启动清理任务
    pub async fn start(&self) {
        // OAuth cleanup is always enabled - removed config.enabled check

        info!(
            "Starting OAuth cleanup task, cleanup interval: {}s, pending expire: {}min",
            self.config.cleanup_interval_seconds, self.config.pending_expire_minutes
        );

        let mut interval = time::interval(Duration::from_secs(self.config.cleanup_interval_seconds));

        loop {
            interval.tick().await;
            
            if let Err(e) = self.cleanup_expired_sessions().await {
                error!("Failed to cleanup expired OAuth sessions: {}", e);
            }
        }
    }

    /// 清理过期的 OAuth 会话
    pub async fn cleanup_expired_sessions(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let cutoff_time = Utc::now() - chrono::Duration::minutes(self.config.pending_expire_minutes as i64);
        
        // 查找需要清理的会话
        let expired_sessions = oauth_client_sessions::Entity::find()
            .filter(oauth_client_sessions::Column::Status.eq("pending"))
            .filter(oauth_client_sessions::Column::CreatedAt.lt(cutoff_time.naive_utc()))
            .limit(self.config.max_cleanup_records)
            .all(&self.db)
            .await?;

        if !expired_sessions.is_empty() {
            let cleanup_count = expired_sessions.len();
            info!("Found {} expired pending OAuth sessions to cleanup", cleanup_count);

            // 批量更新状态为 expired，而不是直接删除
            // 这样可以保留审计记录，便于后续分析
            for session in expired_sessions {
                let mut active_session: oauth_client_sessions::ActiveModel = session.into();
                active_session.status = Set("expired".to_string());
                active_session.error_message = Set(Some(format!("Session expired after {} minutes", self.config.pending_expire_minutes)));
                active_session.updated_at = Set(Utc::now().naive_utc());

                if let Err(e) = active_session.update(&self.db).await {
                    error!("Failed to update expired session status: {}", e);
                }
            }

            info!("Successfully marked {} expired pending OAuth sessions", cleanup_count);
        }

        // 清理过期时间更长的已标记为 expired 的记录
        self.cleanup_old_expired_sessions().await?;

        Ok(())
    }

    /// 清理更老的已标记为 expired 的会话记录
    /// 这些记录保留一段时间后可以完全删除
    async fn cleanup_old_expired_sessions(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 删除配置天数前标记为 expired 的记录
        let retention_days = self.config.expired_records_retention_days;
        let delete_cutoff_time = Utc::now() - chrono::Duration::days(retention_days as i64);
        
        let delete_result = oauth_client_sessions::Entity::delete_many()
            .filter(oauth_client_sessions::Column::Status.eq("expired"))
            .filter(oauth_client_sessions::Column::UpdatedAt.lt(delete_cutoff_time.naive_utc()))
            .exec(&self.db)
            .await?;

        if delete_result.rows_affected > 0 {
            info!("Deleted {} old expired OAuth session records", delete_result.rows_affected);
        }

        Ok(())
    }

    /// 获取清理统计信息
    pub async fn get_cleanup_stats(&self) -> Result<OAuthCleanupStats, Box<dyn std::error::Error + Send + Sync>> {
        let pending_count = oauth_client_sessions::Entity::find()
            .filter(oauth_client_sessions::Column::Status.eq("pending"))
            .count(&self.db)
            .await?;

        let expired_count = oauth_client_sessions::Entity::find()
            .filter(oauth_client_sessions::Column::Status.eq("expired"))
            .count(&self.db)
            .await?;

        let cutoff_time = Utc::now() - chrono::Duration::minutes(self.config.pending_expire_minutes as i64);
        let expired_pending_count = oauth_client_sessions::Entity::find()
            .filter(oauth_client_sessions::Column::Status.eq("pending"))
            .filter(oauth_client_sessions::Column::CreatedAt.lt(cutoff_time.naive_utc()))
            .count(&self.db)
            .await?;

        Ok(OAuthCleanupStats {
            total_pending: pending_count,
            total_expired: expired_count,
            expired_pending: expired_pending_count,
        })
    }
}

/// OAuth 清理统计信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthCleanupStats {
    pub total_pending: u64,
    pub total_expired: u64,
    pub expired_pending: u64,
}

#[cfg(test)]
mod tests {
    // 注意：由于当前 sea-orm 版本不支持 MockDatabase，这些测试被注释掉
    // 请参考 tests/oauth_cleanup_integration_test.rs 查看完整的集成测试
}