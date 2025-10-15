//! # 用户管理处理器
#![allow(clippy::cognitive_complexity, clippy::too_many_lines)]

use crate::error::ProxyError;
use crate::management::middleware::auth::AuthContext;
use crate::management::{response, server::AppState};
use crate::{
    lerror, linfo,
    logging::{LogComponent, LogStage},
};
use axum::extract::{Extension, Path, Query, State};
use axum::response::Json;
use bcrypt::{DEFAULT_COST, hash};
use chrono::{Datelike, Utc};
use entity::{proxy_tracing, proxy_tracing::Entity as ProxyTracing, users, users::Entity as Users};
use rand::{Rng, distributions::Alphanumeric};
use sea_orm::{
    DatabaseConnection,
    entity::{ActiveModelTrait, ColumnTrait, EntityTrait, Set},
    query::{PaginatorTrait, QueryFilter, QueryOrder, QuerySelect},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
// Removed unused serde_json imports

fn business_error(message: impl Into<String>) -> ProxyError {
    crate::error!(Authentication, message)
}

fn permission_error(message: impl Into<String>) -> ProxyError {
    crate::error!(Authentication, message)
}

/// 用户查询参数
#[derive(Debug, Deserialize)]
pub struct UserQuery {
    /// 页码
    pub page: Option<u32>,
    /// 每页大小
    pub limit: Option<u32>,
    /// 搜索关键词
    pub search: Option<String>,
    /// 激活状态筛选
    pub is_active: Option<bool>,
    /// 管理员状态筛选
    pub is_admin: Option<bool>,
    /// 排序字段
    pub sort: Option<String>,
    /// 排序方向
    pub order: Option<String>,
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
    /// 是否管理员
    pub is_admin: Option<bool>,
}

/// 更新用户请求
#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    /// 用户名
    pub username: Option<String>,
    /// 邮箱
    pub email: Option<String>,
    /// 密码
    pub password: Option<String>,
    /// 是否激活
    pub is_active: Option<bool>,
    /// 是否管理员
    pub is_admin: Option<bool>,
}

/// 批量删除请求
#[derive(Debug, Deserialize)]
pub struct BatchDeleteRequest {
    /// 用户ID列表
    pub ids: Vec<i32>,
}

/// 重置密码请求
#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    /// 新密码
    pub new_password: String,
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
    /// 是否激活
    pub is_active: bool,
    /// 是否管理员
    pub is_admin: bool,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// 最后登录时间
    pub last_login: Option<chrono::DateTime<chrono::Utc>>,
    /// 总请求数
    pub total_requests: i64,
    /// 总花费
    pub total_cost: f64,
    /// 总token消耗
    pub total_tokens: i64,
}

/// 将用户实体转换为响应DTO
impl From<users::Model> for UserResponse {
    fn from(user: users::Model) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            is_active: user.is_active,
            is_admin: user.is_admin,
            created_at: user.created_at.and_utc(),
            updated_at: user.updated_at.and_utc(),
            last_login: user.last_login.map(|dt| dt.and_utc()),
            total_requests: 0,
            total_cost: 0.0,
            total_tokens: 0,
        }
    }
}

/// 用户统计数据
#[derive(Debug)]
pub struct UserStats {
    pub total_requests: i64,
    pub total_cost: f64,
    pub total_tokens: i64,
}

impl UserResponse {
    /// 从用户实体和统计数据创建响应DTO
    #[must_use]
    pub fn from_user_with_stats(user: users::Model, stats: &UserStats) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            is_active: user.is_active,
            is_admin: user.is_admin,
            created_at: user.created_at.and_utc(),
            updated_at: user.updated_at.and_utc(),
            last_login: user.last_login.map(|dt| dt.and_utc()),
            total_requests: stats.total_requests,
            total_cost: stats.total_cost,
            total_tokens: stats.total_tokens,
        }
    }
}

