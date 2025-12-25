//! # 用户管理服务
//!
//! 集中管理用户查询、创建、更新等业务逻辑，供 HTTP handler 复用。

use bcrypt::{DEFAULT_COST, hash, verify};
use chrono::{Datelike, Utc};
use entity::{proxy_tracing, proxy_tracing::Entity as ProxyTracing, users, users::Entity as Users};
use rand::{Rng, distributions::Alphanumeric};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Select, Set,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::{Context, ProxyError, Result},
    lerror,
    logging::{LogComponent, LogStage},
    management::middleware::auth::AuthContext,
    management::response::Pagination,
    management::server::ManagementState,
    types::{TimezoneContext, timezone_utils},
};

use super::shared::{PaginationParams, ServiceResponse, build_page};

/// 用户列表查询参数
#[derive(Debug, Deserialize)]
pub struct UserQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub search: Option<String>,
    pub is_active: Option<bool>,
    pub is_admin: Option<bool>,
    pub sort: Option<String>,
    pub order: Option<String>,
}

/// 创建用户请求
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub is_admin: Option<bool>,
}

/// 更新用户请求
#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
    pub is_active: Option<bool>,
    pub is_admin: Option<bool>,
}

/// 批量删除请求
#[derive(Debug, Deserialize)]
pub struct BatchDeleteRequest {
    pub ids: Vec<i32>,
}

/// 重置密码请求
#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub new_password: String,
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

/// 用户响应
#[derive(Debug, Serialize, Clone)]
pub struct UserResponse {
    pub id: i32,
    pub username: String,
    pub email: String,
    pub is_active: bool,
    pub is_admin: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_login: Option<String>,
    pub total_requests: i64,
    pub total_cost: f64,
    pub total_tokens: i64,
}

impl UserResponse {
    fn from_user_with_timezone(user: users::Model, timezone: &TimezoneContext) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            is_active: user.is_active,
            is_admin: user.is_admin,
            created_at: timezone_utils::format_utc_for_response(
                &user.created_at.and_utc(),
                &timezone.timezone,
            ),
            updated_at: timezone_utils::format_utc_for_response(
                &user.updated_at.and_utc(),
                &timezone.timezone,
            ),
            last_login: user.last_login.map(|dt| {
                timezone_utils::format_utc_for_response(&dt.and_utc(), &timezone.timezone)
            }),
            total_requests: 0,
            total_cost: 0.0,
            total_tokens: 0,
        }
    }

    fn with_stats(self, stats: &UserStats) -> Self {
        Self {
            total_requests: stats.requests,
            total_cost: stats.cost,
            total_tokens: stats.tokens,
            ..self
        }
    }
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

/// 用户统计（用于管理端页面顶部统计卡片）
#[derive(Debug, Serialize)]
pub struct UsersStatsResponse {
    pub total: u64,
    pub active: u64,
    pub admin: u64,
    pub inactive: u64,
}

#[derive(Debug)]
pub struct ListUsersResult {
    pub users: Vec<UserResponse>,
    pub pagination: Pagination,
}

#[derive(Debug, Default)]
struct UserStats {
    requests: i64,
    cost: f64,
    tokens: i64,
}

