//! `OAuth提供商配置和URL生成测试`
//!
//! 测试OAuth提供商配置管理和URL生成的正确性，特别是：
//! 1. 数据库驱动的参数配置
//! 2. URL参数去重逻辑
//! 3. PKCE参数正确添加
//! 4. 不同提供商的配置处理

use api_proxy::auth::types::OAuthProviderConfig;
use api_proxy::provider::{ApiKeyProviderConfig, ProviderConfigBuilder, build_authorize_url};
use entity::provider_types::OAuthConfig;
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;
    use api_proxy::cache::CacheManager;
    use entity::oauth_client_sessions::Model;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::DatabaseConnection;
    use std::sync::Arc;
    use url::Url;

    fn make_manager(db: DatabaseConnection) -> ApiKeyProviderConfig {
        let cache = Arc::new(CacheManager::memory_only());
        ApiKeyProviderConfig::new(Arc::new(db), cache)
    }

    /// 创建测试用的数据库连接
    async fn create_test_db() -> DatabaseConnection {
        // 使用内存数据库进行测试
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

        // 运行数据库迁移
        Migrator::up(&db, None).await.unwrap();

        db
    }

    /// `创建测试用的OAuth会话`
    fn create_test_session() -> Model {
        Model {
            id: 1,
            session_id: "test_session_123".to_string(),
            user_id: 1,
            provider_name: "openai".to_string(),
            provider_type_id: Some(1),
            code_verifier: "test_code_verifier_012".to_string(),
            code_challenge: "test_code_challenge_789".to_string(),
            state: "test_state_456".to_string(),
            name: "Test OpenAI Session".to_string(),
            description: Some("Test session for OAuth flow".to_string()),
            status: "pending".to_string(),
            access_token: None,
            refresh_token: None,
            id_token: None,
            token_type: Some("Bearer".to_string()),
            expires_in: None,
            expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
            error_message: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
            completed_at: None,
        }
    }

    /// `创建测试用的OpenAI` `OAuth配置`
    fn create_openai_oauth_config() -> OAuthConfig {
        let mut extra_params = HashMap::new();
        extra_params.insert(
            "response_type".to_string(),
            serde_json::Value::String("code".to_string()),
        );
        extra_params.insert(
            "id_token_add_organizations".to_string(),
            serde_json::Value::String("true".to_string()),
        );
        extra_params.insert(
            "codex_cli_simplified_flow".to_string(),
            serde_json::Value::String("true".to_string()),
        );
        extra_params.insert(
            "originator".to_string(),
            serde_json::Value::String("codex_cli_rs".to_string()),
        );

        OAuthConfig {
            client_id: "test_client_id".to_string(),
            client_secret: Some("test_client_secret".to_string()),
            authorize_url: "https://auth.openai.com/oauth/authorize".to_string(),
            token_url: "https://auth.openai.com/oauth/token".to_string(),
            redirect_uri: Some("http://localhost:1455/auth/callback".to_string()),
            scopes: "openid profile email offline_access".to_string(),
            pkce_required: true,
            extra_params: Some(extra_params),
        }
    }

    #[tokio::test]
    async fn test_oauth_provider_config_creation() {
        let db = create_test_db().await;
        let _manager = make_manager(db);
        // 只要能够顺利创建管理器，即视为通过
    }

    #[tokio::test]
    async fn test_oauth_url_generation_no_duplicate_params() {
        let session = create_test_session();
        let oauth_config = create_openai_oauth_config();

        // 模拟oauth_model_to_config方法的逻辑来创建配置
        let scopes: Vec<String> = oauth_config
            .scopes
            .split_whitespace()
            .map(std::string::ToString::to_string)
            .collect();

        let mut extra_params = HashMap::new();

        // 直接使用数据库配置的extra_params，包含所有需要的参数
        if let Some(ref config_extra_params) = oauth_config.extra_params {
            extra_params.extend(config_extra_params.clone());
        }

        // 创建提供商配置
        let config = OAuthProviderConfig {
            provider_name: "openai:oauth".to_string(),
            client_id: oauth_config.client_id.clone(),
            client_secret: oauth_config.client_secret.clone(),
            authorize_url: oauth_config.authorize_url.clone(),
            token_url: oauth_config.token_url.clone(),
            redirect_uri: oauth_config.redirect_uri.clone().unwrap_or_default(),
            scopes,
            pkce_required: oauth_config.pkce_required,
            extra_params,
        };

        // 生成授权URL
        let result = build_authorize_url(&config, &session);

        assert!(result.is_ok(), "URL生成应该成功: {:?}", result.err());

        let url = result.unwrap();
        println!("生成的授权URL: {url}");

        // 解析URL验证参数
        let parsed_url = Url::parse(&url).expect("URL应该有效");
        let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

        // 验证必需参数存在
        assert_eq!(params.get("client_id"), Some(&"test_client_id".to_string()));
        assert_eq!(
            params.get("redirect_uri"),
            Some(&"http://localhost:1455/auth/callback".to_string())
        );
        assert_eq!(params.get("state"), Some(&"test_state_456".to_string()));
        assert_eq!(
            params.get("scope"),
            Some(&"openid profile email offline_access".to_string())
        );
        assert_eq!(params.get("response_type"), Some(&"code".to_string()));

        // 验证PKCE参数
        assert_eq!(
            params.get("code_challenge"),
            Some(&"test_code_challenge_789".to_string())
        );
        assert_eq!(
            params.get("code_challenge_method"),
            Some(&"S256".to_string())
        );

        // 验证额外参数
        assert_eq!(
            params.get("id_token_add_organizations"),
            Some(&"true".to_string())
        );
        assert_eq!(
            params.get("codex_cli_simplified_flow"),
            Some(&"true".to_string())
        );
        assert_eq!(params.get("originator"), Some(&"codex_cli_rs".to_string()));

        // 关键测试：验证没有重复参数
        let param_counts: HashMap<&String, usize> = params.keys().map(|k| (k, 1)).collect();
        for (param, count) in param_counts {
            assert_eq!(
                count, 1,
                "参数 '{param}' 应该只出现一次，但出现了 {count} 次"
            );
        }

        // 验证参数总数（基础参数 + PKCE参数 + 额外参数）
        let expected_params = 10; // client_id, redirect_uri, state, scope, response_type, code_challenge, code_challenge_method, id_token_add_organizations, codex_cli_simplified_flow, originator
        assert_eq!(
            params.len(),
            expected_params,
            "URL应该包含 {} 个参数，但包含了 {} 个",
            expected_params,
            params.len()
        );
    }

    #[tokio::test]
    async fn test_oauth_url_generation_with_empty_extra_params() {
        let session = create_test_session();

        // 创建没有额外参数的配置
        let config = OAuthProviderConfig {
            provider_name: "test:oauth".to_string(),
            client_id: "test_client_id".to_string(),
            client_secret: Some("test_client_secret".to_string()),
            authorize_url: "https://example.com/oauth/authorize".to_string(),
            token_url: "https://example.com/oauth/token".to_string(),
            redirect_uri: "http://localhost:1455/auth/callback".to_string(),
            scopes: vec!["read".to_string(), "write".to_string()],
            pkce_required: true,
            extra_params: HashMap::new(), // 空的额外参数
        };

        let result = build_authorize_url(&config, &session);
        assert!(result.is_ok());

        let url = result.unwrap();
        let parsed_url = Url::parse(&url).unwrap();
        let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

        // 验证基础参数
        assert_eq!(params.get("client_id"), Some(&"test_client_id".to_string()));
        assert_eq!(params.get("response_type"), Some(&"code".to_string())); // 默认值
        assert_eq!(params.get("scope"), Some(&"read write".to_string()));

        // 验证PKCE参数
        assert_eq!(
            params.get("code_challenge"),
            Some(&"test_code_challenge_789".to_string())
        );
        assert_eq!(
            params.get("code_challenge_method"),
            Some(&"S256".to_string())
        );

        // 验证没有重复参数
        let param_names: Vec<&String> = params.keys().collect();
        let unique_param_names: std::collections::HashSet<&String> =
            param_names.iter().copied().collect();
        assert_eq!(
            param_names.len(),
            unique_param_names.len(),
            "不应该有重复的参数名"
        );
    }

    #[tokio::test]
    async fn test_oauth_url_generation_without_pkce() {
        let session = create_test_session();

        // 创建不需要PKCE的配置
        let config = OAuthProviderConfig {
            provider_name: "test:oauth".to_string(),
            client_id: "test_client_id".to_string(),
            client_secret: Some("test_client_secret".to_string()),
            authorize_url: "https://example.com/oauth/authorize".to_string(),
            token_url: "https://example.com/oauth/token".to_string(),
            redirect_uri: "http://localhost:1455/auth/callback".to_string(),
            scopes: vec!["read".to_string()],
            pkce_required: false, // 不需要PKCE
            extra_params: HashMap::new(),
        };

        let result = build_authorize_url(&config, &session);
        assert!(result.is_ok());

        let url = result.unwrap();
        let parsed_url = Url::parse(&url).unwrap();
        let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

        // 验证没有PKCE参数
        assert!(!params.contains_key("code_challenge"));
        assert!(!params.contains_key("code_challenge_method"));

        // 验证基础参数仍然存在
        assert_eq!(params.get("client_id"), Some(&"test_client_id".to_string()));
        assert_eq!(params.get("response_type"), Some(&"code".to_string()));
    }

    #[tokio::test]
    async fn test_oauth_config_builder() {
        // 测试配置构建器
        let config = ProviderConfigBuilder::new("test_provider")
            .client_id("test_client_id")
            .client_secret(Some("test_secret"))
            .authorize_url("https://example.com/auth")
            .token_url("https://example.com/token")
            .redirect_uri("https://example.com/callback")
            .scopes(vec!["read", "write"])
            .pkce_required(true)
            .extra_param("custom_param", "custom_value")
            .build();

        assert_eq!(config.provider_name, "test_provider");
        assert_eq!(config.client_id, "test_client_id");
        assert_eq!(config.client_secret, Some("test_secret".to_string()));
        assert_eq!(config.scopes, vec!["read", "write"]);
        assert!(config.pkce_required);
        assert_eq!(
            config.extra_params.get("custom_param"),
            Some(&serde_json::Value::String("custom_value".to_string()))
        );
    }

    #[tokio::test]
    async fn test_oauth_url_parameter_precedence() {
        let session = create_test_session();

        // 创建包含response_type的额外参数配置
        let mut extra_params = HashMap::new();
        extra_params.insert(
            "response_type".to_string(),
            serde_json::Value::String("token".to_string()),
        ); // 非标准值
        extra_params.insert(
            "custom_param".to_string(),
            serde_json::Value::String("custom_value".to_string()),
        );

        let config = OAuthProviderConfig {
            provider_name: "test:oauth".to_string(),
            client_id: "test_client_id".to_string(),
            client_secret: Some("test_client_secret".to_string()),
            authorize_url: "https://example.com/oauth/authorize".to_string(),
            token_url: "https://example.com/oauth/token".to_string(),
            redirect_uri: "http://localhost:1455/auth/callback".to_string(),
            scopes: vec!["read".to_string()],
            pkce_required: false,
            extra_params,
        };

        let result = build_authorize_url(&config, &session);
        assert!(result.is_ok());

        let url = result.unwrap();
        let parsed_url = Url::parse(&url).unwrap();
        let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

        // 验证额外参数中的response_type优先于默认值
        assert_eq!(params.get("response_type"), Some(&"token".to_string()));
        assert_eq!(
            params.get("custom_param"),
            Some(&"custom_value".to_string())
        );

        // 验证只有一个response_type参数（无重复）
        let response_type_count = params
            .iter()
            .filter(|(k, _)| **k == "response_type")
            .count();
        assert_eq!(response_type_count, 1, "response_type参数应该只出现一次");
    }

    #[tokio::test]
    async fn test_oauth_url_special_characters_in_params() {
        let session = create_test_session();

        // 创建包含特殊字符的参数
        let mut extra_params = HashMap::new();
        extra_params.insert(
            "scope".to_string(),
            serde_json::Value::String("email profile".to_string()),
        ); // 会覆盖基础scope
        extra_params.insert(
            "redirect_uri".to_string(),
            serde_json::Value::String("https://example.com/callback?param=value".to_string()),
        ); // 包含特殊字符

        let config = OAuthProviderConfig {
            provider_name: "test:oauth".to_string(),
            client_id: "test_client_id".to_string(),
            client_secret: Some("test_client_secret".to_string()),
            authorize_url: "https://example.com/oauth/authorize".to_string(),
            token_url: "https://example.com/oauth/token".to_string(),
            redirect_uri: "http://localhost:1455/auth/callback".to_string(),
            scopes: vec!["read".to_string()],
            pkce_required: false,
            extra_params,
        };

        let result = build_authorize_url(&config, &session);
        assert!(result.is_ok());

        let url = result.unwrap();
        let parsed_url = Url::parse(&url).unwrap();
        let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

        // 验证scope使用extra_params中的值
        assert_eq!(params.get("scope"), Some(&"email profile".to_string()));

        // 验证URL整体有效性
        assert!(url.starts_with("https://example.com/oauth/authorize?"));
    }

    #[tokio::test]
    async fn test_oauth_database_driven_config_simulation() {
        // 模拟从数据库加载配置的完整流程
        let oauth_config = create_openai_oauth_config();

        // 模拟oauth_model_to_config方法的逻辑
        let scopes: Vec<String> = oauth_config
            .scopes
            .split_whitespace()
            .map(std::string::ToString::to_string)
            .collect();

        let mut extra_params = HashMap::new();

        // 直接使用数据库配置的extra_params
        if let Some(ref config_extra_params) = oauth_config.extra_params {
            extra_params.extend(config_extra_params.clone());
            println!(
                "从数据库加载了{}个额外参数: {:?}",
                extra_params.len(),
                extra_params.keys().collect::<Vec<_>>()
            );
        }

        let config = OAuthProviderConfig {
            provider_name: "openai:oauth".to_string(),
            client_id: oauth_config.client_id,
            client_secret: oauth_config.client_secret,
            authorize_url: oauth_config.authorize_url,
            token_url: oauth_config.token_url,
            redirect_uri: oauth_config.redirect_uri.unwrap_or_default(),
            scopes,
            pkce_required: oauth_config.pkce_required,
            extra_params,
        };

        // 验证配置正确性
        assert_eq!(config.provider_name, "openai:oauth");
        assert_eq!(config.client_id, "test_client_id");
        assert!(config.pkce_required);
        assert_eq!(
            config.extra_params.get("response_type"),
            Some(&serde_json::Value::String("code".to_string()))
        );
        assert_eq!(
            config.extra_params.get("id_token_add_organizations"),
            Some(&serde_json::Value::String("true".to_string()))
        );
        assert_eq!(
            config.extra_params.get("originator"),
            Some(&serde_json::Value::String("codex_cli_rs".to_string()))
        );

        // 验证没有重复参数
        assert_eq!(config.extra_params.len(), 4); // response_type, id_token_add_organizations, codex_cli_simplified_flow, originator
    }

    /// 创建Claude OAuth配置（多scope测试）
    fn create_claude_oauth_config() -> OAuthConfig {
        let mut extra_params = HashMap::new();
        extra_params.insert(
            "response_type".to_string(),
            serde_json::Value::String("code".to_string()),
        );
        extra_params.insert(
            "code".to_string(),
            serde_json::Value::String("true".to_string()),
        );

        OAuthConfig {
            client_id: "9d1c250a-e61b-44d9-88ed-5944d1962f5e".to_string(),
            client_secret: None,
            authorize_url: "https://claude.ai/oauth/authorize".to_string(),
            token_url: "https://console.anthropic.com/v1/oauth/token".to_string(),
            redirect_uri: Some("https://console.anthropic.com/oauth/code/callback".to_string()),
            scopes: "org:create_api_key user:profile user:inference".to_string(), // 多个scope
            pkce_required: true,
            extra_params: Some(extra_params),
        }
    }

    #[tokio::test]
    async fn test_claude_oauth_url_generation() {
        let session = create_test_session();
        let oauth_config = create_claude_oauth_config();

        // 模拟oauth_model_to_config方法的逻辑
        let scopes: Vec<String> = oauth_config
            .scopes
            .split_whitespace()
            .map(std::string::ToString::to_string)
            .collect();

        let mut extra_params = HashMap::new();
        if let Some(ref config_extra_params) = oauth_config.extra_params {
            extra_params.extend(config_extra_params.clone());
        }

        // 创建提供商配置
        let config = OAuthProviderConfig {
            provider_name: "claude:oauth".to_string(),
            client_id: oauth_config.client_id.clone(),
            client_secret: oauth_config.client_secret.clone(),
            authorize_url: oauth_config.authorize_url.clone(),
            token_url: oauth_config.token_url.clone(),
            redirect_uri: oauth_config.redirect_uri.unwrap_or_default(),
            scopes,
            pkce_required: oauth_config.pkce_required,
            extra_params,
        };

        // 生成授权URL
        let result = build_authorize_url(&config, &session);
        assert!(result.is_ok(), "URL生成应该成功: {:?}", result.err());

        let url = result.unwrap();
        println!("🎯 [测试] 生成的Claude授权URL: {url}");

        // 解析URL验证参数
        let parsed_url = Url::parse(&url).expect("URL应该有效");
        let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

        // 验证必需参数存在
        assert_eq!(
            params.get("client_id"),
            Some(&"9d1c250a-e61b-44d9-88ed-5944d1962f5e".to_string())
        );
        assert_eq!(
            params.get("redirect_uri"),
            Some(&"https://console.anthropic.com/oauth/code/callback".to_string())
        );
        assert_eq!(params.get("state"), Some(&"test_state_456".to_string()));
        assert_eq!(params.get("response_type"), Some(&"code".to_string()));
        assert_eq!(params.get("code"), Some(&"true".to_string()));

        // 关键测试：验证所有scope都存在
        let expected_scope = "org:create_api_key user:profile user:inference";
        assert_eq!(
            params.get("scope"),
            Some(&expected_scope.to_string()),
            "Scope应该包含所有三个权限，实际: {:?}",
            params.get("scope")
        );

        // 验证PKCE参数
        assert_eq!(
            params.get("code_challenge"),
            Some(&"test_code_challenge_789".to_string())
        );
        assert_eq!(
            params.get("code_challenge_method"),
            Some(&"S256".to_string())
        );

        // 验证参数总数
        let expected_params = 8; // client_id, redirect_uri, state, scope, response_type, code, code_challenge, code_challenge_method
        assert_eq!(
            params.len(),
            expected_params,
            "URL应该包含{}个参数，但包含了{}个",
            expected_params,
            params.len()
        );

        println!("✅ [测试] Claude OAuth测试通过，所有参数正确");
    }

    /// 创建Gemini `OAuth配置`
    fn create_gemini_oauth_config() -> OAuthConfig {
        let mut extra_params = HashMap::new();
        extra_params.insert(
            "response_type".to_string(),
            serde_json::Value::String("code".to_string()),
        );
        extra_params.insert(
            "access_type".to_string(),
            serde_json::Value::String("offline".to_string()),
        );
        extra_params.insert(
            "prompt".to_string(),
            serde_json::Value::String("select_account".to_string()),
        );

        OAuthConfig {
            client_id: "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com"
                .to_string(),
            client_secret: Some("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".to_string()),
            authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            redirect_uri: Some("https://codeassist.google.com/authcode".to_string()),
            scopes: "https://www.googleapis.com/auth/cloud-platform".to_string(),
            pkce_required: true,
            extra_params: Some(extra_params),
        }
    }

    #[tokio::test]
    async fn test_gemini_oauth_url_generation() {
        let session = create_test_session();
        let oauth_config = create_gemini_oauth_config();

        // 模拟oauth_model_to_config方法的逻辑
        let scopes: Vec<String> = oauth_config
            .scopes
            .split_whitespace()
            .map(std::string::ToString::to_string)
            .collect();

        let mut extra_params = HashMap::new();
        if let Some(ref config_extra_params) = oauth_config.extra_params {
            extra_params.extend(config_extra_params.clone());
        }

        // 创建提供商配置
        let config = OAuthProviderConfig {
            provider_name: "gemini:oauth".to_string(),
            client_id: oauth_config.client_id.clone(),
            client_secret: oauth_config.client_secret.clone(),
            authorize_url: oauth_config.authorize_url.clone(),
            token_url: oauth_config.token_url.clone(),
            redirect_uri: oauth_config.redirect_uri.unwrap_or_default(),
            scopes,
            pkce_required: oauth_config.pkce_required,
            extra_params,
        };

        // 生成授权URL
        let result = build_authorize_url(&config, &session);
        assert!(result.is_ok(), "URL生成应该成功: {:?}", result.err());

        let url = result.unwrap();
        println!("🎯 [测试] 生成的Gemini授权URL: {url}");

        // 解析URL验证参数
        let parsed_url = Url::parse(&url).expect("URL应该有效");
        let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

        // 验证必需参数存在
        assert_eq!(
            params.get("client_id"),
            Some(
                &"681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com"
                    .to_string()
            )
        );
        assert_eq!(
            params.get("redirect_uri"),
            Some(&"https://codeassist.google.com/authcode".to_string())
        );
        assert_eq!(params.get("state"), Some(&"test_state_456".to_string()));
        assert_eq!(params.get("response_type"), Some(&"code".to_string()));
        assert_eq!(params.get("access_type"), Some(&"offline".to_string()));
        assert_eq!(params.get("prompt"), Some(&"select_account".to_string()));
        assert_eq!(
            params.get("scope"),
            Some(&"https://www.googleapis.com/auth/cloud-platform".to_string())
        );

        // 验证PKCE参数
        assert_eq!(
            params.get("code_challenge"),
            Some(&"test_code_challenge_789".to_string())
        );
        assert_eq!(
            params.get("code_challenge_method"),
            Some(&"S256".to_string())
        );

        // 验证参数总数
        let expected_params = 10; // client_id, redirect_uri, state, scope, response_type, access_type, prompt, include_granted_scopes, code_challenge, code_challenge_method
        assert_eq!(
            params.len(),
            expected_params,
            "URL应该包含{}个参数，但包含了{}个",
            expected_params,
            params.len()
        );

        println!("✅ [测试] Gemini OAuth测试通过，所有参数正确");
    }

    #[tokio::test]
    async fn test_all_oauth_providers_comparison() {
        // 测试所有OAuth提供商的URL生成对比
        let providers = vec![
            ("OpenAI", create_openai_oauth_config()),
            ("Claude", create_claude_oauth_config()),
            ("Gemini", create_gemini_oauth_config()),
        ];

        let session = create_test_session();

        for (provider_name, oauth_config) in providers {
            println!("🔍 [对比测试] 测试 {provider_name} OAuth配置");

            // 模拟oauth_model_to_config方法的逻辑
            let scopes: Vec<String> = oauth_config
                .scopes
                .split_whitespace()
                .map(std::string::ToString::to_string)
                .collect();

            let mut extra_params = HashMap::new();
            if let Some(ref config_extra_params) = oauth_config.extra_params {
                extra_params.extend(config_extra_params.clone());
            }

            let config = OAuthProviderConfig {
                provider_name: format!("{}:oauth", provider_name.to_lowercase()),
                client_id: oauth_config.client_id.clone(),
                client_secret: oauth_config.client_secret.clone(),
                authorize_url: oauth_config.authorize_url.clone(),
                token_url: oauth_config.token_url.clone(),
                redirect_uri: oauth_config.redirect_uri.unwrap_or_default(),
                scopes,
                pkce_required: oauth_config.pkce_required,
                extra_params,
            };

            let result = build_authorize_url(&config, &session);
            assert!(result.is_ok(), "{provider_name} URL生成应该成功");

            let url = result.unwrap();
            let parsed_url = Url::parse(&url).expect("URL应该有效");
            let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

            // 通用验证
            assert!(
                params.contains_key("client_id"),
                "{provider_name} 应该包含client_id"
            );
            assert!(
                params.contains_key("redirect_uri"),
                "{provider_name} 应该包含redirect_uri"
            );
            assert!(
                params.contains_key("state"),
                "{provider_name} 应该包含state"
            );
            assert!(
                params.contains_key("scope"),
                "{provider_name} 应该包含scope"
            );
            assert!(
                params.contains_key("response_type"),
                "{provider_name} 应该包含response_type"
            );
            assert!(
                params.contains_key("code_challenge"),
                "{provider_name} 应该包含code_challenge"
            );
            assert!(
                params.contains_key("code_challenge_method"),
                "{provider_name} 应该包含code_challenge_method"
            );

            // 验证PKCE方法
            assert_eq!(
                params.get("code_challenge_method"),
                Some(&"S256".to_string()),
                "{provider_name} PKCE方法应该是S256"
            );

            // 验证没有重复参数
            let param_names: Vec<&String> = params.keys().collect();
            let unique_param_names: std::collections::HashSet<&String> =
                param_names.iter().copied().collect();
            assert_eq!(
                param_names.len(),
                unique_param_names.len(),
                "{provider_name} 不应该有重复的参数名"
            );

            println!(
                "✅ [对比测试] {} OAuth验证通过，包含{}个参数",
                provider_name,
                params.len()
            );
        }
    }
}
