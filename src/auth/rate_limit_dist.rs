//! 分布式速率限制器（Redis/CacheManager 后端）
//!
//! 使用 CacheManager（UnifiedCacheManager） 的 `incr` + `expire` 实现跨实例一致的 QPS/日配额计数。
//! 先提供最小实现与接口；集成到 `ApiKeyManager` 可作为后续任务。

use crate::cache::{CacheManager, keys::CacheKeyBuilder};
use crate::error::Result;
use entity::proxy_tracing;
use sea_orm::prelude::Decimal;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use std::sync::Arc;
use std::time::Duration;

/// 分布式速率限制检查结果
#[derive(Debug, Clone)]
pub struct DistRateLimitOutcome {
    pub allowed: bool,
    pub current: i64,
    pub limit: i64,
    pub ttl_seconds: i64,
}

/// 简单的分布式限流器
pub struct DistributedRateLimiter {
    cache: Arc<CacheManager>,
    db: Arc<DatabaseConnection>,
}

impl DistributedRateLimiter {
    /// 创建新的限流器实例，要求提供缓存与数据库
    pub const fn new(cache: Arc<CacheManager>, db: Arc<DatabaseConnection>) -> Self {
        Self { cache, db }
    }

    /// 以“用户+端点”为维度的每分钟请求限制
    pub async fn check_per_minute(
        &self,
        user_id: i32,
        endpoint: &str,
        limit: i64,
    ) -> Result<DistRateLimitOutcome> {
        let key = CacheKeyBuilder::rate_limit(user_id, endpoint).build();

        // 使用 INCR 原子自增
        let current = self.cache.incr(&key, 1).await?;

        // 初次创建时设置 60s 过期，形成分片计数窗口
        if current == 1 {
            let _ = self.cache.expire(&key, Duration::from_secs(60)).await;
        }

        Ok(DistRateLimitOutcome {
            allowed: current <= limit,
            current,
            limit,
            ttl_seconds: 60,
        })
    }

    /// 简单的每日请求限制（自然日）
    pub async fn check_per_day(
        &self,
        user_id: i32,
        endpoint: &str,
        limit: i64,
    ) -> Result<DistRateLimitOutcome> {
        // 将 endpoint 继续复用，若需更细粒度可在外层区分
        let date = chrono::Utc::now().format("%Y%m%d").to_string();
        let key = format!(
            "{}:{}",
            CacheKeyBuilder::rate_limit(user_id, endpoint).build(),
            date
        );

        let current = self.cache.incr(&key, 1).await?;
        // 设置到当天结束的 TTL
        let now = chrono::Utc::now();
        let tomorrow = (now.date_naive() + chrono::Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap();
        #[allow(clippy::cast_sign_loss)]
        let ttl = (tomorrow.and_utc() - now).num_seconds().max(60) as u64;
        if current == 1 {
            let _ = self.cache.expire(&key, Duration::from_secs(ttl)).await;
        }

        Ok(DistRateLimitOutcome {
            allowed: current <= limit,
            current,
            limit,
            #[allow(clippy::cast_possible_wrap)]
            ttl_seconds: ttl as i64,
        })
    }

    /// 基于数据库检查每日Token限制
    pub async fn check_daily_token_limit_db(&self, user_api_id: i32, limit: i64) -> Result<()> {
        let now = chrono::Utc::now();
        let start_of_day = now.date_naive().and_hms_opt(0, 0, 0).unwrap();

        let current_tokens: Option<i64> = proxy_tracing::Entity::find()
            .select_only()
            .column_as(proxy_tracing::Column::TokensTotal.sum(), "total_tokens")
            .filter(proxy_tracing::Column::UserServiceApiId.eq(user_api_id))
            .filter(proxy_tracing::Column::CreatedAt.gte(start_of_day))
            .into_tuple::<Option<i64>>()
            .one(self.db.as_ref())
            .await?
            .unwrap_or_default();

        let current_tokens = current_tokens.unwrap_or(0);

        if current_tokens >= limit {
            return Err(crate::error!(
                Authentication,
                "Daily token limit reached (limit = {}, current = {})",
                limit,
                current_tokens
            ));
        }
        Ok(())
    }

    /// 基于数据库检查每日成本限制
    pub async fn check_daily_cost_limit_db(&self, user_api_id: i32, limit: Decimal) -> Result<()> {
        let now = chrono::Utc::now();
        let start_of_day = now.date_naive().and_hms_opt(0, 0, 0).unwrap();

        let current_cost: Option<f64> = proxy_tracing::Entity::find()
            .select_only()
            .column_as(proxy_tracing::Column::Cost.sum(), "total_cost")
            .filter(proxy_tracing::Column::UserServiceApiId.eq(user_api_id))
            .filter(proxy_tracing::Column::CreatedAt.gte(start_of_day))
            .into_tuple::<Option<f64>>()
            .one(self.db.as_ref())
            .await?
            .unwrap_or_default();

        let current_cost = current_cost.unwrap_or(0.0);
        let limit_value = limit
            .to_string()
            .parse::<f64>()
            .map_err(|_| crate::error!(Internal, "Invalid daily cost limit configuration"))?;

        if current_cost >= limit_value {
            return Err(crate::error!(
                Authentication,
                "Daily cost limit reached (limit = {}, current = {:.4})",
                limit,
                current_cost
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn smoke_test_memory_backend() {
        // 使用内存后端快速冒烟
        let cache = Arc::new(CacheManager::memory_only());
        let db = Arc::new(
            sea_orm::Database::connect("sqlite::memory:")
                .await
                .expect("create in-memory db"),
        );
        let rl = DistributedRateLimiter::new(cache, db);

        for i in 1..=3 {
            let out = rl.check_per_minute(1, "/v1/test", 2).await.unwrap();
            if i <= 2 {
                assert!(out.allowed);
            } else {
                assert!(!out.allowed);
            }
        }
    }
}
