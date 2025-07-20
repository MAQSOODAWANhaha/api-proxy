//! # 测试辅助函数
//!
//! 提供通用的测试工具和辅助函数

use sea_orm::{Database, DatabaseConnection, DbErr, ConnectionTrait};
use sea_orm_migration::MigratorTrait;
use std::sync::Once;
use tempfile::TempDir;
use tracing::Level;

static INIT: Once = Once::new();

/// 初始化测试环境
pub fn init_test_env() {
    INIT.call_once(|| {
        // 初始化日志
        tracing_subscriber::fmt()
            .with_max_level(Level::DEBUG)
            .with_test_writer()
            .try_init()
            .ok();

        // 设置测试环境变量
        std::env::set_var("RUST_ENV", "test");
    });
}

/// 创建内存数据库连接
pub async fn create_test_db() -> Result<DatabaseConnection, DbErr> {
    let db = Database::connect("sqlite::memory:").await?;
    
    // 运行迁移
    migration::Migrator::up(&db, None).await?;
    
    Ok(db)
}

/// 创建临时数据库文件
pub async fn create_temp_db() -> Result<(DatabaseConnection, TempDir), DbErr> {
    let temp_dir = tempfile::tempdir().map_err(|e| {
        DbErr::Custom(format!("创建临时目录失败: {}", e))
    })?;
    
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite:{}", db_path.display());
    
    let db = Database::connect(&db_url).await?;
    migration::Migrator::up(&db, None).await?;
    
    Ok((db, temp_dir))
}

/// 创建测试用的缓存管理器
#[cfg(feature = "redis")]
pub async fn create_test_cache_manager() -> crate::cache::CacheManager {
    use crate::config::RedisConfig;
    
    let redis_config = RedisConfig {
        url: "redis://127.0.0.1:6379/15".to_string(),
        pool_size: 1,
        host: "127.0.0.1".to_string(),
        port: 6379,
        database: 15, // 使用专门的测试数据库
        password: None,
        connection_timeout: 5,
        default_ttl: 300,
        max_connections: 1,
    };
    
    crate::cache::CacheManager::from_config(&redis_config)
        .await
        .expect("创建测试缓存管理器失败")
}

/// 清理测试缓存
#[cfg(feature = "redis")]
pub async fn cleanup_test_cache() {
    if let Ok(cache_manager) = create_test_cache_manager().await {
        // 清理测试缓存数据库
        let _ = cache_manager.client().delete_pattern("*").await;
    }
}

/// 断言错误类型
#[macro_export]
macro_rules! assert_error_type {
    ($result:expr, $error_type:pat) => {
        match $result {
            Err($error_type) => (),
            Err(other) => panic!("Expected error type, got: {:?}", other),
            Ok(val) => panic!("Expected error, got Ok: {:?}", val),
        }
    };
}

/// 断言包含文本
#[macro_export]
macro_rules! assert_contains {
    ($text:expr, $substring:expr) => {
        assert!(
            $text.contains($substring),
            "Text '{}' does not contain '{}'",
            $text,
            $substring
        );
    };
}

/// 异步断言宏
#[macro_export]
macro_rules! assert_async {
    ($condition:expr) => {
        assert!($condition.await);
    };
    ($condition:expr, $message:expr) => {
        assert!($condition.await, $message);
    };
}

/// 测试数据库事务包装器
pub struct TestTransaction {
    pub db: DatabaseConnection,
}

impl TestTransaction {
    /// 创建新的测试事务
    pub async fn new() -> Result<Self, DbErr> {
        let db = create_test_db().await?;
        Ok(Self { db })
    }

    /// 获取数据库连接引用
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// 插入测试用户并返回 ID
    pub async fn insert_test_user(&self, fixture: crate::testing::UserFixture) -> Result<i32, DbErr> {
        use entity::users;
        use sea_orm::EntityTrait;

        let user_model = fixture.to_active_model();
        let result = users::Entity::insert(user_model)
            .exec(&self.db)
            .await?;

        Ok(result.last_insert_id)
    }

    /// 插入测试提供商类型并返回 ID
    pub async fn insert_provider_type(&self, fixture: entity::provider_types::ActiveModel) -> Result<i32, DbErr> {
        use entity::provider_types;
        use sea_orm::EntityTrait;

        let result = provider_types::Entity::insert(fixture)
            .exec(&self.db)
            .await?;

        Ok(result.last_insert_id)
    }
}

/// 性能测试辅助函数
pub struct PerformanceTest;

