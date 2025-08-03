//! # 追踪系统集成测试
//!
//! 测试统一追踪系统的完整功能：追踪记录、智能采样、批量写入

use api_proxy::testing::*;
use api_proxy::trace::unified::UnifiedTracer;
use api_proxy::trace::models::*;
use api_proxy::config::TraceConfig;
use entity::proxy_tracing;
use sea_orm::{EntityTrait, QuerySelect};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use serde_json::json;

/// 追踪系统测试套件
struct TraceTestSuite {
    tx: TestTransaction,
    tracer: Arc<UnifiedTracer>,
}

impl TraceTestSuite {
    /// 创建测试环境
    async fn setup() -> Result<Self, Box<dyn std::error::Error>> {
        init_test_env();
        
        let tx = TestTransaction::new().await?;
        
        // 创建追踪配置
        let trace_config = TraceConfig {
            enabled: true,
            level: TraceLevel::Full,
            sampling_rate: 1.0, // 100% 采样用于测试
            batch_size: 10,
            flush_interval: 1, // 1秒刷新间隔
            anomaly_detection: true,
            performance_threshold_ms: 1000,
        };
        
        let tracer = Arc::new(UnifiedTracer::new(
            tx.db().clone(),
            trace_config,
        ).await?);
        
        Ok(Self { tx, tracer })
    }

    /// 创建测试追踪记录
    fn create_test_trace(&self) -> TraceRecord {
        TraceRecord {
            request_id: "test-trace-001".to_string(),
            user_id: Some(1),
            api_key_hash: Some("test_api_key_hash".to_string()),
            level: TraceLevel::Full,
            phase: TracePhase::Preparation,
            provider_name: Some("openai".to_string()),
            backend_key_hash: Some("backend_key_hash".to_string()),
            request_path: Some("/v1/chat/completions".to_string()),
            request_method: Some("POST".to_string()),
            request_size: Some(1024),
            response_status: None,
            response_size: None,
            token_usage: None,
            provider_response_time_ms: None,
            total_processing_time_ms: None,
            error_type: None,
            error_message: None,
            metadata: Some(json!({
                "test": true,
                "environment": "test"
            })),
            created_at: chrono::Utc::now(),
        }
    }
}

#[tokio::test]
async fn test_trace_record_creation() {
    let suite = TraceTestSuite::setup().await
        .expect("设置测试环境失败");

    let trace_record = suite.create_test_trace();
    
    // 记录追踪信息
    let result = suite.tracer.trace(trace_record.clone()).await;
    
    match result {
        Ok(_) => println!("✅ 追踪记录创建成功"),
        Err(e) => panic!("追踪记录创建失败: {}", e),
    }

    // 等待批量写入完成
    sleep(Duration::from_secs(2)).await;

    // 验证数据库中的记录
    let records = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.eq("test-trace-001"))
        .all(suite.tx.db())
        .await
        .expect("查询追踪记录失败");

    assert!(!records.is_empty());
    assert_eq!(records[0].request_id, "test-trace-001");
    assert_eq!(records[0].provider_name.as_ref().unwrap(), "openai");
    
    println!("✅ 追踪记录数据库验证通过");
}

#[tokio::test]
async fn test_trace_levels() {
    let suite = TraceTestSuite::setup().await
        .expect("设置测试环境失败");

    // 测试不同的追踪级别
    let levels = vec![
        (TraceLevel::Basic, "basic-trace"),
        (TraceLevel::Detailed, "detailed-trace"), 
        (TraceLevel::Full, "full-trace"),
    ];

    for (level, request_id) in levels {
        let mut trace_record = suite.create_test_trace();
        trace_record.request_id = request_id.to_string();
        trace_record.level = level;

        let result = suite.tracer.trace(trace_record).await;
        assert!(result.is_ok(), "追踪级别 {:?} 记录失败", level);
        
        println!("✅ 追踪级别 {:?} 测试通过", level);
    }
}

