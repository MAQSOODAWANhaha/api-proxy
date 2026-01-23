//! # 提供商密钥 Gemini 特定逻辑
//!
//! 处理 Gemini Code Assist 相关的特殊逻辑，包括 `project_id` 获取等。

use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tokio::spawn;

use crate::{
    auth::{gemini_code_assist_client::GeminiCodeAssistClient, types::AuthStatus},
    error::{Context, Result},
    key_pool::types::ApiKeyHealthStatus,
    lerror, linfo,
    logging::{LogComponent, LogStage},
    lwarn,
};

use entity::{
    oauth_client_sessions, oauth_client_sessions::Entity as OAuthSession, user_provider_keys,
    user_provider_keys::Entity as UserProviderKey,
};

use super::models::PrepareGeminiContext;

const GEMINI_PROVIDER_NAME: &str = "gemini";
const OAUTH_AUTH_TYPE: &str = "oauth";

/// 准备 Gemini 上下文
pub async fn prepare_gemini_context(
    db: &DatabaseConnection,
    user_id: i32,
    api_key: Option<&String>,
    project_id: Option<String>,
    provider_type_name: &str,
) -> Result<PrepareGeminiContext> {
    let mut context = PrepareGeminiContext {
        final_project_id: project_id,
        health_status: ApiKeyHealthStatus::Healthy.to_string(),
        needs_auto_get_project_id_async: false,
    };

    if !is_gemini_oauth_flow(OAUTH_AUTH_TYPE, provider_type_name) {
        return Ok(context);
    }

    let Some(session_id) = api_key else {
        log_missing_session_id(user_id);
        context.health_status = ApiKeyHealthStatus::Unhealthy.to_string();
        return Ok(context);
    };

    let Some(oauth_session) = fetch_authorized_session(db, user_id, session_id).await? else {
        log_missing_authorized_session(user_id);
        context.health_status = ApiKeyHealthStatus::Unhealthy.to_string();
        return Ok(context);
    };

    let access_token = oauth_session.access_token.as_deref().unwrap_or("");
    let gemini_client = GeminiCodeAssistClient::new();

    if let Some(provided_pid) = context.final_project_id.clone() {
        process_provided_project_id(
            &gemini_client,
            access_token,
            provided_pid,
            user_id,
            &mut context,
        )
        .await;
    } else {
        mark_project_id_pending(user_id, &mut context);
    }

    Ok(context)
}

/// 检查是否为 Gemini OAuth 流程
fn is_gemini_oauth_flow(auth_type: &str, provider_type_name: &str) -> bool {
    auth_type == OAUTH_AUTH_TYPE && provider_type_name == GEMINI_PROVIDER_NAME
}

/// 获取已授权的会话
async fn fetch_authorized_session(
    db: &DatabaseConnection,
    user_id: i32,
    session_id: &str,
) -> Result<Option<oauth_client_sessions::Model>> {
    OAuthSession::find()
        .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
        .filter(oauth_client_sessions::Column::UserId.eq(user_id))
        .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
        .one(db)
        .await
        .context("Failed to fetch authorized OAuth session")
}

/// 记录缺少 `session_id`
fn log_missing_session_id(user_id: i32) {
    lerror!(
        "system",
        LogStage::Authentication,
        LogComponent::OAuth,
        "gemini_missing_session_id",
        "Gemini OAuth: Missing session_id (api_key field), cannot complete validation",
        user_id = user_id,
    );
}

/// 记录缺少已授权会话
fn log_missing_authorized_session(user_id: i32) {
    lerror!(
        "system",
        LogStage::Authentication,
        LogComponent::OAuth,
        "gemini_no_auth_session",
        "Gemini OAuth: Authorized OAuth session not found, cannot validate project_id",
        user_id = user_id,
    );
}

