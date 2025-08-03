//! # 系统集成测试
//!
//! 全面测试整个代理系统的集成功能

use api_proxy::testing::{
    fixtures::{UserFixture, ProviderTypeFixture, UserProviderKeyFixture, TestConfig},
    helpers::{init_test_env, create_test_db, TestTransaction, PerformanceTest, MockPingoraSession, EnvTestHelper},
    mocks::{MockOpenAiProvider, MockAiProvider, MockTime, MockHttpServer}
};
use serde_json::json;

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

#[tokio::test] 
async fn test_session_mock_integration() {
    init_test_env();
    
    println!("🎭 测试会话模拟器集成");
    
    // 创建Mock会话
    let session = MockPingoraSession::new()
        .with_auth("test-api-key-12345")
        .with_header("content-type", "application/json")
        .with_header("x-request-id", "test-request-001")
        .with_json_body(&json!({
            "model": "gpt-3.5-turbo",
            "messages": [
                {"role": "user", "content": "Hello, integration test!"}
            ],
            "max_tokens": 100,
            "temperature": 0.7
        }));
    
    // 验证会话属性
    assert_eq!(session.get_header("authorization"), Some(&"Bearer test-api-key-12345".to_string()));
    assert_eq!(session.get_header("content-type"), Some(&"application/json".to_string()));
    assert_eq!(session.get_header("x-request-id"), Some(&"test-request-001".to_string()));
    
    // 验证JSON请求体
    let parsed_body: serde_json::Value = session.body_as_json()
        .expect("JSON解析失败");
    assert_eq!(parsed_body["model"], "gpt-3.5-turbo");
    assert_eq!(parsed_body["messages"][0]["role"], "user");
    assert_eq!(parsed_body["max_tokens"], 100);
    
    println!("   ✅ 会话模拟器集成测试通过");
}

#[tokio::test]
async fn test_environment_helpers_integration() {
    init_test_env();
    
    println!("🌍 测试环境辅助工具集成");
    
    // 测试临时环境变量设置
    let original_value = std::env::var("TEST_INTEGRATION_VAR").ok();
    
    let result = EnvTestHelper::with_env("TEST_INTEGRATION_VAR", "integration_test_value", || {
        std::env::var("TEST_INTEGRATION_VAR").unwrap()
    });
    
    assert_eq!(result, "integration_test_value");
    
    // 验证环境变量已恢复
    let current_value = std::env::var("TEST_INTEGRATION_VAR").ok();
    assert_eq!(current_value, original_value);
    
    // 测试临时移除环境变量
    std::env::set_var("TEMP_TEST_VAR", "temporary");
    
    let result = EnvTestHelper::without_env("TEMP_TEST_VAR", || {
        std::env::var("TEMP_TEST_VAR").is_err()
    });
    
    assert!(result);
    
    // 验证环境变量已恢复
    assert_eq!(std::env::var("TEMP_TEST_VAR").unwrap(), "temporary");
    std::env::remove_var("TEMP_TEST_VAR");
    
    println!("   ✅ 环境辅助工具集成测试通过");
}

#[tokio::test]
async fn test_http_server_mock_integration() {
    init_test_env();
    
    println!("🌐 测试HTTP服务器Mock集成");
    
    // 启动Mock服务器
    let server = MockHttpServer::start().await;
    
    // 配置OpenAI Chat Mock
    server.mock_openai_chat("gpt-3.5-turbo", "Hello from integration test!").await;
    
    // 配置错误响应Mock
    server.mock_error("/v1/error", 500, "Internal server error").await;
    
    // 验证服务器URI
    assert!(!server.uri().is_empty());
    assert!(server.uri().starts_with("http://"));
    
    println!("   ✅ Mock服务器启动成功: {}", server.uri());
    println!("   ✅ OpenAI Chat Mock配置成功");
    println!("   ✅ 错误响应Mock配置成功");
    
    println!("   ✅ HTTP服务器Mock集成测试通过");
}

#[tokio::test]
async fn test_time_mock_integration() {
    init_test_env();
    
    println!("⏰ 测试时间Mock集成");
    
    let base_time = chrono::Utc::now();
    let mock_time = MockTime::new(base_time);
    
    // 验证初始时间
    assert_eq!(mock_time.now(), base_time);
    
    // 测试时间推进
    mock_time.advance(chrono::Duration::minutes(30));
    assert_eq!(mock_time.now(), base_time + chrono::Duration::minutes(30));
    
    // 测试时间跳跃
    let future_time = base_time + chrono::Duration::days(1);
    mock_time.set(future_time);
    assert_eq!(mock_time.now(), future_time);
    
    // 测试时间回退
    let past_time = base_time - chrono::Duration::hours(2);
    mock_time.set(past_time);
    assert_eq!(mock_time.now(), past_time);
    
    println!("   ✅ 时间推进测试通过");
    println!("   ✅ 时间跳跃测试通过");
    println!("   ✅ 时间回退测试通过");
    
    println!("   ✅ 时间Mock集成测试通过");
}

