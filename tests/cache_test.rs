//! # 缓存模块集成测试
//!
//! 测试 Redis 缓存的集成功能

use api_proxy::cache::{CacheKeyBuilder, CacheManager};
use api_proxy::config::load_config;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestData {
    id: u32,
    name: String,
    values: HashMap<String, String>,
}

#[tokio::test]
#[ignore] // 需要 Redis 服务器运行
async fn test_cache_integration() {
    // 加载配置
    let config = load_config().expect("加载配置失败");
    
    // 创建缓存管理器
    let cache_manager = CacheManager::from_config(&config.redis)
        .await
        .expect("创建缓存管理器失败");
    
    // 测试连接
    cache_manager.ping().await.expect("Redis 连接测试失败");
    println!("✅ Redis 连接测试成功");
    
    // 测试基础缓存操作
    test_basic_cache_operations(&cache_manager).await;
    
    // 测试缓存策略
    test_cache_strategies(&cache_manager).await;
    
    // 测试缓存键管理
    test_cache_key_management(&cache_manager).await;
    
    println!("✅ 所有缓存测试通过");
}

async fn test_basic_cache_operations(cache_manager: &CacheManager) {
    println!("🧪 测试基础缓存操作...");
    
    // 准备测试数据
    let mut test_values = HashMap::new();
    test_values.insert("env".to_string(), "development".to_string());
    test_values.insert("version".to_string(), "0.1.0".to_string());
    
    let test_data = TestData {
        id: 12345,
        name: "test_config".to_string(),
        values: test_values,
    };
    
    // 测试配置缓存
    let config_key = CacheKeyBuilder::config("app_settings");
    
    // 设置缓存
    cache_manager
        .set_with_strategy(&config_key, &test_data)
        .await
        .expect("设置缓存失败");
    
    // 获取缓存
    let retrieved: Option<TestData> = cache_manager
        .get(&config_key)
        .await
        .expect("获取缓存失败");
    
    assert_eq!(retrieved, Some(test_data.clone()));
    println!("  ✓ 配置缓存设置和获取成功");
    
    // 检查缓存存在性
    let exists = cache_manager
        .exists(&config_key)
        .await
        .expect("检查缓存存在性失败");
    assert!(exists);
    println!("  ✓ 缓存存在性检查成功");
    
    // 删除缓存
    let deleted = cache_manager
        .delete(&config_key)
        .await
        .expect("删除缓存失败");
    assert!(deleted);
    
    // 验证删除结果
    let after_delete: Option<TestData> = cache_manager
        .get(&config_key)
        .await
        .expect("获取缓存失败");
    assert_eq!(after_delete, None);
    println!("  ✓ 缓存删除成功");
}

async fn test_cache_strategies(&cache_manager: &CacheManager) {
    println!("🧪 测试缓存策略...");
    
    // 测试用户会话缓存（短期）
    let session_key = CacheKeyBuilder::user_session(1001, "session_abc123");
    let session_data = "user_session_data".to_string();
    
    cache_manager
        .set_with_strategy(&session_key, &session_data)
        .await
        .expect("设置会话缓存失败");
    
    let retrieved_session: Option<String> = cache_manager
        .get(&session_key)
        .await
        .expect("获取会话缓存失败");
    assert_eq!(retrieved_session, Some(session_data));
    println!("  ✓ 用户会话缓存策略测试成功");
    
    // 测试API健康状态缓存（中期）
    let health_key = CacheKeyBuilder::api_health("openai", "chat");
    let health_data = HashMap::from([
        ("status".to_string(), "healthy".to_string()),
        ("latency_ms".to_string(), "120".to_string()),
    ]);
    
    cache_manager
        .set_with_strategy(&health_key, &health_data)
        .await
        .expect("设置健康状态缓存失败");
    
    let retrieved_health: Option<HashMap<String, String>> = cache_manager
        .get(&health_key)
        .await
        .expect("获取健康状态缓存失败");
    assert_eq!(retrieved_health, Some(health_data));
    println!("  ✓ API健康状态缓存策略测试成功");
    
    // 清理测试数据
    cache_manager.delete(&session_key).await.ok();
    cache_manager.delete(&health_key).await.ok();
}

