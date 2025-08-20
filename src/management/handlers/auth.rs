//! # 认证管理处理器

use crate::management::{response, server::AppState};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use bcrypt;
use chrono::{Duration, Utc};
use entity::{
    provider_types, provider_types::Entity as ProviderTypes, users::Entity as Users,
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sea_orm::{entity::*, query::*};
use serde::{Deserialize, Serialize};
use serde_json::json; // remove unused Value

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
) -> impl IntoResponse {
    // 基本输入验证
    if request.username.is_empty() || request.password.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Username and password cannot be empty",
        );
    }

    // 从数据库查找用户
    use entity::users::{Column as UserColumn, Entity as Users};
    let user = match Users::find()
        .filter(UserColumn::Username.eq(&request.username))
        .filter(UserColumn::IsActive.eq(true))
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::warn!(
                "Login attempt with non-existent or inactive user: {}",
                request.username
            );
            return response::error(
                StatusCode::UNAUTHORIZED,
                "INVALID_CREDENTIALS",
                "Invalid username or password",
            );
        }
        Err(err) => {
            tracing::error!("Database error during login: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Database error during login",
            );
        }
    };

    // 验证密码
    match bcrypt::verify(&request.password, &user.password_hash) {
        Ok(true) => {
            tracing::info!("Successful login for user: {}", request.username);
        }
        Ok(false) => {
            tracing::warn!(
                "Failed login attempt - invalid password for user: {}",
                request.username
            );
            return response::error(
                StatusCode::UNAUTHORIZED,
                "INVALID_CREDENTIALS",
                "Invalid username or password",
            );
        }
        Err(err) => {
            tracing::error!("Password verification error: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "HASH_ERROR",
                "Password verification error",
            );
        }
    }

    // 创建JWT token
    let now = Utc::now();
    let exp = now + Duration::hours(24);

    let claims = Claims {
        sub: user.id.to_string(),
        username: user.username.clone(),
        is_admin: user.is_admin,
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    // 从环境变量或配置获取JWT密钥
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "change-me-in-production-jwt-secret-key".to_string());

    let token = match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_ref()),
    ) {
        Ok(token) => token,
        Err(err) => {
            tracing::error!("JWT encoding error: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "JWT_ERROR",
                "JWT token generation failed",
            );
        }
    };

    tracing::info!("User {} logged in successfully", request.username);

    let response = LoginResponse {
        token,
        user: UserInfo {
            id: user.id,
            username: user.username,
            email: user.email,
            is_admin: user.is_admin,
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
pub async fn logout(State(_state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    // 从Authorization头中提取token
    let auth_header = match headers.get("Authorization") {
        Some(header) => match header.to_str() {
            Ok(header_str) => header_str,
            Err(err) => {
                tracing::warn!("Invalid Authorization header format: {}", err);
                return response::error(
                    StatusCode::BAD_REQUEST,
                    "VALIDATION_ERROR",
                    "Invalid Authorization header format",
                );
            }
        },
        None => {
            tracing::warn!("No Authorization header found in logout request");
            return response::error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "No Authorization header found",
            );
        }
    };

    // 检查Bearer前缀
    if !auth_header.starts_with("Bearer ") {
        return response::error(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid Authorization header format",
        );
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
) -> impl IntoResponse {
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

/// 获取服务提供商类型列表
pub async fn list_provider_types(State(state): State<AppState>) -> impl IntoResponse {
    // changed
    // 获取所有活跃的服务提供商类型
    let provider_types_result = ProviderTypes::find()
        .filter(provider_types::Column::IsActive.eq(true))
        .order_by_asc(provider_types::Column::Id)
        .all(state.database.as_ref())
        .await;

    let provider_types_data = match provider_types_result {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to fetch provider types: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch provider types",
            );
        }
    };

    // 转换为响应格式
    let provider_types: Vec<_> = provider_types_data
        .into_iter()
        .map(|provider| {
            json!({
                "id": provider.id,
                "name": provider.name,
                "display_name": provider.display_name,
                "base_url": provider.base_url,
                "api_format": provider.api_format,
                "default_model": provider.default_model,
                "is_active": provider.is_active
            })
        })
        .collect();

    response::success(provider_types)
}
