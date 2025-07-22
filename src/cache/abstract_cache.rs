//! # 缓存抽象层
//!
//! 提供统一的缓存接口，支持内存缓存和Redis缓存

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use crate::error::{ProxyError, Result};
use crate::config::{CacheConfig, CacheType};

/// 缓存项
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    expires_at: Option<Instant>,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Option<Duration>) -> Self {
        Self {
            value,
            expires_at: ttl.map(|t| Instant::now() + t),
        }
    }
    
    fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Instant::now() > expires_at
        } else {
            false
        }
    }
}

/// 缓存抽象trait
#[async_trait]
pub trait CacheProvider: Send + Sync {
    /// 设置缓存值
    async fn set<T>(&self, key: &str, value: T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send;
    
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
    pub fn hit_rate(&self) -> f64 {
        let total = self.hit_count + self.miss_count;
        if total == 0 {
            0.0
        } else {
            self.hit_count as f64 / total as f64
        }
    }
}

/// 内存缓存实现
pub struct MemoryCache {
    data: Arc<RwLock<HashMap<String, CacheEntry<Vec<u8>>>>>,
    max_entries: usize,
    hit_count: Arc<RwLock<u64>>,
    miss_count: Arc<RwLock<u64>>,
}

impl MemoryCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            max_entries,
            hit_count: Arc::new(RwLock::new(0)),
            miss_count: Arc::new(RwLock::new(0)),
        }
    }
    
    fn cleanup_expired(&self) {
        let mut data = self.data.write().unwrap();
        data.retain(|_, entry| !entry.is_expired());
    }
    
    fn ensure_capacity(&self) {
        let mut data = self.data.write().unwrap();
        if data.len() >= self.max_entries {
            // 简单的LRU：移除第一个找到的过期项，如果没有则移除第一个项
            let mut to_remove = None;
            
            // 先尝试移除过期项
            for (key, entry) in data.iter() {
                if entry.is_expired() {
                    to_remove = Some(key.clone());
                    break;
                }
            }
            
            // 如果没有过期项，移除第一个项（简化的LRU）
            if to_remove.is_none() {
                to_remove = data.keys().next().cloned();
            }
            
            if let Some(key) = to_remove {
                data.remove(&key);
            }
        }
    }
}

#[async_trait]
impl CacheProvider for MemoryCache {
    async fn set<T>(&self, key: &str, value: T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send,
    {
        let serialized = serde_json::to_vec(&value)
            .map_err(|e| ProxyError::cache_with_source("序列化缓存值失败", e))?;
        
        self.ensure_capacity();
        
        let entry = CacheEntry::new(serialized, ttl);
        let mut data = self.data.write().unwrap();
        data.insert(key.to_string(), entry);
        
        Ok(())
    }
    
    async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send,
    {
        self.cleanup_expired();
        
        let data = self.data.read().unwrap();
        if let Some(entry) = data.get(key) {
            if entry.is_expired() {
                *self.miss_count.write().unwrap() += 1;
                Ok(None)
            } else {
                *self.hit_count.write().unwrap() += 1;
                let value = serde_json::from_slice(&entry.value)
                    .map_err(|e| ProxyError::cache_with_source("反序列化缓存值失败", e))?;
                Ok(Some(value))
            }
        } else {
            *self.miss_count.write().unwrap() += 1;
            Ok(None)
        }
    }
    
    async fn delete(&self, key: &str) -> Result<()> {
        let mut data = self.data.write().unwrap();
        data.remove(key);
        Ok(())
    }
    
    async fn exists(&self, key: &str) -> Result<bool> {
        self.cleanup_expired();
        let data = self.data.read().unwrap();
        Ok(data.get(key).map_or(false, |entry| !entry.is_expired()))
    }
    
    async fn expire(&self, key: &str, ttl: Duration) -> Result<()> {
        let mut data = self.data.write().unwrap();
        if let Some(entry) = data.get_mut(key) {
            entry.expires_at = Some(Instant::now() + ttl);
        }
        Ok(())
    }
    
    async fn incr(&self, key: &str, delta: i64) -> Result<i64> {
        let mut data = self.data.write().unwrap();
        
        let current_value = if let Some(entry) = data.get(key) {
            if entry.is_expired() {
                0
            } else {
                serde_json::from_slice::<i64>(&entry.value).unwrap_or(0)
            }
        } else {
            0
        };
        
        let new_value = current_value + delta;
        let serialized = serde_json::to_vec(&new_value)
            .map_err(|e| ProxyError::cache_with_source("序列化数字值失败", e))?;
        
        let entry = CacheEntry::new(serialized, None);
        data.insert(key.to_string(), entry);
        
        Ok(new_value)
    }
    
    async fn clear(&self) -> Result<()> {
        let mut data = self.data.write().unwrap();
        data.clear();
        Ok(())
    }
    
    async fn stats(&self) -> Result<CacheStats> {
        self.cleanup_expired();
        
        let data = self.data.read().unwrap();
        let total_keys = data.len();
        let expired_keys = data.values().filter(|entry| entry.is_expired()).count();
        
        Ok(CacheStats {
            total_keys,
            expired_keys,
            hit_count: *self.hit_count.read().unwrap(),
            miss_count: *self.miss_count.read().unwrap(),
            cache_type: "Memory".to_string(),
        })
    }
}

/// Redis缓存实现
pub struct RedisCache {
    client: redis::Client,
    hit_count: Arc<RwLock<u64>>,
    miss_count: Arc<RwLock<u64>>,
}

impl RedisCache {
    pub fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| ProxyError::cache_with_source("创建Redis客户端失败", e))?;
        
