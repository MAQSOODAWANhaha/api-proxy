//! 分布式速率限制器（Redis/CacheManager 后端）
//!
//! 使用 CacheManager（UnifiedCacheManager） 的 `incr` + `expire` 实现跨实例一致的 QPS/日配额计数。
//! 先提供最小实现与接口；集成到 `ApiKeyManager` 可作为后续任务。

use crate::cache::{CacheManager, keys::CacheKeyBuilder};
use crate::error::{
    ProxyError, Result,
    auth::{AuthError, RateLimitInfo, RateLimitKind},
};
use entity::{proxy_tracing, user_service_apis};
use sea_orm::prelude::Decimal;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, QueryFilter, QuerySelect,
};
use std::collections::HashMap;
use std::convert::TryInto;
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
pub struct RateLimiter {
    cache: Arc<CacheManager>,
    db: Arc<DatabaseConnection>,
}

#[derive(Debug, FromQueryResult)]
struct DailyUsageAggregate {
    user_service_api_id: i32,
    total_tokens: Option<i64>,
    total_cost: Option<f64>,
    total_requests: Option<i64>,
}

#[derive(Debug, FromQueryResult)]
struct ApiOwner {
    id: i32,
    user_id: i32,
}

impl RateLimiter {
    pub(crate) const PLAN_TYPE: &'static str = "pro";

    const TOKEN_PREFIX: &'static str = "ratelimit:daily:tokens";
    const COST_PREFIX: &'static str = "ratelimit:daily:cost";
    /// 创建新的限流器实例，要求提供缓存与数据库
    pub const fn new(cache: Arc<CacheManager>, db: Arc<DatabaseConnection>) -> Self {
        Self { cache, db }
    }

