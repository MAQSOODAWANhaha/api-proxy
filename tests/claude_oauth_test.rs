//! Claude OAuth配置专项测试
//!
//! 测试Claude OAuth配置的scope处理问题

use api_proxy::auth::oauth_client::OAuthProviderConfig;
use api_proxy::auth::oauth_client::providers::OAuthProviderManager;
use entity::provider_types::OAuthConfig;
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;
    use entity::oauth_client_sessions::Model;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::DatabaseConnection;
    use url::Url;

    /// 创建测试用的数据库连接
    async fn create_test_db() -> DatabaseConnection {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        db
    }

    /// 创建测试用的OAuth会话
    fn create_test_session() -> Model {
        Model {
            id: 1,
            session_id: "test_claude_session_123".to_string(),
            user_id: 1,
            provider_name: "claude".to_string(),
            provider_type_id: Some(1),
            code_verifier: "test_code_verifier_012".to_string(),
            code_challenge: "test_code_challenge_789".to_string(),
            state: "test_claude_state_456".to_string(),
            name: "Test Claude Session".to_string(),
            description: Some("Test session for Claude OAuth flow".to_string()),
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

    /// 创建Claude OAuth配置（多scope测试）
    fn create_claude_oauth_config() -> OAuthConfig {
        let mut extra_params = HashMap::new();
        extra_params.insert("response_type".to_string(), "code".to_string());
        extra_params.insert("code".to_string(), "true".to_string());

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
    async fn test_claude_multiple_scopes_url_generation() {
        let db = create_test_db().await;
        let manager = OAuthProviderManager::new(db);
        let session = create_test_session();
        let oauth_config = create_claude_oauth_config();

        println!("🔍 [测试] Claude配置scopes: {}", oauth_config.scopes);

        // 模拟oauth_model_to_config方法的逻辑
        let scopes: Vec<String> = oauth_config
            .scopes
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        println!("🔍 [测试] 解析后的scopes数组: {:?}", scopes);

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

        println!("🔍 [测试] 配置的scopes: {:?}", config.scopes);

        // 生成授权URL
        let result = manager.build_authorize_url(&config, &session);
        assert!(result.is_ok(), "URL生成应该成功: {:?}", result.err());

        let url = result.unwrap();
        println!("🎯 [测试] 生成的Claude授权URL: {}", url);

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
        assert_eq!(
            params.get("state"),
            Some(&"test_claude_state_456".to_string())
        );
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

        println!("✅ [测试] Claude多scope测试通过，所有scope都正确显示");
    }

    #[tokio::test]
    async fn test_scope_split_and_join_logic() {
        // 测试scope的split和join逻辑
        let test_scopes = vec![
            "org:create_api_key user:profile user:inference",
            "openid profile email offline_access",
            "https://www.googleapis.com/auth/cloud-platform",
            "read write",
        ];

        for scope_string in test_scopes {
            println!("🔍 [测试] 原始scope字符串: '{}'", scope_string);

            // 模拟split_whitespace逻辑
            let scopes: Vec<String> = scope_string
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();

            println!("🔍 [测试] split后: {:?}", scopes);

            // 模拟join逻辑
            let rejoined = scopes.join(" ");
            println!("🔍 [测试] join后: '{}'", rejoined);

            // 验证往返转换的一致性
            assert_eq!(
                scope_string,
                rejoined,
                "Scope往返转换应该一致: '{}' -> '{}' -> '{}'",
                scope_string,
                scopes.join(" "),
                rejoined
            );
        }
    }

    #[tokio::test]
    async fn test_claude_config_with_url_encoding() {
        let db = create_test_db().await;
        let manager = OAuthProviderManager::new(db);
        let session = create_test_session();

        // 创建Claude配置，测试URL编码
        let config = OAuthProviderConfig {
            provider_name: "claude:oauth".to_string(),
            client_id: "9d1c250a-e61b-44d9-88ed-5944d1962f5e".to_string(),
            client_secret: None,
            authorize_url: "https://claude.ai/oauth/authorize".to_string(),
            token_url: "https://console.anthropic.com/v1/oauth/token".to_string(),
            redirect_uri: "https://console.anthropic.com/oauth/code/callback".to_string(),
            scopes: vec![
                "org:create_api_key".to_string(),
                "user:profile".to_string(),
                "user:inference".to_string(),
            ],
            pkce_required: true,
            extra_params: {
                let mut params = HashMap::new();
                params.insert("response_type".to_string(), "code".to_string());
                params.insert("code".to_string(), "true".to_string());
                params
            },
        };

        let result = manager.build_authorize_url(&config, &session);
        assert!(result.is_ok());

        let url = result.unwrap();
        println!("🎯 [测试] Claude URL (直接配置): {}", url);

        // 解析URL验证scope编码
        let parsed_url = Url::parse(&url).unwrap();
        let scope_param = parsed_url
            .query_pairs()
            .find(|(k, _)| k == "scope")
            .map(|(_, v)| v.to_string());

        assert_eq!(
            scope_param,
            Some("org:create_api_key user:profile user:inference".to_string())
        );
    }
}