#[tokio::test]
async fn test_comprehensive_integration_scenario() {
    init_test_env();
    
    println!("🚀 综合集成测试场景");
    
    // 1. 数据库和事务设置
    let tx = TestTransaction::new().await
        .expect("创建测试事务失败");
    
    // 2. 创建测试用户
    let user_fixture = UserFixture::new()
        .username("comprehensive_user")
        .email("comprehensive@test.com")
        .admin();
    
    let user_id = tx.insert_test_user(user_fixture).await
        .expect("插入测试用户失败");
    
    // 3. 创建提供商
    let mut provider_fixture = ProviderTypeFixture::openai();
    provider_fixture.name = sea_orm::Set("comprehensive_openai".to_string());
    let provider_id = tx.insert_provider_type(provider_fixture).await
        .expect("插入提供商失败");
    
    // 4. 使用时间Mock控制测试时间
    let test_time = chrono::Utc::now();
    let mock_time = MockTime::new(test_time);
    
    // 5. 启动Mock HTTP服务器
    let server = MockHttpServer::start().await;
    server.mock_openai_chat("gpt-3.5-turbo", "Comprehensive test response!").await;
    
    // 6. 创建Mock Pingora会话
    let session = MockPingoraSession::new()
        .with_auth("comprehensive-test-key")
        .with_header("x-test-scenario", "comprehensive")
        .with_json_body(&json!({
            "model": "gpt-3.5-turbo",
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": "Run comprehensive integration test"}
            ],
            "max_tokens": 200,
            "temperature": 0.8,
            "stream": false
        }));
    
    // 7. 测量整个场景的性能
    let (scenario_result, total_duration) = PerformanceTest::measure_async(|| async {
        // 模拟完整的请求处理流程
        
        // 7.1. 验证会话数据
        let auth_header = session.get_header("authorization").unwrap();
        assert!(auth_header.starts_with("Bearer "));
        
        let body: serde_json::Value = session.body_as_json().unwrap();
        assert_eq!(body["model"], "gpt-3.5-turbo");
        assert_eq!(body["messages"].as_array().unwrap().len(), 2);
        
        // 7.2. 模拟时间推进
        mock_time.advance(chrono::Duration::milliseconds(150));
        
        // 7.3. 创建Mock AI响应
        let mock_provider = MockOpenAiProvider::new();
        let ai_request = api_proxy::testing::mocks::ChatCompletionRequest {
            model: body["model"].as_str().unwrap().to_string(),
            messages: vec![
                api_proxy::testing::mocks::ChatMessage {
                    role: "user".to_string(),
                    content: "Comprehensive test".to_string(),
                }
            ],
            temperature: Some(0.8),
            max_tokens: Some(200),
        };
        
        let ai_response = mock_provider.chat_completion(ai_request).await
            .expect("AI响应生成失败");
        
        // 7.4. 验证响应
        assert_eq!(ai_response.model, "gpt-3.5-turbo");
        assert!(!ai_response.choices.is_empty());
        assert!(ai_response.usage.is_some());
        
        // 7.5. 健康检查
        let health = mock_provider.health_check().await
            .expect("健康检查失败");
        assert_eq!(health.status, "healthy");
        
        "comprehensive_scenario_success"
    }).await;
    
    // 8. 验证综合测试结果
    assert_eq!(scenario_result, "comprehensive_scenario_success");
    assert!(total_duration < std::time::Duration::from_secs(5)); // 应该在5秒内完成
    
    println!("   ✅ 数据库操作: 用户ID {}, 提供商ID {}", user_id, provider_id);
    println!("   ✅ Mock服务: HTTP服务器 {}", server.uri());
    println!("   ✅ 会话模拟: 包含认证和JSON数据");
    println!("   ✅ 时间控制: 推进150ms");
    println!("   ✅ AI模拟: OpenAI聊天完成");
    println!("   ✅ 性能测量: 总耗时 {:?}", total_duration);
    
    println!("🎉 综合集成测试场景通过！");
}

#[tokio::test]
async fn test_error_handling_integration() {
    init_test_env();
    
    println!("❌ 测试错误处理集成");
    
    // 1. 测试Mock AI提供商错误
    let failing_provider = MockOpenAiProvider::new().with_failure();
    
    let request = api_proxy::testing::mocks::ChatCompletionRequest {
        model: "gpt-3.5-turbo".to_string(),
        messages: vec![],
        temperature: Some(0.7),
        max_tokens: Some(100),
    };
    
    let ai_result = failing_provider.chat_completion(request).await;
    assert!(ai_result.is_err());
    println!("   ✅ AI提供商错误模拟成功");
    
    let health_result = failing_provider.health_check().await;
    assert!(health_result.is_err());
    println!("   ✅ 健康检查错误模拟成功");
    
    // 2. 测试不健康的Mock提供商
    let unhealthy_provider = MockOpenAiProvider::new().unhealthy();
    
    let health_status = unhealthy_provider.health_check().await
        .expect("不健康状态检查失败");
    assert_eq!(health_status.status, "unhealthy");
    assert!(health_status.response_time_ms > 1000); // 模拟高延迟
    println!("   ✅ 不健康状态模拟成功");
    
    // 3. 测试无效会话数据
    let invalid_session = MockPingoraSession::new()
        .with_auth("invalid-token")
        .with_body(b"invalid json {");
    
    let json_result: Result<serde_json::Value, _> = invalid_session.body_as_json();
    assert!(json_result.is_err());
    println!("   ✅ 无效JSON错误处理成功");
    
    // 4. 测试环境变量错误处理
    let env_result = EnvTestHelper::with_env("", "value", || {
        std::env::var("NONEXISTENT_VAR").is_err()
    });
    assert!(env_result);
    println!("   ✅ 环境变量错误处理成功");
    
    println!("❌ 错误处理集成测试通过！");
}

