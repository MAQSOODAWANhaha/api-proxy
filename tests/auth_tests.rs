//! # 认证模块集成测试
//!
//! 测试认证服务的完整功能：API密钥验证、JWT管理、权限检查

use api_proxy::testing::*;
use api_proxy::auth::{AuthService, types::*};
use entity::{users, user_service_apis, provider_types};
use sea_orm::{EntityTrait, Set};
use chrono::{Utc, Duration as ChronoDuration};

/// 认证功能测试套件
struct AuthTestSuite {
    tx: TestTransaction,
    auth_service: AuthService,
}

impl AuthTestSuite {
    /// 创建测试环境
    async fn setup() -> Result<Self, Box<dyn std::error::Error>> {
        init_test_env();
        
        let tx = TestTransaction::new().await?;
        let auth_service = TestConfig::auth_service();
        
        Ok(Self {
            tx,
            auth_service,
        })
    }

    /// 准备测试数据
    async fn prepare_test_data(&self) -> Result<TestData, Box<dyn std::error::Error>> {
        // 插入测试用户
        let user_fixture = UserFixture::new()
            .username("auth_test_user")
            .email("auth@test.com");
        let user_id = self.tx.insert_test_user(user_fixture).await?;

        // 插入OpenAI提供商
        let provider_fixture = ProviderTypeFixture::openai();
        let provider_id = self.tx.insert_provider_type(provider_fixture).await?;

        // 创建有效的API密钥
        let valid_api = user_service_apis::ActiveModel {
            user_id: Set(user_id),
            provider_type_id: Set(provider_id),
            api_key: Set("test-valid-api-12345".to_string()),
            api_secret: Set("secret123".to_string()),
            name: Set(Some("测试API密钥".to_string())),
            rate_limit: Set(Some(100)),
            expires_at: Set(Some((Utc::now() + ChronoDuration::days(30)).naive_utc())),
            is_active: Set(true),
            ..Default::default()
        };
        let valid_result = user_service_apis::Entity::insert(valid_api)
            .exec(self.tx.db())
            .await?;

        // 创建已过期的API密钥
        let expired_api = user_service_apis::ActiveModel {
            user_id: Set(user_id),
            provider_type_id: Set(provider_id),
            api_key: Set("test-expired-api-67890".to_string()),
            api_secret: Set("secret456".to_string()),
            name: Set(Some("已过期API密钥".to_string())),
            rate_limit: Set(Some(50)),
            expires_at: Set(Some((Utc::now() - ChronoDuration::days(1)).naive_utc())),
            is_active: Set(true),
            ..Default::default()
        };
        let expired_result = user_service_apis::Entity::insert(expired_api)
            .exec(self.tx.db())
            .await?;

        Ok(TestData {
            user_id,
            provider_id,
            valid_api_key: valid_result.last_insert_id,
            expired_api_key: expired_result.last_insert_id,
        })
    }
}

#[derive(Debug)]
struct TestData {
    user_id: i32,
    provider_id: i32,
    valid_api_key: i32,
    expired_api_key: i32,
}

#[tokio::test]
async fn test_api_key_validation() {
    let suite = AuthTestSuite::setup().await.expect("设置测试环境失败");
    let test_data = suite.prepare_test_data().await.expect("准备测试数据失败");

    // 测试有效API密钥验证
    let valid_result = suite.auth_service
        .validate_api_key("test-valid-api-12345")
        .await;
    
    match valid_result {
        Ok(auth_info) => {
            assert_eq!(auth_info.user_id, test_data.user_id);
            assert!(auth_info.is_active);
            println!("✅ 有效API密钥验证通过");
        }
        Err(e) => panic!("有效API密钥验证失败: {}", e),
    }

    // 测试无效API密钥
    let invalid_result = suite.auth_service
        .validate_api_key("invalid-api-key-999")
        .await;
    
    assert!(invalid_result.is_err());
    println!("✅ 无效API密钥正确拒绝");

    // 测试已过期API密钥
    let expired_result = suite.auth_service
        .validate_api_key("test-expired-api-67890")
        .await;
    
    assert!(expired_result.is_err());
    println!("✅ 已过期API密钥正确拒绝");
}

