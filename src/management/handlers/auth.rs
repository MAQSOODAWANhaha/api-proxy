//! # 认证管理处理器

use crate::auth::types::UserInfo as AuthUserInfo;
use crate::management::{response, server::AppState};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Json;
use chrono::Utc;
use entity::users::Entity as Users;
use jsonwebtoken::{DecodingKey, Validation, decode};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
// remove unused Value

/// 登录请求
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// 用户名
    pub username: String,
    /// 密码  
    pub password: String,
}

/// 登录响应
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    /// JWT token
    pub token: String,
    /// 用户信息
    pub user: UserInfo,
}

/// 用户信息
#[derive(Debug, Serialize)]
pub struct UserInfo {
    /// 用户ID
    pub id: i32,
    /// 用户名
    pub username: String,
    /// 邮箱
    pub email: String,
    /// 是否为管理员
    pub is_admin: bool,
}

/// JWT Claims
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// 用户ID
    pub sub: String,
    /// 用户名
    pub username: String,
    /// 是否为管理员
    pub is_admin: bool,
    /// 过期时间
    pub exp: usize,
    /// 签发时间
    pub iat: usize,
}

/// 用户登录（完整密码验证版本）
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> axum::response::Response {
    // 基本输入验证
    if request.username.is_empty() || request.password.is_empty() {
        return crate::manage_error!(crate::proxy_err!(
            business,
            "Username and password cannot be empty"
        ));
    }

    let token_pair = match state
        .auth_service
        .login(&request.username, &request.password)
        .await
    {
        Ok(pair) => pair,
        Err(err) => {
            tracing::warn!("Login failed for user {}: {}", request.username, err);
            return crate::manage_error!(err);
        }
    };

    let claims = match state
        .auth_service
        .jwt_manager
        .validate_token(&token_pair.access_token)
    {
        Ok(claims) => claims,
        Err(err) => {
            tracing::error!(
                "Generated access token failed validation for user {}: {}",
                request.username,
                err
            );
            return crate::manage_error!(err);
        }
    };

    let user_id = match claims.user_id() {
        Ok(id) => id,
        Err(err) => {
            tracing::error!("Failed to parse user id from access token: {}", err);
            return crate::manage_error!(err);
        }
    };

    let auth_user: AuthUserInfo = match state.auth_service.get_user_info(user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::error!("User {} not found after successful login", user_id);
            return crate::manage_error!(crate::proxy_err!(auth, "Invalid username or password"));
        }
        Err(err) => {
            tracing::error!("Failed to load user info for {}: {}", user_id, err);
            return crate::manage_error!(err);
        }
    };

    tracing::info!("User {} logged in successfully", request.username);

    let response = LoginResponse {
        token: token_pair.access_token,
        user: UserInfo {
            id: auth_user.id,
            username: auth_user.username,
            email: auth_user.email,
            is_admin: auth_user.is_admin,
        },
    };

    response::success_with_message(response, "Login successful")
}

/// 验证token响应
#[derive(Debug, Serialize)]
pub struct ValidateTokenResponse {
    /// token是否有效
    pub valid: bool,
    /// 用户信息（如果token有效）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserInfo>,
}

/// 用户登出
pub async fn logout(
    State(_state): State<AppState>,
    headers: HeaderMap,
) -> axum::response::Response {
    // 从Authorization头中提取token
    let auth_header = match headers.get("Authorization") {
        Some(header) => match header.to_str() {
            Ok(header_str) => header_str,
            Err(err) => {
                tracing::warn!("Invalid Authorization header format: {}", err);
                return crate::manage_error!(crate::proxy_err!(
                    business,
                    "Invalid Authorization header format"
                ));
            }
        },
        None => {
            tracing::warn!("No Authorization header found in logout request");
            return crate::manage_error!(crate::proxy_err!(
                business,
                "No Authorization header found"
            ));
        }
    };

    // 检查Bearer前缀
    if !auth_header.starts_with("Bearer ") {
        return crate::manage_error!(crate::proxy_err!(
            business,
            "Invalid Authorization header format"
        ));
    }

    let token = &auth_header[7..]; // 移除"Bearer "前缀

    // 从环境变量或配置获取JWT密钥
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "change-me-in-production-jwt-secret-key".to_string());

    // 验证JWT token
    let validation = Validation::default();
    let token_data = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_ref()),
        &validation,
    ) {
        Ok(data) => data,
        Err(err) => {
            tracing::debug!("Token validation failed during logout: {}", err);
            // 即使token无效，也返回成功，避免客户端异常
            return response::success_without_data("Logout successful");
        }
    };

    // TODO: 在生产环境中，应该将token加入黑名单
    // 这里可以将token的jti添加到Redis黑名单中

    tracing::info!(
        "User {} logged out successfully",
        token_data.claims.username
    );

    response::success_without_data("Logout successful")
}

/// 验证JWT Token
pub async fn validate_token(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> axum::response::Response {
    // 记录所有请求头用于调试
    tracing::info!("Validate token request headers:");
    for (name, value) in headers.iter() {
        if let Ok(value_str) = value.to_str() {
            tracing::info!("  {}: {}", name, value_str);
        }
    }

    // 从Authorization头中提取token
    let auth_header = match headers.get("Authorization") {
        Some(header) => match header.to_str() {
            Ok(header_str) => {
                tracing::info!("Found Authorization header: {}", header_str);
                header_str
            }
            Err(err) => {
                tracing::warn!("Invalid Authorization header format: {}", err);
                let response_data = ValidateTokenResponse {
                    valid: false,
                    user: None,
                };
                return response::success(response_data);
            }
        },
        None => {
            tracing::warn!("No Authorization header found in request");
            let response_data = ValidateTokenResponse {
                valid: false,
                user: None,
            };
            return response::success(response_data);
        }
    };

    // 检查Bearer前缀
    if !auth_header.starts_with("Bearer ") {
        let response_data = ValidateTokenResponse {
            valid: false,
            user: None,
        };
        return response::success(response_data);
    }

    let token = &auth_header[7..]; // 移除"Bearer "前缀

    // 从环境变量或配置获取JWT密钥
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "change-me-in-production-jwt-secret-key".to_string());

    // 验证JWT token
    let validation = Validation::default();
    let token_data = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_ref()),
        &validation,
    ) {
        Ok(data) => data,
        Err(err) => {
            tracing::debug!("Token validation failed: {}", err);
            let response_data = ValidateTokenResponse {
                valid: false,
                user: None,
            };
            return response::success(response_data);
        }
    };

    // 检查token是否过期
    let now = Utc::now().timestamp() as usize;
    if token_data.claims.exp < now {
        let response_data = ValidateTokenResponse {
            valid: false,
            user: None,
        };
        return response::success(response_data);
    }

    // 从数据库获取用户信息
    let user_id: i32 = token_data.claims.sub.parse().unwrap_or(1);
    let user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::warn!("Token valid but user {} not found in database", user_id);
            let response_data = ValidateTokenResponse {
                valid: false,
                user: None,
            };
            return response::success(response_data);
        }
        Err(err) => {
            tracing::error!("Database error during token validation: {}", err);
            let response_data = ValidateTokenResponse {
                valid: false,
                user: None,
            };
            return response::success(response_data);
        }
    };

    // 构造用户信息
    let user_info = UserInfo {
        id: user.id,
        username: user.username,
        email: user.email,
        is_admin: user.is_admin,
    };

    let response_data = ValidateTokenResponse {
        valid: true,
        user: Some(user_info),
    };
    response::success(response_data)
}
