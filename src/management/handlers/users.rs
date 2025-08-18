//! # 用户管理处理器

use crate::management::{response, server::AppState};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use bcrypt::{hash, DEFAULT_COST};
use chrono::Utc;
use entity::{users, users::Entity as Users};
use jsonwebtoken::{decode, DecodingKey, Validation};
use rand::{distributions::Alphanumeric, Rng};
use sea_orm::{entity::*, query::*};
use serde::{Deserialize, Serialize};
// Removed unused serde_json imports

/// JWT Claims (与auth.rs保持一致)
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

/// 从Authorization头中提取JWT用户信息
fn extract_user_from_jwt(headers: &HeaderMap) -> Result<Claims, StatusCode> {
    // 从Authorization头中提取token
    let auth_header = headers
        .get("Authorization")
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_str()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // 检查Bearer前缀
    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = &auth_header[7..]; // 移除"Bearer "前缀

    // 从环境变量或配置获取JWT密钥
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "change-me-in-production-jwt-secret-key".to_string());

    // 验证JWT token
    let validation = Validation::default();
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_ref()),
        &validation,
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // 检查token是否过期
    let now = chrono::Utc::now().timestamp() as usize;
    if token_data.claims.exp < now {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(token_data.claims)
}

/// 用户查询参数
#[derive(Debug, Deserialize)]
pub struct UserQuery {
    /// 页码
    pub page: Option<u32>,
    /// 每页大小
    pub limit: Option<u32>,
    /// 状态过滤
    pub status: Option<String>,
}

/// 创建用户请求
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    /// 用户名
    pub username: String,
    /// 邮箱
    pub email: String,
    /// 密码
    pub password: String,
    /// 角色
    pub role: Option<String>,
}

/// 用户响应
#[derive(Debug, Serialize)]
pub struct UserResponse {
    /// 用户ID
    pub id: i32,
    /// 用户名
    pub username: String,
    /// 邮箱
    pub email: String,
    /// 角色
    pub role: String,
    /// 状态
    pub status: String,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最后登录时间
    pub last_login: Option<chrono::DateTime<chrono::Utc>>,
}

/// 将用户实体转换为响应DTO
impl From<users::Model> for UserResponse {
    fn from(user: users::Model) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            role: if user.is_admin {
                "admin".to_string()
            } else {
                "user".to_string()
            },
            status: if user.is_active {
                "active".to_string()
            } else {
                "inactive".to_string()
            },
            created_at: user.created_at.and_utc(),
            last_login: user.last_login.map(|dt| dt.and_utc()),
        }
    }
}

/// 列出用户
pub async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<UserQuery>,
) -> impl IntoResponse {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;

    // 构建查询条件
    let mut select = Users::find();

    // 状态过滤
    if let Some(status) = &query.status {
        match status.as_str() {
            "active" => select = select.filter(users::Column::IsActive.eq(true)),
            "inactive" => select = select.filter(users::Column::IsActive.eq(false)),
            _ => {}
        }
    }

    // 分页查询
    let users_result = select
        .offset(offset as u64)
        .limit(limit as u64)
        .order_by_asc(users::Column::Id)
        .all(state.database.as_ref())
        .await;

    let users = match users_result {
        Ok(users) => users,
        Err(err) => {
            tracing::error!("Failed to fetch users: {}", err);
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch users",
            );
        }
    };

    // 获取总数
    let mut count_select = Users::find();
    if let Some(status) = &query.status {
        match status.as_str() {
            "active" => count_select = count_select.filter(users::Column::IsActive.eq(true)),
            "inactive" => count_select = count_select.filter(users::Column::IsActive.eq(false)),
            _ => {}
        }
    }

    let total = match count_select.count(state.database.as_ref()).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!("Failed to count users: {}", err);
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to count users",
            );
        }
    };

    // 转换为响应DTO
    let user_responses: Vec<UserResponse> = users.into_iter().map(UserResponse::from).collect();

    let pagination = response::Pagination {
        page: page as u64,
        limit: limit as u64,
        total,
        pages: ((total as f64) / (limit as f64)).ceil() as u64,
    };

    response::paginated(user_responses, pagination)
}