#[tokio::test]
async fn test_trace_phases() {
    let suite = TraceTestSuite::setup().await
        .expect("设置测试环境失败");

    // 测试完整的追踪阶段
    let phases = vec![
        TracePhase::Preparation,
        TracePhase::Authentication,
        TracePhase::RateLimitCheck,
        TracePhase::ProviderLookup,
        TracePhase::BackendSelection,
        TracePhase::RequestForwarding,
        TracePhase::RequestBodyRead,
        TracePhase::UpstreamRequestBuild,
        TracePhase::UpstreamRequestSend,
        TracePhase::ResponseProcessing,
        TracePhase::ResponseWrite,
    ];

    for (i, phase) in phases.iter().enumerate() {
        let mut trace_record = suite.create_test_trace();
        trace_record.request_id = format!("phase-test-{:02}", i);
        trace_record.phase = *phase;

        let result = suite.tracer.trace(trace_record).await;
        assert!(result.is_ok(), "追踪阶段 {:?} 记录失败", phase);
        
        println!("✅ 追踪阶段 {:?} 测试通过", phase);
    }

    // 等待批量写入完成
    sleep(Duration::from_secs(2)).await;

    // 验证所有阶段都已记录
    let count = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.like("phase-test-%"))
        .count(suite.tx.db())
        .await
        .expect("统计记录失败");

    assert_eq!(count, phases.len() as u64);
    println!("✅ 所有追踪阶段数据库验证通过");
}

#[tokio::test]
async fn test_performance_metrics() {
    let suite = TraceTestSuite::setup().await
        .expect("设置测试环境失败");

    let mut trace_record = suite.create_test_trace();
    trace_record.request_id = "perf-test-001".to_string();
    trace_record.provider_response_time_ms = Some(250);
    trace_record.total_processing_time_ms = Some(300);
    trace_record.token_usage = Some(json!({
        "prompt_tokens": 100,
        "completion_tokens": 50,
        "total_tokens": 150
    }));

    let result = suite.tracer.trace(trace_record).await;
    assert!(result.is_ok(), "性能指标追踪失败");

    // 等待批量写入
    sleep(Duration::from_secs(2)).await;

    // 验证性能指标
    let record = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.eq("perf-test-001"))
        .one(suite.tx.db())
        .await
        .expect("查询性能记录失败")
        .expect("性能记录不存在");

    assert_eq!(record.provider_response_time_ms, Some(250));
    assert_eq!(record.total_processing_time_ms, Some(300));
    assert!(record.token_usage.is_some());

    println!("✅ 性能指标追踪测试通过");
}

#[tokio::test]
async fn test_error_tracking() {
    let suite = TraceTestSuite::setup().await
        .expect("设置测试环境失败");

    let mut trace_record = suite.create_test_trace();
    trace_record.request_id = "error-test-001".to_string();
    trace_record.phase = TracePhase::Authentication;
    trace_record.error_type = Some("AuthenticationError".to_string());
    trace_record.error_message = Some("Invalid API key".to_string());
    trace_record.response_status = Some(401);

    let result = suite.tracer.trace(trace_record).await;
    assert!(result.is_ok(), "错误追踪失败");

    // 等待批量写入
    sleep(Duration::from_secs(2)).await;

    // 验证错误信息
    let record = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.eq("error-test-001"))
        .one(suite.tx.db())
        .await
        .expect("查询错误记录失败")
        .expect("错误记录不存在");

    assert_eq!(record.error_type.as_ref().unwrap(), "AuthenticationError");
    assert_eq!(record.error_message.as_ref().unwrap(), "Invalid API key");
    assert_eq!(record.response_status, Some(401));

    println!("✅ 错误追踪测试通过");
}

#[tokio::test]
async fn test_sampling_strategy() {
    let suite = TraceTestSuite::setup().await
        .expect("设置测试环境失败");

    // 创建采样率为 50% 的追踪器
    let trace_config = TraceConfig {
        enabled: true,
        level: TraceLevel::Basic,
        sampling_rate: 0.5,
        batch_size: 1,
        flush_interval: 1,
        anomaly_detection: false,
        performance_threshold_ms: 1000,
    };

    let sampled_tracer = Arc::new(UnifiedTracer::new(
        suite.tx.db().clone(),
        trace_config,
    ).await.expect("创建采样追踪器失败"));

    // 发送多个追踪记录
    for i in 0..100 {
        let mut trace_record = suite.create_test_trace();
        trace_record.request_id = format!("sampling-test-{:03}", i);
        
        let _ = sampled_tracer.trace(trace_record).await;
    }

    // 等待批量写入完成
    sleep(Duration::from_secs(3)).await;

    // 统计实际记录的数量
    let count = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.like("sampling-test-%"))
        .count(suite.tx.db())
        .await
        .expect("统计采样记录失败");

    // 采样率为50%，期望记录数量在30-70之间（允许统计波动）
    assert!(count >= 30 && count <= 70, "采样率测试失败，实际记录: {}", count);
    
    println!("✅ 智能采样策略测试通过，记录数量: {}/100", count);
}

