//! # 缓存抽象层
//!
//! 提供统一的缓存接口，支持内存缓存和 Redis 缓存

use crate::config::{CacheConfig, CacheType, RedisConfig};
use crate::error::{Context, Result};
use crate::{
    linfo,
    logging::{LogComponent, LogStage},
};
use async_trait::async_trait;
use dashmap::DashMap;
use moka::future::Cache;
use redis::AsyncCommands;
use serde::{Serialize, de::DeserializeOwned};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, OnceCell};

#[derive(Clone)]
struct CacheEntry {
    data: Arc<Vec<u8>>,
    expires_at: Option<Instant>,
}

impl CacheEntry {
    fn new(bytes: Vec<u8>, ttl: Option<Duration>) -> Self {
        let now = Instant::now();
        let expires_at = ttl.and_then(|duration| {
            if duration.is_zero() {
                Some(now)
            } else {
                now.checked_add(duration)
            }
        });

        Self {
            data: Arc::new(bytes),
            expires_at,
        }
    }

    const fn from_parts(data: Arc<Vec<u8>>, expires_at: Option<Instant>) -> Self {
        Self { data, expires_at }
    }

    fn is_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn remaining_ttl(&self) -> Option<Duration> {
        self.expires_at
            .and_then(|deadline| deadline.checked_duration_since(Instant::now()))
    }
}

/// 缓存抽象 trait
#[async_trait]
pub trait CacheProvider: Send + Sync {
    /// 设置缓存值
    async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send + Sync;

    /// 获取缓存值
    async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send;

    /// 删除缓存值
    async fn delete(&self, key: &str) -> Result<()>;

    /// 检查键是否存在
    async fn exists(&self, key: &str) -> Result<bool>;

    /// 设置过期时间
    async fn expire(&self, key: &str, ttl: Duration) -> Result<()>;

    /// 增加数字值
    async fn incr(&self, key: &str, delta: i64) -> Result<i64>;

    /// 清空所有缓存
    async fn clear(&self) -> Result<()>;

    /// 获取缓存统计信息
    async fn stats(&self) -> Result<CacheStats>;
}

/// 缓存统计信息
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_keys: usize,
    pub expired_keys: usize,
    pub hit_count: u64,
    pub miss_count: u64,
    pub cache_type: String,
}

impl CacheStats {
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // 对于比率计算，精度损失是可接受的
    pub fn hit_rate(&self) -> f64 {
        let total = self.hit_count + self.miss_count;
        if total == 0 {
            0.0
        } else {
            // 对于缓存命中比率，精度损失是可以接受的
            // 缓存命中次数通常不会达到会导致精度问题的数量级（一般不会超过 2^52）
            // 这里使用 as 转换是合理的，因为这是一个比率计算，不是精确的数值计算
            self.hit_count as f64 / total as f64
        }
    }
}

/// 基于 moka 的内存缓存实现
pub struct MemoryCache {
    cache: Cache<String, CacheEntry>,
    hit_count: AtomicU64,
    miss_count: AtomicU64,
    key_guards: DashMap<String, Arc<Mutex<()>>>,
    default_ttl: Option<Duration>,
}

impl MemoryCache {
    #[must_use]
    pub fn new(max_entries: usize, default_ttl: Option<Duration>) -> Self {
        let cache = Cache::builder().max_capacity(max_entries as u64).build();

        Self {
            cache,
            hit_count: AtomicU64::new(0),
            miss_count: AtomicU64::new(0),
            key_guards: DashMap::new(),
            default_ttl,
        }
    }

    fn guard_for(&self, key: &str) -> Arc<Mutex<()>> {
        self.key_guards
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn encode<T>(value: &T) -> Result<Vec<u8>>
    where
        T: Serialize + Send + Sync,
    {
        serde_json::to_vec(value).context("序列化缓存值失败")
    }

    fn decode<T>(value: &[u8]) -> Result<T>
    where
        T: DeserializeOwned + Send,
    {
        serde_json::from_slice(value).context("反序列化缓存值失败")
    }
}

#[async_trait]
impl CacheProvider for MemoryCache {
    async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        let encoded = Self::encode(value)?;
        let entry = CacheEntry::new(encoded, ttl.or(self.default_ttl));
        self.cache.insert(key.to_string(), entry).await;