        Ok(Self {
            client,
            hit_count: Arc::new(RwLock::new(0)),
            miss_count: Arc::new(RwLock::new(0)),
        })
    }
}

#[async_trait]
impl CacheProvider for RedisCache {
    async fn set<T>(&self, key: &str, value: T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send,
    {
        let serialized = serde_json::to_string(&value)
            .map_err(|e| ProxyError::cache_with_source("序列化缓存值失败", e))?;
        
        let mut conn = self.client.get_connection()
            .map_err(|e| ProxyError::cache_with_source("获取Redis连接失败", e))?;
        
        if let Some(ttl) = ttl {
            redis::cmd("SETEX")
                .arg(key)
                .arg(ttl.as_secs())
                .arg(&serialized)
                .execute(&mut conn);
        } else {
            redis::cmd("SET")
                .arg(key)
                .arg(&serialized)
                .execute(&mut conn);
        }
        
        Ok(())
    }
    
    async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send,
    {
        let mut conn = self.client.get_connection()
            .map_err(|e| ProxyError::cache_with_source("获取Redis连接失败", e))?;
        
        let result: Option<String> = redis::cmd("GET")
            .arg(key)
            .query(&mut conn)
            .map_err(|e| ProxyError::cache_with_source("Redis GET失败", e))?;
        
        if let Some(data) = result {
            *self.hit_count.write().unwrap() += 1;
            let value = serde_json::from_str(&data)
                .map_err(|e| ProxyError::cache_with_source("反序列化缓存值失败", e))?;
            Ok(Some(value))
        } else {
            *self.miss_count.write().unwrap() += 1;
            Ok(None)
        }
    }
    
    async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.client.get_connection()
            .map_err(|e| ProxyError::cache_with_source("获取Redis连接失败", e))?;
        
        redis::cmd("DEL")
            .arg(key)
            .execute(&mut conn);
        
        Ok(())
    }
    
    async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.client.get_connection()
            .map_err(|e| ProxyError::cache_with_source("获取Redis连接失败", e))?;
        
        let exists: bool = redis::cmd("EXISTS")
            .arg(key)
            .query(&mut conn)
            .map_err(|e| ProxyError::cache_with_source("Redis EXISTS失败", e))?;
        
        Ok(exists)
    }
    
    async fn expire(&self, key: &str, ttl: Duration) -> Result<()> {
        let mut conn = self.client.get_connection()
            .map_err(|e| ProxyError::cache_with_source("获取Redis连接失败", e))?;
        
        redis::cmd("EXPIRE")
            .arg(key)
            .arg(ttl.as_secs())
            .execute(&mut conn);
        
        Ok(())
    }
    
    async fn incr(&self, key: &str, delta: i64) -> Result<i64> {
        let mut conn = self.client.get_connection()
            .map_err(|e| ProxyError::cache_with_source("获取Redis连接失败", e))?;
        
        let result: i64 = redis::cmd("INCRBY")
            .arg(key)
            .arg(delta)
            .query(&mut conn)
            .map_err(|e| ProxyError::cache_with_source("Redis INCRBY失败", e))?;
        
        Ok(result)
    }
    
    async fn clear(&self) -> Result<()> {
        let mut conn = self.client.get_connection()
            .map_err(|e| ProxyError::cache_with_source("获取Redis连接失败", e))?;
        
        redis::cmd("FLUSHDB")
            .execute(&mut conn);
        
        Ok(())
    }
    
    async fn stats(&self) -> Result<CacheStats> {
        let mut conn = self.client.get_connection()
            .map_err(|e| ProxyError::cache_with_source("获取Redis连接失败", e))?;
        
        let info: String = redis::cmd("INFO")
            .arg("keyspace")
            .query(&mut conn)
            .map_err(|e| ProxyError::cache_with_source("Redis INFO失败", e))?;
        
        // 简化的Redis统计解析
        let total_keys = info.lines()
            .filter(|line| line.starts_with("db"))
            .map(|line| {
                line.split("keys=").nth(1)
                    .and_then(|s| s.split(',').next())
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0)
            })
            .sum();
        
        Ok(CacheStats {
            total_keys,
            expired_keys: 0, // Redis自动清理过期键
            hit_count: *self.hit_count.read().unwrap(),
            miss_count: *self.miss_count.read().unwrap(),
            cache_type: "Redis".to_string(),
        })
    }
}

