use crate::auth::types::{OAuthProviderConfig, OAuthTokenConfig};
use crate::error::Result;
use crate::provider::template::{
    OAuthTemplateRequest, build_oauth_template_context, lookup_oauth_template, render_json_value,
    render_template,
};
use entity::oauth_client_sessions;
use std::collections::HashMap;

/// Token 请求载荷（用于 exchange/refresh）
#[derive(Debug, Clone)]
pub struct TokenRequestPayload {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    /// `application/x-www-form-urlencoded` 的表单参数
    pub form: HashMap<String, String>,
}

pub fn build_exchange_request(
    config: &OAuthProviderConfig,
    session: &oauth_client_sessions::Model,
    authorization_code: &str,
) -> Result<TokenRequestPayload> {
    build_token_request(
        &config.exchange,
        config,
        session,
        Some(authorization_code),
        None,
    )
}

pub fn build_refresh_request(
    config: &OAuthProviderConfig,
    session: &oauth_client_sessions::Model,
    refresh_token: &str,
) -> Result<TokenRequestPayload> {
    build_token_request(&config.refresh, config, session, None, Some(refresh_token))
}

fn build_token_request(
    flow: &OAuthTokenConfig,
    config: &OAuthProviderConfig,
    session: &oauth_client_sessions::Model,
    authorization_code: Option<&str>,
    refresh_token: Option<&str>,
) -> Result<TokenRequestPayload> {
    let context = build_oauth_template_context(
        config,
        session,
        OAuthTemplateRequest {
            authorization_code,
            refresh_token,
        },
    );

    let mut headers = HashMap::new();
    for (k, v) in &flow.headers {
        headers.insert(
            k.clone(),
            render_template(v, |key| lookup_oauth_template(&context, key))?,
        );
    }

    let mut form = HashMap::new();
    for (k, v) in &flow.body {
        let rendered = render_json_value(v, |key| lookup_oauth_template(&context, key))?;
        if let Some(rendered) = rendered {
            form.insert(k.clone(), rendered);
        }
    }

    Ok(TokenRequestPayload {
        url: flow.url.clone(),
        method: flow.method.clone(),
        headers,
        form,
    })
}
