//! # 提供商密钥 CRUD 操作
//!
//! 提供商密钥的基本数据库操作。

use chrono::Utc;
use entity::{
    provider_types, provider_types::Entity as ProviderType, user_provider_keys,
    user_provider_keys::Entity as UserProviderKey,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::{
    ProxyError,
    auth::types::AuthStatus,
    error::{Context, Result, auth::AuthError},
};

use super::models::{CreateProviderKeyRequest, UpdateProviderKeyRequest};

/// 加载提供商类型或返回错误
pub async fn load_provider_type_or_error(
    db: &DatabaseConnection,
    provider_type_id: i32,
) -> Result<provider_types::Model> {
    ProviderType::find_by_id(provider_type_id)
        .one(db)
        .await
        .context("Failed to fetch provider type")?
        .ok_or_else(|| {
            ProxyError::Authentication(AuthError::Message("服务商类型不存在".to_string()))
        })
}

/// 确保提供商密钥名称唯一
pub async fn ensure_unique_provider_key(
    db: &DatabaseConnection,
    user_id: i32,
    payload: &CreateProviderKeyRequest,
) -> Result<()> {
    let existing = UserProviderKey::find()
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .filter(user_provider_keys::Column::Name.eq(&payload.name))
        .filter(user_provider_keys::Column::ProviderTypeId.eq(payload.provider_type_id))
        .one(db)
        .await;

    match existing {
        Ok(Some(_)) => Err(ProxyError::Authentication(AuthError::Message(format!(
            "ProviderKey conflict: {}",
            &payload.name
        )))),
        Err(err) => {
            use crate::{lerror, logging::LogComponent, logging::LogStage};

            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "check_exist_fail",
                &format!("Failed to check existing provider key: {err}")
            );
            Err(crate::error::database::DatabaseError::Connection(format!(
                "Failed to check existing provider key: {err}"
            ))
            .into())
        }
        _ => Ok(()),
    }
}

/// 插入提供商密钥记录
pub async fn insert_provider_key_record(
    db: &DatabaseConnection,
    user_id: i32,
    payload: &CreateProviderKeyRequest,
    final_project_id: Option<String>,
    health_status: String,
    auth_type: &str,
) -> Result<user_provider_keys::Model> {
    let new_provider_key = user_provider_keys::ActiveModel {
        user_id: Set(user_id),
        provider_type_id: Set(payload.provider_type_id),
        name: Set(payload.name.clone()),
        api_key: Set(payload.api_key.clone().unwrap_or_default()),
        auth_type: Set(auth_type.to_string()),
        auth_status: Set(Some(AuthStatus::Authorized.to_string())),
        weight: Set(payload.weight),
        max_requests_per_minute: Set(payload.max_requests_per_minute),
        max_tokens_prompt_per_minute: Set(payload.max_tokens_prompt_per_minute),
        max_requests_per_day: Set(payload.max_requests_per_day),
        is_active: Set(payload.is_active.unwrap_or(true)),
        project_id: Set(final_project_id),
        health_status: Set(health_status),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };

    new_provider_key
        .insert(db)
        .await
        .context("Failed to create provider key")
}

/// 加载现有密钥
pub async fn load_existing_key(
    db: &DatabaseConnection,
    key_id: i32,
    user_id: i32,
) -> Result<user_provider_keys::Model> {
    UserProviderKey::find()
        .filter(user_provider_keys::Column::Id.eq(key_id))
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .one(db)
        .await
        .context("Failed to find provider key")?
        .ok_or_else(|| {
            ProxyError::Authentication(AuthError::Message(format!(
                "ProviderKey not found: {key_id}"
            )))
        })
}

/// 加载密钥及其关联的提供商类型
pub async fn load_key_with_provider(
    db: &DatabaseConnection,
    key_id: i32,
    user_id: i32,
) -> Result<(user_provider_keys::Model, Option<provider_types::Model>)> {
    UserProviderKey::find()
        .filter(user_provider_keys::Column::Id.eq(key_id))
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .find_also_related(ProviderType)
        .one(db)
        .await
        .context("Failed to fetch provider key detail")?
        .ok_or_else(|| {
            ProxyError::Authentication(AuthError::Message(format!(
                "ProviderKey not found: {key_id}"
            )))
        })
}

/// 持久化更新的密钥
pub async fn persist_updated_key(
    db: &DatabaseConnection,
    existing_key: user_provider_keys::Model,
    payload: &UpdateProviderKeyRequest,
    auth_type: &str,
) -> Result<user_provider_keys::Model> {
    let mut active_model: user_provider_keys::ActiveModel = existing_key.into();
    active_model.provider_type_id = Set(payload.provider_type_id);
    active_model.name = Set(payload.name.clone());
    active_model.api_key = Set(payload.api_key.clone().unwrap_or_default());
    active_model.auth_type = Set(auth_type.to_string());
    active_model.weight = Set(payload.weight);
    active_model.max_requests_per_minute = Set(payload.max_requests_per_minute);
    active_model.max_tokens_prompt_per_minute = Set(payload.max_tokens_prompt_per_minute);
    active_model.max_requests_per_day = Set(payload.max_requests_per_day);
    active_model.is_active = Set(payload.is_active.unwrap_or(true));
    active_model.project_id = Set(payload.project_id.clone());
    active_model.updated_at = Set(Utc::now().naive_utc());

    active_model
        .update(db)
        .await
        .context("Failed to update provider key")
}

/// 删除密钥
pub async fn delete_key(db: &DatabaseConnection, key: user_provider_keys::Model) -> Result<()> {
    let active_model: user_provider_keys::ActiveModel = key.into();
    active_model
        .delete(db)
        .await
        .context("Failed to delete provider key")?;
    Ok(())
}
