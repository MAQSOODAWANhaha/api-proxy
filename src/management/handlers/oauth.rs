//! # OAuth认证处理器
//!
//! 处理OAuth认证流程，包括授权、回调、刷新等功能

use crate::auth::extract_user_id_from_headers;
use crate::management::{response, server::AppState};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::Json;
use chrono::{Duration, Utc};
use entity::{oauth_sessions, provider_types, user_provider_keys};
use sea_orm::{entity::*, query::*, ActiveValue};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
use base64::{Engine as _, engine::general_purpose};
use sha2::{Sha256, Digest};
use std::collections::HashMap;

/// OAuth授权请求
#[derive(Debug, Deserialize)]
pub struct OAuthAuthorizeRequest {
    /// 服务商类型ID
    pub provider_type_id: i32,
    /// 认证类型
    pub auth_type: String,
    /// API Key名称
    pub name: String,
    /// 描述信息
    pub description: Option<String>,
    /// 重定向URI
    pub redirect_uri: Option<String>,
}

/// OAuth回调查询参数
#[derive(Debug, Deserialize)]
pub struct OAuthCallbackParams {
    /// 授权码
    pub code: String,
    /// 状态参数
    pub state: String,
    /// 会话ID（可选）
    pub session_id: Option<String>,
}

/// OAuth刷新请求
#[derive(Debug, Deserialize)]
pub struct OAuthRefreshRequest {
    /// 用户提供商密钥ID
    pub provider_key_id: i32,
}

/// 启动OAuth授权流程
pub async fn initiate_oauth_flow(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<OAuthAuthorizeRequest>,
) -> axum::response::Response {
    // 验证用户身份
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // 获取服务商类型信息
    let provider = match provider_types::Entity::find_by_id(req.provider_type_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(provider)) => provider,
        Ok(None) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "INVALID_PROVIDER",
                "无效的服务商类型",
            );
        }
        Err(err) => {
            tracing::error!("Failed to fetch provider: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "数据库查询失败",
            );
        }
    };

    // 验证认证类型是否支持
    let supported_auth_types: Vec<String> = serde_json::from_str::<Vec<String>>(&provider.supported_auth_types).unwrap_or_else(|_| vec!["api_key".to_string()]);

    if !supported_auth_types.contains(&req.auth_type) {
        return response::error(
            StatusCode::BAD_REQUEST,
            "UNSUPPORTED_AUTH_TYPE",
            &format!("服务商不支持认证类型: {}", req.auth_type),
        );
    }

    // 解析OAuth配置
    let auth_configs: serde_json::Value = provider.auth_configs_json
        .as_ref()
        .and_then(|config_json| serde_json::from_str(config_json).ok())
        .unwrap_or_else(|| json!({}));

    let oauth_config = match auth_configs.get(&req.auth_type) {
        Some(config) => config,
        None => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "MISSING_AUTH_CONFIG",
                &format!("缺少认证类型配置: {}", req.auth_type),
            );
        }
    };

    // 生成PKCE参数
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);
    
    // 生成state参数
    let state_param = Uuid::new_v4().to_string();
    let session_id = Uuid::new_v4().to_string();

    // 获取OAuth配置的scopes
    let scopes = oauth_config["scopes"]
        .as_str()
        .unwrap_or("default_scope")
        .split(' ')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    // 构建重定向URI
    let redirect_uri = req.redirect_uri.unwrap_or_else(|| {
        format!("{}://{}:{}/oauth/callback", 
            if cfg!(debug_assertions) { "http" } else { "https" },
            if cfg!(debug_assertions) { "localhost" } else { "api.example.com" },
            if cfg!(debug_assertions) { "9090" } else { "443" }
        )
    });

    // 创建OAuth会话记录
    let oauth_session = oauth_sessions::ActiveModel {
        id: ActiveValue::NotSet,
        session_id: ActiveValue::Set(session_id.clone()),
        user_id: ActiveValue::Set(user_id),
        provider_type_id: ActiveValue::Set(req.provider_type_id),
        auth_type: ActiveValue::Set(req.auth_type.clone()),
        state: ActiveValue::Set(state_param.clone()),
        code_verifier: ActiveValue::Set(Some(code_verifier.clone())),
        code_challenge: ActiveValue::Set(Some(code_challenge.clone())),
        redirect_uri: ActiveValue::Set(redirect_uri.clone()),
        scopes: ActiveValue::Set(Some(scopes.join(" "))),
        created_at: ActiveValue::Set(Utc::now().naive_utc()),
        expires_at: ActiveValue::Set((Utc::now() + Duration::minutes(15)).naive_utc()), // 15分钟过期
        completed_at: ActiveValue::Set(None),
        error_message: ActiveValue::Set(None),
    };

    let _inserted_session = match oauth_sessions::Entity::insert(oauth_session)
        .exec(state.database.as_ref())
        .await
    {
        Ok(result) => result,
        Err(err) => {
            tracing::error!("Failed to create OAuth session: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "创建OAuth会话失败",
            );
        }
    };

    // 构建授权URL
    let authorize_url = oauth_config.get("authorize_url")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    
    let client_id = oauth_config.get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    
    let scopes = oauth_config.get("scopes")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    // 获取可选的OAuth参数
    let access_type = oauth_config.get("access_type")
        .and_then(|v| v.as_str())
        .unwrap_or("online");
    
    let prompt = oauth_config.get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("consent");

    let mut auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&access_type={}&prompt={}",
        authorize_url,
        urlencoding::encode(client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(scopes),
        state_param,
        access_type,
        prompt
    );

    // 如果需要PKCE
    if oauth_config.get("pkce_required").and_then(|v| v.as_bool()).unwrap_or(false) {
        auth_url.push_str(&format!(
            "&code_challenge={}&code_challenge_method=S256",
            code_challenge
        ));
    }

    let response_data = json!({
        "authorization_url": auth_url,
        "session_id": session_id,
        "state": state_param,
        "expires_at": (Utc::now() + Duration::minutes(15)).to_rfc3339()
    });

    response::success(response_data)
}

