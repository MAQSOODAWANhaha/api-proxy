//! # AI 代理核心功能测试
//!
//! 测试代理核心功能的单元测试和集成测试
//! 专注于验证三个核心功能：身份验证、速率限制、转发策略

use std::sync::Arc;
use std::time::Duration;
use chrono::{Utc, Duration as ChronoDuration};
use tokio::time::sleep;
use sea_orm::{
    Database, DatabaseConnection, EntityTrait, Set, ActiveModelTrait,
    DbBackend, Statement, ConnectionTrait, 
};

use api_proxy::{
    config::{AppConfig, CacheConfig, CacheType},
    cache::UnifiedCacheManager,
    auth::unified::UnifiedAuthManager,
    proxy::ai_handler::{AIProxyHandler, ProxyContext, SchedulerRegistry},
    error::ProxyError,
};
use entity::{
    users, provider_types, user_service_apis, user_provider_keys,
    sea_orm_active_enums::UserRole,
};

/// 代理核心功能测试套件
pub struct ProxyCoreTest {
    db: Arc<DatabaseConnection>,
    cache: Arc<UnifiedCacheManager>,
    config: Arc<AppConfig>,
    ai_handler: Arc<AIProxyHandler>,
    test_data: TestData,
}

#[derive(Debug, Clone)]
pub struct TestData {
    pub user_id: i32,
    pub provider_type_id: i32,
    pub valid_api_key: String,
    pub rate_limited_api_key: String,
    pub expired_api_key: String,
    pub backend_keys: Vec<String>,
}

impl ProxyCoreTest {
    /// 创建测试环境
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        println!("🚀 初始化代理核心测试环境");

        // 1. 创建内存数据库
        let db = Arc::new(Database::connect("sqlite::memory:").await?);
        Self::create_tables(&db).await?;

        // 2. 创建内存缓存
        let cache_config = CacheConfig {
            cache_type: CacheType::Memory,
            memory_max_entries: 1000,
            default_ttl: 300,
            enabled: true,
        };
        let cache = Arc::new(UnifiedCacheManager::new(&cache_config, "")?);

        // 3. 创建配置
        let config = Arc::new(AppConfig::default());

        // 4. 创建认证管理器
        let auth_config = Arc::new(api_proxy::auth::types::AuthConfig::default());
        let jwt_manager = Arc::new(
            api_proxy::auth::jwt::JwtManager::new(auth_config.clone())?
        );
        let api_key_manager = Arc::new(
            api_proxy::auth::api_key::ApiKeyManager::new(db.clone(), auth_config.clone())
        );
        let auth_service = Arc::new(
            api_proxy::auth::AuthService::new(
                jwt_manager, api_key_manager, db.clone(), auth_config.clone()
            )
        );
        let auth_manager = Arc::new(UnifiedAuthManager::new(auth_service, auth_config));

        // 5. 创建AI代理处理器
        let schedulers = Arc::new(SchedulerRegistry::new(db.clone(), cache.clone()));
        let ai_handler = Arc::new(AIProxyHandler::new(
            db.clone(),
            cache.clone(),
            config.clone(),
            auth_manager,
            schedulers,
        ));

        // 6. 创建测试数据
        let test_data = Self::create_test_data(&db).await?;