/// 用户服务
pub struct UsersService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> UsersService<'a> {
    #[must_use]
    pub fn new(state: &'a ManagementState) -> Self {
        Self {
            db: state.database.as_ref(),
        }
    }

    const fn db(&self) -> &'a DatabaseConnection {
        self.db
    }

    /// 列出用户
    pub async fn list(
        &self,
        auth: &AuthContext,
        timezone: &TimezoneContext,
        query: &UserQuery,
    ) -> Result<ListUsersResult> {
        if !auth.is_admin {
            return self.list_single_user(auth.user_id, timezone).await;
        }

        self.list_admin_users(timezone, query).await
    }

    /// 获取用户统计信息
    ///
    /// - 管理员：返回全量用户统计
    /// - 非管理员：仅返回当前用户的“视图内统计”（总数=1）
    pub async fn stats(&self, auth: &AuthContext) -> Result<UsersStatsResponse> {
        if !auth.is_admin {
            let user = self.fetch_user(auth.user_id).await?;
            let total = 1_u64;
            let active = u64::from(user.is_active);
            let inactive = u64::from(!user.is_active);
            let admin = u64::from(user.is_admin);
            return Ok(UsersStatsResponse {
                total,
                active,
                admin,
                inactive,
            });
        }

        let total = Users::find()
            .count(self.db())
            .await
            .context("Failed to count users")?;
        let active = Users::find()
            .filter(users::Column::IsActive.eq(true))
            .count(self.db())
            .await
            .context("Failed to count active users")?;
        let inactive = Users::find()
            .filter(users::Column::IsActive.eq(false))
            .count(self.db())
            .await
            .context("Failed to count inactive users")?;
        let admin = Users::find()
            .filter(users::Column::IsAdmin.eq(true))
            .count(self.db())
            .await
            .context("Failed to count admin users")?;

        Ok(UsersStatsResponse {
            total,
            active,
            admin,
            inactive,
        })
    }

    async fn list_single_user(
        &self,
        user_id: i32,
        timezone: &TimezoneContext,
    ) -> Result<ListUsersResult> {
        let user = self.fetch_user(user_id).await?;
        let stats = self.get_user_statistics(user.id).await;
        let response = UserResponse::from_user_with_timezone(user, timezone).with_stats(&stats);
        Ok(ListUsersResult {
            users: vec![response],
            pagination: Pagination {
                page: 1,
                limit: 1,
                total: 1,
                pages: 1,
            },
        })
    }

    async fn list_admin_users(
        &self,
        timezone: &TimezoneContext,
        query: &UserQuery,
    ) -> Result<ListUsersResult> {
        let params = PaginationParams::new(
            query.page.map(u64::from),
            query.limit.map(u64::from),
            10,
            100,
        );

        let select = Self::filtered_users(query);
        let count_select = Self::filtered_users(query);
        let select = Self::apply_user_sort(select, query);

        let total = count_select
            .count(self.db())
            .await
            .context("Failed to count users")?;

        let users = select
            .offset(params.offset())
            .limit(params.limit)
            .all(self.db())
            .await
            .context("Failed to fetch users")?;

        let responses = self.build_user_responses(users, timezone).await;
        let pagination = build_page(total, params).into();

        Ok(ListUsersResult {
            users: responses,
            pagination,
        })
    }

    fn apply_user_sort(select: Select<Users>, query: &UserQuery) -> Select<Users> {
        let sort_field = query.sort.as_deref().unwrap_or("created_at");
        let asc = matches!(query.order.as_deref(), Some("asc"));

        match sort_field {
            "username" => {
                if asc {
                    select.order_by_asc(users::Column::Username)
                } else {
                    select.order_by_desc(users::Column::Username)
                }
            }
            "email" => {
                if asc {
                    select.order_by_asc(users::Column::Email)
                } else {
                    select.order_by_desc(users::Column::Email)
                }
            }
            "updated_at" => {
                if asc {
                    select.order_by_asc(users::Column::UpdatedAt)
                } else {
                    select.order_by_desc(users::Column::UpdatedAt)
                }
            }
            _ => {
                if asc {
                    select.order_by_asc(users::Column::CreatedAt)
                } else {
                    select.order_by_desc(users::Column::CreatedAt)
                }
            }
        }
    }

    async fn build_user_responses(
        &self,
        users: Vec<users::Model>,
        timezone: &TimezoneContext,
    ) -> Vec<UserResponse> {
        let mut responses = Vec::with_capacity(users.len());
        for user in users {
            let stats = self.get_user_statistics(user.id).await;
            responses
                .push(UserResponse::from_user_with_timezone(user, timezone).with_stats(&stats));
        }
        responses
    }

    fn filtered_users(query: &UserQuery) -> Select<Users> {
        let mut select = Users::find();

        if let Some(search) = query
            .search
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            let pattern = format!("%{search}%");
            let condition = users::Column::Username
                .like(&pattern)
                .or(users::Column::Email.like(&pattern));
            select = select.filter(condition);
        }

        if let Some(is_active) = query.is_active {
            select = select.filter(users::Column::IsActive.eq(is_active));
        }

        if let Some(is_admin) = query.is_admin {
            select = select.filter(users::Column::IsAdmin.eq(is_admin));
        }

        select
    }

    /// 创建用户
    pub async fn create(
        &self,
        auth: &AuthContext,
        timezone: &TimezoneContext,
        request: &CreateUserRequest,
    ) -> Result<ServiceResponse<UserResponse>> {
        ensure_admin(auth)?;
        validate_new_user_input(request)?;
        self.ensure_unique_user(None, request.username.as_str(), request.email.as_str())
            .await?;

        let salt: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        let password_hash = hash_password(&request.password)?;
        let now = Utc::now().naive_utc();
        let is_admin = request.is_admin.unwrap_or(false);

        let user_model = users::ActiveModel {
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

        let insert_result = Users::insert(user_model)
            .exec(self.db())
            .await
            .context("Failed to create user")?;

        let created_user = self.fetch_user(insert_result.last_insert_id).await?;
        let stats = self.get_user_statistics(created_user.id).await;
        let response =
            UserResponse::from_user_with_timezone(created_user, timezone).with_stats(&stats);

        Ok(ServiceResponse::with_message(response, "用户创建成功"))
    }

    /// 获取单个用户
    pub async fn get(
        &self,
        auth: &AuthContext,
        user_id: i32,
        timezone: &TimezoneContext,
    ) -> Result<UserResponse> {
        ensure_positive_id(user_id)?;
        if !auth.is_admin && auth.user_id != user_id {
            return Err(permission_error("权限不足"));
        }

        let user = self.fetch_user(user_id).await?;
        let stats = self.get_user_statistics(user.id).await;
        Ok(UserResponse::from_user_with_timezone(user, timezone).with_stats(&stats))
    }

    /// 获取当前用户档案
    pub async fn profile(
        &self,
        user_id: i32,
        timezone: &TimezoneContext,
    ) -> Result<UserProfileResponse> {
        let user = self.fetch_user(user_id).await?;
        let stats = self.get_user_statistics(user.id).await;
        let monthly_requests = self.get_user_monthly_requests(user.id).await;
        let avatar = generate_avatar_url(&user.email);
        let role = if user.is_admin {
            "系统管理员".to_string()
        } else {
            "普通用户".to_string()
        };

        Ok(UserProfileResponse {
            name: user.username,
            email: user.email,
            avatar,
            role,
            created_at: timezone_utils::format_utc_for_response(
                &user.created_at.and_utc(),
                &timezone.timezone,
            ),
            last_login: user.last_login.map(|dt| {
                timezone_utils::format_utc_for_response(&dt.and_utc(), &timezone.timezone)
            }),
            total_requests: stats.requests,
            monthly_requests,
        })
    }

    /// 更新当前用户档案
    pub async fn update_profile(
        &self,
        user_id: i32,
        timezone: &TimezoneContext,
        request: &UpdateProfileRequest,
    ) -> Result<ServiceResponse<UserProfileResponse>> {
        if let Some(email) = &request.email {
            validate_email(email)?;
        }

        let user = self.fetch_user(user_id).await?;
        let mut active_model: users::ActiveModel = user.into();

        if let Some(email) = &request.email {
            active_model.email = Set(email.clone());
        }
        active_model.updated_at = Set(Utc::now().naive_utc());

        let updated_user = active_model
            .update(self.db())
            .await
            .context("Failed to update user profile")?;

        let stats = self.get_user_statistics(updated_user.id).await;
        let monthly = self.get_user_monthly_requests(updated_user.id).await;
        let avatar = generate_avatar_url(&updated_user.email);
        let role = if updated_user.is_admin {
            "系统管理员".to_string()
        } else {
            "普通用户".to_string()
        };

        let profile = UserProfileResponse {
            name: updated_user.username,
            email: updated_user.email,
            avatar,
            role,
            created_at: timezone_utils::format_utc_for_response(
                &updated_user.created_at.and_utc(),
                &timezone.timezone,
            ),
            last_login: updated_user.last_login.map(|dt| {
                timezone_utils::format_utc_for_response(&dt.and_utc(), &timezone.timezone)
            }),
            total_requests: stats.requests,
            monthly_requests: monthly,
        };

        Ok(ServiceResponse::with_message(
            profile,
            "Profile updated successfully",
        ))
    }

    /// 修改密码
    pub async fn change_password(
        &self,
        user_id: i32,
        request: &ChangePasswordRequest,
    ) -> Result<ServiceResponse<()>> {
        if request.new_password.len() < 6 {
            return Err(business_error(
                "New password must be at least 6 characters long",
            ));
        }

        let user = self.fetch_user(user_id).await?;

        match verify(&request.current_password, &user.password_hash) {
            Ok(true) => {}
            Ok(false) => return Err(business_error("Current password is incorrect")),
            Err(err) => {
                lerror!(
                    "system",
                    LogStage::Internal,
                    LogComponent::Auth,
                    "verify_password_fail",
                    &format!("Failed to verify current password: {err}")
                );
                return Err(business_error("Failed to verify current password"));
            }
        }

        let new_hash = hash_password(&request.new_password)?;
        let mut active_model: users::ActiveModel = user.into();
        active_model.password_hash = Set(new_hash);
        active_model.updated_at = Set(Utc::now().naive_utc());

        active_model
            .update(self.db())
            .await
            .context("Failed to change password")?;

        Ok(ServiceResponse::with_message((), "密码修改成功"))
    }

    /// 管理员更新用户
    pub async fn update_user(
        &self,
        auth: &AuthContext,
        user_id: i32,
        timezone: &TimezoneContext,
        request: &UpdateUserRequest,
    ) -> Result<ServiceResponse<UserResponse>> {
        ensure_admin(auth)?;
        ensure_positive_id(user_id)?;

        validate_update_input(request)?;

        let user = self.fetch_user(user_id).await?;

        if request.username.is_some() || request.email.is_some() {
            self.ensure_unique_user(
                Some(user_id),
                request.username.as_deref().unwrap_or(""),
                request.email.as_deref().unwrap_or(""),
            )
            .await?;
        }

        let mut active_model: users::ActiveModel = user.into();

        if let Some(username) = &request.username {
            active_model.username = Set(username.clone());
        }
        if let Some(email) = &request.email {
            validate_email(email)?;
            active_model.email = Set(email.clone());
        }
        if let Some(is_active) = request.is_active {
            active_model.is_active = Set(is_active);
        }
        if let Some(is_admin) = request.is_admin {
            active_model.is_admin = Set(is_admin);
        }
        if let Some(password) = &request.password {
            ensure_password_strength(password)?;
            active_model.password_hash = Set(hash_password(password)?);
        }

        active_model.updated_at = Set(Utc::now().naive_utc());

        let updated_user = active_model
            .update(self.db())
            .await
            .context("Failed to update user")?;

        let stats = self.get_user_statistics(updated_user.id).await;
        let response =
            UserResponse::from_user_with_timezone(updated_user, timezone).with_stats(&stats);

        Ok(ServiceResponse::with_message(response, "用户更新成功"))
    }

    /// 删除用户
    pub async fn delete_user(
        &self,
        auth: &AuthContext,
        user_id: i32,
    ) -> Result<ServiceResponse<()>> {
        ensure_admin(auth)?;
        ensure_positive_id(user_id)?;
        if auth.user_id == user_id {
            return Err(business_error("不能删除自己"));
        }

        self.fetch_user(user_id).await?;

        Users::delete_by_id(user_id)
            .exec(self.db())
            .await
            .context("Failed to delete user")?;

        Ok(ServiceResponse::with_message((), "用户删除成功"))
    }

    /// 批量删除用户
    pub async fn batch_delete(
        &self,
        auth: &AuthContext,
        request: &BatchDeleteRequest,
    ) -> Result<ServiceResponse<()>> {
        ensure_admin(auth)?;
        if request.ids.is_empty() {
            return Err(business_error("用户ID列表不能为空"));
        }
        if request.ids.contains(&auth.user_id) {
            return Err(business_error("不能删除自己"));
        }

        let result = Users::delete_many()
            .filter(users::Column::Id.is_in(request.ids.clone()))
            .exec(self.db())
            .await
            .context("Failed to batch delete users")?;

        Ok(ServiceResponse::with_message(
            (),
            format!("成功删除 {} 个用户", result.rows_affected),
        ))
    }

    /// 切换用户状态
    pub async fn toggle_status(
        &self,
        auth: &AuthContext,
        user_id: i32,
        timezone: &TimezoneContext,
    ) -> Result<ServiceResponse<UserResponse>> {
        ensure_admin(auth)?;

        let user = self.fetch_user(user_id).await?;
        let mut active_model: users::ActiveModel = user.into();
        let current_active = match active_model.is_active.clone() {
            ActiveValue::Set(value) | ActiveValue::Unchanged(value) => value,
            ActiveValue::NotSet => false,
        };
        active_model.is_active = Set(!current_active);
        active_model.updated_at = Set(Utc::now().naive_utc());

        let updated_user = active_model
            .update(self.db())
            .await
            .context("Failed to toggle user status")?;

        let stats = self.get_user_statistics(updated_user.id).await;
        let response =
            UserResponse::from_user_with_timezone(updated_user, timezone).with_stats(&stats);

        Ok(ServiceResponse::with_message(response, "用户状态更新成功"))
    }

    /// 重置密码
    pub async fn reset_password(
        &self,
        auth: &AuthContext,
        user_id: i32,
        request: &ResetPasswordRequest,
    ) -> Result<ServiceResponse<()>> {
        ensure_admin(auth)?;
        ensure_positive_id(user_id)?;
        ensure_password_strength(&request.new_password)?;

        let user = self.fetch_user(user_id).await?;
        let mut active_model: users::ActiveModel = user.into();
        active_model.password_hash = Set(hash_password(&request.new_password)?);
        active_model.updated_at = Set(Utc::now().naive_utc());

        active_model
            .update(self.db())
            .await
            .context("Failed to reset password")?;

        Ok(ServiceResponse::with_message((), "密码重置成功"))
    }

    async fn fetch_user(&self, user_id: i32) -> Result<users::Model> {
        Users::find_by_id(user_id)
            .one(self.db())
            .await
            .context("Failed to fetch user")?
            .ok_or_else(|| business_error(format!("User not found: {user_id}")))
    }

    async fn ensure_unique_user(
        &self,
        exclude_id: Option<i32>,
        username: &str,
        email: &str,
    ) -> Result<()> {
        if username.is_empty() && email.is_empty() {
            return Ok(());
        }

        let mut query = Users::find();
        if let Some(id) = exclude_id {
            query = query.filter(users::Column::Id.ne(id));
        }
        if !username.is_empty() {
            query = query.filter(users::Column::Username.eq(username));
        }
        if !email.is_empty() {
            query = query.filter(users::Column::Email.eq(email));
        }

        if let Some(existing) = query
            .one(self.db())
            .await
            .context("Failed to check existing user")?
        {
            if existing.username == username {
                return Err(business_error(format!("username conflict: {username}")));
            }
            if existing.email == email {
                return Err(business_error(format!("email conflict: {email}")));
            }
        }
        Ok(())
    }

    async fn get_user_statistics(&self, user_id: i32) -> UserStats {
        let stats = ProxyTracing::find()
            .select_only()
            .column_as(proxy_tracing::Column::Id.count(), "total_requests")
            .column_as(proxy_tracing::Column::Cost.sum(), "total_cost")
            .column_as(proxy_tracing::Column::TokensTotal.sum(), "total_tokens")
            .filter(proxy_tracing::Column::UserId.eq(user_id))
            .into_tuple::<(Option<i64>, Option<f64>, Option<i64>)>()
            .one(self.db())
            .await
            .ok()
            .flatten();

        match stats {
            Some((requests, cost, tokens)) => UserStats {
                requests: requests.unwrap_or(0),
                cost: cost.unwrap_or(0.0),
                tokens: tokens.unwrap_or(0),
            },
            None => UserStats::default(),
        }
    }

    async fn get_user_monthly_requests(&self, user_id: i32) -> i64 {
        let now = Utc::now().naive_utc();
        let month_start = now
            .date()
            .with_day(1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();

        ProxyTracing::find()
            .select_only()
            .column_as(proxy_tracing::Column::Id.count(), "monthly_requests")
            .filter(proxy_tracing::Column::UserId.eq(user_id))
            .filter(proxy_tracing::Column::CreatedAt.gte(month_start))
            .into_tuple::<Option<i64>>()
            .one(self.db())
            .await
            .ok()
            .flatten()
            .flatten()
            .unwrap_or(0)
    }
}

fn generate_avatar_url(email: &str) -> String {
    let email_hash = md5::compute(email.to_lowercase());
    let hash_str = format!("{email_hash:x}");
    format!("https://www.gravatar.com/avatar/{hash_str}?d=identicon&s=200")
}

fn hash_password(password: &str) -> Result<String> {
    match hash(password, DEFAULT_COST) {
        Ok(hashed) => Ok(hashed),
        Err(err) => {
            lerror!(
                "system",
                LogStage::Internal,
                LogComponent::Auth,
                "password_hash_fail",
                &format!("Failed to hash password: {err}")
            );
            Err(business_error("密码加密失败"))
        }
    }
}

fn ensure_admin(auth: &AuthContext) -> Result<()> {
    if auth.is_admin {
        Ok(())
    } else {
        Err(permission_error("权限不足"))
    }
}

fn ensure_positive_id(id: i32) -> Result<()> {
    if id > 0 {
        Ok(())
    } else {
        Err(business_error("Invalid user ID"))
    }
}

fn validate_new_user_input(request: &CreateUserRequest) -> Result<()> {
    ensure_username(&request.username)?;
    validate_email(&request.email)?;
    ensure_password_strength(&request.password)?;
    Ok(())
}

fn validate_update_input(request: &UpdateUserRequest) -> Result<()> {
    if let Some(username) = &request.username {
        ensure_username(username)?;
    }
    if let Some(email) = &request.email {
        validate_email(email)?;
    }
    if let Some(password) = &request.password {
        ensure_password_strength(password)?;
    }
    Ok(())
}

fn ensure_username(username: &str) -> Result<()> {
    if (3..=50).contains(&username.len()) {
        Ok(())
    } else {
        Err(business_error("用户名长度必须在3-50字符之间"))
    }
}

fn validate_email(email: &str) -> Result<()> {
    if email.len() <= 100 && email.contains('@') {
        Ok(())
    } else {
        Err(business_error("邮箱格式无效或长度超过100字符"))
    }
}

fn ensure_password_strength(password: &str) -> Result<()> {
    if password.len() >= 8 {
        Ok(())
    } else {
        Err(business_error("密码长度至少8字符"))
    }
}

fn business_error(message: impl Into<String>) -> ProxyError {
    crate::error::auth::AuthError::Message(message.into()).into()
}

fn permission_error(message: impl Into<String>) -> ProxyError {
    crate::error::auth::AuthError::PermissionDenied {
        required: message.into(),
        actual: "none".to_string(),
    }
    .into()
}
