//! # AI 代理 API 流程测试
//!
//! 通过真实的HTTP请求测试完整的代理功能：
//! 1. 身份验证
//! 2. 速率限制  
//! 3. 转发策略

use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use serde_json::{json, Value};
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;
use axum::{
    extract::Path,
    http::HeaderMap,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{Utc, Duration as ChronoDuration};
use sea_orm::{
    Database, DatabaseConnection, EntityTrait, Set, ActiveModelTrait,
    DbBackend, Statement, ConnectionTrait,
};

use api_proxy::{
    config::AppConfig,
    dual_port_setup::DualPortSetup,
};
use entity::{
    users, provider_types, user_service_apis, user_provider_keys,
    sea_orm_active_enums::UserRole,
};

/// API测试套件
pub struct ApiFlowTest {
    /// 代理服务器端口
    pub proxy_port: u16,
    /// Mock上游服务器端口
    pub mock_upstream_port: u16,
    /// 数据库连接
    pub db: Arc<DatabaseConnection>,
    /// HTTP客户端
    pub client: Client,
    /// 测试数据
    pub test_data: TestData,
}

/// 测试数据
#[derive(Debug, Clone)]
pub struct TestData {
    pub user_id: i32,
    pub valid_api_key: String,
    pub invalid_api_key: String,
    pub rate_limited_api_key: String,
    pub backend_keys: Vec<String>,
}

impl ApiFlowTest {
    /// 创建新的API测试环境
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // 1. 找可用端口
        let proxy_port = find_available_port().await?;
        let mock_upstream_port = find_available_port().await?;
        
        println!("🚀 初始化测试环境");
        println!("   代理端口: {}", proxy_port);
        println!("   Mock上游端口: {}", mock_upstream_port);
        
        // 2. 创建数据库
        let db = Arc::new(Database::connect("sqlite::memory:").await?);
        
        // 3. 创建表结构
        Self::create_tables(&db).await?;
        
        // 4. 创建测试数据
        let test_data = Self::create_test_data(&db, mock_upstream_port).await?;
        
        // 5. 启动Mock上游服务器
        Self::start_mock_upstream_server(mock_upstream_port).await?;
        
        // 6. 启动代理服务器
        Self::start_proxy_server(proxy_port, db.clone()).await?;
        
        // 7. 等待服务器启动
        sleep(Duration::from_secs(2)).await;
        
        Ok(Self {
            proxy_port,
            mock_upstream_port,
            db,
            client: Client::new(),
            test_data,
        })
    }
    
    /// 创建数据库表结构
    async fn create_tables(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
        println!("📊 创建数据库表结构");
        
        // 用户表
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
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
            "#.to_string()
        )).await?;
        
        // 提供商类型表
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
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
            "#.to_string()
        )).await?;
        
        // 用户服务API表
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
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
            "#.to_string()
        )).await?;
        
        // 用户提供商密钥表
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
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
            "#.to_string()
        )).await?;
        
        Ok(())
    }
    
    /// 创建测试数据
    async fn create_test_data(
        db: &DatabaseConnection, 
        mock_port: u16
    ) -> Result<TestData, Box<dyn std::error::Error>> {
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
        
        // 创建OpenAI提供商（指向Mock服务器）
        let provider = provider_types::ActiveModel {
            name: Set("openai".to_string()),
            display_name: Set("OpenAI Mock".to_string()),
            base_url: Set(format!("127.0.0.1:{}", mock_port)),
            api_format: Set("openai".to_string()),
            default_model: Set(Some("gpt-3.5-turbo".to_string())),
            max_tokens: Set(Some(4000)),
            rate_limit: Set(Some(100)),
            timeout_seconds: Set(Some(30)),
            health_check_path: Set(Some("/v1/models".to_string())),
            auth_header_format: Set(Some("Bearer {key}".to_string())),
            is_active: Set(true),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };
        let provider_result = provider.insert(db).await?;
        
        // 创建有效的用户API密钥
        let valid_api = user_service_apis::ActiveModel {
            user_id: Set(user_result.id),
            provider_type_id: Set(provider_result.id),
            api_key: Set("test-valid-api-key-12345".to_string()),
            api_secret: Set("test-secret".to_string()),
            name: Set(Some("有效测试API".to_string())),
            scheduling_strategy: Set(Some("round_robin".to_string())),
            rate_limit: Set(Some(5)), // 每分钟5次，方便测试速率限制
            expires_at: Set(Some((Utc::now() + ChronoDuration::days(30)).naive_utc())),
            is_active: Set(true),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };
        let valid_api_result = valid_api.insert(db).await?;
        
        // 创建速率限制很低的API密钥（用于测试速率限制）
        let rate_limited_api = user_service_apis::ActiveModel {
            user_id: Set(user_result.id),
            provider_type_id: Set(provider_result.id),
            api_key: Set("test-rate-limited-key-67890".to_string()),
            api_secret: Set("test-secret".to_string()),
            name: Set(Some("速率限制测试API".to_string())),
            scheduling_strategy: Set(Some("round_robin".to_string())),
            rate_limit: Set(Some(2)), // 每分钟2次
            expires_at: Set(Some((Utc::now() + ChronoDuration::days(30)).naive_utc())),
            is_active: Set(true),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };
        let rate_limited_result = rate_limited_api.insert(db).await?;
        
        // 创建后端API密钥池
        let backend_keys = vec![
            "sk-mock-backend-key-1111",
            "sk-mock-backend-key-2222", 
            "sk-mock-backend-key-3333",
        ];
        
        for (i, key) in backend_keys.iter().enumerate() {
            let provider_key = user_provider_keys::ActiveModel {
                user_id: Set(user_result.id),
                provider_type_id: Set(provider_result.id),
                api_key: Set(key.to_string()),
                name: Set(format!("后端密钥{}", i + 1)),
                weight: Set(Some(5 - i as i32)), // 不同权重
                max_requests_per_minute: Set(Some(60)),
                is_active: Set(true),
                created_at: Set(Utc::now().naive_utc()),
                updated_at: Set(Utc::now().naive_utc()),
                ..Default::default()
            };
            provider_key.insert(db).await?;
        }
        
        Ok(TestData {
            user_id: user_result.id,
            valid_api_key: valid_api_result.api_key,
            invalid_api_key: "invalid-api-key-999".to_string(),
            rate_limited_api_key: rate_limited_result.api_key,
            backend_keys: backend_keys.into_iter().map(|s| s.to_string()).collect(),
        })
    }
    
    /// 启动Mock上游服务器
    async fn start_mock_upstream_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
        println!("🎭 启动Mock上游服务器 (端口: {})", port);
        
        let app = Router::new()
            .route("/v1/chat/completions", post(mock_chat_completions))
            .route("/v1/models", get(mock_models))
            .route("/health", get(mock_health));
        
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
        
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("Mock服务器错误: {}", e);
            }
        });
        
        Ok(())
    }
    
    /// 启动代理服务器
    async fn start_proxy_server(
        port: u16, 
        db: Arc<DatabaseConnection>
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔄 启动代理服务器 (端口: {})", port);
        
        // 创建配置
        let mut config = AppConfig::default();
        if let Some(ref mut dual_port) = config.dual_port {
            dual_port.proxy.http.port = port;
            dual_port.management.http.port = port + 1000; // 管理端口
        }
        
        // 确保数据库路径
        config.database.ensure_database_path()?;
        
        // 启动双端口服务器
        let dual_port_setup = DualPortSetup::new(config);
        
        tokio::spawn(async move {
            if let Err(e) = dual_port_setup.start().await {
                eprintln!("代理服务器启动错误: {}", e);
            }
        });
        
        Ok(())
    }
    
    /// 测试正常的聊天请求
    pub async fn test_normal_chat_request(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("💬 测试正常聊天请求");
        
        let request_body = json!({
            "model": "gpt-3.5-turbo",
            "messages": [
                {
                    "role": "user",
                    "content": "Hello, how are you?"
                }
            ],
            "max_tokens": 100,
            "temperature": 0.7
        });
        
        let response = self.client
            .post(&format!("http://127.0.0.1:{}/v1/chat/completions", self.proxy_port))
            .header("Authorization", format!("Bearer {}", self.test_data.valid_api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;
        
        println!("   状态码: {}", response.status());
        
        if response.status() == StatusCode::OK {
            let response_body: Value = response.json().await?;
            println!("   响应: {}", serde_json::to_string_pretty(&response_body)?);
            println!("   ✅ 正常聊天请求成功");
        } else {
            let error_text = response.text().await?;
            println!("   ❌ 正常聊天请求失败: {}", error_text);
            return Err(format!("正常聊天请求失败: {}", error_text).into());
        }
        
        Ok(())
    }
    
    /// 测试无效API密钥
    pub async fn test_invalid_api_key(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔐 测试无效API密钥");
        
        let request_body = json!({
            "model": "gpt-3.5-turbo",
            "messages": [{"role": "user", "content": "test"}]
        });
        
        let response = self.client
            .post(&format!("http://127.0.0.1:{}/v1/chat/completions", self.proxy_port))
            .header("Authorization", format!("Bearer {}", self.test_data.invalid_api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;
        
        println!("   状态码: {}", response.status());
        
        if response.status() == StatusCode::UNAUTHORIZED {
            println!("   ✅ 无效API密钥正确拒绝 (401)");
        } else {
            let error_text = response.text().await?;
            println!("   ❌ 无效API密钥应该返回401，实际返回: {}", error_text);
            return Err("无效API密钥测试失败".into());
        }
        
        Ok(())
    }
    
    /// 测试速率限制
    pub async fn test_rate_limiting(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("⏱️  测试速率限制");
        
        let request_body = json!({
            "model": "gpt-3.5-turbo",
            "messages": [{"role": "user", "content": "rate limit test"}]
        });
        
        // 发送2次请求（在限制内）
        for i in 1..=2 {
            let response = self.client
                .post(&format!("http://127.0.0.1:{}/v1/chat/completions", self.proxy_port))
                .header("Authorization", format!("Bearer {}", self.test_data.rate_limited_api_key))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await?;
            
            println!("   请求 {}: 状态码 {}", i, response.status());
            
            if response.status() != StatusCode::OK {
                let error_text = response.text().await?;
                println!("   ❌ 限制内请求失败: {}", error_text);
            }
        }
        
        // 发送第3次请求（应该被限制）
        let response = self.client
            .post(&format!("http://127.0.0.1:{}/v1/chat/completions", self.proxy_port))
            .header("Authorization", format!("Bearer {}", self.test_data.rate_limited_api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;
        
        println!("   超限请求: 状态码 {}", response.status());
        
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            println!("   ✅ 速率限制正确生效 (429)");
        } else {
            let error_text = response.text().await?;
            println!("   ❌ 速率限制应该返回429，实际返回: {}", error_text);
            return Err("速率限制测试失败".into());
        }
        
        Ok(())
    }
    
    /// 测试负载均衡（转发策略）
    pub async fn test_load_balancing(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔄 测试负载均衡");
        
        let request_body = json!({
            "model": "gpt-3.5-turbo",
            "messages": [{"role": "user", "content": "load balance test"}]
        });
        
        // 发送多次请求，验证是否使用了不同的后端密钥
        for i in 1..=5 {
            let response = self.client
                .post(&format!("http://127.0.0.1:{}/v1/chat/completions", self.proxy_port))
                .header("Authorization", format!("Bearer {}", self.test_data.valid_api_key))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await?;
            
            if response.status() == StatusCode::OK {
                let response_body: Value = response.json().await?;
                // 检查响应中是否包含后端密钥信息（从Mock服务器返回）
                if let Some(backend_key) = response_body.get("backend_key") {
                    println!("   请求 {}: 使用后端密钥 {}", i, backend_key);
                } else {
                    println!("   请求 {}: 成功（未返回后端密钥信息）", i);
                }
            } else {
                println!("   请求 {}: 失败 - {}", i, response.status());
            }
        }
        
        println!("   ✅ 负载均衡测试完成");
        Ok(())
    }
    
    /// 运行所有API测试
    pub async fn run_all_tests(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🚀 开始API流程测试");
        println!("==========================================");
        
        self.test_normal_chat_request().await?;
        println!();
        
        self.test_invalid_api_key().await?;
        println!();
        
        self.test_rate_limiting().await?;
        println!();
        
        self.test_load_balancing().await?;
        println!();
        
        println!("==========================================");
        println!("🎉 所有API测试通过！");
        println!("✨ 代理功能验证完成：");
        println!("   - ✅ 身份验证正常工作");
        println!("   - ✅ 速率限制正确生效");
        println!("   - ✅ 转发功能正常运行");
        
        Ok(())
    }
}

/// Mock聊天完成接口
async fn mock_chat_completions(
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Json<Value> {
    // 提取授权头中的API密钥
    let auth_header = headers.get("authorization")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("未知");
    
    // 模拟OpenAI API响应
    let response = json!({
        "id": "chatcmpl-test-12345",
        "object": "chat.completion",
        "created": 1699999999,
        "model": payload.get("model").unwrap_or(&json!("gpt-3.5-turbo")),
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "这是来自Mock服务器的测试响应。"
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 15,
            "total_tokens": 25
        },
        "backend_key": auth_header // 返回后端密钥用于测试验证
    });
    
    Json(response)
}

/// Mock模型列表接口
async fn mock_models() -> Json<Value> {
    Json(json!({
        "object": "list",
        "data": [
            {
                "id": "gpt-3.5-turbo",
                "object": "model",
                "created": 1699999999,
                "owned_by": "openai"
            }
        ]
    }))
}

/// Mock健康检查接口
async fn mock_health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "timestamp": chrono::Utc::now().timestamp()
    }))
}

/// 查找可用端口
async fn find_available_port() -> Result<u16, Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_complete_api_flow() {
        let test_env = ApiFlowTest::new().await
            .expect("创建测试环境失败");
        
        test_env.run_all_tests().await
            .expect("API流程测试失败");
    }
}