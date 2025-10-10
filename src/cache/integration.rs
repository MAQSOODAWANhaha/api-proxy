//! # 缓存集成模块
//!
//! 提供高级缓存操作和策略集成

use crate::{
    config::{CacheConfig, CacheType},
    ldebug, lerror, linfo,
    logging::{LogComponent, LogStage},
    lwarn,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::{client::CacheClient, keys::CacheKey, strategies::CacheStrategies};
use crate::error::{ProxyError, Result};
use entity::*;

/// 缓存的提供商配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: i32,
    pub name: String,
    pub display_name: String,
    pub base_url: String,
    pub api_format: String,
    pub default_model: Option<String>,
    pub max_tokens: Option<i32>,
    pub rate_limit: Option<i32>,
    pub timeout_seconds: i32,
    pub health_check_path: Option<String>,
    pub config_json: Option<String>,
}

/// 缓存的用户API配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserApiConfig {
    pub id: i32,
    pub user_id: i32,
    pub provider_type_id: i32,
    pub api_key: String,
    pub user_provider_keys_ids: Vec<i32>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub scheduling_strategy: Option<String>,
    pub retry_count: Option<i32>,
    pub timeout_seconds: Option<i32>,
    pub max_request_per_min: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub max_tokens_per_day: Option<i64>,
    pub max_cost_per_day: Option<sea_orm::prelude::Decimal>,
}

/// 高级缓存门面（避免与基础 CacheManager 混淆）
#[derive(Clone)]
pub struct CacheFacade {
    /// Redis 客户端
    client: CacheClient,
    /// 数据库连接（可选，用于预热功能）
    database: Option<Arc<DatabaseConnection>>,
    /// 缓存配置
    cache_config: CacheConfig,
}

impl CacheFacade {
    /// 从应用配置创建缓存管理器
    pub async fn from_config(cache_config: &CacheConfig) -> Result<Self> {
        if !matches!(cache_config.cache_type, CacheType::Redis) {
            return Err(ProxyError::cache(
                "CacheFacade 仅在 cache_type 为 redis 时可用",
            ));
        }

        let redis_config = cache_config
            .redis
            .as_ref()
            .ok_or_else(|| ProxyError::cache("Redis 缓存配置缺失"))?;
        // 转换配置格式
        let client_redis_config = super::client::RedisConfig {
            host: redis_config.host.clone(),
            port: redis_config.port,
            database: redis_config.database,
            password: redis_config.password.clone(),
            connection_timeout: redis_config.connection_timeout,
            max_connections: redis_config.max_connections,
        };

        let client = CacheClient::new(client_redis_config).await?;

        Ok(Self {
            client,
            database: None,
            cache_config: cache_config.clone(),
        })
    }

    /// 创建带数据库连接的缓存管理器
    pub async fn with_database(
        cache_config: &CacheConfig,
        database: Arc<DatabaseConnection>,
    ) -> Result<Self> {
        let mut manager = Self::from_config(cache_config).await?;
        manager.database = Some(database);
        Ok(manager)
    }

    /// 使用策略设置缓存
    pub async fn set_with_strategy<T>(&self, key: &CacheKey, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        let strategy = CacheStrategies::for_key(key);
        let json_value = serde_json::to_string(value)
            .map_err(|e| ProxyError::cache_with_source("序列化缓存值失败", e))?;

        // 验证值是否符合策略要求
        if !strategy.validate_value(&json_value) {
            lwarn!(
                "system",
                LogStage::Cache,
                LogComponent::Cache,
                "invalid_value",
                &format!("缓存值不符合策略要求: key={}", key)
            );
            return Ok(());
        }

        let ttl_seconds = strategy
            .ttl
            .as_seconds()
            .unwrap_or(self.cache_config.default_ttl);

        self.client
            .set_with_ttl(&key.build(), &json_value, ttl_seconds)
            .await?;

        ldebug!(
            "system",
            LogStage::Cache,
            LogComponent::Cache,
            "set_with_strategy",
            &format!("使用策略设置缓存成功: key={}, ttl={}s", key, ttl_seconds)
        );
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

        // 删除用户API配置
        let api_config_key = super::keys::CacheKeyBuilder::user_api_config(user_id, 0);
        total_deleted += self.delete_pattern(&api_config_key).await?;

        // 删除用户统计
        let stats_key = super::keys::CacheKeyBuilder::daily_stats(user_id, "*");
        total_deleted += self.delete_pattern(&stats_key).await?;

        // 删除速率限制
        let rate_limit_key = super::keys::CacheKeyBuilder::rate_limit(user_id, "*");
        total_deleted += self.delete_pattern(&rate_limit_key).await?;

        linfo!(
            "system",
            LogStage::Cache,
            LogComponent::Cache,
            "clear_user_cache_complete",
            &format!(
                "清空用户缓存完成: user_id={}, deleted={}",
                user_id, total_deleted
            )
        );
        Ok(total_deleted)
    }

