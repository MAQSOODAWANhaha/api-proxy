use crate::auth::types::OAuthProviderConfig;
use crate::error::{Context, Result};
use crate::ldebug;
use crate::logging::{LogComponent, LogStage};
use entity::oauth_client_sessions;
use std::collections::HashMap;
use url::Url;

use super::template::{
    OAuthTemplateRequest, build_oauth_template_context, lookup_oauth_template, render_json_value,
};

/// 根据会话与配置构建授权 URL。
///
/// 说明：
/// - 授权 URL 的 `query` 参数完全由数据库配置驱动（包含基础参数与 PKCE 参数）。
/// - 业务侧不再根据 OpenAI/Gemini/Anthropic 等做分支判断。
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
            "🔗 [OAuth] 构建授权URL: provider_name={}, session_id={}",
            config.provider_name, session.session_id
        )
    );

    let mut url = Url::parse(&config.authorize.url)
        .with_context(|| format!("Invalid authorize URL: {}", config.authorize.url))?;

    let context = build_oauth_template_context(
        config,
        session,
        OAuthTemplateRequest {
            authorization_code: None,
        },
    );

    let mut params: HashMap<String, String> = HashMap::new();

    for (key, value) in &config.authorize.query {
        if let Some(rendered) = render_json_value(value, |k| lookup_oauth_template(&context, k))? {
            params.insert(key.clone(), rendered);
        }
    }

    // 基础参数必须存在，否则无法完成授权流程
    for required in [
        "client_id",
        "redirect_uri",
        "state",
        "scope",
        "response_type",
    ] {
        crate::ensure!(
            params.contains_key(required),
            crate::error::conversion::ConversionError::message(format!(
                "authorize.query 缺少必需参数: {required}"
            ))
        );
    }

    url.query_pairs_mut().extend_pairs(&params);
    Ok(url.to_string())
}