/// 获取用户统计数据的辅助函数
async fn get_user_statistics(user_id: i32, db: &DatabaseConnection) -> UserStats {
    let stats_result = ProxyTracing::find()
        .select_only()
        .column_as(proxy_tracing::Column::Id.count(), "total_requests")
        .column_as(proxy_tracing::Column::Cost.sum(), "total_cost")
        .column_as(proxy_tracing::Column::TokensTotal.sum(), "total_tokens")
        .filter(proxy_tracing::Column::UserId.eq(user_id))
        .into_tuple::<(Option<i64>, Option<f64>, Option<i64>)>()
        .one(db)
        .await;

    match stats_result {
        Ok(Some((requests, cost, tokens))) => UserStats {
            total_requests: requests.unwrap_or(0),
            total_cost: cost.unwrap_or(0.0),
            total_tokens: tokens.unwrap_or(0),
        },
        _ => UserStats {
            total_requests: 0,
            total_cost: 0.0,
            total_tokens: 0,
        },
    }
}

/// 获取用户本月请求数的辅助函数
async fn get_user_monthly_requests(user_id: i32, db: &DatabaseConnection) -> i64 {
    let now = chrono::Utc::now().naive_utc();
    let month_start = now
        .date()
        .with_day(1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();

    let monthly_result = ProxyTracing::find()
        .select_only()
        .column_as(proxy_tracing::Column::Id.count(), "monthly_requests")
        .filter(proxy_tracing::Column::UserId.eq(user_id))
        .filter(proxy_tracing::Column::CreatedAt.gte(month_start))
        .into_tuple::<Option<i64>>()
        .one(db)
        .await;

    match monthly_result {
        Ok(Some(Some(count))) => count,
        _ => 0,
    }
}

/// 生成用户头像URL
fn generate_avatar_url(email: &str) -> String {
    // 使用Gravatar生成头像，如果没有则使用默认头像
    let email_hash = md5::compute(email.to_lowercase());
    let hash_str = format!("{email_hash:x}");
    format!("https://www.gravatar.com/avatar/{hash_str}?d=identicon&s=200")
}

/// 列出用户
pub async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<UserQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let user_id = auth_context.user_id;
    let is_admin = auth_context.is_admin;

    // 权限控制：非管理员只能查看自己的用户信息
    if !is_admin {
        // 非管理员只能查看自己的信息，直接返回单个用户数据
        let self_user = match Users::find_by_id(user_id)
            .one(state.database.as_ref())
            .await
        {
            Ok(Some(user)) => user,
            Ok(None) => {
                return crate::management::response::app_error(business_error(format!(
                    "User not found: {user_id}"
                )));
            }
            Err(err) => {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "get_user_fail",
                    &format!("获取用户信息失败: {err}")
                );
                return crate::management::response::app_error(crate::error!(
                    Database,
                    format!("获取用户信息失败: {}", err)
                ));
            }
        };

        let user_stats = get_user_statistics(self_user.id, state.database.as_ref()).await;
        let user_response = UserResponse::from_user_with_stats(self_user, &user_stats);

        let pagination = response::Pagination {
            page: 1,
            limit: 1,
            total: 1,
            pages: 1,
        };

        linfo!(
            "system",
            LogStage::Authentication,
            LogComponent::Auth,
            "non_admin_access",
            &format!("Non-admin user {user_id} accessing only their own user info")
        );
        return response::paginated(vec![user_response], pagination);
    }

    linfo!(
        "system",
        LogStage::Authentication,
        LogComponent::Auth,
        "admin_access",
        &format!("Admin user {user_id} accessing all users list")
    );
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(10).min(100);
    let offset = (page - 1) * limit;

    // 构建查询条件
    let mut select = Users::find();
    let mut count_select = Users::find();

    // 搜索过滤
    if let Some(search) = &query.search
        && !search.trim().is_empty()
    {
        let search_pattern = format!("%{}%", search.trim());
        let search_condition = users::Column::Username
            .like(&search_pattern)
            .or(users::Column::Email.like(&search_pattern));
        select = select.filter(search_condition.clone());
        count_select = count_select.filter(search_condition);
    }

    // 激活状态过滤
    if let Some(is_active) = query.is_active {
        select = select.filter(users::Column::IsActive.eq(is_active));
        count_select = count_select.filter(users::Column::IsActive.eq(is_active));
    }

    // 管理员状态过滤
    if let Some(is_admin) = query.is_admin {
        select = select.filter(users::Column::IsAdmin.eq(is_admin));
        count_select = count_select.filter(users::Column::IsAdmin.eq(is_admin));
    }

    // 排序
    let sort_field = query.sort.as_deref().unwrap_or("created_at");
    let order_desc = query.order.as_deref() == Some("asc");

    select = match sort_field {
        "username" => {
            if order_desc {
                select.order_by_asc(users::Column::Username)
            } else {
                select.order_by_desc(users::Column::Username)
            }
        }
        "email" => {
            if order_desc {
                select.order_by_asc(users::Column::Email)
            } else {
                select.order_by_desc(users::Column::Email)
            }
        }
        "updated_at" => {
            if order_desc {
                select.order_by_asc(users::Column::UpdatedAt)
            } else {
                select.order_by_desc(users::Column::UpdatedAt)
            }
        }
        _ => {
            if order_desc {
                select.order_by_asc(users::Column::CreatedAt)
            } else {
                select.order_by_desc(users::Column::CreatedAt)
            }
        }
    };

    // 分页查询
    let users_result = select
        .offset(u64::from(offset))
        .limit(u64::from(limit))
        .all(state.database.as_ref())
        .await;

    let users = match users_result {
        Ok(users) => users,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "get_users_fail",
                &format!("获取用户列表失败: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("获取用户列表失败: {}", err)
            ));
        }
    };

    // 获取总数
    let total = match count_select.count(state.database.as_ref()).await {
        Ok(count) => count,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "get_user_count_fail",
                &format!("获取用户总数失败: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("获取用户总数失败: {}", err)
            ));
        }
    };

    // 获取用户统计数据并转换为响应DTO
    let mut user_responses: Vec<UserResponse> = Vec::new();

    for user in users {
        let user_stats = get_user_statistics(user.id, state.database.as_ref()).await;
        user_responses.push(UserResponse::from_user_with_stats(user, &user_stats));
    }

    let limit_u64 = u64::from(limit);
    let pages = if limit_u64 == 0 {
        0
    } else {
        total.div_ceil(limit_u64)
    };

    let pagination = response::Pagination {
        page: u64::from(page),
        limit: u64::from(limit),
        total,
        pages,
    };

    response::paginated(user_responses, pagination)
}