#[tokio::test]
async fn test_jwt_token_management() {
    let suite = AuthTestSuite::setup().await.expect("设置测试环境失败");
    let test_data = suite.prepare_test_data().await.expect("准备测试数据失败");

    // 创建JWT token
    let token_result = suite.auth_service
        .create_jwt_token(test_data.user_id, "auth_test_user")
        .await;

    let token = match token_result {
        Ok(t) => {
            println!("✅ JWT token创建成功");
            t
        }
        Err(e) => panic!("JWT token创建失败: {}", e),
    };

    // 验证JWT token
    let validation_result = suite.auth_service
        .validate_jwt_token(&token.access_token)
        .await;

    match validation_result {
        Ok(claims) => {
            assert_eq!(claims.user_id, test_data.user_id);
            assert_eq!(claims.username, "auth_test_user");
            println!("✅ JWT token验证成功");
        }
        Err(e) => panic!("JWT token验证失败: {}", e),
    }

    // 测试无效token
    let invalid_token_result = suite.auth_service
        .validate_jwt_token("invalid.jwt.token")
        .await;
    
    assert!(invalid_token_result.is_err());
    println!("✅ 无效JWT token正确拒绝");
}

#[tokio::test]
async fn test_rate_limiting() {
    let suite = AuthTestSuite::setup().await.expect("设置测试环境失败");
    let test_data = suite.prepare_test_data().await.expect("准备测试数据失败");

    // 测试速率限制检查
    for i in 1..=5 {
        let check_result = suite.auth_service
            .check_rate_limit(test_data.user_id, "/v1/chat/completions")
            .await;

        match check_result {
            Ok(_) => println!("✅ 请求 {}/5 通过速率限制", i),
            Err(e) => panic!("速率限制检查失败: {}", e),
        }
    }

    println!("✅ 速率限制功能测试完成");
}

#[tokio::test]
async fn test_user_permissions() {
    let suite = AuthTestSuite::setup().await.expect("设置测试环境失败");
    let test_data = suite.prepare_test_data().await.expect("准备测试数据失败");

    // 测试用户权限检查
    let permission_result = suite.auth_service
        .check_user_permission(test_data.user_id, Permission::UseApi)
        .await;

    match permission_result {
        Ok(has_permission) => {
            assert!(has_permission);
            println!("✅ 用户权限检查通过");
        }
        Err(e) => panic!("权限检查失败: {}", e),
    }

    // 测试管理员权限检查
    let admin_result = suite.auth_service
        .check_user_permission(test_data.user_id, Permission::AdminAccess)
        .await;

    match admin_result {
        Ok(has_admin) => {
            assert!(!has_admin); // 普通用户不应该有管理员权限
            println!("✅ 管理员权限检查正确");
        }
        Err(e) => panic!("管理员权限检查失败: {}", e),
    }
}

#[tokio::test]
async fn test_auth_integration() {
    let suite = AuthTestSuite::setup().await.expect("设置测试环境失败");
    let test_data = suite.prepare_test_data().await.expect("准备测试数据失败");

    // 完整的认证流程测试
    println!("🔐 开始完整认证流程测试");

    // 1. API密钥验证
    let auth_info = suite.auth_service
        .validate_api_key("test-valid-api-12345")
        .await
        .expect("API密钥验证失败");

    // 2. 速率限制检查
    suite.auth_service
        .check_rate_limit(auth_info.user_id, "/v1/chat/completions")
        .await
        .expect("速率限制检查失败");

    // 3. 权限验证
    let has_permission = suite.auth_service
        .check_user_permission(auth_info.user_id, Permission::UseApi)
        .await
        .expect("权限检查失败");

    assert!(has_permission);

    println!("✅ 完整认证流程测试通过");
    println!("   - API密钥验证: ✓");
    println!("   - 速率限制检查: ✓");
    println!("   - 用户权限验证: ✓");
}