        Ok(())
    }

    async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send,
    {
        if let Some(entry) = self.cache.get(key).await {
            if entry.is_expired() {
                self.cache.invalidate(key).await;
                self.miss_count.fetch_add(1, Ordering::Relaxed);
                Ok(None)
            } else {
                self.hit_count.fetch_add(1, Ordering::Relaxed);
                Self::decode(entry.data.as_ref()).map(Some)
            }
        } else {
            self.miss_count.fetch_add(1, Ordering::Relaxed);
            Ok(None)
        }
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.cache.invalidate(key).await;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        if let Some(entry) = self.cache.get(key).await {
            if entry.is_expired() {
                self.cache.invalidate(key).await;
                Ok(false)
            } else {
                Ok(true)
            }
        } else {
            Ok(false)
        }
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<()> {
        if let Some(entry) = self.cache.get(key).await {
            if entry.is_expired() {
                self.cache.invalidate(key).await;
            } else {
                let new_deadline = if ttl.is_zero() {
                    Some(Instant::now())
                } else {
                    Instant::now().checked_add(ttl)
                };
                let new_entry = CacheEntry::from_parts(Arc::clone(&entry.data), new_deadline);
                self.cache.insert(key.to_string(), new_entry).await;
            }
        }
        Ok(())
    }

    async fn incr(&self, key: &str, delta: i64) -> Result<i64> {
        let lock = self.guard_for(key);
        let _guard = lock.lock().await;

        let mut ttl = None;
        let current_value: i64 = match self.cache.get(key).await {
            Some(entry) => {
                if entry.is_expired() {
                    self.cache.invalidate(key).await;
                    0
                } else {
                    ttl = entry.remaining_ttl();
                    Self::decode::<i64>(entry.data.as_ref()).unwrap_or(0)
                }
            }
            None => 0,
        };

        let new_value = current_value.saturating_add(delta);
        let encoded = Self::encode(&new_value)?;
        let entry = CacheEntry::new(encoded, ttl);
        self.cache.insert(key.to_string(), entry).await;
        Ok(new_value)
    }

    async fn clear(&self) -> Result<()> {
        self.cache.invalidate_all();
        Ok(())
    }

    async fn stats(&self) -> Result<CacheStats> {
        Ok(CacheStats {
            total_keys: self.cache.entry_count().try_into().unwrap_or(usize::MAX),
            expired_keys: 0,
            hit_count: self.hit_count.load(Ordering::Relaxed),
            miss_count: self.miss_count.load(Ordering::Relaxed),
            cache_type: "Memory(moka/json)".to_string(),
        })
    }
}

/// Redis 缓存实现
pub struct RedisCache {
    client: redis::Client,
    connection_manager: OnceCell<redis::aio::ConnectionManager>,
    hit_count: AtomicU64,
    miss_count: AtomicU64,
}

impl RedisCache {
    pub fn new(redis_config: &RedisConfig) -> Result<Self> {
        let client = redis::Client::open(redis_config.url.as_str())
            .context("创建 Redis 客户端失败")?;

        Ok(Self {
            client,
            connection_manager: OnceCell::new(),
            hit_count: AtomicU64::new(0),
            miss_count: AtomicU64::new(0),
        })
    }

    async fn connection(&self) -> Result<redis::aio::ConnectionManager> {
        let client = self.client.clone();
        let manager = self
            .connection_manager
            .get_or_try_init(|| async {
                redis::aio::ConnectionManager::new(client)
                    .await
                    .context("建立 Redis 连接失败")
            })
            .await?;
        Ok(manager.clone())
    }

    fn encode<T>(value: &T) -> Result<Vec<u8>>
    where
        T: Serialize + Send + Sync,
    {
        serde_json::to_vec(value).context("序列化缓存值失败")
    }

    fn decode<T>(bytes: &[u8]) -> Result<T>
    where
        T: DeserializeOwned + Send,
    {
        serde_json::from_slice(bytes).context("反序列化缓存值失败")
    }
}

#[async_trait]
impl CacheProvider for RedisCache {
    async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        let serialized = Self::encode(value)?;
        let mut conn = self.connection().await?;

