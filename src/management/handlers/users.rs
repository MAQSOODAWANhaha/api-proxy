//! # 用户管理处理器
#![allow(clippy::cognitive_complexity, clippy::too_many_lines)]

use crate::logging::{LogComponent, LogStage, log_management_error};
use crate::management::middleware::{RequestId, auth::AuthContext};
use crate::management::services::shared::ServiceResponse;
use crate::management::services::users::{
    BatchDeleteRequest, ChangePasswordRequest, CreateUserRequest, ListUsersResult,
    ResetPasswordRequest, UpdateProfileRequest, UpdateUserRequest, UserQuery, UsersService,
};
use crate::management::{response, server::ManagementState};
use crate::types::TimezoneContext;
use axum::extract::{Extension, Path, Query, State};
use axum::response::Json;
use std::sync::Arc;

/// 获取用户统计信息（用于管理端统计卡片）
pub async fn get_user_stats(
    State(state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let service = UsersService::new(&state);
    match service.stats(auth_context.as_ref()).await {
        Ok(user_stats) => response::success(user_stats),
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "get_user_stats_failed",
                "获取用户统计失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

pub async fn list_users(
    State(state): State<ManagementState>,
    Query(query): Query<UserQuery>,
    Extension(request_id): Extension<RequestId>,
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
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "list_users_failed",
                "获取用户列表失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 创建用户
pub async fn create_user(
    State(state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
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
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "create_user_failed",
                "创建用户失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 获取单个用户
pub async fn get_user(
    State(state): State<ManagementState>,
    Path(user_id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
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
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "get_user_failed",
                "获取用户失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 获取用户档案
pub async fn get_user_profile(
    State(state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
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
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "get_user_profile_failed",
                "获取用户档案失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 更新用户档案
pub async fn update_user_profile(
    State(state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
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
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "update_user_profile_failed",
                "更新用户档案失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 修改密码
pub async fn change_password(
    State(state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
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
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "change_password_failed",
                "修改密码失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 更新用户
pub async fn update_user(
    State(state): State<ManagementState>,
    Path(user_id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
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
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "update_user_failed",
                "更新用户失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 删除用户
pub async fn delete_user(
    State(state): State<ManagementState>,
    Path(user_id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    let service = UsersService::new(&state);
    match service.delete_user(auth_context.as_ref(), user_id).await {
        Ok(ServiceResponse { data: (), message }) => {
            let msg = message.unwrap_or_else(|| "用户删除成功".to_string());
            response::success_with_message((), &msg)
        }
        Err(err) => {
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "delete_user_failed",
                "删除用户失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 批量删除用户
pub async fn batch_delete_users(
    State(state): State<ManagementState>,
    Extension(request_id): Extension<RequestId>,
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
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "batch_delete_users_failed",
                "批量删除用户失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 切换用户状态
pub async fn toggle_user_status(
    State(state): State<ManagementState>,
    Path(user_id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
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
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "toggle_user_status_failed",
                "切换用户状态失败",
                &err,
            );
            response::app_error(err)
        }
    }
}

/// 重置用户密码
pub async fn reset_user_password(
    State(state): State<ManagementState>,
    Path(user_id): Path<i32>,
    Extension(request_id): Extension<RequestId>,
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
            log_management_error(
                &request_id,
                LogStage::Db,
                LogComponent::Database,
                "reset_user_password_failed",
                "重置用户密码失败",
                &err,
            );
            response::app_error(err)
        }
    }
}
