//! Claude OAuth 配置专项测试
//!
//! 关注点：
//! - scopes 字符串保持原样（空格分隔）
//! - 授权 URL 生成时 scope 参数正确
//! - URL 编码正确

use api_proxy::auth::types::{OAuthAuthorizeConfig, OAuthProviderConfig, OAuthTokenConfig};
use api_proxy::provider::build_authorize_url;
use entity::oauth_client_sessions::Model;
use std::collections::HashMap;
use url::Url;

fn create_test_session() -> Model {
    Model {
        id: 1,
        session_id: "test_claude_session_123".to_string(),
        user_id: 1,
        provider_name: "claude:oauth".to_string(),
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

fn create_claude_config() -> OAuthProviderConfig {
    OAuthProviderConfig {
        provider_name: "claude:oauth".to_string(),
        client_id: "9d1c250a-e61b-44d9-88ed-5944d1962f5e".to_string(),
        client_secret: None,
        redirect_uri: "https://console.anthropic.com/oauth/code/callback".to_string(),
        scopes: "org:create_api_key user:profile user:inference".to_string(),
        pkce_required: true,
        authorize: OAuthAuthorizeConfig {
            url: "https://claude.ai/oauth/authorize".to_string(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            query: {
                let mut q = HashMap::new();
                q.insert(
                    "client_id".to_string(),
                    serde_json::Value::String("{{client_id}}".to_string()),
                );
                q.insert(
                    "redirect_uri".to_string(),
                    serde_json::Value::String("{{redirect_uri}}".to_string()),
                );
                q.insert(
                    "state".to_string(),
                    serde_json::Value::String("{{session.state}}".to_string()),
                );
                q.insert(
                    "scope".to_string(),
                    serde_json::Value::String("{{scopes}}".to_string()),
                );
                q.insert(
                    "response_type".to_string(),
                    serde_json::Value::String("code".to_string()),
                );
                q.insert(
                    "code".to_string(),
                    serde_json::Value::String("true".to_string()),
                );
                q.insert(
                    "code_challenge".to_string(),
                    serde_json::Value::String("{{session.code_challenge}}".to_string()),
                );
                q.insert(
                    "code_challenge_method".to_string(),
                    serde_json::Value::String("S256".to_string()),
                );
                q
            },
        },
        exchange: OAuthTokenConfig {
            url: "https://console.anthropic.com/v1/oauth/token".to_string(),
            method: "POST".to_string(),
            headers: HashMap::new(),
            body: HashMap::new(),
        },
        refresh: OAuthTokenConfig {
            url: "https://console.anthropic.com/v1/oauth/token".to_string(),
            method: "POST".to_string(),
            headers: HashMap::new(),
            body: HashMap::new(),
        },
        extra: HashMap::new(),
    }
}

#[test]
fn claude_scopes_should_be_joined_by_space() {
    let scope_string = "org:create_api_key user:profile user:inference";
    let parts: Vec<String> = scope_string
        .split_whitespace()
        .map(std::string::ToString::to_string)
        .collect();
    assert_eq!(parts.join(" "), scope_string);
}

#[tokio::test]
async fn test_claude_authorize_url_contains_all_scopes() {
    let session = create_test_session();
    let config = create_claude_config();

    let url = build_authorize_url(&config, &session).unwrap();
    let parsed_url = Url::parse(&url).unwrap();
    let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

    assert_eq!(
        params.get("scope"),
        Some(&"org:create_api_key user:profile user:inference".to_string())
    );
    assert_eq!(params.get("code"), Some(&"true".to_string()));
    assert_eq!(params.get("response_type"), Some(&"code".to_string()));
}

#[tokio::test]
async fn test_claude_url_encoding() {
    let session = create_test_session();
    let mut config = create_claude_config();
    config.scopes = "org:create_api_key user:profile".to_string();

    let url = build_authorize_url(&config, &session).unwrap();
    let parsed_url = Url::parse(&url).unwrap();
    let scope_param = parsed_url
        .query_pairs()
        .find(|(k, _)| k == "scope")
        .map(|(_, v)| v.to_string());

    assert_eq!(
        scope_param,
        Some("org:create_api_key user:profile".to_string())
    );
}