    /// 预热关键缓存
    pub async fn warmup_cache(&self) -> Result<()> {
        linfo!(
            "system",
            LogStage::Cache,
            LogComponent::Cache,
            "warmup_start",
            "开始预热缓存..."
        );

        let Some(db) = &self.database else {
            lwarn!(
                "system",
                LogStage::Cache,
                LogComponent::Cache,
                "warmup_no_db",
                "数据库连接不可用，跳过缓存预热"
            );
            return Ok(());
        };

        let mut warmup_count = 0;

        // 预热提供商配置缓存
        match self.warmup_provider_configs(db).await {
            Ok(count) => {
                linfo!(
                    "system",
                    LogStage::Cache,
                    LogComponent::Cache,
                    "warmup_provider_configs_ok",
                    &format!("成功预热 {} 个提供商配置到缓存", count)
                );
                warmup_count += count;
            }
            Err(e) => lerror!(
                "system",
                LogStage::Cache,
                LogComponent::Cache,
                "warmup_provider_configs_fail",
                &format!("预热提供商配置失败: {}", e)
            ),
        }

        // 预热活跃用户的API密钥配置
        match self.warmup_active_user_configs(db).await {
            Ok(count) => {
                linfo!(
                    "system",
                    LogStage::Cache,
                    LogComponent::Cache,
                    "warmup_user_configs_ok",
                    &format!("成功预热 {} 个用户API配置到缓存", count)
                );
                warmup_count += count;
            }
            Err(e) => lerror!(
                "system",
                LogStage::Cache,
                LogComponent::Cache,
                "warmup_user_configs_fail",
                &format!("预热用户API配置失败: {}", e)
            ),
        }

        // 预热系统配置
        match self.warmup_system_configs(db).await {
            Ok(count) => {
                linfo!(
                    "system",
                    LogStage::Cache,
                    LogComponent::Cache,
                    "warmup_system_configs_ok",
                    &format!("成功预热 {} 个系统配置到缓存", count)
                );
                warmup_count += count;
            }
            Err(e) => lerror!(
                "system",
                LogStage::Cache,
                LogComponent::Cache,
                "warmup_system_configs_fail",
                &format!("预热系统配置失败: {}", e)
            ),
        }

        linfo!(
            "system",
            LogStage::Cache,
            LogComponent::Cache,
            "warmup_complete",
            &format!("缓存预热完成，共预热 {} 个配置项", warmup_count)
        );
        Ok(())
    }

