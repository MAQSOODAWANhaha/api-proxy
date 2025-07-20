//! # 缓存性能基准测试

use api_proxy::cache::{CacheKeyBuilder, CacheManager};
use api_proxy::config::RedisConfig;
use criterion::{black_box, Criterion};
use std::collections::HashMap;
use tokio::runtime::Runtime;

/// 缓存操作基准测试
pub fn cache_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    // 创建测试缓存管理器
    let cache_manager = rt.block_on(async {
        let redis_config = RedisConfig {
            url: "redis://127.0.0.1:6379/15".to_string(),
            pool_size: 10,
            host: "127.0.0.1".to_string(),
            port: 6379,
            database: 15,
            password: None,
            connection_timeout: 5,
            default_ttl: 300,
            max_connections: 10,
        };
        
        CacheManager::from_config(&redis_config).await.unwrap()
    });

    // 基准测试：缓存设置操作
    c.bench_function("cache_set", |b| {
        b.to_async(&rt).iter(|| async {
            let key = CacheKeyBuilder::config(&format!("test_key_{}", fastrand::u32(..)));
            let value = HashMap::from([
                ("data".to_string(), "test_value".to_string()),
                ("timestamp".to_string(), chrono::Utc::now().to_rfc3339()),
            ]);
            
            cache_manager.set_with_strategy(black_box(&key), black_box(&value)).await.unwrap();
        });
    });

    // 基准测试：缓存获取操作
    c.bench_function("cache_get", |b| {
        // 预设一些缓存数据
        rt.block_on(async {
            for i in 0..100 {
                let key = CacheKeyBuilder::config(&format!("bench_key_{}", i));
                let value = HashMap::from([
                    ("id".to_string(), i.to_string()),
                    ("data".to_string(), "benchmark_data".to_string()),
                ]);
                cache_manager.set_with_strategy(&key, &value).await.unwrap();
            }
        });
        
        b.to_async(&rt).iter(|| async {
            let key_id = fastrand::usize(0..100);
            let key = CacheKeyBuilder::config(&format!("bench_key_{}", key_id));
            
            let _result: Option<HashMap<String, String>> = cache_manager
                .get(black_box(&key))
                .await
                .unwrap();
        });
    });

    // 基准测试：批量缓存操作
    c.bench_function("cache_batch_operations", |b| {
        b.to_async(&rt).iter(|| async {
            let batch_size = black_box(10);
            
            // 批量设置
            for i in 0..batch_size {
                let key = CacheKeyBuilder::user_session(1000, &format!("session_{}", i));
                let value = format!("session_data_{}", i);
                cache_manager.set_with_strategy(&key, &value).await.unwrap();
            }
            
            // 批量获取
            for i in 0..batch_size {
                let key = CacheKeyBuilder::user_session(1000, &format!("session_{}", i));
                let _result: Option<String> = cache_manager.get(&key).await.unwrap();
            }
        });
    });

    // 基准测试：不同 TTL 策略的性能
    c.bench_function("cache_ttl_strategies", |b| {
        b.to_async(&rt).iter(|| async {
            let strategies = [
                CacheKeyBuilder::user_session(1, "short_ttl"),      // 短期缓存
                CacheKeyBuilder::config("medium_ttl"),              // 长期缓存
                CacheKeyBuilder::api_health("openai", "health"),    // 中期缓存
            ];
            
            for key in black_box(&strategies) {
                let value = "benchmark_value";
                cache_manager.set_with_strategy(key, &value).await.unwrap();
            }
        });
    });
}