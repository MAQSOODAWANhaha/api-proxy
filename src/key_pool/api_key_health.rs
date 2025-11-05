//! # API 密钥健康状态服务（简化版）
//!
//! 结合用户反馈，移除了主动探测与本地缓存逻辑，仅保留基于数据库的状态读写接口。

use crate::error::{Context, Result};
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, lerror, linfo, lwarn};
use chrono::{NaiveDateTime, Utc};
use entity::user_provider_keys;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;

// 前向声明
use crate::key_pool::api_key_rate_limit_reset_task::ApiKeyRateLimitResetTask;
use serde::Serialize;

use super::types::ApiKeyHealthStatus;

/// API密钥健康状态服务
pub struct ApiKeyHealthService {
    db: Arc<DatabaseConnection>,
    reset_task: RwLock<Option<Weak<ApiKeyRateLimitResetTask>>>,
}

impl ApiKeyHealthService {
    /// 创建健康状态服务实例
    #[must_use]
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            reset_task: RwLock::new(None),
        }
    }

    /// 设置恢复任务引用
    pub async fn set_reset_task(&self, reset_task: &Arc<ApiKeyRateLimitResetTask>) {
        *self.reset_task.write().await = Some(Arc::downgrade(reset_task));
    }

    /// 从数据库中获取所有健康密钥的 ID
    pub async fn get_healthy_keys(&self) -> Vec<i32> {
        match user_provider_keys::Entity::find()
            .filter(
                user_provider_keys::Column::HealthStatus
                    .eq(ApiKeyHealthStatus::Healthy.to_string()),
            )
            .all(self.db.as_ref())
            .await
        {
            Ok(keys) => keys.into_iter().map(|k| k.id).collect(),
            Err(e) => {
                lerror!(
                    "system",
                    LogStage::HealthCheck,
                    LogComponent::HealthChecker,
                    "get_healthy_keys_error",
                    "Failed to get healthy keys",
                    error = %e
                );
                vec![]
            }
        }
    }

    /// 根据ID获取密钥信息
    pub async fn get_key_by_id(&self, key_id: i32) -> Option<user_provider_keys::Model> {
        user_provider_keys::Entity::find_by_id(key_id)
            .one(self.db.as_ref())
            .await
            .ok()?
    }

    /// 获取数据库连接引用（供内部服务使用）
    #[must_use]
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// 从数据库加载待恢复的限流任务
    pub async fn load_pending_resets_from_db(&self) -> Result<Vec<(i32, chrono::NaiveDateTime)>> {
        let now = Utc::now().naive_utc();

        let pending_keys = user_provider_keys::Entity::find()
            .filter(
                user_provider_keys::Column::HealthStatus
                    .eq(ApiKeyHealthStatus::RateLimited.to_string()),
            )
            .filter(user_provider_keys::Column::RateLimitResetsAt.is_not_null())
            .all(self.db.as_ref())
            .await?;

        let mut pending_resets = Vec::new();
        for key in pending_keys {
            if let Some(resets_at) = key.rate_limit_resets_at {
                // 只恢复未过期的任务
                if resets_at > now {
                    pending_resets.push((key.id, resets_at));
                    linfo!(
                        "system",
                        LogStage::Startup,
                        LogComponent::HealthChecker,
                        "restored_rate_limit_reset",
                        "Restored pending rate limit reset task",
                        key_id = key.id,
                        resets_at = %resets_at
                    );
                } else {
                    // 已过期但状态未更新，记录日志但不处理
                    // 重置任务会由 ResetTask 在启动时处理
                }
            }
        }

        Ok(pending_resets)
    }

    /// 重置密钥状态（带延迟验证）
    pub async fn reset_key_status(&self, key_id: i32) -> Result<()> {
        // 延迟验证：只有确实处于 rate_limited 状态时才重置
        if let Some(key) = self.get_key_by_id(key_id).await
            && key.health_status == ApiKeyHealthStatus::RateLimited.to_string()
        {
            self.mark_key_healthy(key_id).await?;
            linfo!(
                "system",
                LogStage::HealthCheck,
                LogComponent::HealthChecker,
                "key_status_reset",
                "Key status reset to healthy",
                key_id = key_id
            );
        }
        Ok(())
    }

    /// 将密钥标记为不健康
    pub async fn mark_key_unhealthy(&self, key_id: i32, reason: String) -> Result<()> {
        let now = Utc::now().naive_utc();
        let mut model: user_provider_keys::ActiveModel =
            user_provider_keys::Entity::find_by_id(key_id)
                .one(self.db.as_ref())
                .await?
                .ok_or_else(|| crate::error!(Database, format!("API密钥不存在: {key_id}")))?
                .into();

        let detail = serde_json::json!({
            "error_message": reason,
            "updated_at": now,
            "health_score": 0.0,
        })
        .to_string();

        model.health_status = Set(ApiKeyHealthStatus::Unhealthy.to_string());
        model.health_status_detail = Set(Some(detail));
        model.rate_limit_resets_at = Set(None);
        model.last_error_time = Set(Some(now));
        model.updated_at = Set(now);

        model.update(self.db.as_ref()).await?;

        linfo!(
            "system",
            LogStage::HealthCheck,
            LogComponent::HealthChecker,
            "mark_unhealthy",
            "API key marked as unhealthy",
            key_id = key_id
        );
        Ok(())
    }

    /// 将密钥标记为限流状态
    pub async fn mark_key_rate_limited(
        &self,
        key_id: i32,
        resets_at: Option<NaiveDateTime>,
        details: &str,
    ) -> Result<()> {
        let now = Utc::now().naive_utc();
        let mut model: user_provider_keys::ActiveModel =
            user_provider_keys::Entity::find_by_id(key_id)
                .one(self.db.as_ref())
                .await?
                .ok_or_else(|| crate::error!(Database, format!("API密钥不存在: {key_id}")))?
                .into();

        model.health_status = Set(ApiKeyHealthStatus::RateLimited.to_string());
        model.health_status_detail = Set(Some(details.to_string()));
        model.rate_limit_resets_at = Set(resets_at);
        model.last_error_time = Set(Some(now));
        model.updated_at = Set(now);

        model
            .update(self.db.as_ref())
            .await
            .context(format!("更新API密钥健康状态失败，ID: {key_id}"))?;

        ldebug!(
            "system",
            LogStage::HealthCheck,
            LogComponent::HealthChecker,
            "mark_rate_limited",
            "API key marked as rate limited",
            key_id = key_id,
            resets_at = ?resets_at
        );

        // 如果有恢复时间，调度恢复任务
        if let Some(resets_at) = resets_at {
            let reset_task = {
                let guard = self.reset_task.read().await;
                guard.as_ref().and_then(Weak::upgrade)
            };

            if let Some(reset_task) = reset_task {
                if let Err(e) = reset_task.schedule_reset(key_id, resets_at).await {
                    lwarn!(
                        "system",
                        LogStage::HealthCheck,
                        LogComponent::HealthChecker,
                        "schedule_reset_failed",
                        "调度限流恢复任务失败，将依赖被动扫描",
                        key_id = key_id,
                        resets_at = ?resets_at,
                        error = %e
                    );
                } else {
                    ldebug!(
                        "system",
                        LogStage::HealthCheck,
                        LogComponent::HealthChecker,
                        "schedule_reset_success",
                        "成功调度限流恢复任务",
                        key_id = key_id,
                        resets_at = ?resets_at
                    );
                }
            } else {
                lwarn!(
                    "system",
                    LogStage::HealthCheck,
                    LogComponent::HealthChecker,
                    "reset_task_unavailable",
                    "有限流恢复时间但无恢复任务服务，将依赖被动扫描",
                    key_id = key_id,
                    resets_at = ?resets_at
                );
            }
        }

        Ok(())
    }

    /// 将密钥直接标记为健康
    pub async fn mark_key_healthy(&self, key_id: i32) -> Result<()> {
        let now = Utc::now().naive_utc();
        let mut model: user_provider_keys::ActiveModel =
            user_provider_keys::Entity::find_by_id(key_id)
                .one(self.db.as_ref())
                .await?
                .ok_or_else(|| crate::error!(Database, format!("API密钥不存在: {key_id}")))?
                .into();

        model.health_status = Set(ApiKeyHealthStatus::Healthy.to_string());
        model.health_status_detail = Set(None);
        model.rate_limit_resets_at = Set(None);
        model.last_error_time = Set(None);
        model.updated_at = Set(now);

        model.update(self.db.as_ref()).await?;

        linfo!(
            "system",
            LogStage::HealthCheck,
            LogComponent::HealthChecker,
            "mark_healthy",
            "API key marked as healthy",
            key_id = key_id
        );

        Ok(())
    }

    /// 更新密钥的健康状态详情信息（不改变健康状态）
    pub async fn update_health_status_detail<T: Serialize + Sync>(
        &self,
        key_id: i32,
        data: &T,
    ) -> Result<()> {
        let now = Utc::now().naive_utc();
        let mut model: user_provider_keys::ActiveModel =
            user_provider_keys::Entity::find_by_id(key_id)
                .one(self.db.as_ref())
                .await?
                .ok_or_else(|| crate::error!(Database, format!("API密钥不存在: {key_id}")))?
                .into();

        // 构造健康状态详情JSON
        let health_detail = serde_json::json!({
            "data": data,
            "updated_at": now
        })
        .to_string();

        // 只更新 health_status_detail，不改变 health_status
        model.health_status_detail = Set(Some(health_detail));
        model.updated_at = Set(now);

        model
            .update(self.db.as_ref())
            .await
            .context(format!("更新API密钥健康状态详情失败，ID: {key_id}"))?;

        ldebug!(
            "system",
            LogStage::HealthCheck,
            LogComponent::HealthChecker,
            "update_health_status_detail",
            "API key health status detail updated",
            key_id = key_id
        );

        Ok(())
    }
}