/// 处理OAuth回调
pub async fn handle_oauth_callback(
    State(state): State<AppState>,
    Query(params): Query<OAuthCallbackParams>,
) -> axum::response::Response {
    // 查找OAuth会话
    let session = match oauth_sessions::Entity::find()
        .filter(oauth_sessions::Column::State.eq(&params.state))
        .filter(oauth_sessions::Column::CompletedAt.is_null())
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(session)) => session,
        Ok(None) => {
            return create_oauth_error_html("无效的OAuth会话", "会话不存在或状态参数错误");
        }
        Err(err) => {
            tracing::error!("Failed to fetch OAuth session: {}", err);
            return create_oauth_error_html("数据库查询失败", &err.to_string());
        }
    };

    // 检查会话是否过期
    if session.expires_at < Utc::now().naive_utc() {
        return create_oauth_error_html("OAuth会话已过期", "请重新发起授权流程");
    }

    // 获取服务商信息
    let provider = match provider_types::Entity::find_by_id(session.provider_type_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(provider)) => provider,
        Err(err) => {
            tracing::error!("Failed to fetch provider: {}", err);
            return create_oauth_error_html("查询服务商信息失败", &err.to_string());
        }
        _ => {
            return create_oauth_error_html("无效的服务商", "服务商配置不存在");
        }
    };

    // 解析OAuth配置
    let auth_configs: serde_json::Value = provider.auth_configs_json
        .as_ref()
        .and_then(|config_json| serde_json::from_str(config_json).ok())
        .unwrap_or_else(|| json!({}));

    let oauth_config = auth_configs.get(&session.auth_type).unwrap();

    // 克隆session以避免所有权问题
    let session_clone = session.clone();
    
    // 真实的OAuth token交换
    let (access_token, refresh_token, expires_in) = match exchange_code_for_tokens(
        &params.code,
        &session_clone.code_verifier.clone().unwrap_or_default(),
        &session_clone.redirect_uri,
        oauth_config,
    ).await {
        Ok(tokens) => tokens,
        Err(err) => {
            tracing::error!("OAuth token交换失败: {}", err);
            return create_oauth_error_html("OAuth token交换失败", &err.to_string());
        }
    };

    // 更新OAuth会话状态，标记为已完成
    let mut session_update: oauth_sessions::ActiveModel = session.into();
    session_update.completed_at = ActiveValue::Set(Some(Utc::now().naive_utc()));
    session_update.error_message = ActiveValue::Set(None);

    if let Err(err) = oauth_sessions::Entity::update(session_update)
        .exec(state.database.as_ref())
        .await
    {
        tracing::error!("Failed to update OAuth session: {}", err);
        return create_oauth_error_html("更新OAuth会话失败", &err.to_string());
    }

    // 构造OAuth成功的数据
    let oauth_result = json!({
        "access_token": access_token,
        "refresh_token": refresh_token,
        "token_type": "Bearer",
        "expires_in": expires_in,
        "expires_at": (Utc::now() + Duration::seconds(expires_in as i64)).to_rfc3339(),
        "auth_type": session_clone.auth_type,
        "provider_type_id": session_clone.provider_type_id,
        "session_id": session_clone.session_id,
        "auth_status": "authorized"
    });

    // 返回HTML页面，通过postMessage将结果发送给父窗口
    let html_content = format!(r#"
<!DOCTYPE html>
<html>
<head>
    <title>OAuth授权成功</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
        }}
        .container {{
            text-align: center;
            padding: 2rem;
            background: rgba(255, 255, 255, 0.1);
            border-radius: 10px;
            backdrop-filter: blur(10px);
        }}
        .checkmark {{
            width: 60px;
            height: 60px;
            border-radius: 50%;
            display: block;
            stroke-width: 2;
            stroke: #4CAF50;
            stroke-miterlimit: 10;
            margin: 10px auto;
            box-shadow: inset 0px 0px 0px #4CAF50;
            animation: fill 0.4s ease-in-out 0.4s forwards, scale 0.3s ease-in-out 0.9s both;
        }}
        .checkmark-circle {{
            stroke-dasharray: 166;
            stroke-dashoffset: 166;
            stroke-width: 2;
            stroke-miterlimit: 10;
            stroke: #4CAF50;
            fill: none;
            animation: stroke 0.6s cubic-bezier(0.65, 0, 0.45, 1) forwards;
        }}
        .checkmark-check {{
            transform-origin: 50% 50%;
            stroke-dasharray: 48;
            stroke-dashoffset: 48;
            animation: stroke 0.3s cubic-bezier(0.65, 0, 0.45, 1) 0.8s forwards;
        }}
        @keyframes stroke {{
            100% {{ stroke-dashoffset: 0; }}
        }}
        @keyframes scale {{
            0%, 100% {{ transform: none; }}
            50% {{ transform: scale3d(1.1, 1.1, 1); }}
        }}
        @keyframes fill {{
            100% {{ box-shadow: inset 0px 0px 0px 30px #4CAF50; }}
        }}
        h1 {{ margin: 1rem 0; font-size: 1.5rem; }}
        p {{ margin: 0.5rem 0; opacity: 0.9; }}
    </style>
</head>
<body>
    <div class="container">
        <svg class="checkmark" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 52 52">
            <circle class="checkmark-circle" cx="26" cy="26" r="25" fill="none"/>
            <path class="checkmark-check" fill="none" d="m14.1 27.2l7.1 7.2 16.7-16.8"/>
        </svg>
        <h1>OAuth授权成功</h1>
        <p>正在返回授权信息...</p>
        <p style="font-size: 0.9rem; margin-top: 1rem;">此窗口将自动关闭</p>
    </div>

    <script>
        try {{
            // 向父窗口发送OAuth成功消息
            const result = {oauth_result};
            window.parent.postMessage({{
                type: 'OAUTH_SUCCESS',
                data: result
            }}, window.location.origin);
            
            console.log('OAuth授权成功，已发送数据给父窗口');
            
            // 2秒后自动关闭窗口
            setTimeout(() => {{
                window.close();
            }}, 2000);
        }} catch (error) {{
            console.error('发送OAuth结果时出错:', error);
            window.parent.postMessage({{
                type: 'OAUTH_ERROR',
                error: {{ message: '发送授权结果失败: ' + error.message }}
            }}, window.location.origin);
        }}
    </script>
</body>
</html>
    "#, oauth_result = serde_json::to_string(&oauth_result).unwrap_or_else(|_| "{}".to_string()));

    Html(html_content).into_response()
}

/// 查询OAuth状态
pub async fn get_oauth_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> axum::response::Response {
    // 验证用户身份
    let _user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // 查找OAuth会话
    let session = match oauth_sessions::Entity::find()
        .filter(oauth_sessions::Column::SessionId.eq(&session_id))
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(session)) => session,
        Ok(None) => {
            return response::error(
                StatusCode::NOT_FOUND,
                "SESSION_NOT_FOUND",
                "OAuth会话不存在",
            );
        }
        Err(err) => {
            tracing::error!("Failed to fetch OAuth session: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "查询OAuth会话失败",
            );
        }
    };

    let response_data = json!({
        "session_id": session.session_id,
        "status": if session.completed_at.is_some() { "completed" } else { "pending" },
        "provider_type_id": session.provider_type_id,
        "auth_type": session.auth_type,
        "created_at": session.created_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "expires_at": session.expires_at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    });

    response::success(response_data)
}

