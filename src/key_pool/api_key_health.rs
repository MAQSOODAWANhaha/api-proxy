//! # API 密钥健康状态服务（简化版）
//!
//! 结合用户反馈，移除了主动探测与本地缓存逻辑，仅保留基于数据库的状态读写接口。

use crate::error::{Context, Result};
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, linfo, lwarn};
use chrono::{NaiveDateTime, Utc};
use entity::user_provider_keys;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Serialize;
use std::sync::Arc;

use super::types::ApiKeyHealthStatus;

/// 仅依赖数据库的健康状态服务
pub struct ApiKeyHealthService {
    db: Arc<DatabaseConnection>,
}

impl ApiKeyHealthService {
    /// 创建健康状态服务实例
    #[must_use]
    pub const fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// 从数据库中获取所有健康密钥的 ID
    pub async fn get_healthy_keys(&self) -> Vec<i32> {
        match user_provider_keys::Entity::find()
            .filter(
                user_provider_keys::Column::HealthStatus
                    .eq(ApiKeyHealthStatus::Healthy.to_string()),
            )
            .filter(user_provider_keys::Column::IsActive.eq(true))
            .all(self.db.as_ref())
            .await
        {
            Ok(keys) => keys.into_iter().map(|k| k.id).collect(),
            Err(err) => {
                lwarn!(
                    "system",
                    LogStage::HealthCheck,
                    LogComponent::HealthChecker,
                    "load_healthy_keys_failed",
                    "Failed to query healthy keys from database",
                    error = %err
                );
                Vec::new()
            }
        }
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
    pub async fn mark_key_as_rate_limited(
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
        }).to_string();

        // 只更新 health_status_detail，不改变 health_status
        model.health_status_detail = Set(Some(health_detail));
        model.updated_at = Set(now);

        model.update(self.db.as_ref()).await
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