async fn test_cache_key_management(&cache_manager: &CacheManager) {
    println!("🧪 测试缓存键管理...");
    
    let user_id = 2001;
    
    // 创建多个用户相关的缓存
    let session1_key = CacheKeyBuilder::user_session(user_id, "session_1");
    let session2_key = CacheKeyBuilder::user_session(user_id, "session_2");
    let api_key = CacheKeyBuilder::user_api_key(user_id, 101);
    let stats_key = CacheKeyBuilder::daily_stats(user_id, "2024-01-01");
    let rate_limit_key = CacheKeyBuilder::rate_limit(user_id, "/api/v1/chat");
    
    let test_value = "test_data".to_string();
    
    // 设置所有缓存
    cache_manager.set_with_strategy(&session1_key, &test_value).await.expect("设置缓存失败");
    cache_manager.set_with_strategy(&session2_key, &test_value).await.expect("设置缓存失败");
    cache_manager.set_with_strategy(&api_key, &test_value).await.expect("设置缓存失败");
    cache_manager.set_with_strategy(&stats_key, &test_value).await.expect("设置缓存失败");
    cache_manager.set_with_strategy(&rate_limit_key, &test_value).await.expect("设置缓存失败");
    
    // 验证所有缓存都存在
    assert!(cache_manager.exists(&session1_key).await.expect("检查缓存失败"));
    assert!(cache_manager.exists(&session2_key).await.expect("检查缓存失败"));
    assert!(cache_manager.exists(&api_key).await.expect("检查缓存失败"));
    assert!(cache_manager.exists(&stats_key).await.expect("检查缓存失败"));
    assert!(cache_manager.exists(&rate_limit_key).await.expect("检查缓存失败"));
    println!("  ✓ 用户相关缓存创建成功");
    
    // 批量清理用户缓存
    let deleted_count = cache_manager
        .clear_user_cache(user_id)
        .await
        .expect("清理用户缓存失败");
    
    assert!(deleted_count > 0);
    println!("  ✓ 用户缓存批量清理成功，删除 {} 个缓存项", deleted_count);
    
    // 验证缓存已被删除
    assert!(!cache_manager.exists(&session1_key).await.expect("检查缓存失败"));
    assert!(!cache_manager.exists(&session2_key).await.expect("检查缓存失败"));
    println!("  ✓ 缓存清理验证成功");
}

#[tokio::test]
#[ignore]
async fn test_cache_decorator() {
    println!("🧪 测试缓存装饰器...");
    
    let config = load_config().expect("加载配置失败");
    let cache_manager = CacheManager::from_config(&config.redis)
        .await
        .expect("创建缓存管理器失败");
    
    let key = CacheKeyBuilder::custom("decorator_test", "expensive_computation");
    
    // 清理可能存在的缓存
    cache_manager.delete(&key).await.ok();
    
    let decorator = api_proxy::cache::CacheDecorator::new(&cache_manager, key.clone());
    
    let mut call_count = 0;
    
    // 第一次调用应该执行计算函数
    let result1 = decorator.get_or_compute(|| {
        call_count += 1;
        async {
            // 模拟昂贵的计算
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            Ok("expensive_result".to_string())
        }
    }).await.expect("计算失败");
    
    assert_eq!(call_count, 1);
    assert_eq!(result1, "expensive_result");
    println!("  ✓ 首次计算并缓存成功");
    
    // 第二次调用应该从缓存获取，不执行计算函数
    let result2 = decorator.get_or_compute(|| {
        call_count += 1;
        async {
            panic!("不应该被调用 - 值应该已被缓存");
        }
    }).await.expect("获取缓存失败");
    
    assert_eq!(call_count, 1); // 确保计算函数没有被再次调用
    assert_eq!(result2, "expensive_result");
    println!("  ✓ 缓存命中，避免重复计算成功");
    
    // 清理测试数据
    cache_manager.delete(&key).await.ok();
    println!("✅ 缓存装饰器测试通过");
}