/// 创建用户
pub async fn create_user(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<CreateUserRequest>,
) -> axum::response::Response {
    // 权限检查：只有管理员可以创建用户
    if !auth_context.is_admin {
        return crate::management::response::app_error(permission_error("权限不足"));
    }

    // 验证输入
    if request.username.len() < 3 || request.username.len() > 50 {
        return crate::management::response::app_error(business_error(
            "用户名长度必须在3-50字符之间 (field: username)",
        ));
    }

    if request.email.len() > 100 || !request.email.contains('@') {
        return crate::management::response::app_error(business_error(
            "邮箱格式无效或长度超过100字符 (field: email)",
        ));
    }

    if request.password.len() < 8 {
        return crate::management::response::app_error(business_error(
            "密码长度至少8字符 (field: password)",
        ));
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
        Ok(Some(existing)) => {
            if existing.username == request.username {
                return crate::management::response::app_error(business_error(format!(
                    "User conflict: {}",
                    request.username
                )));
            }
            return crate::management::response::app_error(business_error(format!(
                "UserEmail conflict: {}",
                request.email
            )));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "check_existing_user_fail",
                &format!("Failed to check existing user: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to check existing user: {}", err)
            ));
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
            lerror!(
                "system",
                LogStage::Internal,
                LogComponent::Auth,
                "hash_password_fail",
                &format!("Failed to hash password: {err}")
            );
            return crate::management::response::app_error(ProxyError::internal_with_source(
                "Failed to hash password",
                err,
            ));
        }
    };

    // 创建用户
    let is_admin = request.is_admin.unwrap_or(false);
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
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "create_user_fail",
                &format!("Failed to create user: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to create user: {}", err)
            ));
        }
    };

    // 获取新创建的用户
    let created_user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "user_not_found_after_creation",
                "User not found after creation"
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                "User not found after creation"
            ));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_created_user_fail",
                &format!("Failed to fetch created user: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to fetch created user: {}", err)
            ));
        }
    };

    let created_user_stats = get_user_statistics(created_user.id, state.database.as_ref()).await;
    let user_response = UserResponse::from_user_with_stats(created_user, &created_user_stats);

    response::success_with_message(user_response, "用户创建成功")
}