/// 刷新OAuth访问令牌
pub async fn refresh_oauth_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<OAuthRefreshRequest>,
) -> axum::response::Response {
    // 验证用户身份
    let _user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // 查找用户提供商密钥
    let user_key = match user_provider_keys::Entity::find_by_id(req.provider_key_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(key)) => key,
        Ok(None) => {
            return response::error(
                StatusCode::NOT_FOUND,
                "KEY_NOT_FOUND",
                "API密钥不存在",
            );
        }
        Err(err) => {
            tracing::error!("Failed to fetch user provider key: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "查询API密钥失败",
            );
        }
    };

    // 真实的OAuth token刷新
    let user_key_clone = user_key.clone();
    let auth_config: serde_json::Value = serde_json::from_str(
        &user_key_clone.auth_config_json.unwrap_or_default()
    ).unwrap_or_else(|_| json!({}));
    
    let refresh_token = match auth_config["refresh_token"].as_str() {
        Some(token) => token,
        None => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "MISSING_REFRESH_TOKEN",
                "缺少刷新令牌",
            );
        }
    };

    // 获取provider配置进行token刷新
    let provider = match provider_types::Entity::find_by_id(user_key.provider_type_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(provider)) => provider,
        _ => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "PROVIDER_NOT_FOUND",
                "服务商类型不存在",
            );
        }
    };

    let auth_configs: serde_json::Value = serde_json::from_str(
        &provider.auth_configs_json.unwrap_or_default()
    ).unwrap_or_else(|_| json!({}));
    
    let oauth_config = match auth_configs.get(&user_key.auth_type) {
        Some(config) => config,
        None => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "MISSING_OAUTH_CONFIG",
                "缺少OAuth配置",
            );
        }
    };

    let (new_access_token, _new_refresh_token, expires_in) = match refresh_access_token(
        refresh_token,
        oauth_config,
    ).await {
        Ok(tokens) => tokens,
        Err(err) => {
            tracing::error!("OAuth token刷新失败: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "TOKEN_REFRESH_FAILED",
                "Token刷新失败",
            );
        }
    };
    
    let new_expires_at = (Utc::now() + Duration::seconds(expires_in as i64)).naive_utc();

    // 更新API密钥
    let mut key_update: user_provider_keys::ActiveModel = user_key.into();
    key_update.api_key = ActiveValue::Set(new_access_token);
    key_update.expires_at = ActiveValue::Set(Some(new_expires_at));
    key_update.last_auth_check = ActiveValue::Set(Some(Utc::now().naive_utc()));
    key_update.updated_at = ActiveValue::Set(Utc::now().naive_utc());

    if let Err(err) = user_provider_keys::Entity::update(key_update)
        .exec(state.database.as_ref())
        .await
    {
        tracing::error!("Failed to update user provider key: {}", err);
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_ERROR",
            "更新API密钥失败",
        );
    }

    let response_data = json!({
        "provider_key_id": req.provider_key_id,
        "new_expires_at": new_expires_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "refreshed_at": Utc::now().to_rfc3339()
    });

    response::success(response_data)
}

