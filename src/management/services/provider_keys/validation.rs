//! # 提供商密钥验证
//!
//! 提供商密钥相关的数据验证逻辑。

use entity::{
    oauth_client_sessions, oauth_client_sessions::Entity as OAuthSession, user_provider_keys,
    user_provider_keys::Entity as UserProviderKey,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::{
    ProxyError,
    auth::types::AuthStatus,
    error::{Context, Result, auth::AuthError},
};

use super::models::{CreateProviderKeyRequest, UpdateProviderKeyRequest};

/// 验证创建请求的 payload
pub fn validate_create_payload(payload: &CreateProviderKeyRequest, auth_type: &str) -> Result<()> {
    if auth_type == "api_key" && payload.api_key.is_none() {
        return Err(ProxyError::Authentication(AuthError::Message(
            "API Key认证类型需要提供api_key字段 (field: api_key)".to_string(),
        )));
    }

    if auth_type == "oauth" && payload.api_key.is_none() {
        return Err(ProxyError::Authentication(AuthError::Message(
            "OAuth认证类型需要通过api_key字段提供session_id (field: api_key)".to_string(),
        )));
    }

    Ok(())
}

/// 验证更新请求的要求
pub fn validate_update_requirements(
    payload: &UpdateProviderKeyRequest,
    auth_type: &str,
) -> Result<()> {
    if auth_type == "api_key" && payload.api_key.is_none() {
        return Err(ProxyError::Authentication(AuthError::Message(
            "API Key认证类型需要提供api_key字段 (field: api_key)".to_string(),
        )));
    }

    if auth_type == "oauth" && payload.api_key.is_none() {
        return Err(ProxyError::Authentication(AuthError::Message(
            "OAuth认证类型需要通过api_key字段提供session_id (field: api_key)".to_string(),
        )));
    }

    Ok(())
}

/// 验证创建时的 OAuth 会话
pub async fn validate_oauth_session_for_creation(
    db: &DatabaseConnection,
    user_id: i32,
    payload: &CreateProviderKeyRequest,
    auth_type: &str,
) -> Result<()> {
    if auth_type != "oauth" {
        return Ok(());
    }

    let Some(session_id) = &payload.api_key else {
        return Ok(());
    };

    match OAuthSession::find()
        .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
        .filter(oauth_client_sessions::Column::UserId.eq(user_id))
        .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
        .one(db)
        .await
    {
        Ok(Some(_)) => {
            let existing_usage = UserProviderKey::find()
                .filter(user_provider_keys::Column::ApiKey.eq(session_id))
                .filter(user_provider_keys::Column::AuthType.eq("oauth"))
                .filter(user_provider_keys::Column::IsActive.eq(true))
                .one(db)
                .await;

            match existing_usage {
                Ok(Some(_)) => Err(ProxyError::Authentication(AuthError::Message(
                    "指定的OAuth会话已被其他provider key使用".to_string(),
                ))),
                Err(err) => {
                    use crate::{lerror, logging::LogComponent, logging::LogStage};

                    lerror!(
                        "system",
                        LogStage::Db,
                        LogComponent::OAuth,
                        "check_session_usage_fail",
                        &format!("Failed to check OAuth session usage: {err}")
                    );
                    Err(crate::error::database::DatabaseError::Connection(format!(
                        "Failed to check OAuth session usage: {err}"
                    ))
                    .into())
                }
                _ => Ok(()),
            }
        }
        Ok(None) => Err(ProxyError::Authentication(AuthError::Message(
            "指定的OAuth会话不存在或未完成授权 (field: api_key)".to_string(),
        ))),
        Err(err) => {
            use crate::{lerror, logging::LogComponent, logging::LogStage};

            lerror!(
                "system",
                LogStage::Db,
                LogComponent::OAuth,
                "validate_session_fail",
                &format!("Failed to validate OAuth session: {err}")
            );
            Err(crate::error::database::DatabaseError::Connection(format!(
                "Failed to validate OAuth session: {err}"
            ))
            .into())
        }
    }
}

/// 验证更新时的 OAuth 会话
pub async fn validate_oauth_session_for_update(
    db: &DatabaseConnection,
    user_id: i32,
    key_id: i32,
    payload: &UpdateProviderKeyRequest,
    auth_type: &str,
) -> Result<()> {
    if auth_type != "oauth" {
        return Ok(());
    }

    let Some(session_id) = &payload.api_key else {
        return Ok(());
    };

    let session_exists = OAuthSession::find()
        .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
        .filter(oauth_client_sessions::Column::UserId.eq(user_id))
        .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
        .one(db)
        .await
        .context("Failed to validate OAuth session")?
        .is_some();

    if !session_exists {
        return Err(ProxyError::Authentication(AuthError::Message(
            "指定的OAuth会话不存在或未完成授权 (field: api_key)".to_string(),
        )));
    }

    let existing_usage = UserProviderKey::find()
        .filter(user_provider_keys::Column::ApiKey.eq(session_id))
        .filter(user_provider_keys::Column::AuthType.eq("oauth"))
        .filter(user_provider_keys::Column::IsActive.eq(true))
        .filter(user_provider_keys::Column::Id.ne(key_id))
        .one(db)
        .await
        .context("Failed to check OAuth session usage")?;

    if existing_usage.is_some() {
        return Err(ProxyError::Authentication(AuthError::Message(
            "指定的OAuth会话已被其他provider key使用".to_string(),
        )));
    }

    Ok(())
}

/// 确保名称唯一（更新时）
pub async fn ensure_unique_name(
    db: &DatabaseConnection,
    user_id: i32,
    key_id: i32,
    existing_key: &user_provider_keys::Model,
    payload: &UpdateProviderKeyRequest,
) -> Result<()> {
    if existing_key.name == payload.name {
        return Ok(());
    }

    let duplicate = UserProviderKey::find()
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .filter(user_provider_keys::Column::Name.eq(&payload.name))
        .filter(user_provider_keys::Column::ProviderTypeId.eq(payload.provider_type_id))
        .filter(user_provider_keys::Column::Id.ne(key_id))
        .one(db)
        .await
        .context("Failed to check duplicate name")?;

    if duplicate.is_some() {
        return Err(ProxyError::Authentication(AuthError::Message(format!(
            "ProviderKey conflict: {}",
            payload.name
        ))));
    }

    Ok(())
}
