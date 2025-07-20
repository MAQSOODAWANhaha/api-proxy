//! # 集成测试
//!
//! 全面测试测试框架的各种功能

use api_proxy::testing::{
    fixtures::{UserFixture, ProviderTypeFixture, UserProviderKeyFixture, TestConfig},
    helpers::{init_test_env, create_test_db, TestTransaction, PerformanceTest},
    mocks::{MockOpenAiProvider, MockAiProvider, MockTime}
};

#[tokio::test]
async fn test_complete_testing_framework() {
    init_test_env();
    
    // 1. 测试数据库和事务
    let tx = TestTransaction::new().await.unwrap();
    
    // 2. 测试用户 fixture
    let user_fixture = UserFixture::new()
        .username("test_integration_user")
        .email("integration@test.com")
        .admin();
    
    let user_id = tx.insert_test_user(user_fixture).await.unwrap();
    assert!(user_id > 0);
    
    // 3. 测试提供商类型 fixture - 使用唯一名称避免冲突
    let mut provider_fixture = ProviderTypeFixture::openai();
    provider_fixture.name = sea_orm::Set("test_openai_provider".to_string());
    let provider_id = tx.insert_provider_type(provider_fixture).await.unwrap();
    assert!(provider_id > 0);
    
    // 4. 测试用户提供商密钥 fixture
    let key_fixture = UserProviderKeyFixture::new()
        .user_id(user_id)
        .provider_type_id(provider_id)
        .name("Integration Test Key")
        .api_key("sk-integration-test-key")
        .weight(50);
    
    let key_model = key_fixture.to_active_model();
    assert_eq!(key_model.user_id.as_ref(), &user_id);
    assert_eq!(key_model.provider_type_id.as_ref(), &provider_id);
    assert_eq!(key_model.name.as_ref(), "Integration Test Key");
    
    // 5. 测试配置
    let config = TestConfig::app_config();
    assert_eq!(config.server.host, "127.0.0.1");
    assert_eq!(config.database.url, ":memory:");
    assert_eq!(config.redis.database, 15);
    
    // 6. 测试 Mock AI 提供商
    let mock_provider = MockOpenAiProvider::new()
        .with_failure();
    
    let request = api_proxy::testing::mocks::ChatCompletionRequest {
        model: "gpt-3.5-turbo".to_string(),
        messages: vec![],
        temperature: Some(0.7),
        max_tokens: Some(100),
    };
    
    let result = mock_provider.chat_completion(request).await;
    assert!(result.is_err());
    
    // 7. 测试性能测量
    let (result, duration) = PerformanceTest::measure(|| {
        std::thread::sleep(std::time::Duration::from_millis(1));
        42
    });
    assert_eq!(result, 42);
    assert!(duration >= std::time::Duration::from_millis(1));
    
    // 8. 测试 Mock Time
    let mock_time = MockTime::new(chrono::Utc::now());
    let initial_time = mock_time.now();
    
    mock_time.advance(chrono::Duration::hours(1));
    let advanced_time = mock_time.now();
    
    assert!(advanced_time > initial_time);
    assert_eq!(
        advanced_time - initial_time,
        chrono::Duration::hours(1)
    );
}

#[tokio::test]
async fn test_database_operations() {
    init_test_env();
    
    let db = create_test_db().await.unwrap();
    
    // 验证可以连接数据库
    use sea_orm::ConnectionTrait;
    let backend = db.get_database_backend();
    assert!(!format!("{:?}", backend).is_empty());
}

#[test]
fn test_all_fixtures() {
    // 测试所有 fixture 的基本功能
    let user = UserFixture::new().admin().to_model_with_id(1);
    assert!(user.is_admin);
    
    let openai = ProviderTypeFixture::openai();
    assert_eq!(openai.name.as_ref(), "openai");
    
    let gemini = ProviderTypeFixture::gemini();
    assert_eq!(gemini.name.as_ref(), "gemini");
    
    let claude = ProviderTypeFixture::claude();
    assert_eq!(claude.name.as_ref(), "claude");
    
    let key = UserProviderKeyFixture::new()
        .weight(75)
        .api_key("sk-custom-key")
        .to_active_model();
    
    assert_eq!(key.weight.as_ref(), &Some(75));
    assert_eq!(key.api_key.as_ref(), "sk-custom-key");
}

#[tokio::test]
async fn test_mock_ai_provider_success() {
    let mock_provider = MockOpenAiProvider::new();
    
    let request = api_proxy::testing::mocks::ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![],
        temperature: Some(0.5),
        max_tokens: Some(150),
    };
    
    let response = mock_provider.chat_completion(request).await.unwrap();
    assert_eq!(response.model, "gpt-4");
    assert!(!response.choices.is_empty());
    assert!(response.usage.is_some());
    
    let health = mock_provider.health_check().await.unwrap();
    assert_eq!(health.status, "healthy");
}

#[tokio::test]
async fn test_performance_benchmarking() {
    let start = std::time::Instant::now();
    
    PerformanceTest::benchmark_async(
        "简单异步操作", 
        5, 
        || async {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    ).await;
    
    let duration = start.elapsed();
    // 基准测试应该在合理时间内完成
    assert!(duration < std::time::Duration::from_secs(1));
}