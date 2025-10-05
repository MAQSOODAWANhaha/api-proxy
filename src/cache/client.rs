//! # Redis 缓存客户端
//!
//! 提供 Redis 连接管理和基础操作

use crate::{ldebug, lerror, linfo, lwarn, logging::{LogComponent, LogStage}};
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use serde::{Deserialize, Serialize};

use crate::error::{ProxyError, Result};

/// Redis 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis 服务器地址
    pub host: String,
    /// Redis 服务器端口
    pub port: u16,
    /// 数据库编号
    pub database: u8,
    /// 连接密码（可选）
    pub password: Option<String>,
    /// 连接超时时间（秒）
    pub connection_timeout: u64,
    /// 默认 TTL（秒）
    pub default_ttl: u64,
    /// 最大连接数
    pub max_connections: u32,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6379,
            database: 0,
            password: None,
            connection_timeout: 10,
            default_ttl: 3600, // 1 小时
            max_connections: 10,
        }
    }
}

impl RedisConfig {
    /// 构建 Redis 连接 URL
    pub fn build_url(&self) -> String {
        if let Some(password) = &self.password {
            format!(
                "redis://:{}@{}:{}/{}",
                password, self.host, self.port, self.database
            )
        } else {
            format!("redis://{}:{}/{}", self.host, self.port, self.database)
        }
    }
}

/// Redis 缓存客户端
pub struct CacheClient {
    /// Redis 连接管理器
    connection_manager: ConnectionManager,
    /// 配置信息
    config: RedisConfig,
}

impl CacheClient {
    /// 创建新的缓存客户端
    pub async fn new(config: RedisConfig) -> Result<Self> {
        linfo!("system", LogStage::Cache, LogComponent::Cache, "connect_to_redis", &format!("正在连接 Redis 服务器: {}:{}", config.host, config.port));

        let client = Client::open(config.build_url())
            .map_err(|e| ProxyError::cache_with_source("创建 Redis 客户端失败", e))?;

        let connection_manager = ConnectionManager::new(client)
            .await
            .map_err(|e| ProxyError::cache_with_source("建立 Redis 连接失败", e))?;

        linfo!("system", LogStage::Cache, LogComponent::Cache, "redis_connected", "Redis 连接建立成功");

        Ok(Self {
            connection_manager,
            config,
        })
    }

    /// 设置缓存值
    pub async fn set<T>(&self, key: &str, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        let json_value = serde_json::to_string(value)
            .map_err(|e| ProxyError::cache_with_source("序列化缓存值失败", e))?;

        self.set_with_ttl(key, &json_value, self.config.default_ttl)
            .await
    }

    /// 设置缓存值并指定 TTL
    pub async fn set_with_ttl(&self, key: &str, value: &str, ttl_seconds: u64) -> Result<()> {
        ldebug!("system", LogStage::Cache, LogComponent::Cache, "set_cache", &format!("设置缓存: key={}, ttl={}s", key, ttl_seconds));

        let mut conn = self.connection_manager.clone();

        conn.set_ex::<_, _, ()>(key, value, ttl_seconds)
            .await
            .map_err(|e| ProxyError::cache_with_source(&format!("设置缓存失败: {key}"), e))?;

        ldebug!("system", LogStage::Cache, LogComponent::Cache, "set_cache_ok", &format!("缓存设置成功: {}", key));
        Ok(())
    }

    /// 获取缓存值
    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        ldebug!("system", LogStage::Cache, LogComponent::Cache, "get_cache", &format!("获取缓存: key={}", key));

        let mut conn = self.connection_manager.clone();

        let result: Option<String> = conn
            .get(key)
            .await
            .map_err(|e| ProxyError::cache_with_source(&format!("获取缓存失败: {key}"), e))?;