/// 获取单个用户
pub async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    // 权限检查：非管理员只能获取自己的信息
    if !auth_context.is_admin && auth_context.user_id != user_id {
        return crate::management::response::app_error(permission_error("权限不足"));
    }

    if user_id <= 0 {
        return crate::management::response::app_error(business_error("Invalid user ID"));
    }

    // 从数据库获取用户
    let user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return crate::management::response::app_error(business_error(format!(
                "User not found: {user_id}"
            )));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_user_fail",
                &format!("Failed to fetch user {user_id}: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to fetch user: {}", err)
            ));
        }
    };

    // 获取用户统计数据
    let user_stats = get_user_statistics(user.id, state.database.as_ref()).await;
    let user_response = UserResponse::from_user_with_stats(user, &user_stats);
    response::success(user_response)
}

/// 用户档案响应
#[derive(Debug, Serialize)]
pub struct UserProfileResponse {
    pub name: String,
    pub email: String,
    pub avatar: String,
    pub role: String,
    pub created_at: String,
    pub last_login: Option<String>,
    pub total_requests: i64,
    pub monthly_requests: i64,
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
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let user_id = auth_context.user_id;

    let user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return crate::management::response::app_error(business_error(format!(
                "User not found: {user_id}"
            )));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_user_profile_fail",
                &format!("Failed to fetch user profile: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to fetch user profile: {}", err)
            ));
        }
    };

    // 获取用户统计数据
    let user_stats = get_user_statistics(user_id, state.database.as_ref()).await;
    let monthly_requests = get_user_monthly_requests(user_id, state.database.as_ref()).await;

    // 生成头像URL
    let avatar_url = generate_avatar_url(&user.email);

    // 确定用户角色
    let role = if user.is_admin {
        "系统管理员".to_string()
    } else {
        "普通用户".to_string()
    };

    let profile = UserProfileResponse {
        name: user.username,
        email: user.email,
        avatar: avatar_url,
        role,
        created_at: user.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        last_login: user
            .last_login
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        total_requests: user_stats.total_requests,
        monthly_requests,
    };

    response::success(profile)
}