/// 创建用户
pub async fn create_user(
    State(state): State<AppState>,
    Json(request): Json<CreateUserRequest>,
) -> impl IntoResponse {
    // 验证输入
    if request.username.is_empty() {
        return response::error(
            axum::http::StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Username cannot be empty",
        );
    }

    if request.email.is_empty() || !request.email.contains('@') {
        return response::error(
            axum::http::StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid email format",
        );
    }

    if request.password.len() < 6 {
        return response::error(
            axum::http::StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Password must be at least 6 characters long",
        );
    }

    // 检查用户名和邮箱是否已存在
    let existing_user = Users::find()
        .filter(
            users::Column::Username
                .eq(&request.username)
                .or(users::Column::Email.eq(&request.email)),
        )
        .one(state.database.as_ref())
        .await;

    match existing_user {
        Ok(Some(_)) => {
            return response::error(
                axum::http::StatusCode::CONFLICT,
                "USER_EXISTS",
                "Username or email already exists",
            );
        }
        Err(err) => {
            tracing::error!("Failed to check existing user: {}", err);
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to check existing user",
            );
        }
        Ok(None) => {}
    }

    // 生成salt和密码哈希
    let salt: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let password_hash = match hash(&request.password, DEFAULT_COST) {
        Ok(hash) => hash,
        Err(err) => {
            tracing::error!("Failed to hash password: {}", err);
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "HASH_ERROR",
                "Failed to hash password",
            );
        }
    };

    // 创建用户
    let is_admin = request.role.as_deref() == Some("admin");
    let now = Utc::now().naive_utc();

    let user = users::ActiveModel {
        username: Set(request.username.clone()),
        email: Set(request.email.clone()),
        password_hash: Set(password_hash),
        salt: Set(salt),
        is_active: Set(true),
        is_admin: Set(is_admin),
        last_login: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };

    let insert_result = Users::insert(user).exec(state.database.as_ref()).await;

    let user_id = match insert_result {
        Ok(result) => result.last_insert_id,
        Err(err) => {
            tracing::error!("Failed to create user: {}", err);
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to create user",
            );
        }
    };

    // 获取新创建的用户
    let created_user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::error!("User not found after creation");
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "User not found after creation",
            );
        }
        Err(err) => {
            tracing::error!("Failed to fetch created user: {}", err);
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch created user",
            );
        }
    };

    let user_response = UserResponse::from(created_user);

    response::success_with_message(user_response, "User created successfully")
}

/// 获取单个用户
pub async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
) -> impl IntoResponse {
    if user_id <= 0 {
        return response::error(
            axum::http::StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid user ID",
        );
    }

    // 从数据库获取用户
    let user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return response::error(
                axum::http::StatusCode::NOT_FOUND,
                "USER_NOT_FOUND",
                "User not found",
            );
        }
        Err(err) => {
            tracing::error!("Failed to fetch user {}: {}", user_id, err);
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch user",
            );
        }
    };

    let user_response = UserResponse::from(user);
    response::success(user_response)
}

/// 用户档案响应
#[derive(Debug, Serialize)]
pub struct UserProfileResponse {
    pub username: String,
    pub email: String,
    pub last_login: Option<String>,
    pub is_admin: bool,
    pub created_at: String,
}

/// 更新用户档案请求
#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub email: Option<String>,
}

/// 修改密码请求
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// 获取用户档案
pub async fn get_user_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // 从JWT token中获取用户信息
    let claims = match extract_user_from_jwt(&headers) {
        Ok(claims) => claims,
        Err(status_code) => {
            return response::error(
                status_code,
                "AUTHENTICATION_REQUIRED",
                "Invalid or expired token",
            );
        }
    };
    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "Invalid user ID in token",
            );
        }
    };

    let user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return response::error(StatusCode::NOT_FOUND, "USER_NOT_FOUND", "User not found");
        }
        Err(err) => {
            tracing::error!("Failed to fetch user profile: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch user profile",
            );
        }
    };

    let profile = UserProfileResponse {
        username: user.username,
        email: user.email,
        last_login: user
            .last_login
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        is_admin: user.is_admin,
        created_at: user.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    response::success(profile)
}