        match ttl {
            Some(duration) if duration.is_zero() => {
                let _: () = conn
                    .set(key, serialized)
                    .await
                    .context("Redis SET 失败")?;
            }
            Some(duration) => {
                let _: () = conn
                    .set_ex(key, serialized, duration.as_secs())
                    .await
                    .context("Redis SETEX 失败")?;
            }
            None => {
                let _: () = conn
                    .set(key, serialized)
                    .await
                    .context("Redis SET 失败")?;
            }
        }

        Ok(())
    }

    async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send,
    {
        let mut conn = self.connection().await?;
        let result: Option<Vec<u8>> = conn.get(key).await.context("Redis GET 失败")?;

        result.map_or_else(
            || {
                self.miss_count.fetch_add(1, Ordering::Relaxed);
                Ok(None)
            },
            |data| {
                self.hit_count.fetch_add(1, Ordering::Relaxed);
                Self::decode(&data).map(Some)
            },
        )
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.connection().await?;
        let _: usize = conn.del(key).await.context("Redis DEL 失败")?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.connection().await?;
        conn.exists(key).await.context("Redis EXISTS 失败")
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<()> {
        let mut conn = self.connection().await?;
        // Redis 的 EXPIRE 命令期望 i64 类型，这里转换是安全的
        let expire_seconds = i64::try_from(ttl.as_secs())
            .map_err(|_| crate::error!(Internal, "TTL 转换失败，超出 i64 范围"))?;
        let _: bool = conn
            .expire(key, expire_seconds)
            .await
            .context("Redis EXPIRE 失败")?;
        Ok(())
    }

    async fn incr(&self, key: &str, delta: i64) -> Result<i64> {
        let mut conn = self.connection().await?;
        conn.incr(key, delta)
            .await
            .context("Redis INCRBY 失败")
    }

    async fn clear(&self) -> Result<()> {
        let mut conn = self.connection().await?;
        let _: () = conn.flushdb().await.context("Redis FLUSHDB 失败")?;
        Ok(())
    }

    async fn stats(&self) -> Result<CacheStats> {
        let mut conn = self.connection().await?;
        let info: String = redis::cmd("INFO")
            .arg("keyspace")
            .query_async(&mut conn)
            .await
            .context("Redis INFO 失败")?;

        let total_keys = info
            .lines()
            .filter(|line| line.starts_with("db"))
            .filter_map(|line| {
                line.split("keys=")
                    .nth(1)
                    .and_then(|s| s.split(',').next())
                    .and_then(|s| s.parse::<usize>().ok())
            })
            .sum();

        Ok(CacheStats {
            total_keys,
            expired_keys: 0,
            hit_count: self.hit_count.load(Ordering::Relaxed),
            miss_count: self.miss_count.load(Ordering::Relaxed),
            cache_type: "Redis(json)".to_string(),
        })
    }
}

/// 缓存提供者类型 - 封装具体实现
pub enum CacheProviderType {
    Memory(MemoryCache),
    Redis(RedisCache),
}

impl CacheProviderType {
    /// 设置缓存值
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        match self {
            Self::Memory(cache) => cache.set(key, value, ttl).await,
            Self::Redis(cache) => cache.set(key, value, ttl).await,
        }
    }

    /// 获取缓存值
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send,
    {
        match self {
            Self::Memory(cache) => cache.get(key).await,
            Self::Redis(cache) => cache.get(key).await,
        }
    }

    /// 删除缓存值
    pub async fn delete(&self, key: &str) -> Result<()> {
        match self {
            Self::Memory(cache) => cache.delete(key).await,
            Self::Redis(cache) => cache.delete(key).await,
        }
    }

    /// 检查键是否存在
    pub async fn exists(&self, key: &str) -> Result<bool> {
        match self {
            Self::Memory(cache) => cache.exists(key).await,
            Self::Redis(cache) => cache.exists(key).await,
        }
    }

    /// 设置过期时间
    pub async fn expire(&self, key: &str, ttl: Duration) -> Result<()> {
        match self {
            Self::Memory(cache) => cache.expire(key, ttl).await,
            Self::Redis(cache) => cache.expire(key, ttl).await,
        }
    }

