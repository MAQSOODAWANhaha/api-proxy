//! # 用户管理处理器
#![allow(clippy::cognitive_complexity, clippy::too_many_lines)]

use crate::management::middleware::auth::AuthContext;
use crate::management::services::shared::ServiceResponse;
use crate::management::services::users::{
    BatchDeleteRequest, ChangePasswordRequest, CreateUserRequest, ListUsersResult,
    ResetPasswordRequest, UpdateProfileRequest, UpdateUserRequest, UserQuery, UsersService,
};
use crate::management::{response, server::AppState};
use crate::types::TimezoneContext;
use axum::extract::{Extension, Path, Query, State};
use axum::response::Json;
use std::sync::Arc;

pub async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<UserQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = UsersService::new(&state);
    match service
        .list(auth_context.as_ref(), &timezone_context, &query)
        .await
    {
        Ok(ListUsersResult { users, pagination }) => response::paginated(users, pagination),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 创建用户
pub async fn create_user(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
    Json(request): Json<CreateUserRequest>,
) -> axum::response::Response {
    let service = UsersService::new(&state);
    match service
        .create(auth_context.as_ref(), &timezone_context, &request)
        .await
    {
        Ok(ServiceResponse { data, message }) => {
            let msg = message.unwrap_or_else(|| "用户创建成功".to_string());
            response::success_with_message(data, &msg)
        }
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 获取单个用户
pub async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = UsersService::new(&state);
    match service
        .get(auth_context.as_ref(), user_id, &timezone_context)
        .await
    {
        Ok(user) => response::success(user),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 获取用户档案
pub async fn get_user_profile(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = UsersService::new(&state);
    match service
        .profile(auth_context.user_id, &timezone_context)
        .await
    {
        Ok(profile) => response::success(profile),
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 更新用户档案
pub async fn update_user_profile(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
    Json(request): Json<UpdateProfileRequest>,
) -> axum::response::Response {
    let service = UsersService::new(&state);
    match service
        .update_profile(auth_context.user_id, &timezone_context, &request)
        .await
    {
        Ok(ServiceResponse { data, message }) => {
            let msg = message.unwrap_or_else(|| "Profile updated successfully".to_string());
            response::success_with_message(data, &msg)
        }
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 修改密码
pub async fn change_password(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<ChangePasswordRequest>,
) -> axum::response::Response {
    let service = UsersService::new(&state);
    match service
        .change_password(auth_context.user_id, &request)
        .await
    {
        Ok(ServiceResponse { data: (), message }) => {
            let msg = message.unwrap_or_else(|| "密码修改成功".to_string());
            response::success_with_message((), &msg)
        }
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 更新用户
pub async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
    Json(request): Json<UpdateUserRequest>,
) -> axum::response::Response {
    let service = UsersService::new(&state);
    match service
        .update_user(auth_context.as_ref(), user_id, &timezone_context, &request)
        .await
    {
        Ok(ServiceResponse { data, message }) => {
            let msg = message.unwrap_or_else(|| "用户更新成功".to_string());
            response::success_with_message(data, &msg)
        }
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 删除用户
pub async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let service = UsersService::new(&state);
    match service.delete_user(auth_context.as_ref(), user_id).await {
        Ok(ServiceResponse { data: (), message }) => {
            let msg = message.unwrap_or_else(|| "用户删除成功".to_string());
            response::success_with_message((), &msg)
        }
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 批量删除用户
pub async fn batch_delete_users(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<BatchDeleteRequest>,
) -> axum::response::Response {
    let service = UsersService::new(&state);
    match service.batch_delete(auth_context.as_ref(), &request).await {
        Ok(ServiceResponse { data: (), message }) => {
            let msg = message.unwrap_or_else(|| "批量删除成功".to_string());
            response::success_with_message((), &msg)
        }
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}

/// 切换用户状态
pub async fn toggle_user_status(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Extension(timezone_context): Extension<Arc<TimezoneContext>>,
) -> axum::response::Response {
    let service = UsersService::new(&state);
    match service
        .toggle_status(auth_context.as_ref(), user_id, &timezone_context)
        .await
    {
        Ok(ServiceResponse { data, message }) => {
            let msg = message.unwrap_or_else(|| "用户状态更新成功".to_string());
            response::success_with_message(data, &msg)
        }
        Err(err) => {
            err.log();
            response::app_error(err)
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
    let service = UsersService::new(&state);
    match service
        .reset_password(auth_context.as_ref(), user_id, &request)
        .await
    {
        Ok(ServiceResponse { data: (), message }) => {
            let msg = message.unwrap_or_else(|| "密码重置成功".to_string());
            response::success_with_message((), &msg)
        }
        Err(err) => {
            err.log();
            response::app_error(err)
        }
    }
}
