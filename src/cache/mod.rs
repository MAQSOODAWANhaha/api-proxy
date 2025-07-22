//! # 缓存模块
//!
//! Redis 缓存客户端和缓存策略实现

pub mod client;
pub mod keys;
pub mod strategies;
pub mod integration;
pub mod abstract_cache;

pub use client::{CacheClient, RedisConfig};
pub use keys::{CacheKey, CacheKeyBuilder};
pub use strategies::{CacheStrategy, CacheStrategies, CacheTtl};
pub use integration::{CacheManager, CacheDecorator};
pub use abstract_cache::{CacheProvider, CacheProviderType, UnifiedCacheManager, CacheStats, MemoryCache, RedisCache};
