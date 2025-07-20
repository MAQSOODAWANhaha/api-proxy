//! # 缓存集成模块
//!
//! 提供高级缓存操作和策略集成

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::{client::CacheClient, keys::CacheKey, strategies::CacheStrategies};
use crate::{config::RedisConfig, error::Result};

/// 高级缓存管理器
#[derive(Clone)]
pub struct CacheManager {
    /// Redis 客户端
    client: CacheClient,
}

impl CacheManager {
    /// 从应用配置创建缓存管理器
    pub async fn from_config(redis_config: &RedisConfig) -> Result<Self> {
        // 转换配置格式
        let cache_config = super::client::RedisConfig {
            host: redis_config.host.clone(),
            port: redis_config.port,
            database: redis_config.database,
            password: redis_config.password.clone(),
            connection_timeout: redis_config.connection_timeout,
            default_ttl: redis_config.default_ttl,
            max_connections: redis_config.max_connections,
        };
        
        let client = CacheClient::new(cache_config).await?;
        
        Ok(Self { client })
    }
    
    /// 使用策略设置缓存
    pub async fn set_with_strategy<T>(&self, key: &CacheKey, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        let strategy = CacheStrategies::for_key(key);
        let json_value = serde_json::to_string(value)
            .map_err(|e| crate::error::ProxyError::cache_with_source("序列化缓存值失败", e))?;
        
        // 验证值是否符合策略要求
        if !strategy.validate_value(&json_value) {
            warn!("缓存值不符合策略要求: key={}", key);
            return Ok(());
        }
        
        if let Some(ttl_seconds) = strategy.ttl.as_seconds() {
            self.client.set_with_ttl(&key.build(), &json_value, ttl_seconds).await?;
        } else {
            // 永不过期的情况，使用一个很大的TTL值
            self.client.set_with_ttl(&key.build(), &json_value, u64::MAX).await?;
        }
        
        debug!("使用策略设置缓存成功: key={}, ttl={:?}", key, strategy.ttl);
        Ok(())
    }
    
    /// 获取缓存值
    pub async fn get<T>(&self, key: &CacheKey) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.client.get(&key.build()).await
    }
    
    /// 删除缓存
    pub async fn delete(&self, key: &CacheKey) -> Result<bool> {
        self.client.delete(&key.build()).await
    }
    
    /// 检查缓存是否存在
    pub async fn exists(&self, key: &CacheKey) -> Result<bool> {
        self.client.exists(&key.build()).await
    }
    
    /// 批量删除缓存
    pub async fn delete_pattern(&self, key: &CacheKey) -> Result<u64> {
        self.client.delete_pattern(&key.pattern()).await
    }
    
    /// 清空用户相关的所有缓存
    pub async fn clear_user_cache(&self, user_id: i32) -> Result<u64> {
        let mut total_deleted = 0;
        
        // 删除用户会话
        let session_key = super::keys::CacheKeyBuilder::user_session(user_id, "*");
        total_deleted += self.delete_pattern(&session_key).await?;
        
        // 删除用户API密钥
        let api_key = super::keys::CacheKeyBuilder::user_api_key(user_id, 0);
        total_deleted += self.delete_pattern(&api_key).await?;
        
        // 删除用户统计
        let stats_key = super::keys::CacheKeyBuilder::daily_stats(user_id, "*");
        total_deleted += self.delete_pattern(&stats_key).await?;
        
        // 删除速率限制
        let rate_limit_key = super::keys::CacheKeyBuilder::rate_limit(user_id, "*");
        total_deleted += self.delete_pattern(&rate_limit_key).await?;
        
        info!("清空用户缓存完成: user_id={}, deleted={}", user_id, total_deleted);
        Ok(total_deleted)
    }
    
    /// 预热关键缓存
    pub async fn warmup_cache(&self) -> Result<()> {
        info!("开始预热缓存...");
        
        // 预热配置缓存
        // TODO: 从数据库加载配置并缓存
        
        // 预热提供商配置
        // TODO: 从数据库加载提供商配置并缓存
        
        info!("缓存预热完成");
        Ok(())
    }
    
    /// 测试连接
    pub async fn ping(&self) -> Result<()> {
        self.client.ping().await
    }
    
    /// 获取底层客户端
    pub fn client(&self) -> &CacheClient {
        &self.client
    }
}

/// 缓存装饰器 - 用于方法级缓存
pub struct CacheDecorator<'a> {
    manager: &'a CacheManager,
    key: CacheKey,
}

impl<'a> CacheDecorator<'a> {
    /// 创建缓存装饰器
    pub fn new(manager: &'a CacheManager, key: CacheKey) -> Self {
        Self { manager, key }
    }
    
    /// 获取或计算值
    pub async fn get_or_compute<T, F, Fut>(&self, compute_fn: F) -> Result<T>
    where
        T: Serialize + for<'de> Deserialize<'de> + Clone,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // 先尝试从缓存获取
        if let Some(cached_value) = self.manager.get::<T>(&self.key).await? {
            debug!("缓存命中: key={}", self.key);
            return Ok(cached_value);
        }
        
        // 缓存未命中，计算新值
        debug!("缓存未命中，计算新值: key={}", self.key);
        let computed_value = compute_fn().await?;
        
        // 将计算结果存入缓存
        if let Err(e) = self.manager.set_with_strategy(&self.key, &computed_value).await {
            warn!("设置缓存失败: key={}, error={}", self.key, e);
        }
        
        Ok(computed_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::keys::CacheKeyBuilder;

    // 注意：这些测试需要 Redis 服务器运行
    #[tokio::test]
    #[ignore] // 默认忽略，需要手动运行
    async fn test_cache_manager_basic_operations() {
        let redis_config = RedisConfig::default();
        let cache_manager = CacheManager::from_config(&redis_config).await.unwrap();
        
        // 测试连接
        cache_manager.ping().await.unwrap();
        
        // 测试设置和获取
        let key = CacheKeyBuilder::config("test");
        let value = "test_value".to_string();
        
        cache_manager.set_with_strategy(&key, &value).await.unwrap();
        let retrieved: Option<String> = cache_manager.get(&key).await.unwrap();
        
        assert_eq!(retrieved, Some(value));
        
        // 测试删除
        let deleted = cache_manager.delete(&key).await.unwrap();
        assert!(deleted);
        
        let after_delete: Option<String> = cache_manager.get(&key).await.unwrap();
        assert_eq!(after_delete, None);
    }
    
    #[tokio::test]
    #[ignore]
    async fn test_cache_decorator() {
        let redis_config = RedisConfig::default();
        let cache_manager = CacheManager::from_config(&redis_config).await.unwrap();
        
        let key = CacheKeyBuilder::config("decorator_test");
        let decorator = CacheDecorator::new(&cache_manager, key.clone());
        
        // 清理可能存在的缓存
        let _ = cache_manager.delete(&key).await;
        
        // 第一次调用应该执行计算函数
        let result1 = decorator.get_or_compute(|| async {
            Ok("computed_value".to_string())
        }).await.unwrap();
        
        // 第二次调用应该从缓存获取
        let result2: String = decorator.get_or_compute(|| async {
            panic!("Should not be called - value should be cached");
        }).await.unwrap();
        
        assert_eq!(result1, result2);
        assert_eq!(result1, "computed_value");
    }
}