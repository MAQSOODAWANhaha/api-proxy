//! # 用户管理处理器

use crate::management::server::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sea_orm::{entity::*, query::*};
use entity::{users, users::Entity as Users};
use chrono::Utc;
use bcrypt::{hash, DEFAULT_COST};
use rand::{distributions::Alphanumeric, Rng};

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
            role: if user.is_admin { "admin".to_string() } else { "user".to_string() },
            status: if user.is_active { "active".to_string() } else { "inactive".to_string() },
            created_at: user.created_at.and_utc(),
            last_login: user.last_login.map(|dt| dt.and_utc()),
        }
    }
}

/// 列出用户
pub async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<UserQuery>,
) -> Result<Json<Value>, StatusCode> {
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
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
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
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // 转换为响应DTO
    let user_responses: Vec<UserResponse> = users.into_iter().map(UserResponse::from).collect();
    
    let response = json!({
        "users": user_responses,
        "pagination": {
            "page": page,
            "limit": limit,
            "total": total,
            "pages": ((total as f64) / (limit as f64)).ceil() as u32
        }
    });

    Ok(Json(response))
}

/// 创建用户
pub async fn create_user(
    State(state): State<AppState>,
    Json(request): Json<CreateUserRequest>,
) -> Result<Json<Value>, StatusCode> {
    // 验证输入
    if request.username.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    if request.email.is_empty() || !request.email.contains('@') {
        return Err(StatusCode::BAD_REQUEST);
    }

    if request.password.len() < 6 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 检查用户名和邮箱是否已存在
    let existing_user = Users::find()
        .filter(
            users::Column::Username.eq(&request.username)
                .or(users::Column::Email.eq(&request.email))
        )
        .one(state.database.as_ref())
        .await;
        
    match existing_user {
        Ok(Some(_)) => {
            return Ok(Json(json!({
                "success": false,
                "message": "Username or email already exists"
            })));
        }
        Err(err) => {
            tracing::error!("Failed to check existing user: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
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
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
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
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // 获取新创建的用户
    let created_user = match Users::find_by_id(user_id).one(state.database.as_ref()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::error!("User not found after creation");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Err(err) => {
            tracing::error!("Failed to fetch created user: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let user_response = UserResponse::from(created_user);

    let response = json!({
        "success": true,
        "user": user_response,
        "message": "User created successfully"
    });

    Ok(Json(response))
}

/// 获取单个用户
pub async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
) -> Result<Json<Value>, StatusCode> {
    if user_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 从数据库获取用户
    let user = match Users::find_by_id(user_id).one(state.database.as_ref()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err(StatusCode::NOT_FOUND);
        }
        Err(err) => {
            tracing::error!("Failed to fetch user {}: {}", user_id, err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let user_response = UserResponse::from(user);
    Ok(Json(serde_json::to_value(user_response).unwrap()))
}