//! # 缓存模块
//!
//! Redis 缓存客户端和缓存策略实现

pub mod abstract_cache;
pub mod client;
pub mod integration;
pub mod keys;
pub mod strategies;

pub use abstract_cache::{
    CacheProvider, CacheProviderType, CacheStats, MemoryCache, RedisCache, UnifiedCacheManager,
};
pub use client::{CacheClient, RedisConfig};
pub use integration::{CacheDecorator, CacheManager};
pub use keys::{CacheKey, CacheKeyBuilder};
pub use strategies::{CacheStrategies, CacheStrategy, CacheTtl};
