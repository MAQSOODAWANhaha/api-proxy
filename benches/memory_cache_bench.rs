//! # 内存缓存性能基准测试
//!
//! 测试统一缓存管理器的内存缓存实现性能

use api_proxy::cache::{UnifiedCacheManager, CacheKeyBuilder};
use api_proxy::config::{CacheConfig, CacheType};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use tokio::runtime::Runtime;
use serde_json::json;

/// 创建测试用的内存缓存管理器
fn create_cache_manager() -> UnifiedCacheManager {
    let cache_config = CacheConfig {
        cache_type: CacheType::Memory,
        memory_max_entries: 10000, // 足够大的容量用于基准测试
        default_ttl: 3600,         // 1小时TTL
        enabled: true,
    };
    
    UnifiedCacheManager::new(&cache_config, "")
        .expect("创建内存缓存管理器失败")
}

/// 缓存设置操作基准测试
fn bench_cache_set(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let cache_manager = create_cache_manager();
    
    let test_data = json!({
        "id": 12345,
        "name": "benchmark_test",
        "data": vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    c.bench_function("cache_set_operation", |b| {
        b.to_async(&rt).iter(|| async {
            let key = CacheKeyBuilder::new()
                .category("benchmark")
                .identifier(&format!("set_test_{}", fastrand::u32(..)))
                .build();
            
            cache_manager.set(black_box(&key), black_box(&test_data), None).await.unwrap();
        });
    });
}

/// 缓存获取操作基准测试
fn bench_cache_get(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let cache_manager = create_cache_manager();
    
    // 预设缓存数据
    rt.block_on(async {
        let test_data = json!({"benchmark": "get_test"});
        
        for i in 0..1000 {
            let key = CacheKeyBuilder::new()
                .category("benchmark")
                .identifier(&format!("get_test_{}", i))
                .build();
            
            cache_manager.set(&key, &test_data, None).await.unwrap();
        }
    });

    c.bench_function("cache_get_operation", |b| {
        b.to_async(&rt).iter(|| async {
            let key_id = fastrand::usize(0..1000);
            let key = CacheKeyBuilder::new()
                .category("benchmark")
                .identifier(&format!("get_test_{}", key_id))
                .build();
            
            let _result: Option<serde_json::Value> = cache_manager
                .get(black_box(&key))
                .await
                .unwrap();
        });
    });
}

/// 缓存存在性检查基准测试
fn bench_cache_exists(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let cache_manager = create_cache_manager();
    
    // 预设缓存数据
    rt.block_on(async {
        let test_data = json!({"benchmark": "exists_test"});
        
        for i in 0..500 {
            let key = CacheKeyBuilder::new()
                .category("benchmark")
                .identifier(&format!("exists_test_{}", i))
                .build();
            
            cache_manager.set(&key, &test_data, None).await.unwrap();
        }
    });

    c.bench_function("cache_exists_operation", |b| {
        b.to_async(&rt).iter(|| async {
            let key_id = fastrand::usize(0..1000); // 包含不存在的键
            let key = CacheKeyBuilder::new()
                .category("benchmark")
                .identifier(&format!("exists_test_{}", key_id))
                .build();
            
            let _exists = cache_manager.exists(black_box(&key)).await.unwrap();
        });
    });
}

/// 缓存删除操作基准测试
fn bench_cache_delete(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("cache_delete_operation", |b| {
        b.iter_batched(
            || {
                // 为每次迭代创建新的缓存管理器和数据
                let cache_manager = create_cache_manager();
                let key = CacheKeyBuilder::new()
                    .category("benchmark")
                    .identifier(&format!("delete_test_{}", fastrand::u32(..)))
                    .build();
                let test_data = json!({"benchmark": "delete_test"});
                
                // 预设数据
                rt.block_on(async {
                    cache_manager.set(&key, &test_data, None).await.unwrap();
                });
                
                (cache_manager, key)
            },
            |(cache_manager, key)| {
                rt.block_on(async {
                    cache_manager.delete(black_box(&key)).await.unwrap();
                });
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

/// 不同数据大小的缓存性能测试
fn bench_cache_data_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let cache_manager = create_cache_manager();
    
    let mut group = c.benchmark_group("cache_data_sizes");
    
    // 测试不同大小的数据
    for size in [10, 100, 1000, 10000].iter() {
        let test_data = json!({
            "size": size,
            "data": vec![42; *size]
        });
        
        group.bench_with_input(BenchmarkId::new("set", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let key = CacheKeyBuilder::new()
                    .category("benchmark")
                    .identifier(&format!("size_test_{}_{}", size, fastrand::u32(..)))
                    .build();
                
                cache_manager.set(black_box(&key), black_box(&test_data), None).await.unwrap();
            });
        });
    }
    
    group.finish();
}

/// 并发缓存操作基准测试
fn bench_cache_concurrent(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let cache_manager = create_cache_manager();
    
    c.bench_function("cache_concurrent_operations", |b| {
        b.to_async(&rt).iter(|| async {
            let tasks: Vec<_> = (0..10).map(|i| {
                let cache_manager = &cache_manager;
                async move {
                    let key = CacheKeyBuilder::new()
                        .category("benchmark")
                        .identifier(&format!("concurrent_test_{}_{}", i, fastrand::u32(..)))
                        .build();
                    
                    let test_data = json!({"concurrent": i});
                    
                    // 设置缓存
                    cache_manager.set(&key, &test_data, None).await.unwrap();
                    
                    // 获取缓存
                    let _result: Option<serde_json::Value> = cache_manager.get(&key).await.unwrap();
                    
                    // 检查存在性
                    let _exists = cache_manager.exists(&key).await.unwrap();
                }
            }).collect();
            
            futures::future::join_all(black_box(tasks)).await;
        });
    });
}

/// 缓存命中率基准测试
fn bench_cache_hit_rate(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let cache_manager = create_cache_manager();
    
    // 预设100个缓存项
    rt.block_on(async {
        let test_data = json!({"benchmark": "hit_rate_test"});
        
        for i in 0..100 {
            let key = CacheKeyBuilder::new()
                .category("benchmark")
                .identifier(&format!("hit_rate_test_{}", i))
                .build();
            
            cache_manager.set(&key, &test_data, None).await.unwrap();
        }
    });
    
    c.bench_function("cache_hit_rate_90_percent", |b| {
        b.to_async(&rt).iter(|| async {
            // 90% 的请求命中缓存
            let key_id = if fastrand::f32() < 0.9 {
                fastrand::usize(0..100)  // 命中
            } else {
                fastrand::usize(100..200) // 未命中
            };
            
            let key = CacheKeyBuilder::new()
                .category("benchmark")
                .identifier(&format!("hit_rate_test_{}", key_id))
                .build();
            
            let _result: Option<serde_json::Value> = cache_manager
                .get(black_box(&key))
                .await
                .unwrap();
        });
    });
}

/// TTL过期处理基准测试
fn bench_cache_ttl_handling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let cache_manager = create_cache_manager();
    
    c.bench_function("cache_ttl_handling", |b| {
        b.to_async(&rt).iter(|| async {
            let key = CacheKeyBuilder::new()
                .category("benchmark")
                .identifier(&format!("ttl_test_{}", fastrand::u32(..)))
                .build();
            
            let test_data = json!({"benchmark": "ttl_test"});
            
            // 设置短期TTL
            cache_manager.set(black_box(&key), black_box(&test_data), Some(1)).await.unwrap();
            
            // 立即获取
            let _result: Option<serde_json::Value> = cache_manager.get(black_box(&key)).await.unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_cache_set,
    bench_cache_get,
    bench_cache_exists,
    bench_cache_delete,
    bench_cache_data_sizes,
    bench_cache_concurrent,
    bench_cache_hit_rate,
    bench_cache_ttl_handling
);

criterion_main!(benches);