impl PerformanceTest {
    /// 测量函数执行时间
    pub async fn measure_async<F, Fut, T>(f: F) -> (T, std::time::Duration)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let start = std::time::Instant::now();
        let result = f().await;
        let duration = start.elapsed();
        (result, duration)
    }

    /// 测量同步函数执行时间
    pub fn measure<F, T>(f: F) -> (T, std::time::Duration)
    where
        F: FnOnce() -> T,
    {
        let start = std::time::Instant::now();
        let result = f();
        let duration = start.elapsed();
        (result, duration)
    }

    /// 基准测试
    pub async fn benchmark_async<F, Fut>(name: &str, iterations: usize, f: F)
    where
        F: Fn() -> Fut + Send + Sync,
        Fut: std::future::Future<Output = ()>,
    {
        let mut total_duration = std::time::Duration::ZERO;
        
        for _ in 0..iterations {
            let (_, duration) = Self::measure_async(&f).await;
            total_duration += duration;
        }
        
        let avg_duration = total_duration / iterations as u32;
        println!(
            "基准测试 {}: {} 次迭代, 平均耗时: {:?}",
            name, iterations, avg_duration
        );
    }
}

/// HTTP 客户端测试辅助
pub struct HttpTestClient {
    client: reqwest::Client,
    base_url: String,
}

impl HttpTestClient {
    /// 创建新的测试客户端
    pub fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.to_string(),
        }
    }

    /// 发送 GET 请求
    pub async fn get(&self, path: &str) -> reqwest::Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, path);
        self.client.get(&url).send().await
    }

    /// 发送 POST 请求
    pub async fn post<T: serde::Serialize>(&self, path: &str, body: &T) -> reqwest::Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, path);
        self.client.post(&url).json(body).send().await
    }

    /// 发送带认证的请求
    pub async fn get_with_auth(&self, path: &str, token: &str) -> reqwest::Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, path);
        self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
    }
}

/// 环境变量测试辅助
pub struct EnvTestHelper;

impl EnvTestHelper {
    /// 临时设置环境变量
    pub fn with_env<F, T>(key: &str, value: &str, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        let old_value = std::env::var(key).ok();
        std::env::set_var(key, value);
        
        let result = f();
        
        match old_value {
            Some(val) => std::env::set_var(key, val),
            None => std::env::remove_var(key),
        }
        
        result
    }

    /// 临时移除环境变量
    pub fn without_env<F, T>(key: &str, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        let old_value = std::env::var(key).ok();
        std::env::remove_var(key);
        
        let result = f();
        
        if let Some(val) = old_value {
            std::env::set_var(key, val);
        }
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_test_db() {
        init_test_env();
        let db = create_test_db().await.unwrap();
        
        // 验证数据库连接可用
        let tables = db.get_database_backend();
        assert!(!format!("{:?}", tables).is_empty());
    }

    #[tokio::test]
    async fn test_create_temp_db() {
        let (db, _temp_dir) = create_temp_db().await.unwrap();
        
        // 验证数据库连接可用
        let tables = db.get_database_backend();
        assert!(!format!("{:?}", tables).is_empty());
    }

    #[tokio::test]
    async fn test_transaction_wrapper() {
        let tx = TestTransaction::new().await.unwrap();
        
        // 插入测试用户
        let user_fixture = crate::testing::UserFixture::new();
        let user_id = tx.insert_test_user(user_fixture).await.unwrap();
        assert!(user_id > 0);
    }

    #[test]
    fn test_performance_measure() {
        let (result, duration) = PerformanceTest::measure(|| {
            std::thread::sleep(std::time::Duration::from_millis(10));
            42
        });
        
        assert_eq!(result, 42);
        assert!(duration >= std::time::Duration::from_millis(10));
    }

    #[tokio::test]
    async fn test_performance_measure_async() {
        let (result, duration) = PerformanceTest::measure_async(|| async {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            42
        }).await;
        
        assert_eq!(result, 42);
        assert!(duration >= std::time::Duration::from_millis(10));
    }

    #[test]
    fn test_env_helper() {
        let result = EnvTestHelper::with_env("TEST_VAR", "test_value", || {
            std::env::var("TEST_VAR").unwrap()
        });
        
        assert_eq!(result, "test_value");
        assert!(std::env::var("TEST_VAR").is_err());
    }

    #[test]
    fn test_assert_macros() {
        // 测试 assert_contains
        assert_contains!("hello world", "world");
        
        // 测试 assert_error_type
        let result: Result<(), crate::error::ProxyError> = Err(crate::error::ProxyError::config("test"));
        assert_error_type!(result, crate::error::ProxyError::Config { .. });
    }
}