    /// 增加数字值
    pub async fn incr(&self, key: &str, delta: i64) -> Result<i64> {
        match self {
            Self::Memory(cache) => cache.incr(key, delta).await,
            Self::Redis(cache) => cache.incr(key, delta).await,
        }
    }

    /// 清空所有缓存
    pub async fn clear(&self) -> Result<()> {
        match self {
            Self::Memory(cache) => cache.clear().await,
            Self::Redis(cache) => cache.clear().await,
        }
    }

    /// 获取缓存统计信息
    pub async fn stats(&self) -> Result<CacheStats> {
        match self {
            Self::Memory(cache) => cache.stats().await,
            Self::Redis(cache) => cache.stats().await,
        }
    }
}

/// 缓存管理器
pub struct CacheManager {
    provider: CacheProviderType,
    default_ttl: Duration,
}

impl CacheManager {
    /// 根据配置创建缓存管理器
    pub fn new(config: &CacheConfig) -> Result<Self> {
        let default_ttl = Duration::from_secs(config.default_ttl.max(1));
        let provider = match config.cache_type {
            CacheType::Memory => {
                linfo!(
                    "system",
                    LogStage::Cache,
                    LogComponent::Cache,
                    "use_memory_cache",
                    &format!(
                        "使用内存缓存 (moka)，最大条目数: {}，默认 TTL: {}s",
                        config.memory_max_entries, config.default_ttl
                    )
                );
                CacheProviderType::Memory(MemoryCache::new(
                    config.memory_max_entries,
                    Some(default_ttl),
                ))
            }
            CacheType::Redis => {
                let redis_config = config
                    .redis
                    .as_ref()
                    .ok_or_else(|| crate::error!(Internal, "Redis 缓存未提供配置"))?;
                linfo!(
                    "system",
                    LogStage::Cache,
                    LogComponent::Cache,
                    "use_redis_cache",
                    &format!("使用 Redis 缓存，URL: {}", redis_config.url)
                );
                CacheProviderType::Redis(RedisCache::new(redis_config)?)
            }
        };

        Ok(Self {
            provider,
            default_ttl,
        })
    }

    /// 创建仅内存缓存管理器（用于测试）
    #[must_use]
    pub fn memory_only() -> Self {
        Self {
            provider: CacheProviderType::Memory(MemoryCache::new(
                1000,
                Some(Duration::from_secs(300)),
            )),
            default_ttl: Duration::from_secs(300),
        }
    }

    /// 获取缓存提供者的引用
    pub const fn provider(&self) -> &CacheProviderType {
        &self.provider
    }

    /// 检查缓存是否启用 - 缓存现在始终启用
    pub const fn is_enabled(&self, _config: &CacheConfig) -> bool {
        true
    }

    /// 设置缓存值
    pub async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        self.provider.set(key, value, ttl).await
    }

    /// 获取缓存值
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send,
    {
        self.provider.get(key).await
    }

    /// 删除缓存值
    pub async fn delete(&self, key: &str) -> Result<()> {
        self.provider.delete(key).await
    }

    /// 检查键是否存在
    pub async fn exists(&self, key: &str) -> Result<bool> {
        self.provider.exists(key).await
    }

    /// 设置过期时间
    pub async fn expire(&self, key: &str, ttl: Duration) -> Result<()> {
        self.provider.expire(key, ttl).await
    }

    /// 增加数字值
    pub async fn incr(&self, key: &str, delta: i64) -> Result<i64> {
        self.provider.incr(key, delta).await
    }

    /// 清空所有缓存
    pub async fn clear(&self) -> Result<()> {
        self.provider.clear().await
    }

    /// 获取缓存统计信息
    pub async fn stats(&self) -> Result<CacheStats> {
        self.provider.stats().await
    }

    /// 使用策略设置缓存（兼容原有 API）
    pub async fn set_with_strategy<T>(
        &self,
        key: &crate::cache::keys::CacheKey,
        value: &T,
    ) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        let ttl = crate::cache::strategies::CacheStrategies::for_key(key)
            .ttl
            .as_duration()
            .unwrap_or(self.default_ttl);
        self.provider.set(&key.build(), value, Some(ttl)).await
    }
}