/// 处理用户提供的 `project_id`
async fn process_provided_project_id(
    gemini_client: &GeminiCodeAssistClient,
    access_token: &str,
    provided_pid: String,
    user_id: i32,
    context: &mut PrepareGeminiContext,
) {
    linfo!(
        "system",
        LogStage::Authentication,
        LogComponent::OAuth,
        "gemini_load_assist_with_project",
        "Gemini OAuth: Using user-provided project_id to call loadCodeAssist",
        user_id = user_id,
        project_id = %provided_pid,
    );

    match gemini_client
        .load_code_assist(access_token, Some(&provided_pid), None)
        .await
    {
        Ok(resp) => {
            if let Some(server_pid) = resp.cloudaicompanion_project {
                context.final_project_id = Some(server_pid);
            } else {
                linfo!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::OAuth,
                    "gemini_invalid_project_id",
                    "loadCodeAssist did not return cloudaicompanionProject, user-provided project_id is invalid",
                    user_id = user_id,
                    provided_project_id = %provided_pid,
                );
                context.health_status = ApiKeyHealthStatus::Unhealthy.to_string();
                context.needs_auto_get_project_id_async = true;
                context.final_project_id = None;
            }
        }
        Err(e) => {
            context.health_status = ApiKeyHealthStatus::Unhealthy.to_string();
            lerror!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "gemini_load_assist_fail",
                "Gemini OAuth: loadCodeAssist call failed (with project_id)",
                user_id = user_id,
                error = %e
            );
        }
    }
}

/// 标记 `project_id` 待获取
fn mark_project_id_pending(user_id: i32, context: &mut PrepareGeminiContext) {
    linfo!(
        "system",
        LogStage::Authentication,
        LogComponent::OAuth,
        "gemini_auto_get_project_id_async",
        "Gemini OAuth: No project_id provided, will auto-get asynchronously (loadCodeAssist / onboardUser)",
        user_id = user_id,
    );
    context.health_status = ApiKeyHealthStatus::Unhealthy.to_string();
    context.needs_auto_get_project_id_async = true;
}

/// 启动 Gemini `project_id` 异步获取任务
pub fn spawn_gemini_project_task(
    needs_auto_get_project_id_async: bool,
    db: DatabaseConnection,
    user_id: i32,
    key_id: i32,
) {
    if !needs_auto_get_project_id_async {
        return;
    }

    let user_id_string = user_id.to_string();
    spawn(async move {
        linfo!(
            "system",
            LogStage::BackgroundTask,
            LogComponent::OAuth,
            "start_auto_get_project_id_task",
            "Starting async auto-get project_id task",
            user_id = user_id_string,
            key_id = %key_id,
        );

        if let Err(e) = execute_auto_get_project_id_async(&db, key_id, &user_id_string).await {
            lerror!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "auto_get_project_id_task_fail",
                "Async auto-get project_id task failed",
                user_id = user_id_string,
                key_id = %key_id,
                error = %e,
            );
        }
    });
}

/// 执行异步自动获取 `project_id`
async fn execute_auto_get_project_id_async(
    db: &DatabaseConnection,
    key_id: i32,
    user_id: &str,
) -> Result<()> {
    let gemini_client = GeminiCodeAssistClient::new();
    let access_token = super::oauth::get_access_token_for_key(db, key_id, user_id).await?;

    match gemini_client
        .auto_get_project_id_with_retry(&access_token)
        .await
    {
        Ok(Some(pid)) => {
            linfo!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "auto_get_project_id_success",
                "Async auto-get project_id success",
                user_id = user_id,
                key_id = %key_id,
                project_id = %pid,
            );

            if let Some(key_model) = UserProviderKey::find_by_id(key_id).one(db).await? {
                let mut active_key: user_provider_keys::ActiveModel = key_model.into();
                active_key.project_id = Set(Some(pid.clone()));
                active_key.health_status = Set(ApiKeyHealthStatus::Healthy.to_string());
                active_key.updated_at = Set(chrono::Utc::now().naive_utc());
                active_key.update(db).await?;
            }
            Ok(())
        }
        Ok(None) => {
            lwarn!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "auto_get_project_id_empty",
                "Async auto-get project_id returned empty",
                user_id = user_id,
                key_id = %key_id,
            );
            Ok(())
        }
        Err(err) => Err(err),
    }
}
