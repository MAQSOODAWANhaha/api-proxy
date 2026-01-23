//! # 提供商密钥 OAuth 辅助功能
//!
//! 处理 OAuth 相关的逻辑，包括会话管理、token 刷新调度等。

use entity::{
    oauth_client_sessions, oauth_client_sessions::Entity as OAuthSession, user_provider_keys,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::{
    auth::{
        api_key_oauth_state_service::ScheduledTokenRefresh,
        api_key_oauth_token_refresh_task::ApiKeyOAuthTokenRefreshTask, types::AuthStatus,
    },
    ensure_context,
    error::{Context, Result, management::ManagementError},
    ldebug,
    logging::{LogComponent, LogStage},
    lwarn,
};

use std::sync::Arc;

const OAUTH_AUTH_TYPE: &str = "oauth";

/// OAuth 辅助器
pub struct OAuthHelper {
    pub db: DatabaseConnection,
    pub refresh_task: Option<Arc<ApiKeyOAuthTokenRefreshTask>>,
}

impl OAuthHelper {
    /// 准备 OAuth 调度
    pub async fn prepare_schedule(
        &self,
        session_id: Option<&String>,
        user_id: i32,
        key_id: Option<i32>,
    ) -> Result<Option<ScheduledTokenRefresh>> {
        prepare_oauth_schedule(self.refresh_task.as_deref(), session_id, user_id, key_id).await
    }

    /// 入队 OAuth 调度
    pub async fn enqueue_schedule(
        &self,
        pending_schedule: Option<ScheduledTokenRefresh>,
        user_id: i32,
        inserted_key: &user_provider_keys::Model,
    ) -> Result<()> {
        enqueue_oauth_schedule(
            self.refresh_task.as_deref(),
            pending_schedule,
            &self.db,
            user_id,
            inserted_key,
        )
        .await
    }

    /// 清理过时的 OAuth 会话
    pub async fn cleanup_obsolete_session(
        &self,
        old_session_id: Option<String>,
        updated_key: &user_provider_keys::Model,
        user_id: i32,
        key_id: i32,
    ) {
        cleanup_obsolete_session_internal(
            self.refresh_task.as_deref(),
            old_session_id,
            updated_key,
            user_id,
            key_id,
        )
        .await;
    }

    /// 提取 OAuth 会话 ID
    pub fn extract_session_id(key: &user_provider_keys::Model) -> Option<String> {
        if key.auth_type == OAUTH_AUTH_TYPE && !key.api_key.is_empty() {
            Some(key.api_key.clone())
        } else {
            None
        }
    }
}

/// 检查是否需要 OAuth 调度
pub fn needs_oauth_schedule(auth_type: &str) -> bool {
    auth_type == OAUTH_AUTH_TYPE
}

/// 准备 OAuth 调度
async fn prepare_oauth_schedule(
    task: Option<&ApiKeyOAuthTokenRefreshTask>,
    session_id: Option<&String>,
    user_id: i32,
    key_id: Option<i32>,
) -> Result<Option<ScheduledTokenRefresh>> {
    let Some(task) = task else {
        lwarn!(
            "system",
            LogStage::Scheduling,
            LogComponent::OAuth,
            "task_unavailable",
            "OAuth refresh task unavailable, skip scheduling",
            user_id = user_id,
            key_id = key_id,
        );
        return Ok(None);
    };

    let Some(session_id) = session_id.filter(|id| !id.is_empty()) else {
        return Ok(None);
    };

    match task.prepare_schedule(session_id).await {
        Ok(schedule) => Ok(Some(schedule)),
        Err(err) => {
            use crate::{lerror, logging::LogComponent, logging::LogStage};

            lerror!(
                "system",
                LogStage::Scheduling,
                LogComponent::OAuth,
                "prepare_schedule_fail",
                &format!("Failed to prepare OAuth refresh schedule: {err}"),
                user_id = user_id,
                key_id = key_id,
                session_id = session_id.as_str(),
            );
            Err(err)
        }
    }
}

/// 入队 OAuth 调度
async fn enqueue_oauth_schedule(
    refresh_task: Option<&ApiKeyOAuthTokenRefreshTask>,
    pending_schedule: Option<ScheduledTokenRefresh>,
    db: &DatabaseConnection,
    user_id: i32,
    inserted_key: &user_provider_keys::Model,
) -> Result<()> {
    let Some(schedule) = pending_schedule else {
        return Ok(());
    };

    let Some(task) = refresh_task else {
        lwarn!(
            "system",
            LogStage::Scheduling,
            LogComponent::OAuth,
            "task_unavailable_no_enqueue",
            "OAuth refresh task unavailable, schedule not enqueued",
            user_id = user_id,
            key_id = inserted_key.id,
        );
        return Ok(());
    };

    if let Err(err) = task.enqueue_schedule(schedule).await {
        use crate::{lerror, logging::LogComponent, logging::LogStage};

        lerror!(
            "system",
            LogStage::Scheduling,
            LogComponent::OAuth,
            "enqueue_schedule_fail",
            &format!("Failed to enqueue OAuth refresh schedule: {err}"),
            user_id = user_id,
            key_id = inserted_key.id,
        );

        if let Err(delete_err) = user_provider_keys::Entity::delete_by_id(inserted_key.id)
            .exec(db)
            .await
        {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "rollback_key_fail",
                &format!("Failed to rollback provider key after enqueue error: {delete_err}"),
                user_id = user_id,
                key_id = inserted_key.id,
            );
        }

        return Err(err);
    }

    Ok(())
}

