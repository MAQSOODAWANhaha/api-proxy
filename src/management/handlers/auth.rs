//! # 认证管理处理器

use crate::management::{response, server::AppState};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Json;
use bcrypt;
use chrono::{Duration, Utc};
use entity::users::Entity as Users;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sea_orm::{entity::*, query::*};
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
            return crate::manage_error!(crate::proxy_err!(auth, "Invalid username or password"));
        }
        Err(err) => {
            tracing::error!("Database error during login: {}", err);
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Database error during login: {}",
                err
            ));
        }
    };

    // 验证密码
    match bcrypt::verify(&request.password, &user.password_hash) {
        Ok(true) => {
            tracing::info!("Successful login for user: {}", request.username);

            // 更新用户最后登录时间
            let user_id = user.id;
            let now = Utc::now().naive_utc();
            match Users::update_many()
                .set(entity::users::ActiveModel {
                    id: Set(user_id),
                    last_login: Set(Some(now)),
                    ..Default::default()
                })
                .filter(entity::users::Column::Id.eq(user_id))
                .exec(state.database.as_ref())
                .await
            {
                Ok(_) => {
                    tracing::info!("Updated last login time for user: {}", request.username);
                }
                Err(err) => {
                    tracing::error!(
                        "Failed to update last login time for user {}: {}",
                        request.username,
                        err
                    );
                    // 不中断登录流程，只记录错误
                }
            }
        }
        Ok(false) => {
            tracing::warn!(
                "Failed login attempt - invalid password for user: {}",
                request.username
            );
            return crate::manage_error!(crate::proxy_err!(auth, "Invalid username or password"));
        }
        Err(err) => {
            tracing::error!("Password verification error: {}", err);
            return crate::manage_error!(crate::proxy_err!(
                internal,
                "Password verification error: {}",
                err
            ));
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
            return crate::manage_error!(crate::proxy_err!(
                internal,
                "JWT token generation failed: {}",
                err
            ));
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