/// 撤销OAuth授权
pub async fn revoke_oauth_authorization(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key_id): Path<i32>,
) -> axum::response::Response {
    // 验证用户身份
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // 查找并验证用户提供商密钥
    let _user_key = match user_provider_keys::Entity::find_by_id(key_id)
        .filter(user_provider_keys::Column::UserId.eq(user_id)) // 确保只能操作自己的密钥
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(key)) => key,
        Ok(None) => {
            return response::error(
                StatusCode::NOT_FOUND,
                "KEY_NOT_FOUND",
                "API密钥不存在或无权限访问",
            );
        }
        Err(err) => {
            tracing::error!("Failed to fetch user provider key: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "查询API密钥失败",
            );
        }
    };

    // 这里应该调用服务商API撤销token
    // 为了演示，我们直接删除本地记录

    // 删除API密钥记录
    if let Err(err) = user_provider_keys::Entity::delete_by_id(key_id)
        .exec(state.database.as_ref())
        .await
    {
        tracing::error!("Failed to delete user provider key: {}", err);
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_ERROR",
            "删除API密钥失败",
        );
    }

    let response_data = json!({
        "provider_key_id": key_id,
        "revoked_at": Utc::now().to_rfc3339()
    });

    response::success(response_data)
}

/// 生成PKCE code_verifier
fn generate_code_verifier() -> String {
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(128)
        .map(char::from)
        .collect()
}