/// 更新用户档案
pub async fn update_user_profile(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<UpdateProfileRequest>,
) -> axum::response::Response {
    let user_id = auth_context.user_id;

    // 验证邮箱格式
    if let Some(ref email) = request.email
        && (email.is_empty() || !email.contains('@'))
    {
        return crate::management::response::app_error(business_error("Invalid email format"));
    }

    // 获取现有用户
    let user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return crate::management::response::app_error(business_error(format!(
                "User not found: {user_id}"
            )));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_user_for_update_fail",
                &format!("Failed to fetch user for update: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to fetch user for update: {}", err)
            ));
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
            // 获取更新后的统计数据
            let user_stats = get_user_statistics(user_id, state.database.as_ref()).await;
            let monthly_requests =
                get_user_monthly_requests(user_id, state.database.as_ref()).await;

            // 生成头像URL
            let avatar_url = generate_avatar_url(&updated_user.email);

            // 确定用户角色
            let role = if updated_user.is_admin {
                "系统管理员".to_string()
            } else {
                "普通用户".to_string()
            };

            let profile = UserProfileResponse {
                name: updated_user.username,
                email: updated_user.email,
                avatar: avatar_url,
                role,
                created_at: updated_user
                    .created_at
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string(),
                last_login: updated_user
                    .last_login
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
                total_requests: user_stats.total_requests,
                monthly_requests,
            };

            response::success_with_message(profile, "Profile updated successfully")
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "update_user_profile_fail",
                &format!("Failed to update user profile: {err}")
            );
            crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to update user profile: {}", err)
            ))
        }
    }
}

/// 修改密码
pub async fn change_password(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<ChangePasswordRequest>,
) -> axum::response::Response {
    let user_id = auth_context.user_id;

    // 验证新密码强度
    if request.new_password.len() < 6 {
        return crate::management::response::app_error(business_error(
            "New password must be at least 6 characters long",
        ));
    }

    // 获取现有用户
    let user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return crate::management::response::app_error(business_error(format!(
                "User not found: {user_id}"
            )));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_user_for_password_change_fail",
                &format!("Failed to fetch user for password change: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to fetch user for password change: {}", err)
            ));
        }
    };

    // 验证当前密码
    match bcrypt::verify(&request.current_password, &user.password_hash) {
        Ok(true) => {}
        Ok(false) => {
            return crate::management::response::app_error(business_error(
                "Current password is incorrect",
            ));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Internal,
                LogComponent::Auth,
                "verify_password_fail",
                &format!("Failed to verify current password: {err}")
            );
            return crate::management::response::app_error(ProxyError::internal_with_source(
                "Failed to verify current password",
                err,
            ));
        }
    }

    // 生成新密码哈希
    let new_password_hash = match hash(&request.new_password, DEFAULT_COST) {
        Ok(hash) => hash,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Internal,
                LogComponent::Auth,
                "hash_password_fail",
                &format!("Failed to hash new password: {err}")
            );
            return crate::management::response::app_error(ProxyError::internal_with_source(
                "Failed to hash new password",
                err,
            ));
        }
    };

    // 更新密码
    let mut active_model: users::ActiveModel = user.into();
    active_model.password_hash = Set(new_password_hash);
    active_model.updated_at = Set(Utc::now().naive_utc());

    match active_model.update(state.database.as_ref()).await {
        Ok(_) => response::success_without_data("Password changed successfully"),
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "update_password_fail",
                &format!("Failed to update password: {err}")
            );
            crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to update password: {}", err)
            ))
        }
    }
}