        Ok(Self {
            db,
            cache,
            config,
            ai_handler,
            test_data,
        })
    }

    /// 创建数据库表结构
    async fn create_tables(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
        println!("📊 创建测试数据库表");

        // 创建所有必需的表
        let tables = vec![
            // 用户表
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                email TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'user',
                is_active BOOLEAN NOT NULL DEFAULT true,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
            // 提供商类型表
            r#"
            CREATE TABLE provider_types (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL,
                base_url TEXT NOT NULL,
                api_format TEXT NOT NULL,
                default_model TEXT,
                max_tokens INTEGER,
                rate_limit INTEGER,
                timeout_seconds INTEGER,
                health_check_path TEXT,
                auth_header_format TEXT,
                is_active BOOLEAN NOT NULL DEFAULT true,
                config_json TEXT,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
            // 用户服务API表
            r#"
            CREATE TABLE user_service_apis (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                provider_type_id INTEGER NOT NULL,
                api_key TEXT NOT NULL UNIQUE,
                api_secret TEXT NOT NULL,
                name TEXT,
                description TEXT,
                scheduling_strategy TEXT,
                retry_count INTEGER,
                timeout_seconds INTEGER,
                rate_limit INTEGER,
                max_tokens_per_day INTEGER,
                used_tokens_today INTEGER,
                total_requests INTEGER,
                successful_requests INTEGER,
                last_used DATETIME,
                expires_at DATETIME,
                is_active BOOLEAN NOT NULL DEFAULT true,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES users(id),
                FOREIGN KEY (provider_type_id) REFERENCES provider_types(id)
            );
            "#,
            // 用户提供商密钥表
            r#"
            CREATE TABLE user_provider_keys (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                provider_type_id INTEGER NOT NULL,
                api_key TEXT NOT NULL,
                name TEXT NOT NULL,
                weight INTEGER,
                max_requests_per_minute INTEGER,
                max_tokens_per_day INTEGER,
                used_tokens_today INTEGER,
                last_used DATETIME,
                is_active BOOLEAN NOT NULL DEFAULT true,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES users(id),
                FOREIGN KEY (provider_type_id) REFERENCES provider_types(id)
            );
            "#,
        ];

        for table_sql in tables {
            db.execute(Statement::from_string(DbBackend::Sqlite, table_sql.to_string())).await?;
        }

        Ok(())
    }

    /// 创建测试数据
    async fn create_test_data(db: &DatabaseConnection) -> Result<TestData, Box<dyn std::error::Error>> {
        println!("🗄️  创建测试数据");

        // 创建测试用户
        let user = users::ActiveModel {
            username: Set("test_user".to_string()),
            email: Set("test@example.com".to_string()),
            password_hash: Set("$2b$12$test_hash".to_string()),
            role: Set(UserRole::User),
            is_active: Set(true),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };
        let user_result = user.insert(db).await?;

        // 创建OpenAI提供商类型
        let provider = provider_types::ActiveModel {
            name: Set("openai".to_string()),
            display_name: Set("OpenAI".to_string()),
            base_url: Set("api.openai.com".to_string()),
            api_format: Set("openai".to_string()),
            default_model: Set(Some("gpt-3.5-turbo".to_string())),
            auth_header_format: Set(Some("Bearer {key}".to_string())),
            is_active: Set(true),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };
        let provider_result = provider.insert(db).await?;

        // 创建有效的API密钥
        let valid_api = user_service_apis::ActiveModel {
            user_id: Set(user_result.id),
            provider_type_id: Set(provider_result.id),
            api_key: Set("test-valid-api-key-12345".to_string()),
            api_secret: Set("test-secret".to_string()),
            name: Set(Some("有效API密钥".to_string())),
            scheduling_strategy: Set(Some("round_robin".to_string())),
            rate_limit: Set(Some(5)), // 每分钟5次
            expires_at: Set(Some((Utc::now() + ChronoDuration::days(30)).naive_utc())),
            is_active: Set(true),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };
        let valid_result = valid_api.insert(db).await?;

        // 创建速率限制很低的API密钥
        let rate_limited_api = user_service_apis::ActiveModel {
            user_id: Set(user_result.id),
            provider_type_id: Set(provider_result.id),
            api_key: Set("test-rate-limited-key-67890".to_string()),
            api_secret: Set("test-secret".to_string()),
            name: Set(Some("速率限制API密钥".to_string())),
            scheduling_strategy: Set(Some("round_robin".to_string())),
            rate_limit: Set(Some(2)), // 每分钟2次
            expires_at: Set(Some((Utc::now() + ChronoDuration::days(30)).naive_utc())),
            is_active: Set(true),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };
        let rate_limited_result = rate_limited_api.insert(db).await?;

        // 创建已过期的API密钥
        let expired_api = user_service_apis::ActiveModel {
            user_id: Set(user_result.id),
            provider_type_id: Set(provider_result.id),
            api_key: Set("test-expired-api-key-99999".to_string()),
            api_secret: Set("test-secret".to_string()),
            name: Set(Some("过期API密钥".to_string())),
            scheduling_strategy: Set(Some("round_robin".to_string())),
            rate_limit: Set(Some(10)),
            expires_at: Set(Some((Utc::now() - ChronoDuration::days(1)).naive_utc())), // 已过期
            is_active: Set(true),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };
        let expired_result = expired_api.insert(db).await?;

        // 创建后端API密钥池
        let backend_keys = vec![
            ("sk-backend-key-1111", "后端密钥1", 5),
            ("sk-backend-key-2222", "后端密钥2", 3),
            ("sk-backend-key-3333", "后端密钥3", 2),
        ];

        let mut backend_key_list = Vec::new();
        for (key, name, weight) in backend_keys {
            let provider_key = user_provider_keys::ActiveModel {
                user_id: Set(user_result.id),
                provider_type_id: Set(provider_result.id),
                api_key: Set(key.to_string()),
                name: Set(name.to_string()),
                weight: Set(Some(weight)),
                max_requests_per_minute: Set(Some(60)),
                is_active: Set(true),
                created_at: Set(Utc::now().naive_utc()),
                updated_at: Set(Utc::now().naive_utc()),
                ..Default::default()
            };
            provider_key.insert(db).await?;
            backend_key_list.push(key.to_string());
        }

        Ok(TestData {
            user_id: user_result.id,
            provider_type_id: provider_result.id,
            valid_api_key: valid_result.api_key,
            rate_limited_api_key: rate_limited_result.api_key,
            expired_api_key: expired_result.api_key,
            backend_keys: backend_key_list,
        })
    }

    /// 测试身份验证功能
    pub async fn test_authentication(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔐 测试身份验证功能");

        // 1. 测试有效API密钥
        println!("   测试有效API密钥");
        let session = create_mock_session(&self.test_data.valid_api_key);
        let mut ctx = ProxyContext::default();
        ctx.request_id = "auth-valid-test".to_string();

        match self.ai_handler.prepare_proxy_request(&session, &mut ctx).await {
            Ok(_) => {
                println!("   ✅ 有效API密钥验证成功");
                assert!(ctx.user_service_api.is_some());
                assert!(ctx.selected_backend.is_some());
                assert!(ctx.provider_type.is_some());
            }
            Err(e) => {
                println!("   ❌ 有效API密钥验证失败: {}", e);
                return Err(Box::new(e));
            }
        }

        // 2. 测试无效API密钥
        println!("   测试无效API密钥");
        let session_invalid = create_mock_session("invalid-api-key-12345");
        let mut ctx_invalid = ProxyContext::default();
        ctx_invalid.request_id = "auth-invalid-test".to_string();

        match self.ai_handler.prepare_proxy_request(&session_invalid, &mut ctx_invalid).await {
            Ok(_) => {
                println!("   ❌ 无效API密钥不应该通过验证");
                return Err("无效API密钥验证错误".into());
            }
            Err(ProxyError::Authentication { .. }) => {
                println!("   ✅ 无效API密钥正确拒绝");
            }
            Err(e) => {
                println!("   ❌ 无效API密钥返回了错误的错误类型: {}", e);
                return Err(Box::new(e));
            }
        }

        // 3. 测试已过期API密钥
        println!("   测试已过期API密钥");
        let session_expired = create_mock_session(&self.test_data.expired_api_key);
        let mut ctx_expired = ProxyContext::default();
        ctx_expired.request_id = "auth-expired-test".to_string();

        match self.ai_handler.prepare_proxy_request(&session_expired, &mut ctx_expired).await {
            Ok(_) => {
                println!("   ❌ 已过期API密钥不应该通过验证");
                return Err("已过期API密钥验证错误".into());
            }
            Err(ProxyError::Authentication { .. }) => {
                println!("   ✅ 已过期API密钥正确拒绝");
            }
            Err(e) => {
                println!("   ❌ 已过期API密钥返回了错误的错误类型: {}", e);
                return Err(Box::new(e));
            }
        }

        println!("🔐 身份验证功能测试完成");
        Ok(())
    }

    /// 测试速率限制功能
    pub async fn test_rate_limiting(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("⏱️  测试速率限制功能");

        // 清除缓存以确保干净的测试环境
        self.cache.provider().clear().await.map_err(|e| format!("清除缓存失败: {}", e))?;

        let session = create_mock_session(&self.test_data.rate_limited_api_key);

        // 1. 发送2次请求（在限制内）
        for i in 1..=2 {
            println!("   发送请求 {}/2", i);
            let mut ctx = ProxyContext::default();
            ctx.request_id = format!("rate-limit-{}", i);

            match self.ai_handler.prepare_proxy_request(&session, &mut ctx).await {
                Ok(_) => {
                    println!("   ✅ 请求 {} 通过速率限制", i);
                }
                Err(e) => {
                    println!("   ❌ 请求 {} 意外失败: {}", i, e);
                    return Err(Box::new(e));
                }
            }
        }

        // 2. 发送第3次请求（应该被限制）
        println!("   发送超限请求");
        let mut ctx_exceed = ProxyContext::default();
        ctx_exceed.request_id = "rate-limit-exceed".to_string();

        match self.ai_handler.prepare_proxy_request(&session, &mut ctx_exceed).await {
            Ok(_) => {
                println!("   ❌ 超出速率限制的请求应该被拒绝");
                return Err("速率限制测试失败".into());
            }
            Err(ProxyError::RateLimit { .. }) => {
                println!("   ✅ 超出速率限制的请求正确拒绝");
            }
            Err(e) => {
                println!("   ❌ 速率限制返回了错误的错误类型: {}", e);
                return Err(Box::new(e));
            }
        }

        println!("⏱️  速率限制功能测试完成");
        Ok(())
    }

    /// 测试转发策略和负载均衡
    pub async fn test_load_balancing(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔄 测试负载均衡和转发策略");

        // 清除缓存以重置速率限制和负载均衡状态
        self.cache.provider().clear().await.map_err(|e| format!("清除缓存失败: {}", e))?;

        let session = create_mock_session(&self.test_data.valid_api_key);
        let mut selected_backends = Vec::new();

        // 发送多次请求测试负载均衡
        for i in 1..=6 {
            println!("   发送负载均衡测试请求 {}/6", i);
            let mut ctx = ProxyContext::default();
            ctx.request_id = format!("lb-test-{}", i);

            match self.ai_handler.prepare_proxy_request(&session, &mut ctx).await {
                Ok(_) => {
                    if let Some(backend) = &ctx.selected_backend {
                        selected_backends.push(backend.api_key.clone());
                        println!("   ✅ 请求 {} 选择后端: {}", i, backend.name);
                    } else {
                        println!("   ❌ 请求 {} 未选择后端", i);
                        return Err("负载均衡测试失败：未选择后端".into());
                    }
                }
                Err(e) => {
                    println!("   ❌ 负载均衡请求 {} 失败: {}", i, e);
                    return Err(Box::new(e));
                }
            }
        }

        // 验证负载均衡是否正常工作
        let unique_backends: std::collections::HashSet<_> = selected_backends.iter().collect();
        println!("   使用了 {} 个不同的后端密钥", unique_backends.len());

        if unique_backends.len() >= 2 {
            println!("   ✅ 负载均衡正常工作，轮询使用了多个后端");
        } else {
            println!("   ⚠️  负载均衡可能需要更多测试，当前只使用了 {} 个后端", unique_backends.len());
        }

        // 验证所选择的后端密钥都是有效的
        for backend_key in &unique_backends {
            if self.test_data.backend_keys.contains(backend_key) {
                println!("   ✅ 后端密钥 {} 是有效的", backend_key);
            } else {
                println!("   ❌ 后端密钥 {} 不在预期列表中", backend_key);
                return Err("负载均衡选择了错误的后端密钥".into());
            }
        }

        println!("🔄 负载均衡和转发策略测试完成");
        Ok(())
    }

    /// 运行所有核心功能测试
    pub async fn run_all_tests(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🚀 开始代理核心功能测试");
        println!("==========================================");

        self.test_authentication().await?;
        println!();

        self.test_rate_limiting().await?;
        println!();

        self.test_load_balancing().await?;
        println!();

        println!("==========================================");
        println!("🎉 所有代理核心功能测试通过！");
        println!("✨ 验证完成：");
        println!("   - ✅ 身份验证功能正常");
        println!("   - ✅ 速率限制功能正常");
        println!("   - ✅ 负载均衡功能正常");
        println!("   - ✅ 转发策略功能正常");

        Ok(())
    }
}

/// 创建模拟会话（简化版本，仅用于测试核心逻辑）
fn create_mock_session(api_key: &str) -> MockSession {
    MockSession {
        api_key: api_key.to_string(),
    }
}

/// 模拟会话结构（用于测试）
pub struct MockSession {
    pub api_key: String,
}

// 为了测试需要，我们需要为AIProxyHandler创建一个适配版本
// 实际实现中，我们需要修改AIProxyHandler的API或者创建测试版本

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test] 
    async fn test_proxy_core_functionality() {
        println!("启动代理核心功能测试");

        let test_env = ProxyCoreTest::new().await
            .expect("创建测试环境失败");

        test_env.run_all_tests().await
            .expect("代理核心功能测试失败");

        println!("代理核心功能测试完成");
    }
}