/// 更新用户档案
pub async fn update_user_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateProfileRequest>,
) -> impl IntoResponse {
    // 从JWT token中获取用户信息
    let claims = match extract_user_from_jwt(&headers) {
        Ok(claims) => claims,
        Err(status_code) => {
            return response::error(
                status_code,
                "AUTHENTICATION_REQUIRED",
                "Invalid or expired token",
            );
        }
    };
    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "Invalid user ID in token",
            );
        }
    };

    // 验证邮箱格式
    if let Some(ref email) = request.email {
        if email.is_empty() || !email.contains('@') {
            return response::error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "Invalid email format",
            );
        }
    }

    // 获取现有用户
    let user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return response::error(StatusCode::NOT_FOUND, "USER_NOT_FOUND", "User not found");
        }
        Err(err) => {
            tracing::error!("Failed to fetch user for update: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch user for update",
            );
        }
    };

    // 更新用户信息
    let mut active_model: users::ActiveModel = user.into();
    if let Some(email) = request.email {
        active_model.email = Set(email);
    }
    active_model.updated_at = Set(Utc::now().naive_utc());

    match active_model.update(state.database.as_ref()).await {
        Ok(updated_user) => {
            let profile = UserProfileResponse {
                username: updated_user.username,
                email: updated_user.email,
                last_login: updated_user
                    .last_login
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
                is_admin: updated_user.is_admin,
                created_at: updated_user
                    .created_at
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string(),
            };

            response::success_with_message(profile, "Profile updated successfully")
        }
        Err(err) => {
            tracing::error!("Failed to update user profile: {}", err);
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to update user profile",
            )
        }
    }
}

/// 修改密码
pub async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    // 从JWT token中获取用户信息
    let claims = match extract_user_from_jwt(&headers) {
        Ok(claims) => claims,
        Err(status_code) => {
            return response::error(
                status_code,
                "AUTHENTICATION_REQUIRED",
                "Invalid or expired token",
            );
        }
    };
    let user_id: i32 = match claims.sub.parse() {
        Ok(id) => id,
        Err(_) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "Invalid user ID in token",
            );
        }
    };

    // 验证新密码强度
    if request.new_password.len() < 6 {
        return response::error(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "New password must be at least 6 characters long",
        );
    }

    // 获取现有用户
    let user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return response::error(StatusCode::NOT_FOUND, "USER_NOT_FOUND", "User not found");
        }
        Err(err) => {
            tracing::error!("Failed to fetch user for password change: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch user for password change",
            );
        }
    };

    // 验证当前密码
    match bcrypt::verify(&request.current_password, &user.password_hash) {
        Ok(true) => {}
        Ok(false) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "Current password is incorrect",
            );
        }
        Err(err) => {
            tracing::error!("Failed to verify current password: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "HASH_ERROR",
                "Failed to verify current password",
            );
        }
    }

    // 生成新密码哈希
    let new_password_hash = match hash(&request.new_password, DEFAULT_COST) {
        Ok(hash) => hash,
        Err(err) => {
            tracing::error!("Failed to hash new password: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "HASH_ERROR",
                "Failed to hash new password",
            );
        }
    };

    // 更新密码
    let mut active_model: users::ActiveModel = user.into();
    active_model.password_hash = Set(new_password_hash);
    active_model.updated_at = Set(Utc::now().naive_utc());

    match active_model.update(state.database.as_ref()).await {
        Ok(_) => response::success_without_data("Password changed successfully"),
        Err(err) => {
            tracing::error!("Failed to update password: {}", err);
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to update password",
            )
        }
    }
}