/// 缓存提供者枚举 - 避免 trait object 兼容性问题
pub enum CacheProviderType {
    Memory(MemoryCache),
    Redis(RedisCache),
}

impl CacheProviderType {
    /// 设置缓存值
    pub async fn set<T>(&self, key: &str, value: T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send,
    {
        match self {
            CacheProviderType::Memory(cache) => cache.set(key, value, ttl).await,
            CacheProviderType::Redis(cache) => cache.set(key, value, ttl).await,
        }
    }
    
    /// 获取缓存值
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned + Send,
    {
        match self {
            CacheProviderType::Memory(cache) => cache.get(key).await,
            CacheProviderType::Redis(cache) => cache.get(key).await,
        }
    }
    
    /// 删除缓存值
    pub async fn delete(&self, key: &str) -> Result<()> {
        match self {
            CacheProviderType::Memory(cache) => cache.delete(key).await,
            CacheProviderType::Redis(cache) => cache.delete(key).await,
        }
    }
    
    /// 检查键是否存在
    pub async fn exists(&self, key: &str) -> Result<bool> {
        match self {
            CacheProviderType::Memory(cache) => cache.exists(key).await,
            CacheProviderType::Redis(cache) => cache.exists(key).await,
        }
    }
    
    /// 设置过期时间
    pub async fn expire(&self, key: &str, ttl: Duration) -> Result<()> {
        match self {
            CacheProviderType::Memory(cache) => cache.expire(key, ttl).await,
            CacheProviderType::Redis(cache) => cache.expire(key, ttl).await,
        }
    }
    
    /// 增加数字值
    pub async fn incr(&self, key: &str, delta: i64) -> Result<i64> {
        match self {
            CacheProviderType::Memory(cache) => cache.incr(key, delta).await,
            CacheProviderType::Redis(cache) => cache.incr(key, delta).await,
        }
    }
    
    /// 清空所有缓存
    pub async fn clear(&self) -> Result<()> {
        match self {
            CacheProviderType::Memory(cache) => cache.clear().await,
            CacheProviderType::Redis(cache) => cache.clear().await,
        }
    }
    
    /// 获取缓存统计信息
    pub async fn stats(&self) -> Result<CacheStats> {
        match self {
            CacheProviderType::Memory(cache) => cache.stats().await,
            CacheProviderType::Redis(cache) => cache.stats().await,
        }
    }
}

/// 统一缓存管理器
pub struct UnifiedCacheManager {
    provider: CacheProviderType,
}

impl UnifiedCacheManager {
    /// 根据配置创建缓存管理器
    pub fn new(config: &CacheConfig, redis_url: &str) -> Result<Self> {
        let provider = match config.cache_type {
            CacheType::Memory => {
                tracing::info!("使用内存缓存，最大条目数: {}", config.memory_max_entries);
                CacheProviderType::Memory(MemoryCache::new(config.memory_max_entries))
            },
            CacheType::Redis => {
                tracing::info!("使用Redis缓存，URL: {}", redis_url);
                CacheProviderType::Redis(RedisCache::new(redis_url)?)
            },
        };
        
        Ok(Self { provider })
    }
    
    /// 获取缓存提供者的引用
    pub fn provider(&self) -> &CacheProviderType {
        &self.provider
    }
    
    /// 检查缓存是否启用
    pub fn is_enabled(&self, config: &CacheConfig) -> bool {
        config.enabled
    }
    
    /// 设置缓存值
    pub async fn set<T>(&self, key: &str, value: T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize + Send,
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
    
    /// 使用策略设置缓存（兼容原有API）
    pub async fn set_with_strategy<T>(&self, key: &crate::cache::keys::CacheKey, value: &T) -> Result<()>
    where
        T: Serialize + Send + Sync + Clone,
    {
        // 使用默认TTL，可以根据key类型优化
        let ttl = Some(Duration::from_secs(300)); // 5分钟默认TTL
        self.provider.set(&key.build(), value.clone(), ttl).await
    }
}