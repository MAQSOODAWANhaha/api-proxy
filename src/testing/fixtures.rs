//! # 测试数据 Fixtures
//!
//! 提供测试用的数据结构和预设数据

use entity::{
    provider_types, user_provider_keys, user_sessions, users,
};
use sea_orm::Set;
use serde_json::json;
use std::collections::HashMap;

/// 用户测试数据构建器
pub struct UserFixture {
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub salt: String,
    pub is_active: bool,
    pub is_admin: bool,
}

impl Default for UserFixture {
    fn default() -> Self {
        Self {
            username: "test_user".to_string(),
            email: "test@example.com".to_string(),
            password_hash: "hashed_password_123".to_string(),
            salt: "random_salt_456".to_string(),
            is_active: true,
            is_admin: false,
        }
    }
}

impl UserFixture {
    /// 创建新的用户 fixture
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置用户名
    pub fn username(mut self, username: &str) -> Self {
        self.username = username.to_string();
        self
    }

    /// 设置邮箱
    pub fn email(mut self, email: &str) -> Self {
        self.email = email.to_string();
        self
    }

    /// 设置为管理员
    pub fn admin(mut self) -> Self {
        self.is_admin = true;
        self
    }

    /// 设置为非激活状态
    pub fn inactive(mut self) -> Self {
        self.is_active = false;
        self
    }

    /// 转换为 Sea-ORM ActiveModel
    pub fn to_active_model(self) -> users::ActiveModel {
        users::ActiveModel {
            username: Set(self.username),
            email: Set(self.email),
            password_hash: Set(self.password_hash),
            salt: Set(self.salt),
            is_active: Set(self.is_active),
            is_admin: Set(self.is_admin),
            ..Default::default()
        }
    }

    /// 转换为 Model（用于测试断言）
    pub fn to_model_with_id(self, id: i32) -> users::Model {
        users::Model {
            id,
            username: self.username,
            email: self.email,
            password_hash: self.password_hash,
            salt: self.salt,
            is_active: self.is_active,
            is_admin: self.is_admin,
            last_login: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        }
    }
}

/// 用户会话测试数据构建器
pub struct UserSessionFixture {
    pub user_id: i32,
    pub token_hash: String,
    pub refresh_token_hash: Option<String>,
    pub expires_at: chrono::NaiveDateTime,
}

impl Default for UserSessionFixture {
    fn default() -> Self {
        Self {
            user_id: 1,
            token_hash: "hashed_token_abc123".to_string(),
            refresh_token_hash: Some("refresh_hash_456".to_string()),
            expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(24),
        }
    }
}

impl UserSessionFixture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn user_id(mut self, user_id: i32) -> Self {
        self.user_id = user_id;
        self
    }

    pub fn token_hash(mut self, token_hash: &str) -> Self {
        self.token_hash = token_hash.to_string();
        self
    }

    pub fn expired(mut self) -> Self {
        self.expires_at = chrono::Utc::now().naive_utc() - chrono::Duration::hours(1);
        self
    }

    pub fn to_active_model(self) -> user_sessions::ActiveModel {
        user_sessions::ActiveModel {
            user_id: Set(self.user_id),
            token_hash: Set(self.token_hash),
            refresh_token_hash: Set(self.refresh_token_hash),
            expires_at: Set(self.expires_at),
            ..Default::default()
        }
    }
}

/// 提供商类型测试数据
pub struct ProviderTypeFixture;

impl ProviderTypeFixture {
    /// 获取 OpenAI 提供商数据
    pub fn openai() -> provider_types::ActiveModel {
        provider_types::ActiveModel {
            name: Set("openai".to_string()),
            display_name: Set("OpenAI".to_string()),
            base_url: Set("https://api.openai.com".to_string()),
            api_format: Set("openai".to_string()),
            default_model: Set(Some("gpt-3.5-turbo".to_string())),
            max_tokens: Set(Some(4096)),
            rate_limit: Set(Some(60)),
            timeout_seconds: Set(Some(30)),
            health_check_path: Set(Some("/v1/models".to_string())),
            auth_header_format: Set(Some("Bearer {api_key}".to_string())),
            is_active: Set(true),
            config_json: Set(Some(json!({
                "api_key": {"type": "string", "required": true},
                "organization": {"type": "string", "required": false}
            }).to_string())),
            ..Default::default()
        }
    }