/// 更新用户
pub async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<UpdateUserRequest>,
) -> axum::response::Response {
    // 权限检查：只有管理员可以更新其他用户
    if !auth_context.is_admin {
        return crate::management::response::app_error(permission_error("权限不足"));
    }

    // 如果不是管理员，不能修改is_admin字段
    if !auth_context.is_admin && request.is_admin.is_some() {
        return crate::management::response::app_error(permission_error("只有管理员可以修改权限"));
    }

    // 验证输入
    if let Some(ref username) = request.username
        && (username.len() < 3 || username.len() > 50)
    {
        return crate::management::response::app_error(business_error(
            "用户名长度必须在3-50字符之间",
        ));
    }

    if let Some(ref email) = request.email
        && (email.len() > 100 || !email.contains('@'))
    {
        return crate::management::response::app_error(business_error(
            "邮箱格式无效或长度超过100字符",
        ));
    }

    if let Some(ref password) = request.password
        && password.len() < 8
    {
        return crate::management::response::app_error(business_error("密码长度至少8字符"));
    }

    // 获取现有用户
    let user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return crate::management::response::app_error(business_error(format!(
                "User not found: {user_id}"
            )));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "get_user_fail",
                &format!("获取用户失败: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("获取用户失败: {}", err)
            ));
        }
    };

    // 检查用户名和邮箱是否与其他用户冲突
    if request.username.is_some() || request.email.is_some() {
        let mut check_query = Users::find().filter(users::Column::Id.ne(user_id));

        if let Some(ref username) = request.username {
            check_query = check_query.filter(users::Column::Username.eq(username));
        }

        if let Some(ref email) = request.email {
            check_query = check_query.filter(users::Column::Email.eq(email));
        }

        if let Ok(Some(existing)) = check_query.one(state.database.as_ref()).await {
            if let Some(ref username) = request.username
                && existing.username == *username
            {
                return crate::management::response::app_error(business_error(format!(
                    "username conflict: {}",
                    username.clone()
                )));
            }
            if let Some(ref email) = request.email
                && existing.email == *email
            {
                return crate::management::response::app_error(business_error(format!(
                    "email conflict: {}",
                    email.clone()
                )));
            }
        }
    }

    // 更新用户信息
    let mut active_model: users::ActiveModel = user.into();

    if let Some(username) = request.username {
        active_model.username = Set(username);
    }

    if let Some(email) = request.email {
        active_model.email = Set(email);
    }

    if let Some(password) = request.password {
        // 生成新密码哈希
        let password_hash = match hash(&password, DEFAULT_COST) {
            Ok(hash) => hash,
            Err(err) => {
                lerror!(
                    "system",
                    LogStage::Internal,
                    LogComponent::Auth,
                    "hash_password_fail",
                    &format!("密码加密失败: {err}")
                );
                return crate::management::response::app_error(ProxyError::internal_with_source(
                    "密码加密失败",
                    err,
                ));
            }
        };
        active_model.password_hash = Set(password_hash);
    }

    if let Some(is_active) = request.is_active {
        active_model.is_active = Set(is_active);
    }

    if let Some(is_admin) = request.is_admin {
        active_model.is_admin = Set(is_admin);
    }

    active_model.updated_at = Set(Utc::now().naive_utc());

    match active_model.update(state.database.as_ref()).await {
        Ok(updated_user) => {
            let updated_stats = get_user_statistics(updated_user.id, state.database.as_ref()).await;
            let user_response = UserResponse::from_user_with_stats(updated_user, &updated_stats);
            response::success_with_message(user_response, "用户更新成功")
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "update_user_fail",
                &format!("更新用户失败: {err}")
            );
            crate::management::response::app_error(crate::error!(
                Database,
                format!("更新用户失败: {}", err)
            ))
        }
    }
}

/// 删除用户
pub async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    // 权限检查：只有管理员可以删除用户
    if !auth_context.is_admin {
        return crate::management::response::app_error(permission_error("权限不足"));
    }

    let current_user_id = auth_context.user_id;

    // 不能删除自己
    if current_user_id == user_id {
        return crate::management::response::app_error(business_error("不能删除自己"));
    }

    // 检查用户是否存在
    let _user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return crate::management::response::app_error(business_error(format!(
                "User not found: {user_id}"
            )));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "get_user_fail",
                &format!("获取用户失败: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("获取用户失败: {}", err)
            ));
        }
    };

    // 删除用户
    match Users::delete_by_id(user_id)
        .exec(state.database.as_ref())
        .await
    {
        Ok(_) => response::success_without_data("用户删除成功"),
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "delete_user_fail",
                &format!("删除用户失败: {err}")
            );
            crate::management::response::app_error(crate::error!(
                Database,
                format!("删除用户失败: {}", err)
            ))
        }
    }
}

