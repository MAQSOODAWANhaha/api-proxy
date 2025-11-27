use crate::auth::types::OAuthProviderConfig;
use crate::error::{Context, Result};
use crate::ldebug;
use crate::logging::{LogComponent, LogStage};
use entity::oauth_client_sessions;
use url::Url;

use super::registry::resolve_oauth_provider;
use super::types::ProviderType;
use std::collections::HashMap;

/// 根据会话与配置构建授权 URL。
pub fn build_authorize_url(
    config: &OAuthProviderConfig,
    session: &oauth_client_sessions::Model,
) -> Result<String> {
    ldebug!(
        "system",
        LogStage::Authentication,
        LogComponent::OAuth,
        "build_auth_url",
        &format!(
            "🔗 [OAuth] 开始构建授权URL: provider_name={}, session_id={}",
            config.provider_name, session.session_id
        )
    );

    let mut url = Url::parse(&config.authorize_url)
        .with_context(|| format!("Invalid authorize URL: {}", config.authorize_url))?;

    let scope = config.scopes.join(" ");
    let mut params = HashMap::new();
    params.insert("client_id".to_string(), config.client_id.clone());
    params.insert("redirect_uri".to_string(), config.redirect_uri.clone());
    params.insert("state".to_string(), session.state.clone());
    params.insert("scope".to_string(), scope);

    let response_type = config
        .extra_params
        .get("response_type")
        .cloned()
        .unwrap_or_else(|| "code".to_string());
    params.insert("response_type".to_string(), response_type);

    if config.pkce_required {
        params.insert("code_challenge".to_string(), session.code_challenge.clone());
        params.insert("code_challenge_method".to_string(), "S256".to_string());
    }

    // 添加额外参数，会覆盖同名的现有参数
    for (key, value) in &config.extra_params {
        params.insert(key.clone(), value.clone());
    }

    let provider_type = ProviderType::parse(
        config
            .provider_name
            .split(':')
            .next()
            .unwrap_or(config.provider_name.as_str()),
    )?;
    let provider = resolve_oauth_provider(&provider_type)?;
    provider.build_authorization_url(&mut params, session, config);

    url.query_pairs_mut().extend_pairs(&params);

    Ok(url.to_string())
}