    /// 获取 Google Gemini 提供商数据
    pub fn gemini() -> provider_types::ActiveModel {
        provider_types::ActiveModel {
            name: Set("gemini".to_string()),
            display_name: Set("Google Gemini".to_string()),
            base_url: Set("https://generativelanguage.googleapis.com".to_string()),
            api_format: Set("gemini".to_string()),
            default_model: Set(Some("gemini-pro".to_string())),
            max_tokens: Set(Some(8192)),
            rate_limit: Set(Some(60)),
            timeout_seconds: Set(Some(30)),
            health_check_path: Set(Some("/v1/models".to_string())),
            auth_header_format: Set(Some("x-goog-api-key: {api_key}".to_string())),
            is_active: Set(true),
            config_json: Set(Some(json!({
                "api_key": {"type": "string", "required": true}
            }).to_string())),
            ..Default::default()
        }
    }

    /// 获取 Anthropic Claude 提供商数据
    pub fn claude() -> provider_types::ActiveModel {
        provider_types::ActiveModel {
            name: Set("claude".to_string()),
            display_name: Set("Anthropic Claude".to_string()),
            base_url: Set("https://api.anthropic.com".to_string()),
            api_format: Set("anthropic".to_string()),
            default_model: Set(Some("claude-3-sonnet-20240229".to_string())),
            max_tokens: Set(Some(4096)),
            rate_limit: Set(Some(50)),
            timeout_seconds: Set(Some(30)),
            health_check_path: Set(Some("/v1/messages".to_string())),
            auth_header_format: Set(Some("x-api-key: {api_key}".to_string())),
            is_active: Set(true),
            config_json: Set(Some(json!({
                "api_key": {"type": "string", "required": true}
            }).to_string())),
            ..Default::default()
        }
    }
}

/// 用户提供商密钥测试数据构建器
pub struct UserProviderKeyFixture {
    pub user_id: i32,
    pub provider_type_id: i32,
    pub api_key: String,
    pub name: String,
    pub weight: Option<i32>,
    pub max_requests_per_minute: Option<i32>,
    pub max_tokens_per_day: Option<i32>,
    pub used_tokens_today: Option<i32>,
    pub is_active: bool,
}

impl Default for UserProviderKeyFixture {
    fn default() -> Self {
        Self {
            user_id: 1,
            provider_type_id: 1,
            api_key: "sk-test1234567890abcdef".to_string(),
            name: "Default OpenAI Key".to_string(),
            weight: Some(100),
            max_requests_per_minute: Some(60),
            max_tokens_per_day: Some(10000),
            used_tokens_today: Some(0),
            is_active: true,
        }
    }
}

impl UserProviderKeyFixture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn user_id(mut self, user_id: i32) -> Self {
        self.user_id = user_id;
        self
    }

    pub fn provider_type_id(mut self, provider_type_id: i32) -> Self {
        self.provider_type_id = provider_type_id;
        self
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn weight(mut self, weight: i32) -> Self {
        self.weight = Some(weight);
        self
    }

    pub fn api_key(mut self, api_key: &str) -> Self {
        self.api_key = api_key.to_string();
        self
    }

    pub fn inactive(mut self) -> Self {
        self.is_active = false;
        self
    }

    pub fn to_active_model(self) -> user_provider_keys::ActiveModel {
        user_provider_keys::ActiveModel {
            user_id: Set(self.user_id),
            provider_type_id: Set(self.provider_type_id),
            api_key: Set(self.api_key),
            name: Set(self.name),
            weight: Set(self.weight),
            max_requests_per_minute: Set(self.max_requests_per_minute),
            max_tokens_per_day: Set(self.max_tokens_per_day),
            used_tokens_today: Set(self.used_tokens_today),
            is_active: Set(self.is_active),
            ..Default::default()
        }
    }
}

/// 测试配置数据
pub struct TestConfig;

impl TestConfig {
    /// 获取测试用的应用配置
    pub fn app_config() -> crate::config::AppConfig {
        crate::config::AppConfig {
            server: crate::config::ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 0, // 使用随机端口
                https_port: 0,
                workers: 1,
            },
            database: crate::config::DatabaseConfig {
                url: ":memory:".to_string(), // 内存数据库
                max_connections: 1,
                connect_timeout: 5,
                query_timeout: 5,
            },
            redis: crate::config::RedisConfig {
                url: "redis://127.0.0.1:6379/15".to_string(), // 使用测试数据库
                pool_size: 1,
                host: "127.0.0.1".to_string(),
                port: 6379,
                database: 15,
                password: None,
                connection_timeout: 5,
                default_ttl: 300,
                max_connections: 1,
            },
            tls: crate::config::TlsConfig {
                cert_path: "./test_certs".to_string(),
                acme_email: "test@example.com".to_string(),
                domains: vec!["localhost".to_string()],
            },
        }
    }

    /// 获取测试数据目录路径
    pub fn test_data_dir() -> std::path::PathBuf {
        std::env::temp_dir().join("api_proxy_tests")
    }

    /// 创建测试数据目录
    pub fn create_test_data_dir() -> std::path::PathBuf {
        let dir = Self::test_data_dir();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}