    /// 预热提供商配置
    async fn warmup_provider_configs(&self, db: &DatabaseConnection) -> Result<usize> {
        ldebug!(
            "system",
            LogStage::Cache,
            LogComponent::Cache,
            "warmup_provider_configs",
            "开始预热提供商配置..."
        );

        // 查询所有活跃的提供商类型
        let providers = ProviderTypes::find()
            .filter(provider_types::Column::IsActive.eq(true))
            .all(db)
            .await
            .map_err(|e| ProxyError::database(format!("查询提供商类型失败: {}", e)))?;

        let mut cached_count = 0;

        for provider in providers {
            // 构建提供商配置缓存键
            let config_key = super::keys::CacheKeyBuilder::provider_config(&provider.name);

            // 构建提供商配置数据
            let provider_config = ProviderConfig {
                id: provider.id,
                name: provider.name.clone(),
                display_name: provider.display_name,
                base_url: provider.base_url,
                api_format: provider.api_format,
                default_model: provider.default_model,
                max_tokens: provider.max_tokens,
                rate_limit: provider.rate_limit,
                timeout_seconds: provider.timeout_seconds.unwrap_or(30),
                health_check_path: provider.health_check_path,
                config_json: provider.config_json,
            };

            // 缓存提供商配置
            if let Err(e) = self.set_with_strategy(&config_key, &provider_config).await {
                lwarn!(
                    "system",
                    LogStage::Cache,
                    LogComponent::Cache,
                    "warmup_provider_fail",
                    &format!(
                        "缓存提供商配置失败: provider={}, error={}",
                        provider.name, e
                    )
                );
                continue;
            }

            cached_count += 1;
            ldebug!(
                "system",
                LogStage::Cache,
                LogComponent::Cache,
                "warmup_provider_config",
                &format!("已缓存提供商配置: {}", provider.name)
            );
        }

        Ok(cached_count)
    }

    /// 预热活跃用户的API配置
    async fn warmup_active_user_configs(&self, db: &DatabaseConnection) -> Result<usize> {
        ldebug!(
            "system",
            LogStage::Cache,
            LogComponent::Cache,
            "warmup_user_configs",
            "开始预热用户API配置..."
        );

        // 查询最近活跃的用户API配置（最近30天有使用记录的）
        let _thirty_days_ago = chrono::Utc::now() - chrono::Duration::days(30);

        let user_apis = UserServiceApis::find()
            .filter(user_service_apis::Column::IsActive.eq(true))
            // 注意：由于删除了last_used字段，这里改为查询所有活跃的API
            // TODO: 可以考虑从proxy_tracing表查询最近使用的API ID列表
            .all(db)
            .await
            .map_err(|e| ProxyError::database(format!("查询用户API配置失败: {}", e)))?;

        let mut cached_count = 0;

        for user_api in user_apis {
            // 构建用户API配置缓存键
            let config_key =
                super::keys::CacheKeyBuilder::user_api_config(user_api.user_id, user_api.id);

            // 构建用户API配置数据
            let user_provider_keys_ids: Vec<i32> =
                serde_json::from_value(user_api.user_provider_keys_ids.clone()).unwrap_or_default();
            let api_config = UserApiConfig {
                id: user_api.id,
                user_id: user_api.user_id,
                provider_type_id: user_api.provider_type_id,
                api_key: user_api.api_key,
                user_provider_keys_ids,
                name: user_api.name,
                description: user_api.description,
                scheduling_strategy: user_api.scheduling_strategy,
                retry_count: user_api.retry_count,
                timeout_seconds: user_api.timeout_seconds,
                max_request_per_min: user_api.max_request_per_min,
                max_requests_per_day: user_api.max_requests_per_day,
                max_tokens_per_day: user_api.max_tokens_per_day,
                max_cost_per_day: user_api.max_cost_per_day,
            };

            // 缓存用户API配置
            if let Err(e) = self.set_with_strategy(&config_key, &api_config).await {
                lwarn!(
                    "system",
                    LogStage::Cache,
                    LogComponent::Cache,
                    "warmup_user_api_fail",
                    &format!(
                        "缓存用户API配置失败: user_id={}, api_id={}, error={}",
                        user_api.user_id, user_api.id, e
                    )
                );
                continue;
            }

            cached_count += 1;
            ldebug!(
                "system",
                LogStage::Cache,
                LogComponent::Cache,
                "warmup_user_config",
                &format!(
                    "已缓存用户API配置: user_id={}, api_id={}",
                    user_api.user_id, user_api.id
                )
            );
        }

        Ok(cached_count)
    }