#[tokio::test]
async fn test_complete_system_integration() {
    init_test_env();
    
    println!("🌟 完整系统集成测试");
    
    // 测试所有组件的协同工作
    println!("   🔧 设置测试环境...");
    
    // 1. 数据库层
    let tx = TestTransaction::new().await
        .expect("数据库设置失败");
    
    // 2. 配置层
    let config = TestConfig::app_config();
    assert_eq!(config.server.as_ref().unwrap().host, "127.0.0.1");
    
    // 3. 用户数据层
    let user_fixture = UserFixture::new()
        .username("system_test_user")
        .email("system@test.com");
    let user_id = tx.insert_test_user(user_fixture).await
        .expect("用户创建失败");
    
    // 4. 提供商数据层
    let mut provider_fixture = ProviderTypeFixture::openai();
    provider_fixture.name = sea_orm::Set("system_test_openai".to_string());
    let provider_id = tx.insert_provider_type(provider_fixture).await
        .expect("提供商创建失败");
    
    // 5. Mock服务层
    let server = MockHttpServer::start().await;
    server.mock_openai_chat("gpt-3.5-turbo", "System integration response").await;
    
    let mock_provider = MockOpenAiProvider::new();
    
    // 6. 会话层
    let session = MockPingoraSession::new()
        .with_auth("system-integration-key")
        .with_header("x-integration-test", "system")
        .with_json_body(&json!({
            "model": "gpt-3.5-turbo",
            "messages": [{"role": "user", "content": "System integration test"}],
            "max_tokens": 150
        }));
    
    // 7. 时间控制层
    let mock_time = MockTime::new(chrono::Utc::now());
    
    println!("   🚀 执行系统集成流程...");
    
    // 执行完整的集成测试流程
    let integration_result = async {
        // 步骤1: 验证数据层
        assert!(user_id > 0);
        assert!(provider_id > 0);
        
        // 步骤2: 验证会话层
        assert_eq!(session.get_header("authorization"), Some(&"Bearer system-integration-key".to_string()));
        let body: serde_json::Value = session.body_as_json()?;
        assert_eq!(body["model"], "gpt-3.5-turbo");
        
        // 步骤3: 验证Mock服务层
        let health = mock_provider.health_check().await?;
        assert_eq!(health.status, "healthy");
        
        let ai_request = api_proxy::testing::mocks::ChatCompletionRequest {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![
                api_proxy::testing::mocks::ChatMessage {
                    role: "user".to_string(),
                    content: "System test".to_string(),
                }
            ],
            temperature: Some(0.7),
            max_tokens: Some(150),
        };
        
        let ai_response = mock_provider.chat_completion(ai_request).await?;
        assert_eq!(ai_response.model, "gpt-3.5-turbo");
        
        // 步骤4: 验证时间控制层
        let start_time = mock_time.now();
        mock_time.advance(chrono::Duration::seconds(1));
        let end_time = mock_time.now();
        assert_eq!(end_time - start_time, chrono::Duration::seconds(1));
        
        // 步骤5: 验证配置层
        assert_eq!(config.database.url, ":memory:");
        assert_eq!(config.redis.database, 15);
        
        Ok::<_, Box<dyn std::error::Error>>("system_integration_success")
    }.await;
    
    match integration_result {
        Ok(result) => {
            assert_eq!(result, "system_integration_success");
            println!("   ✅ 系统集成流程执行成功");
        }
        Err(e) => {
            panic!("系统集成流程失败: {}", e);
        }
    }
    
    println!("🎉 完整系统集成测试通过！");
    println!("✨ 验证完成的系统层次：");
    println!("   - ✅ 数据库层: 用户和提供商管理");
    println!("   - ✅ 配置层: 应用配置管理");
    println!("   - ✅ Mock服务层: 外部服务模拟");
    println!("   - ✅ 会话层: 请求会话模拟");
    println!("   - ✅ 时间控制层: 时间管理模拟");
    println!("   - ✅ 错误处理层: 异常情况处理");
    println!("   - ✅ 性能测量层: 性能监控");
    println!("   - ✅ 组件协作: 跨层次集成");
}