/// API 健康状态测试数据
pub struct ApiHealthFixture;

impl ApiHealthFixture {
    /// 创建健康的 API 状态
    pub fn healthy(provider: &str, api_name: &str) -> HashMap<String, serde_json::Value> {
        HashMap::from([
            ("provider".to_string(), json!(provider)),
            ("api_name".to_string(), json!(api_name)),
            ("status".to_string(), json!("healthy")),
            ("response_time_ms".to_string(), json!(150)),
            ("success_rate".to_string(), json!(0.99)),
            ("last_check".to_string(), json!(chrono::Utc::now().to_rfc3339())),
        ])
    }

    /// 创建不健康的 API 状态
    pub fn unhealthy(provider: &str, api_name: &str) -> HashMap<String, serde_json::Value> {
        HashMap::from([
            ("provider".to_string(), json!(provider)),
            ("api_name".to_string(), json!(api_name)),
            ("status".to_string(), json!("unhealthy")),
            ("response_time_ms".to_string(), json!(5000)),
            ("success_rate".to_string(), json!(0.45)),
            ("last_check".to_string(), json!(chrono::Utc::now().to_rfc3339())),
            ("error_message".to_string(), json!("Connection timeout")),
        ])
    }
}

/// 请求统计测试数据
pub struct RequestStatsFixture;

impl RequestStatsFixture {
    /// 创建请求统计数据
    pub fn create(
        user_id: i32,
        provider: &str,
        api_name: &str,
        request_count: i32,
        success_count: i32,
        total_tokens: i32,
    ) -> HashMap<String, serde_json::Value> {
        HashMap::from([
            ("user_id".to_string(), json!(user_id)),
            ("provider".to_string(), json!(provider)),
            ("api_name".to_string(), json!(api_name)),
            ("request_count".to_string(), json!(request_count)),
            ("success_count".to_string(), json!(success_count)),
            ("error_count".to_string(), json!(request_count - success_count)),
            ("total_tokens".to_string(), json!(total_tokens)),
            ("avg_response_time_ms".to_string(), json!(200)),
            ("timestamp".to_string(), json!(chrono::Utc::now().to_rfc3339())),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_fixture() {
        let user = UserFixture::new()
            .username("test_admin")
            .email("admin@test.com")
            .admin()
            .to_model_with_id(1);

        assert_eq!(user.username, "test_admin");
        assert_eq!(user.email, "admin@test.com");
        assert!(user.is_admin);
        assert!(user.is_active);
    }

    #[test]
    fn test_user_session_fixture() {
        let session = UserSessionFixture::new()
            .user_id(123)
            .token_hash("custom_token_hash")
            .to_active_model();

        assert_eq!(session.user_id.as_ref(), &123);
        assert_eq!(session.token_hash.as_ref(), "custom_token_hash");
    }

    #[test]
    fn test_provider_type_fixtures() {
        let openai = ProviderTypeFixture::openai();
        assert_eq!(openai.name.as_ref(), "openai");
        assert_eq!(openai.display_name.as_ref(), "OpenAI");

        let gemini = ProviderTypeFixture::gemini();
        assert_eq!(gemini.name.as_ref(), "gemini");
        assert_eq!(gemini.display_name.as_ref(), "Google Gemini");

        let claude = ProviderTypeFixture::claude();
        assert_eq!(claude.name.as_ref(), "claude");
        assert_eq!(claude.display_name.as_ref(), "Anthropic Claude");
    }

    #[test]
    fn test_test_config() {
        let config = TestConfig::app_config();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.database.url, ":memory:");
        assert_eq!(config.redis.database, 15);
    }

    #[test]
    fn test_api_health_fixture() {
        let healthy = ApiHealthFixture::healthy("openai", "chat");
        assert_eq!(healthy["status"], json!("healthy"));
        assert_eq!(healthy["provider"], json!("openai"));

        let unhealthy = ApiHealthFixture::unhealthy("gemini", "completion");
        assert_eq!(unhealthy["status"], json!("unhealthy"));
        assert_eq!(unhealthy["provider"], json!("gemini"));
    }

    #[test]
    fn test_request_stats_fixture() {
        let stats = RequestStatsFixture::create(1, "openai", "chat", 100, 95, 15000);
        assert_eq!(stats["user_id"], json!(1));
        assert_eq!(stats["request_count"], json!(100));
        assert_eq!(stats["success_count"], json!(95));
        assert_eq!(stats["error_count"], json!(5));
    }
}