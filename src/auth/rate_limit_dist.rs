//! 分布式速率限制器（Redis/CacheManager 后端）
//!
//! 使用 CacheManager（UnifiedCacheManager） 的 `incr` + `expire` 实现跨实例一致的 QPS/日配额计数。
//! 先提供最小实现与接口；集成到 `ApiKeyManager` 可作为后续任务。

use std::time::Duration;

use anyhow::Result;

use crate::cache::{CacheManager, keys::CacheKeyBuilder};

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
    cache: std::sync::Arc<CacheManager>,
}

impl DistributedRateLimiter {
    pub const fn new(cache: std::sync::Arc<CacheManager>) -> Self {
        Self { cache }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn smoke_test_memory_backend() {
        // 使用内存后端快速冒烟
        let cache = std::sync::Arc::new(CacheManager::memory_only());
        let rl = DistributedRateLimiter::new(cache);

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