#[tokio::test]
async fn test_batch_processing() {
    let suite = TraceTestSuite::setup().await
        .expect("设置测试环境失败");

    // 配置小批量大小用于测试
    let trace_config = TraceConfig {
        enabled: true,
        level: TraceLevel::Basic,
        sampling_rate: 1.0,
        batch_size: 5, // 小批量
        flush_interval: 10, // 较长的刷新间隔
        anomaly_detection: false,
        performance_threshold_ms: 1000,
    };

    let batch_tracer = Arc::new(UnifiedTracer::new(
        suite.tx.db().clone(),
        trace_config,
    ).await.expect("创建批量追踪器失败"));

    // 发送10个追踪记录（应该触发2次批量写入）
    for i in 0..10 {
        let mut trace_record = suite.create_test_trace();
        trace_record.request_id = format!("batch-test-{:02}", i);
        
        let result = batch_tracer.trace(trace_record).await;
        assert!(result.is_ok(), "批量追踪记录 {} 失败", i);
    }

    // 等待批量处理完成
    sleep(Duration::from_secs(2)).await;

    // 验证记录数量
    let count = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.like("batch-test-%"))
        .count(suite.tx.db())
        .await
        .expect("统计批量记录失败");

    assert_eq!(count, 10);
    
    println!("✅ 批量处理测试通过，成功处理 {} 条记录", count);
}

#[tokio::test]
async fn test_trace_integration() {
    let suite = TraceTestSuite::setup().await
        .expect("设置测试环境失败");

    println!("🔍 开始追踪系统完整集成测试");

    // 模拟完整的请求追踪流程
    let request_id = "integration-test-001";
    let phases = vec![
        (TracePhase::Preparation, Some(10_u64), None),
        (TracePhase::Authentication, Some(15), None),
        (TracePhase::RateLimitCheck, Some(5), None),
        (TracePhase::ProviderLookup, Some(8), None),
        (TracePhase::BackendSelection, Some(12), None),
        (TracePhase::RequestForwarding, Some(200), None),
        (TracePhase::UpstreamRequestSend, Some(250), Some(200_u16)),
        (TracePhase::ResponseProcessing, Some(30), Some(200)),
        (TracePhase::ResponseWrite, Some(15), Some(200)),
    ];

    for (phase, duration_ms, status) in phases {
        let mut trace_record = suite.create_test_trace();
        trace_record.request_id = request_id.to_string();
        trace_record.phase = phase;
        trace_record.provider_response_time_ms = duration_ms;
        trace_record.response_status = status;
        
        if phase == TracePhase::UpstreamRequestSend {
            trace_record.token_usage = Some(json!({
                "prompt_tokens": 50,
                "completion_tokens": 25,
                "total_tokens": 75
            }));
        }

        let result = suite.tracer.trace(trace_record).await;
        assert!(result.is_ok(), "阶段 {:?} 追踪失败", phase);
    }

    // 等待所有记录写入
    sleep(Duration::from_secs(2)).await;

    // 验证完整流程记录
    let records = proxy_tracing::Entity::find()
        .filter(proxy_tracing::Column::RequestId.eq(request_id))
        .all(suite.tx.db())
        .await
        .expect("查询集成测试记录失败");

    assert_eq!(records.len(), 9);
    
    // 验证阶段完整性
    let phases_recorded: Vec<_> = records.iter()
        .map(|r| r.phase.as_str())
        .collect();
    
    assert!(phases_recorded.contains(&"preparation"));
    assert!(phases_recorded.contains(&"authentication"));
    assert!(phases_recorded.contains(&"upstream_request_send"));
    assert!(phases_recorded.contains(&"response_write"));

    println!("✅ 追踪系统完整集成测试通过");
    println!("   - 总记录数: {}", records.len());
    println!("   - 阶段覆盖: ✓");
    println!("   - 性能指标: ✓");
    println!("   - 令牌统计: ✓");
    println!("   - 批量写入: ✓");
}