        match result {
            Some(json_str) => {
                let value = serde_json::from_str(&json_str)
                    .map_err(|e| ProxyError::cache_with_source("反序列化缓存值失败", e))?;
                ldebug!("system", LogStage::Cache, LogComponent::Cache, "cache_hit", &format!("缓存命中: {}", key));
                Ok(Some(value))
            }
            None => {
                ldebug!("system", LogStage::Cache, LogComponent::Cache, "cache_miss", &format!("缓存未命中: {}", key));
                Ok(None)
            }
        }
    }

    /// 删除缓存
    pub async fn delete(&self, key: &str) -> Result<bool> {
        ldebug!("system", LogStage::Cache, LogComponent::Cache, "delete_cache", &format!("删除缓存: key={}", key));

        let mut conn = self.connection_manager.clone();

        let deleted_count: i32 = conn
            .del(key)
            .await
            .map_err(|e| ProxyError::cache_with_source(&format!("删除缓存失败: {key}"), e))?;

        let was_deleted = deleted_count > 0;
        ldebug!("system", LogStage::Cache, LogComponent::Cache, "delete_cache_result", &format!("缓存删除结果: key={}, deleted={}", key, was_deleted));
        Ok(was_deleted)
    }

    /// 检查缓存是否存在
    pub async fn exists(&self, key: &str) -> Result<bool> {
        ldebug!("system", LogStage::Cache, LogComponent::Cache, "check_exists", &format!("检查缓存存在性: key={}", key));

        let mut conn = self.connection_manager.clone();

        let exists: bool = conn
            .exists(key)
            .await
            .map_err(|e| ProxyError::cache_with_source(&format!("检查缓存存在性失败: {key}"), e))?;

        ldebug!("system", LogStage::Cache, LogComponent::Cache, "check_exists_result", &format!("缓存存在性检查结果: key={}, exists={}", key, exists));
        Ok(exists)
    }

    /// 设置缓存过期时间
    pub async fn expire(&self, key: &str, ttl_seconds: u64) -> Result<bool> {
        ldebug!("system", LogStage::Cache, LogComponent::Cache, "set_expire", &format!("设置缓存过期时间: key={}, ttl={}s", key, ttl_seconds));

        let mut conn = self.connection_manager.clone();

        let success: bool = conn.expire(key, ttl_seconds as i64).await.map_err(|e| {
            ProxyError::cache_with_source(&format!("设置缓存过期时间失败: {key}"), e)
        })?;

        ldebug!("system", LogStage::Cache, LogComponent::Cache, "set_expire_result", &format!("缓存过期时间设置结果: key={}, success={}", key, success));
        Ok(success)
    }

    /// 获取缓存剩余存活时间
    pub async fn ttl(&self, key: &str) -> Result<i64> {
        ldebug!("system", LogStage::Cache, LogComponent::Cache, "get_ttl", &format!("获取缓存剩余存活时间: key={}", key));

        let mut conn = self.connection_manager.clone();

        let ttl: i64 = conn
            .ttl(key)
            .await
            .map_err(|e| ProxyError::cache_with_source(&format!("获取缓存TTL失败: {key}"), e))?;

        ldebug!("system", LogStage::Cache, LogComponent::Cache, "get_ttl_result", &format!("缓存剩余存活时间: key={}, ttl={}s", key, ttl));
        Ok(ttl)
    }

    /// 批量删除符合模式的缓存
    pub async fn delete_pattern(&self, pattern: &str) -> Result<u64> {
        lwarn!("system", LogStage::Cache, LogComponent::Cache, "delete_pattern", &format!("批量删除缓存: pattern={}", pattern));

        let mut conn = self.connection_manager.clone();

        // 先获取匹配的键
        let keys: Vec<String> = conn.keys(pattern).await.map_err(|e| {
            ProxyError::cache_with_source(&format!("查找匹配的缓存键失败: {pattern}"), e)
        })?;

        if keys.is_empty() {
            ldebug!("system", LogStage::Cache, LogComponent::Cache, "no_matching_keys", &format!("没有找到匹配的缓存键: {}", pattern));
            return Ok(0);
        }

        // 批量删除
        let deleted_count: i32 = conn.del(&keys).await.map_err(|e| {
            ProxyError::cache_with_source(&format!("批量删除缓存失败: {pattern}"), e)
        })?;

        lwarn!(
            "system",
            LogStage::Cache,
            LogComponent::Cache,
            "delete_pattern_complete",
            &format!("批量删除缓存完成: pattern={}, deleted={}", pattern, deleted_count)
        );
        Ok(deleted_count as u64)
    }

    /// 测试连接
    pub async fn ping(&self) -> Result<()> {
        ldebug!("system", LogStage::Cache, LogComponent::Cache, "ping", "测试 Redis 连接");

        let mut conn = self.connection_manager.clone();

        // 使用 cmd 方法执行 PING 命令
        let response: String = redis::Cmd::new()
            .arg("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| ProxyError::cache_with_source("Redis ping 失败", e))?;

        if response == "PONG" {
            linfo!("system", LogStage::Cache, LogComponent::Cache, "ping_success", "Redis 连接测试成功");
            Ok(())
        } else {
            lerror!("system", LogStage::Cache, LogComponent::Cache, "ping_fail", &format!("Redis ping 响应异常: {}", response));
            Err(ProxyError::cache("Redis 连接测试失败"))
        }
    }

    /// 执行原始Redis命令
    pub async fn raw_command<T>(&self, args: &[&str]) -> Result<T>
    where
        T: redis::FromRedisValue + std::fmt::Debug,
    {
        ldebug!("system", LogStage::Cache, LogComponent::Cache, "raw_command", &format!("执行原始Redis命令: {:?}", args));

        let mut conn = self.connection_manager.clone();

        let mut cmd = redis::Cmd::new();
        for arg in args {
            cmd.arg(*arg);
        }

        let result: T = cmd.query_async(&mut conn).await.map_err(|e| {
            ProxyError::cache_with_source(&format!("执行Redis命令失败: {:?}", args), e)
        })?;

        ldebug!("system", LogStage::Cache, LogComponent::Cache, "raw_command_result", &format!("Redis命令执行成功: {:?} -> {:?}", args, result));
        Ok(result)
    }

    /// 获取配置信息
    pub fn config(&self) -> &RedisConfig {
        &self.config
    }
}

impl Clone for CacheClient {
    fn clone(&self) -> Self {
        Self {
            connection_manager: self.connection_manager.clone(),
            config: self.config.clone(),
        }
    }
}