/// 清理过时的 OAuth 会话
async fn cleanup_obsolete_session_internal(
    refresh_task: Option<&ApiKeyOAuthTokenRefreshTask>,
    old_session_id: Option<String>,
    updated_key: &user_provider_keys::Model,
    user_id: i32,
    key_id: i32,
) {
    let Some(old_id) = old_session_id else {
        return;
    };

    let Some(task) = refresh_task else {
        return;
    };

    let updated_session_id = extract_oauth_session_id(updated_key);
    if updated_session_id.as_deref() == Some(old_id.as_str()) {
        return;
    }

    if let Err(err) = task.remove_session(&old_id).await {
        lwarn!(
            "system",
            LogStage::Scheduling,
            LogComponent::OAuth,
            "remove_old_session_fail",
            &format!("Failed to remove old OAuth session from refresh queue: {err}"),
            user_id = user_id,
            key_id = key_id,
            session_id = old_id.as_str(),
        );
    }
}

/// 提取 OAuth 会话 ID
fn extract_oauth_session_id(key: &user_provider_keys::Model) -> Option<String> {
    if key.auth_type == OAUTH_AUTH_TYPE && !key.api_key.is_empty() {
        Some(key.api_key.clone())
    } else {
        None
    }
}

/// 获取 OAuth 密钥的 access token
pub async fn get_access_token_for_key(
    db: &DatabaseConnection,
    key_id: i32,
    user_id: &str,
) -> Result<String> {
    use entity::user_provider_keys::Entity as UserProviderKey;

    let key_record = UserProviderKey::find_by_id(key_id)
        .one(db)
        .await
        .context("Failed to fetch key record")?
        .ok_or_else(|| ManagementError::ProviderKeyNotFound {
            key_id,
            user_id: user_id.to_string(),
        })?;

    ensure_context!(
        key_record.auth_type == OAUTH_AUTH_TYPE,
        ManagementError::InvalidKeyAuthType {
            key_id,
            expected: OAUTH_AUTH_TYPE.to_string(),
            actual: key_record.auth_type.clone(),
        },
        format!("自动获取 project_id 前校验 key 类型失败: key_id={key_id}, user_id={user_id}")
    );

    let session_id = key_record.api_key;
    ensure_context!(
        !session_id.is_empty(),
        ManagementError::MissingOAuthSessionId { key_id },
        format!("自动获取 project_id 时检测到 session_id 为空: key_id={key_id}")
    );

    let oauth_session = OAuthSession::find()
        .filter(oauth_client_sessions::Column::SessionId.eq(&session_id))
        .filter(oauth_client_sessions::Column::UserId.eq(user_id))
        .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
        .one(db)
        .await
        .context("Failed to fetch OAuth session")?
        .ok_or_else(|| ManagementError::OAuthSessionNotFound {
            session_id: session_id.clone(),
            user_id: user_id.to_string(),
        })?;

    let access_token = oauth_session
        .access_token
        .filter(|token| !token.is_empty())
        .ok_or_else(|| ManagementError::OAuthSessionTokenMissing {
            session_id: session_id.clone(),
        })?;

    ldebug!(
        "system",
        LogStage::Authentication,
        LogComponent::OAuth,
        "load_access_token_success",
        "Loaded access token for auto-get project_id task",
        session_id = session_id.as_str(),
    );

    Ok(access_token)
}

/// 验证 OAuth 会话
pub async fn validate_oauth_session(
    db: &DatabaseConnection,
    session_id: &str,
    user_id: i32,
) -> Result<Option<oauth_client_sessions::Model>> {
    OAuthSession::find()
        .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
        .filter(oauth_client_sessions::Column::UserId.eq(user_id))
        .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
        .one(db)
        .await
        .context("Failed to validate OAuth session")
}