/// 批量删除用户
pub async fn batch_delete_users(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<BatchDeleteRequest>,
) -> axum::response::Response {
    // 权限检查：只有管理员可以删除用户
    if !auth_context.is_admin {
        return crate::management::response::app_error(permission_error("权限不足"));
    }

    let current_user_id = auth_context.user_id;

    if request.ids.is_empty() {
        return crate::management::response::app_error(business_error("用户ID列表不能为空"));
    }

    // 检查是否包含当前用户
    if request.ids.contains(&current_user_id) {
        return crate::management::response::app_error(business_error("不能删除自己"));
    }

    // 执行批量删除
    match Users::delete_many()
        .filter(users::Column::Id.is_in(request.ids.clone()))
        .exec(state.database.as_ref())
        .await
    {
        Ok(result) => {
            let deleted_count = result.rows_affected;
            response::success_without_data(&format!("成功删除 {deleted_count} 个用户"))
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "batch_delete_users_fail",
                &format!("批量删除用户失败: {err}")
            );
            crate::management::response::app_error(crate::error!(
                Database,
                format!("批量删除用户失败: {}", err)
            ))
        }
    }
}

/// 切换用户状态
pub async fn toggle_user_status(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    // 权限检查：只有管理员可以切换用户状态
    if !auth_context.is_admin {
        return crate::management::response::app_error(permission_error("权限不足"));
    }

    // 获取现有用户
    let user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return crate::management::response::app_error(business_error(format!(
                "User not found: {user_id}"
            )));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "get_user_fail",
                &format!("获取用户失败: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("获取用户失败: {}", err)
            ));
        }
    };

    // 切换状态
    let mut active_model: users::ActiveModel = user.into();
    active_model.is_active = Set(!active_model.is_active.as_ref());
    active_model.updated_at = Set(Utc::now().naive_utc());

    match active_model.update(state.database.as_ref()).await {
        Ok(updated_user) => {
            let user_status_stats =
                get_user_statistics(updated_user.id, state.database.as_ref()).await;
            let user_response =
                UserResponse::from_user_with_stats(updated_user, &user_status_stats);
            response::success_with_message(user_response, "用户状态更新成功")
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "update_user_status_fail",
                &format!("更新用户状态失败: {err}")
            );
            crate::management::response::app_error(crate::error!(
                Database,
                format!("更新用户状态失败: {}", err)
            ))
        }
    }
}

/// 重置用户密码
pub async fn reset_user_password(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<ResetPasswordRequest>,
) -> axum::response::Response {
    // 权限检查：只有管理员可以重置密码
    if !auth_context.is_admin {
        return crate::management::response::app_error(permission_error("权限不足"));
    }

    // 验证新密码强度
    if request.new_password.len() < 8 {
        return crate::management::response::app_error(business_error("密码长度至少8字符"));
    }

    // 获取现有用户
    let user = match Users::find_by_id(user_id)
        .one(state.database.as_ref())
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return crate::management::response::app_error(business_error(format!(
                "User not found: {user_id}"
            )));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "get_user_fail",
                &format!("获取用户失败: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("获取用户失败: {}", err)
            ));
        }
    };

    // 生成新密码哈希
    let new_password_hash = match hash(&request.new_password, DEFAULT_COST) {
        Ok(hash) => hash,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Internal,
                LogComponent::Auth,
                "hash_password_fail",
                &format!("密码加密失败: {err}")
            );
            return crate::management::response::app_error(ProxyError::internal_with_source(
                "密码加密失败",
                err,
            ));
        }
    };

    // 更新密码
    let mut active_model: users::ActiveModel = user.into();
    active_model.password_hash = Set(new_password_hash);
    active_model.updated_at = Set(Utc::now().naive_utc());

    match active_model.update(state.database.as_ref()).await {
        Ok(_) => response::success_without_data("密码重置成功"),
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "reset_password_fail",
                &format!("重置密码失败: {err}")
            );
            crate::management::response::app_error(crate::error!(
                Database,
                format!("重置密码失败: {}", err)
            ))
        }
    }
}
