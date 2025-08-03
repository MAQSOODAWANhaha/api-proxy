//! # 缓存系统集成测试
//!
//! 测试内存缓存和统一缓存管理器的功能

use api_proxy::testing::*;
use api_proxy::cache::{UnifiedCacheManager, CacheKeyBuilder};
use api_proxy::config::{CacheConfig, CacheType};
use std::time::Duration;
use tokio::time::sleep;
use serde_json::json;

/// 缓存系统测试套件
struct CacheTestSuite {
    cache_manager: UnifiedCacheManager,
}

impl CacheTestSuite {
    /// 创建测试环境
    async fn setup() -> Result<Self, Box<dyn std::error::Error>> {
        init_test_env();
        
        // 使用内存缓存避免依赖外部Redis
        let cache_config = CacheConfig {
            cache_type: CacheType::Memory,
            memory_max_entries: 1000,
            default_ttl: 300, // 5分钟
            enabled: true,
        };
        
        let cache_manager = UnifiedCacheManager::new(&cache_config, "")?;
        
        Ok(Self { cache_manager })
    }
}

#[tokio::test]
async fn test_basic_cache_operations() {
    let suite = CacheTestSuite::setup().await
        .expect("设置缓存测试环境失败");

    println!("💾 测试基础缓存操作");

    let key = CacheKeyBuilder::new()
        .category("test")
        .identifier("basic_test")
        .build();

    let test_data = json!({
        "id": 12345,
        "name": "test_data", 
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    // 测试设置缓存
    let set_result = suite.cache_manager.set(&key, &test_data, None).await;
    assert!(set_result.is_ok(), "设置缓存失败: {:?}", set_result);
    println!("✅ 缓存设置成功");

    // 测试获取缓存
    let get_result: Result<Option<serde_json::Value>, _> = suite.cache_manager.get(&key).await;
    match get_result {
        Ok(Some(cached_data)) => {
            assert_eq!(cached_data["id"], 12345);
            assert_eq!(cached_data["name"], "test_data");
            println!("✅ 缓存获取成功");
        }
        Ok(None) => panic!("缓存数据未找到"),
        Err(e) => panic!("获取缓存失败: {}", e),
    }

    // 测试缓存存在性检查
    let exists_result = suite.cache_manager.exists(&key).await;
    assert!(exists_result.unwrap_or(false), "缓存存在性检查失败");
    println!("✅ 缓存存在性检查通过");

    // 测试删除缓存
    let delete_result = suite.cache_manager.delete(&key).await;
    assert!(delete_result.unwrap_or(false), "删除缓存失败");
    println!("✅ 缓存删除成功");

    // 验证删除后不存在
    let after_delete: Result<Option<serde_json::Value>, _> = suite.cache_manager.get(&key).await;
    assert!(after_delete.unwrap().is_none(), "删除后缓存仍然存在");
    println!("✅ 删除验证通过");
}

#[tokio::test]
async fn test_cache_ttl() {
    let suite = CacheTestSuite::setup().await
        .expect("设置缓存测试环境失败");

    println!("⏰ 测试缓存TTL功能");

    let key = CacheKeyBuilder::new()
        .category("test")
        .identifier("ttl_test")
        .build();

    let test_data = "short_lived_data";

    // 设置短期TTL (1秒)
    let set_result = suite.cache_manager.set(&key, &test_data, Some(1)).await;
    assert!(set_result.is_ok(), "设置短期TTL缓存失败");
    println!("✅ 短期TTL缓存设置成功");

    // 立即获取应该成功
    let immediate_get: Result<Option<String>, _> = suite.cache_manager.get(&key).await;
    assert!(immediate_get.unwrap().is_some(), "立即获取TTL缓存失败");
    println!("✅ 立即获取TTL缓存成功");

    // 等待TTL过期
    sleep(Duration::from_secs(2)).await;

    // 过期后获取应该返回None
    let expired_get: Result<Option<String>, _> = suite.cache_manager.get(&key).await;
    assert!(expired_get.unwrap().is_none(), "过期缓存仍然存在");
    println!("✅ TTL过期验证通过");
}

#[tokio::test]
async fn test_cache_key_builder() {
    let suite = CacheTestSuite::setup().await
        .expect("设置缓存测试环境失败");

    println!("🔑 测试缓存键构建器");

    // 测试用户会话缓存键
    let session_key = CacheKeyBuilder::new()
        .category("user_session")
        .user_id(1001)
        .identifier("session_abc123")
        .build();

    // 测试API健康状态缓存键
    let health_key = CacheKeyBuilder::new()
        .category("api_health")
        .provider("openai")
        .api_name("chat")
        .build();

    // 测试速率限制缓存键
    let rate_limit_key = CacheKeyBuilder::new()
        .category("rate_limit")
        .user_id(1001)
        .endpoint("/v1/chat/completions")
        .build();

    let test_data = "cache_key_test";

    // 设置多个不同类型的缓存
    let session_result = suite.cache_manager.set(&session_key, &test_data, None).await;
    let health_result = suite.cache_manager.set(&health_key, &test_data, None).await;
    let rate_result = suite.cache_manager.set(&rate_limit_key, &test_data, None).await;

    assert!(session_result.is_ok(), "用户会话缓存设置失败");
    assert!(health_result.is_ok(), "API健康缓存设置失败");
    assert!(rate_result.is_ok(), "速率限制缓存设置失败");

    println!("✅ 多种类型缓存键设置成功");

    // 验证所有缓存都可以独立获取
    let session_get: Result<Option<String>, _> = suite.cache_manager.get(&session_key).await;
    let health_get: Result<Option<String>, _> = suite.cache_manager.get(&health_key).await;
    let rate_get: Result<Option<String>, _> = suite.cache_manager.get(&rate_limit_key).await;

    assert!(session_get.unwrap().is_some(), "用户会话缓存获取失败");
    assert!(health_get.unwrap().is_some(), "API健康缓存获取失败");
    assert!(rate_get.unwrap().is_some(), "速率限制缓存获取失败");

    println!("✅ 多种类型缓存键独立获取成功");
}

#[tokio::test]
async fn test_cache_strategies() {
    let suite = CacheTestSuite::setup().await
        .expect("设置缓存测试环境失败");

    println!("📋 测试不同缓存策略");

    // 短期缓存策略 (用户会话)
    let short_term_key = CacheKeyBuilder::new()
        .category("user_session")
        .user_id(2001)
        .identifier("session_short")
        .build();

    let session_data = json!({
        "user_id": 2001,
        "session_token": "short_session_token",
        "expires_at": chrono::Utc::now().to_rfc3339()
    });

    // 中期缓存策略 (API健康状态)
    let medium_term_key = CacheKeyBuilder::new()
        .category("api_health")
        .provider("openai")
        .api_name("completions")
        .build();

    let health_data = json!({
        "status": "healthy",
        "response_time_ms": 120,
        "last_check": chrono::Utc::now().to_rfc3339()
    });

    // 长期缓存策略 (配置数据)
    let long_term_key = CacheKeyBuilder::new()
        .category("config")
        .identifier("app_settings")
        .build();

    let config_data = json!({
        "version": "1.0.0",
        "debug": false,
        "max_tokens": 4096
    });

    // 使用不同TTL设置缓存
    let short_result = suite.cache_manager.set(&short_term_key, &session_data, Some(60)).await;    // 1分钟
    let medium_result = suite.cache_manager.set(&medium_term_key, &health_data, Some(300)).await;   // 5分钟
    let long_result = suite.cache_manager.set(&long_term_key, &config_data, Some(3600)).await;     // 1小时

    assert!(short_result.is_ok(), "短期缓存设置失败");
    assert!(medium_result.is_ok(), "中期缓存设置失败");
    assert!(long_result.is_ok(), "长期缓存设置失败");

    println!("✅ 不同缓存策略设置成功");

    // 验证所有缓存都能正确获取
    let short_get: Result<Option<serde_json::Value>, _> = suite.cache_manager.get(&short_term_key).await;
    let medium_get: Result<Option<serde_json::Value>, _> = suite.cache_manager.get(&medium_term_key).await;
    let long_get: Result<Option<serde_json::Value>, _> = suite.cache_manager.get(&long_term_key).await;

    assert!(short_get.unwrap().is_some(), "短期缓存获取失败");
    assert!(medium_get.unwrap().is_some(), "中期缓存获取失败");
    assert!(long_get.unwrap().is_some(), "长期缓存获取失败");

    println!("✅ 不同缓存策略验证通过");
}

#[tokio::test]
async fn test_batch_cache_operations() {
    let suite = CacheTestSuite::setup().await
        .expect("设置缓存测试环境失败");

    println!("📦 测试批量缓存操作");

    // 创建多个缓存键值对
    let test_cases = vec![
        ("batch_test_1", json!({"id": 1, "data": "first"})),
        ("batch_test_2", json!({"id": 2, "data": "second"})),
        ("batch_test_3", json!({"id": 3, "data": "third"})),
        ("batch_test_4", json!({"id": 4, "data": "fourth"})),
        ("batch_test_5", json!({"id": 5, "data": "fifth"})),
    ];

    // 批量设置缓存
    for (identifier, data) in &test_cases {
        let key = CacheKeyBuilder::new()
            .category("batch_test")
            .identifier(identifier)
            .build();

        let set_result = suite.cache_manager.set(&key, data, None).await;
        assert!(set_result.is_ok(), "批量设置缓存失败: {}", identifier);
    }

    println!("✅ 批量缓存设置完成");

    // 批量获取并验证
    for (identifier, expected_data) in &test_cases {
        let key = CacheKeyBuilder::new()
            .category("batch_test")
            .identifier(identifier)
            .build();

        let get_result: Result<Option<serde_json::Value>, _> = suite.cache_manager.get(&key).await;
        match get_result {
            Ok(Some(cached_data)) => {
                assert_eq!(cached_data["id"], expected_data["id"]);
                assert_eq!(cached_data["data"], expected_data["data"]);
            }
            Ok(None) => panic!("批量缓存数据未找到: {}", identifier),
            Err(e) => panic!("批量获取缓存失败: {} - {}", identifier, e),
        }
    }

    println!("✅ 批量缓存获取验证通过");

    // 批量删除
    for (identifier, _) in &test_cases {
        let key = CacheKeyBuilder::new()
            .category("batch_test")
            .identifier(identifier)
            .build();

        let delete_result = suite.cache_manager.delete(&key).await;
        assert!(delete_result.unwrap_or(false), "批量删除缓存失败: {}", identifier);
    }

    println!("✅ 批量缓存删除完成");
}

#[tokio::test]
async fn test_cache_performance() {
    let suite = CacheTestSuite::setup().await
        .expect("设置缓存测试环境失败");

    println!("⚡ 测试缓存性能");

    let iterations = 100;
    let test_data = json!({
        "performance_test": true,
        "data_size": "medium",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    // 测试设置性能
    let (_, set_duration) = PerformanceTest::measure_async(|| async {
        for i in 0..iterations {
            let key = CacheKeyBuilder::new()
                .category("performance")
                .identifier(&format!("perf_test_{}", i))
                .build();

            let _ = suite.cache_manager.set(&key, &test_data, None).await;
        }
    }).await;

    println!("✅ 设置性能: {} 次操作耗时 {:?}", iterations, set_duration);

    // 测试获取性能
    let (_, get_duration) = PerformanceTest::measure_async(|| async {
        for i in 0..iterations {
            let key = CacheKeyBuilder::new()
                .category("performance")
                .identifier(&format!("perf_test_{}", i))
                .build();

            let _: Result<Option<serde_json::Value>, _> = suite.cache_manager.get(&key).await;
        }
    }).await;

    println!("✅ 获取性能: {} 次操作耗时 {:?}", iterations, get_duration);

    // 性能验证 (内存缓存应该很快)
    assert!(set_duration < Duration::from_millis(1000), "设置操作过慢");
    assert!(get_duration < Duration::from_millis(500), "获取操作过慢");

    println!("✅ 缓存性能测试通过");
}

#[tokio::test]
async fn test_cache_integration() {
    let suite = CacheTestSuite::setup().await
        .expect("设置缓存测试环境失败");

    println!("🚀 开始缓存系统完整集成测试");

    // 模拟真实使用场景
    let user_id = 3001;
    let provider = "openai";
    let api_endpoint = "/v1/chat/completions";

    // 1. 用户会话缓存
    let session_key = CacheKeyBuilder::new()
        .category("user_session")
        .user_id(user_id)
        .identifier("integration_session")
        .build();

    let session_data = json!({
        "user_id": user_id,
        "authenticated": true,
        "permissions": ["use_api", "view_stats"]
    });

    suite.cache_manager.set(&session_key, &session_data, Some(3600)).await
        .expect("设置用户会话缓存失败");
    println!("   ✓ 用户会话缓存设置成功");

    // 2. API健康状态缓存
    let health_key = CacheKeyBuilder::new()
        .category("api_health")
        .provider(provider)
        .api_name("chat")
        .build();

    let health_data = json!({
        "status": "healthy",
        "response_time_ms": 180,
        "success_rate": 0.99,
        "last_check": chrono::Utc::now().to_rfc3339()
    });

    suite.cache_manager.set(&health_key, &health_data, Some(300)).await
        .expect("设置API健康缓存失败");
    println!("   ✓ API健康状态缓存设置成功");

    // 3. 速率限制缓存
    let rate_key = CacheKeyBuilder::new()
        .category("rate_limit")
        .user_id(user_id)
        .endpoint(api_endpoint)
        .build();

    let rate_data = json!({
        "requests_count": 15,
        "window_start": chrono::Utc::now().to_rfc3339(),
        "limit": 100
    });

    suite.cache_manager.set(&rate_key, &rate_data, Some(60)).await
        .expect("设置速率限制缓存失败");
    println!("   ✓ 速率限制缓存设置成功");

    // 4. 后端选择缓存
    let backend_key = CacheKeyBuilder::new()
        .category("backend_selection")
        .user_id(user_id)
        .provider(provider)
        .build();

    let backend_data = json!({
        "selected_backend": "backend_key_1",
        "last_used": chrono::Utc::now().to_rfc3339(),
        "weight": 5
    });

    suite.cache_manager.set(&backend_key, &backend_data, Some(120)).await
        .expect("设置后端选择缓存失败");
    println!("   ✓ 后端选择缓存设置成功");

    // 验证所有缓存都可以正确获取
    let session_check: Result<Option<serde_json::Value>, _> = suite.cache_manager.get(&session_key).await;
    let health_check: Result<Option<serde_json::Value>, _> = suite.cache_manager.get(&health_key).await;
    let rate_check: Result<Option<serde_json::Value>, _> = suite.cache_manager.get(&rate_key).await;
    let backend_check: Result<Option<serde_json::Value>, _> = suite.cache_manager.get(&backend_key).await;

    assert!(session_check.unwrap().is_some(), "用户会话缓存验证失败");
    assert!(health_check.unwrap().is_some(), "API健康缓存验证失败");
    assert!(rate_check.unwrap().is_some(), "速率限制缓存验证失败");
    assert!(backend_check.unwrap().is_some(), "后端选择缓存验证失败");

    println!("🎉 缓存系统完整集成测试通过！");
    println!("✨ 验证完成的功能：");
    println!("   - ✅ 用户会话缓存");
    println!("   - ✅ API健康状态缓存");
    println!("   - ✅ 速率限制缓存");
    println!("   - ✅ 后端选择缓存");
    println!("   - ✅ TTL过期管理");
    println!("   - ✅ 批量操作");
    println!("   - ✅ 性能优化");
}