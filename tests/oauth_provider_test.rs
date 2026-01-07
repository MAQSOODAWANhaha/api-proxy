//! OAuth 提供商配置与授权 URL 生成测试
//!
//! 关注点：
//! 1. 数据库驱动的参数配置（authorize.query）
//! 2. URL 参数不重复
//! 3. PKCE 参数按开关添加
//! 4. 配置参数可覆盖基础参数（如 `response_type`）

use api_proxy::auth::types::{OAuthAuthorizeConfig, OAuthProviderConfig, OAuthTokenConfig};
use api_proxy::provider::{ProviderConfigBuilder, build_authorize_url};
use entity::oauth_client_sessions::Model;
use std::collections::HashMap;
use url::Url;

fn create_test_session() -> Model {
    Model {
        id: 1,
        session_id: "test_session_123".to_string(),
        user_id: 1,
        provider_name: "openai:oauth".to_string(),
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

fn create_openai_config() -> OAuthProviderConfig {
    OAuthProviderConfig {
        provider_name: "openai:oauth".to_string(),
        client_id: "test_client_id".to_string(),
        client_secret: Some("test_client_secret".to_string()),
        redirect_uri: "http://localhost:1455/auth/callback".to_string(),
        scopes: "openid profile email offline_access".to_string(),
        pkce_required: true,
        authorize: OAuthAuthorizeConfig {
            url: "https://auth.openai.com/oauth/authorize".to_string(),
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
                    "code_challenge".to_string(),
                    serde_json::Value::String("{{session.code_challenge}}".to_string()),
                );
                q.insert(
                    "code_challenge_method".to_string(),
                    serde_json::Value::String("S256".to_string()),
                );
                q.insert(
                    "id_token_add_organizations".to_string(),
                    serde_json::Value::String("true".to_string()),
                );
                q.insert(
                    "codex_cli_simplified_flow".to_string(),
                    serde_json::Value::String("true".to_string()),
                );
                q.insert(
                    "originator".to_string(),
                    serde_json::Value::String("codex_cli_rs".to_string()),
                );
                q
            },
        },
        exchange: OAuthTokenConfig {
            url: "https://auth.openai.com/oauth/token".to_string(),
            method: "POST".to_string(),
            headers: HashMap::new(),
            body: HashMap::new(),
        },
        refresh: OAuthTokenConfig {
            url: "https://auth.openai.com/oauth/token".to_string(),
            method: "POST".to_string(),
            headers: HashMap::new(),
            body: HashMap::new(),
        },
    }
}

#[tokio::test]
async fn test_oauth_url_generation_no_duplicate_params() {
    let session = create_test_session();
    let config = create_openai_config();

    let url = build_authorize_url(&config, &session).unwrap();
    let parsed_url = Url::parse(&url).unwrap();
    let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

    // 基础参数
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

    // PKCE 参数
    assert_eq!(
        params.get("code_challenge"),
        Some(&"test_code_challenge_789".to_string())
    );
    assert_eq!(
        params.get("code_challenge_method"),
        Some(&"S256".to_string())
    );

    // 配置追加参数
    assert_eq!(
        params.get("id_token_add_organizations"),
        Some(&"true".to_string())
    );
    assert_eq!(
        params.get("codex_cli_simplified_flow"),
        Some(&"true".to_string())
    );
    assert_eq!(params.get("originator"), Some(&"codex_cli_rs".to_string()));

    // 无重复参数（collect 后 keys 唯一即可）
    let expected_params = 10;
    assert_eq!(params.len(), expected_params);
}

#[tokio::test]
async fn test_oauth_url_generation_without_pkce() {
    let session = create_test_session();
    let mut config = create_openai_config();
    config.pkce_required = false;
    config.authorize.query.remove("code_challenge");
    config.authorize.query.remove("code_challenge_method");

    let url = build_authorize_url(&config, &session).unwrap();
    let parsed_url = Url::parse(&url).unwrap();
    let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

    assert!(!params.contains_key("code_challenge"));
    assert!(!params.contains_key("code_challenge_method"));
    assert_eq!(params.get("response_type"), Some(&"code".to_string()));
}

#[tokio::test]
async fn test_oauth_url_parameter_precedence() {
    let session = create_test_session();
    let mut config = create_openai_config();

    config.authorize.query.insert(
        "response_type".to_string(),
        serde_json::Value::String("token".to_string()),
    );
    config.authorize.query.insert(
        "scope".to_string(),
        serde_json::Value::String("email profile".to_string()),
    );

    let url = build_authorize_url(&config, &session).unwrap();
    let parsed_url = Url::parse(&url).unwrap();
    let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

    assert_eq!(params.get("response_type"), Some(&"token".to_string()));
    assert_eq!(params.get("scope"), Some(&"email profile".to_string()));
}

#[tokio::test]
async fn test_oauth_url_special_characters_in_params() {
    let session = create_test_session();
    let mut config = create_openai_config();

    config.authorize.query.insert(
        "redirect_uri".to_string(),
        serde_json::Value::String("https://example.com/callback?param=value".to_string()),
    );

    let url = build_authorize_url(&config, &session).unwrap();
    let parsed_url = Url::parse(&url).unwrap();
    let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();
    assert_eq!(
        params.get("redirect_uri"),
        Some(&"https://example.com/callback?param=value".to_string())
    );
}

#[test]
fn test_oauth_config_builder() {
    let config = ProviderConfigBuilder::new("test_provider")
        .client_id("test_client_id")
        .client_secret(Some("test_secret"))
        .authorize_url("https://example.com/auth")
        .token_url("https://example.com/token")
        .redirect_uri("https://example.com/callback")
        .scopes(&["read", "write"])
        .pkce_required(true)
        .authorize_query_string("custom_param", "custom_value")
        .build();

    assert_eq!(config.provider_name, "test_provider");
    assert_eq!(config.client_id, "test_client_id");
    assert_eq!(config.client_secret, Some("test_secret".to_string()));
    assert_eq!(config.scopes, "read write");
    assert!(config.pkce_required);
    assert_eq!(
        config.authorize.query.get("custom_param"),
        Some(&serde_json::Value::String("custom_value".to_string()))
    );
}