    /// 预热系统配置
    async fn warmup_system_configs(&self, _db: &DatabaseConnection) -> Result<usize> {
        ldebug!(
            "system",
            LogStage::Cache,
            LogComponent::Cache,
            "warmup_system_configs",
            "开始预热系统配置..."
        );

        let mut cached_count = 0;

        // 缓存默认系统配置
        let system_configs = vec![
            (
                "rate_limit_default",
                serde_json::json!({
                    "requests_per_minute": 60,
                    "tokens_per_day": 10000,
                    "enabled": true
                }),
            ),
            (
                "health_check_interval",
                serde_json::json!({
                    "interval_seconds": 30,
                    "timeout_seconds": 10,
                    "max_failures": 3
                }),
            ),
            (
                "load_balancer_config",
                serde_json::json!({
                    "algorithm": "round_robin",
                    "health_check_enabled": true,
                    "failover_enabled": true
                }),
            ),
        ];

        for (config_name, config_value) in system_configs {
            let config_key = super::keys::CacheKeyBuilder::config(config_name);

            if let Err(e) = self.set_with_strategy(&config_key, &config_value).await {
                lwarn!(
                    "system",
                    LogStage::Cache,
                    LogComponent::Cache,
                    "warmup_system_config_fail",
                    &format!("缓存系统配置失败: config={}, error={}", config_name, e)
                );
                continue;
            }

            cached_count += 1;
            ldebug!(
                "system",
                LogStage::Cache,
                LogComponent::Cache,
                "warmup_system_config",
                &format!("已缓存系统配置: {}", config_name)
            );
        }

        Ok(cached_count)
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
    manager: &'a CacheFacade,
    key: CacheKey,
}

impl<'a> CacheDecorator<'a> {
    /// 创建缓存装饰器
    pub fn new(manager: &'a CacheFacade, key: CacheKey) -> Self {
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
            ldebug!(
                "system",
                LogStage::Cache,
                LogComponent::Cache,
                "cache_hit",
                &format!("缓存命中: key={}", self.key)
            );
            return Ok(cached_value);
        }

        // 缓存未命中，计算新值
        ldebug!(
            "system",
            LogStage::Cache,
            LogComponent::Cache,
            "cache_miss",
            &format!("缓存未命中，计算新值: key={}", self.key)
        );
        let computed_value = compute_fn().await?;

        // 将计算结果存入缓存
        if let Err(e) = self
            .manager
            .set_with_strategy(&self.key, &computed_value)
            .await
        {
            lwarn!(
                "system",
                LogStage::Cache,
                LogComponent::Cache,
                "set_cache_fail",
                &format!("设置缓存失败: key={}, error={}", self.key, e)
            );
        }

        Ok(computed_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cache::keys::CacheKeyBuilder,
        config::{CacheType, RedisConfig},
    };

    // 注意：这些测试需要 Redis 服务器运行
    #[tokio::test]
    #[ignore = "requires Redis instance"] // 默认忽略，需要手动运行
    async fn test_cache_manager_basic_operations() {
        let mut cache_config = CacheConfig::default();
        cache_config.cache_type = CacheType::Redis;
        cache_config.redis = Some(RedisConfig::default());
        let cache_manager = CacheFacade::from_config(&cache_config).await.unwrap();

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
    #[ignore = "requires Redis instance"]
    async fn test_cache_decorator() {
        let mut cache_config = CacheConfig::default();
        cache_config.cache_type = CacheType::Redis;
        cache_config.redis = Some(RedisConfig::default());
        let cache_manager = CacheFacade::from_config(&cache_config).await.unwrap();

        let key = CacheKeyBuilder::config("decorator_test");
        let decorator = CacheDecorator::new(&cache_manager, key.clone());

        // 清理可能存在的缓存
        let _ = cache_manager.delete(&key).await;

        // 第一次调用应该执行计算函数
        let result1 = decorator
            .get_or_compute(|| async { Ok("computed_value".to_string()) })
            .await
            .unwrap();

        // 第二次调用应该从缓存获取
        let result2: String = decorator
            .get_or_compute(|| async {
                panic!("Should not be called - value should be cached");
            })
            .await
            .unwrap();

        assert_eq!(result1, result2);
        assert_eq!(result1, "computed_value");
    }
}