/// 生成PKCE code_challenge
fn generate_code_challenge(code_verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    general_purpose::URL_SAFE_NO_PAD.encode(&hash)
}

/// 真实的OAuth token交换函数
async fn exchange_code_for_tokens(
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
    oauth_config: &serde_json::Value,
) -> Result<(String, String, u64), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    
    // 获取OAuth配置
    let token_url = oauth_config["token_url"]
        .as_str()
        .ok_or("缺少token_url配置")?;
    
    let client_id = oauth_config["client_id"]
        .as_str()
        .ok_or("缺少client_id配置")?;
    
    // 构建token请求参数
    let mut form_params = HashMap::new();
    form_params.insert("grant_type", "authorization_code");
    form_params.insert("code", code);
    form_params.insert("redirect_uri", redirect_uri);
    form_params.insert("client_id", client_id);
    form_params.insert("code_verifier", code_verifier);
    
    // 如果配置了client_secret且不为空，则添加它（用于机密客户端）
    if let Some(client_secret) = oauth_config.get("client_secret")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty() && !s.contains("PLACEHOLDER"))
    {
        form_params.insert("client_secret", client_secret);
    }
    
    // 发送token交换请求
    let response = client
        .post(token_url)
        .form(&form_params)
        .header("Accept", "application/json")
        .send()
        .await?;
    
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Token交换请求失败: {}", error_text).into());
    }
    
    // 解析token响应
    let token_response: serde_json::Value = response.json().await?;
    
    let access_token = token_response["access_token"]
        .as_str()
        .ok_or("响应中缺少access_token")?
        .to_string();
    
    let refresh_token = token_response["refresh_token"]
        .as_str()
        .unwrap_or("")
        .to_string();
    
    let expires_in = token_response["expires_in"]
        .as_u64()
        .unwrap_or(3600); // 默认1小时
    
    Ok((access_token, refresh_token, expires_in))
}