    pub(crate) fn rate_limit_error(
        kind: RateLimitKind,
        limit: Option<f64>,
        current: Option<f64>,
        resets_in: Option<Duration>,
    ) -> ProxyError {
        ProxyError::Authentication(AuthError::RateLimitExceeded(RateLimitInfo {
            kind,
            limit,
            current,
            resets_in,
            plan_type: Self::PLAN_TYPE.to_string(),
        }))
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
        let (_, key, _) = Self::daily_request_cache_key(user_id, endpoint);

        let current = self.cache.incr(&key, 1).await?;
        // 设置到当天结束的 TTL
        let ttl = Self::seconds_until_end_of_day();
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

    /// 查询当前分钟窗口内的请求计数
    pub async fn current_per_minute(&self, user_id: i32, endpoint: &str) -> Result<i64> {
        let key = CacheKeyBuilder::rate_limit(user_id, endpoint).build();
        Ok(self.cache.get::<i64>(&key).await?.unwrap_or(0))
    }

    /// 查询当前自然日内累计的 Token 使用量
    pub async fn current_daily_tokens(&self, user_api_id: i32) -> Result<i64> {
        let (_, cache_key, _) = Self::daily_cache_context(Self::TOKEN_PREFIX, user_api_id);
        Ok(self.cache.get::<i64>(&cache_key).await?.unwrap_or(0))
    }

    /// 查询当前自然日内累计的请求次数
    pub async fn current_daily_requests(&self, user_id: i32, endpoint: &str) -> Result<i64> {
        let (_, cache_key, _) = Self::daily_request_cache_key(user_id, endpoint);
        Ok(self.cache.get::<i64>(&cache_key).await?.unwrap_or(0))
    }

    /// 基于数据库检查每日Token限制
    pub async fn check_daily_token_limit(&self, user_api_id: i32, limit: i64) -> Result<()> {
        let (_, cache_key, ttl) = Self::daily_cache_context(Self::TOKEN_PREFIX, user_api_id);

        if let Some(cached_tokens) = self.cache.get::<i64>(&cache_key).await? {
            if cached_tokens >= limit {
                return Err(Self::rate_limit_error(
                    RateLimitKind::DailyTokens,
                    Some(Self::to_f64(limit)),
                    Some(Self::to_f64(cached_tokens)),
                    Some(ttl),
                ));
            }
            return Ok(());
        }

        let start_of_day = Self::start_of_day();

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

        // 缓存查询结果，减少后续数据库压力
        let _ = self.cache.set(&cache_key, &current_tokens, Some(ttl)).await;

        if current_tokens >= limit {
            return Err(Self::rate_limit_error(
                RateLimitKind::DailyTokens,
                Some(Self::to_f64(limit)),
                Some(Self::to_f64(current_tokens)),
                Some(ttl),
            ));
        }
        Ok(())
    }

    /// 基于数据库检查每日成本限制
    pub async fn check_daily_cost_limit(&self, user_api_id: i32, limit: Decimal) -> Result<()> {
        let (_, cache_key, ttl) = Self::daily_cache_context(Self::COST_PREFIX, user_api_id);

        if let Some(cached_cost) = self.cache.get::<f64>(&cache_key).await? {
            let limit_value = Self::decimal_to_f64(limit)?;
            if cached_cost >= limit_value {
                return Err(Self::rate_limit_error(
                    RateLimitKind::DailyCost,
                    Some(limit_value),
                    Some(cached_cost),
                    Some(ttl),
                ));
            }
            return Ok(());
        }

        let start_of_day = Self::start_of_day();

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
        let _ = self.cache.set(&cache_key, &current_cost, Some(ttl)).await;

        let limit_value = Self::decimal_to_f64(limit)?;

        if current_cost >= limit_value {
            return Err(Self::rate_limit_error(
                RateLimitKind::DailyCost,
                Some(limit_value),
                Some(current_cost),
                Some(ttl),
            ));
        }
        Ok(())
    }

    /// 将成功调用的Token增量写入缓存，保持实时性
    pub async fn increment_daily_token_cache(&self, user_api_id: i32, delta: i64) -> Result<()> {
        if delta <= 0 {
            return Ok(());
        }

        let (_, cache_key, ttl) = Self::daily_cache_context(Self::TOKEN_PREFIX, user_api_id);
        let mut current = self.cache.get::<i64>(&cache_key).await?.unwrap_or(0);
        current = current.saturating_add(delta);
        self.cache.set(&cache_key, &current, Some(ttl)).await?;
        Ok(())
    }

    /// 将成功调用的请求次数写入缓存
    pub async fn increment_daily_request_cache(
        &self,
        user_id: i32,
        endpoint: &str,
        delta: i64,
    ) -> Result<()> {
        if delta <= 0 {
            return Ok(());
        }

        let (_, cache_key, ttl) = Self::daily_request_cache_key(user_id, endpoint);
        let mut current = self.cache.get::<i64>(&cache_key).await?.unwrap_or(0);
        current = current.saturating_add(delta);
        self.cache.set(&cache_key, &current, Some(ttl)).await?;
        Ok(())
    }

    /// 将成功调用的费用增量写入缓存
    pub async fn increment_daily_cost_cache(&self, user_api_id: i32, delta: f64) -> Result<()> {
        if delta <= 0.0 {
            return Ok(());
        }

        let (_, cache_key, ttl) = Self::daily_cache_context(Self::COST_PREFIX, user_api_id);
        let mut current = self.cache.get::<f64>(&cache_key).await?.unwrap_or(0.0);
        current += delta;
        self.cache.set(&cache_key, &current, Some(ttl)).await?;
        Ok(())
    }

    /// 服务启动时预热每日Token/费用/请求使用缓存
    pub async fn warmup_daily_usage_cache(&self) -> Result<()> {
        let start_of_day = Self::start_of_day();
        let ttl = Duration::from_secs(Self::seconds_until_end_of_day());
        let api_owners = user_service_apis::Entity::find()
            .select_only()
            .column(user_service_apis::Column::Id)
            .column(user_service_apis::Column::UserId)
            .into_model::<ApiOwner>()
            .all(self.db.as_ref())
            .await?;

        let owner_map: HashMap<i32, i32> = api_owners
            .into_iter()
            .map(|owner| (owner.id, owner.user_id))
            .collect();

        let aggregates = proxy_tracing::Entity::find()
            .select_only()
            .column(proxy_tracing::Column::UserServiceApiId)
            .column_as(proxy_tracing::Column::TokensTotal.sum(), "total_tokens")
            .column_as(proxy_tracing::Column::Cost.sum(), "total_cost")
            .column_as(proxy_tracing::Column::Id.count(), "total_requests")
            .filter(proxy_tracing::Column::CreatedAt.gte(start_of_day))
            .group_by(proxy_tracing::Column::UserServiceApiId)
            .into_model::<DailyUsageAggregate>()
            .all(self.db.as_ref())
            .await?;

        for aggregate in aggregates {
            let Some(user_id) = owner_map.get(&aggregate.user_service_api_id) else {
                continue;
            };

            let (_, token_key, _) =
                Self::daily_cache_context(Self::TOKEN_PREFIX, aggregate.user_service_api_id);
            let (_, cost_key, _) =
                Self::daily_cache_context(Self::COST_PREFIX, aggregate.user_service_api_id);

            let endpoint = Self::service_api_endpoint(aggregate.user_service_api_id);
            let (_, request_key, _) = Self::daily_request_cache_key(*user_id, &endpoint);

            let tokens = aggregate.total_tokens.unwrap_or(0);
            let cost = aggregate.total_cost.unwrap_or(0.0);
            let requests = aggregate.total_requests.unwrap_or(0);

            let _ = self.cache.set(&token_key, &tokens, Some(ttl)).await;
            let _ = self.cache.set(&cost_key, &cost, Some(ttl)).await;
            let _ = self.cache.set(&request_key, &requests, Some(ttl)).await;
        }

        Ok(())
    }

    fn start_of_day() -> chrono::NaiveDateTime {
        chrono::Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .expect("valid midnight")
    }

    fn seconds_until_end_of_day() -> u64 {
        let now = chrono::Utc::now();
        let tomorrow = (now.date_naive() + chrono::Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .expect("valid midnight");
        let seconds = (tomorrow.and_utc() - now).num_seconds().max(60);
        seconds.try_into().unwrap_or(u64::MAX)
    }

    fn daily_cache_context(
        prefix: &str,
        target_id: i32,
    ) -> (chrono::NaiveDateTime, String, Duration) {
        let start = Self::start_of_day();
        let date_suffix = Self::date_suffix();
        let key = format!("{prefix}:{target_id}:{date_suffix}");
        let ttl = Duration::from_secs(Self::seconds_until_end_of_day());
        (start, key, ttl)
    }

    fn daily_request_cache_key(
        user_id: i32,
        endpoint: &str,
    ) -> (chrono::NaiveDateTime, String, Duration) {
        let start = Self::start_of_day();
        let base = CacheKeyBuilder::rate_limit(user_id, endpoint).build();
        let key = format!("{base}:{}", Self::date_suffix());
        let ttl = Duration::from_secs(Self::seconds_until_end_of_day());
        (start, key, ttl)
    }

    fn decimal_to_f64(value: Decimal) -> Result<f64> {
        value
            .to_string()
            .parse::<f64>()
            .map_err(|_| crate::error!(Internal, "Invalid decimal value for cost limit"))
    }

    #[allow(clippy::cast_precision_loss)]
    const fn to_f64(value: i64) -> f64 {
        value as f64
    }

    fn service_api_endpoint(user_api_id: i32) -> String {
        format!("service_api:{user_api_id}")
    }

    fn date_suffix() -> String {
        chrono::Utc::now().format("%Y%m%d").to_string()
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
        let rl = RateLimiter::new(cache, db);

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