/// 创建OAuth错误的HTML响应
fn create_oauth_error_html(error_title: &str, error_message: &str) -> axum::response::Response {
    let html_content = format!(r#"
<!DOCTYPE html>
<html>
<head>
    <title>OAuth授权失败</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background: linear-gradient(135deg, #FF6B6B 0%, #EE5A52 100%);
            color: white;
        }}
        .container {{
            text-align: center;
            padding: 2rem;
            background: rgba(255, 255, 255, 0.1);
            border-radius: 10px;
            backdrop-filter: blur(10px);
            max-width: 400px;
        }}
        .error-icon {{
            width: 60px;
            height: 60px;
            margin: 0 auto 1rem;
            border-radius: 50%;
            background: #FF4757;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 2rem;
            color: white;
        }}
        h1 {{ margin: 1rem 0; font-size: 1.5rem; }}
        p {{ margin: 0.5rem 0; opacity: 0.9; font-size: 0.9rem; }}
        .error-details {{
            background: rgba(0, 0, 0, 0.2);
            padding: 1rem;
            border-radius: 5px;
            margin: 1rem 0;
            font-family: 'Courier New', monospace;
            font-size: 0.8rem;
            text-align: left;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="error-icon">✕</div>
        <h1>OAuth授权失败</h1>
        <p>{error_title}</p>
        <div class="error-details">{error_message}</div>
        <p style="font-size: 0.9rem; margin-top: 1rem;">此窗口将自动关闭</p>
    </div>

    <script>
        try {{
            // 向父窗口发送OAuth错误消息
            window.parent.postMessage({{
                type: 'OAUTH_ERROR',
                error: {{ 
                    message: '{error_title}: {error_message}' 
                }}
            }}, window.location.origin);
            
            console.error('OAuth授权失败:', '{error_title}', '{error_message}');
            
            // 3秒后自动关闭窗口
            setTimeout(() => {{
                window.close();
            }}, 3000);
        }} catch (error) {{
            console.error('发送OAuth错误时出错:', error);
        }}
    </script>
</body>
</html>
    "#, 
        error_title = error_title.replace("\"", "&quot;").replace("<", "&lt;").replace(">", "&gt;"),
        error_message = error_message.replace("\"", "&quot;").replace("<", "&lt;").replace(">", "&gt;")
    );

    Html(html_content).into_response()
}

/// 真实的OAuth token刷新函数
async fn refresh_access_token(
    refresh_token: &str,
    oauth_config: &serde_json::Value,
) -> Result<(String, String, u64), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    
    // 获取OAuth配置
    let token_url = oauth_config["token_url"]
        .as_str()
        .ok_or("缺少token_url配置")?;
    
    let client_id = oauth_config["client_id"]
        .as_str()
        .ok_or("缺少client_id配置")?;
    
    // 构建刷新token请求参数
    let mut form_params = HashMap::new();
    form_params.insert("grant_type", "refresh_token");
    form_params.insert("refresh_token", refresh_token);
    form_params.insert("client_id", client_id);
    
    // 如果配置了client_secret且不为空，则添加它（用于机密客户端）
    if let Some(client_secret) = oauth_config.get("client_secret")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty() && !s.contains("PLACEHOLDER"))
    {
        form_params.insert("client_secret", client_secret);
    }
    
    // 发送刷新token请求
    let response = client
        .post(token_url)
        .form(&form_params)
        .header("Accept", "application/json")
        .send()
        .await?;
    
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Token刷新请求失败: {}", error_text).into());
    }
    
    // 解析token响应
    let token_response: serde_json::Value = response.json().await?;
    
    let access_token = token_response["access_token"]
        .as_str()
        .ok_or("响应中缺少access_token")?
        .to_string();
    
    let new_refresh_token = token_response["refresh_token"]
        .as_str()
        .unwrap_or(refresh_token) // 如果没有新的refresh_token，使用原来的
        .to_string();
    
    let expires_in = token_response["expires_in"]
        .as_u64()
        .unwrap_or(3600); // 默认1小时
    
    Ok((access_token, new_refresh_token, expires_in))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_generation() {
        let verifier = generate_code_verifier();
        assert_eq!(verifier.len(), 128);
        
        let challenge = generate_code_challenge(&verifier);
        assert!(!challenge.is_empty());
        assert!(!challenge.contains('='));
    }
}