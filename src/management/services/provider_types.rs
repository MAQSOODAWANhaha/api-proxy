//! # 提供商类型服务
//!
//! 提供对管理端提供商类型配置的查询。

use chrono_tz::Tz;
use serde::Serialize;
use std::sync::Arc;

use crate::error::{Context, Result};
use crate::key_pool::types::SchedulingStrategy;
use crate::management::middleware::AuthContext;
use crate::management::server::ManagementState;
use crate::types::timezone_utils;
use crate::{ensure, error};

use entity::{provider_types, provider_types::Entity as ProviderTypes};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::Deserialize;

#[derive(Debug, Serialize)]
pub struct ProviderTypeItem {
    pub id: i32,
    pub name: String,
    pub display_name: String,
    pub auth_type: String,
    pub base_url: String,
    pub is_active: bool,
    pub supported_models: Vec<String>,
    /// 原始 `auth_configs_json` 回显（包含敏感字段，管理端需完整回显）
    pub auth_configs_json: Option<serde_json::Value>,
    pub config_json: Option<serde_json::Value>,
    pub token_mappings_json: Option<serde_json::Value>,
    pub model_extraction_json: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct SchedulingStrategyItem {
    pub value: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub is_default: bool,
}

/// 列出全部激活的提供商类型。
pub async fn list_active_types(
    state: &ManagementState,
    timezone: Tz,
) -> Result<Vec<ProviderTypeItem>> {
    list_types(state, timezone, Some(true)).await
}

/// 列出提供商类型。
///
/// - `is_active` 为 Some(true/false) 时按状态过滤
/// - `is_active` 为 None 时返回全部
pub async fn list_types(
    state: &ManagementState,
    timezone: Tz,
    is_active: Option<bool>,
) -> Result<Vec<ProviderTypeItem>> {
    let mut query = ProviderTypes::find().order_by_asc(provider_types::Column::Id);
    if let Some(active) = is_active {
        query = query.filter(provider_types::Column::IsActive.eq(active));
    }

    let rows = query
        .all(state.database.as_ref())
        .await
        .context("Failed to fetch provider types")?;

    let mut items = Vec::with_capacity(rows.len());
    for provider in rows {
        items.push(to_item(&provider, timezone)?);
    }
    Ok(items)
}

fn parse_json_strict(raw: &str, field: &'static str) -> Result<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(raw).map_err(|e| {
        error::conversion::ConversionError::message(format!("{field} 不是合法 JSON: {e}")).into()
    })
}

pub fn to_item(provider: &provider_types::Model, timezone: Tz) -> Result<ProviderTypeItem> {
    let auth_configs_json = provider
        .auth_configs_json
        .as_deref()
        .map(|raw| parse_json_strict(raw, "auth_configs_json"))
        .transpose()?;

    let config_json = provider
        .config_json
        .as_deref()
        .map(|raw| parse_json_strict(raw, "config_json"))
        .transpose()?;
    let token_mappings_json = provider
        .token_mappings_json
        .as_deref()
        .map(|raw| parse_json_strict(raw, "token_mappings_json"))
        .transpose()?;
    let model_extraction_json = provider
        .model_extraction_json
        .as_deref()
        .map(|raw| parse_json_strict(raw, "model_extraction_json"))
        .transpose()?;

    Ok(ProviderTypeItem {
        id: provider.id,
        name: provider.name.clone(),
        display_name: provider.display_name.clone(),
        auth_type: provider.auth_type.clone(),
        base_url: provider.base_url.clone(),
        is_active: provider.is_active,
        supported_models: Vec::new(),
        auth_configs_json,
        config_json,
        token_mappings_json,
        model_extraction_json,
        created_at: timezone_utils::format_naive_utc_for_response(&provider.created_at, &timezone),
        updated_at: timezone_utils::format_naive_utc_for_response(&provider.updated_at, &timezone),
    })
}

/// 兼容旧调用方：管理端回显完整的 `auth_configs_json`（包含敏感字段）
pub fn to_item_admin(provider: &provider_types::Model, timezone: Tz) -> Result<ProviderTypeItem> {
    to_item(provider, timezone)
}

/// 获取调度策略枚举。
#[must_use]
pub fn list_scheduling_strategies() -> Vec<SchedulingStrategyItem> {
    [
        (
            SchedulingStrategy::RoundRobin,
            "轮询调度",
            "按顺序轮流分配请求到各个上游服务器",
            true,
        ),
        (
            SchedulingStrategy::Weighted,
            "权重调度",
            "根据权重比例分配请求到上游服务器",
            false,
        ),
    ]
    .into_iter()
    .map(
        |(strategy, label, description, is_default)| SchedulingStrategyItem {
            value: strategy.as_str(),
            label,
            description,
            is_default,
        },
    )
    .collect()
}

// =========================
// CRUD（管理端写接口）
// =========================

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProviderTypeRequest {
    pub name: String,
    pub display_name: String,
    /// 仅允许 `api_key` / `oauth`
    pub auth_type: String,
    pub base_url: String,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub config_json: Option<serde_json::Value>,
    #[serde(default)]
    pub token_mappings_json: Option<serde_json::Value>,
    #[serde(default)]
    pub model_extraction_json: Option<serde_json::Value>,
    #[serde(default)]
    pub auth_configs_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UpdateProviderTypeRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub auth_type: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub config_json: Option<serde_json::Value>,
    #[serde(default)]
    pub token_mappings_json: Option<serde_json::Value>,
    #[serde(default)]
    pub model_extraction_json: Option<serde_json::Value>,
    #[serde(default)]
    pub auth_configs_json: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct ProviderTypesCrudService {
    db: Arc<DatabaseConnection>,
}

impl ProviderTypesCrudService {
    #[must_use]
    pub const fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn get(&self, auth: &AuthContext, id: i32) -> Result<provider_types::Model> {
        ensure_admin(auth)?;
        let model = provider_types::Entity::find_by_id(id)
            .one(self.db.as_ref())
            .await
            .context("获取服务商类型失败")?
            .ok_or_else(|| {
                crate::error::auth::AuthError::Message("服务商类型不存在".to_string())
            })?;
        Ok(model)
    }

    pub async fn create(
        &self,
        auth: &AuthContext,
        request: &CreateProviderTypeRequest,
    ) -> Result<provider_types::Model> {
        ensure_admin(auth)?;
        validate_name(&request.name)?;
        validate_display_name(&request.display_name)?;
        validate_auth_type(&request.auth_type)?;
        validate_base_url(&request.base_url)?;

        let now = chrono::Utc::now().naive_utc();

        let active = provider_types::ActiveModel {
            name: Set(request.name.trim().to_string()),
            display_name: Set(request.display_name.trim().to_string()),
            auth_type: Set(request.auth_type.trim().to_string()),
            base_url: Set(request.base_url.trim().to_string()),
            is_active: Set(request.is_active.unwrap_or(true)),
            config_json: Set(json_to_string(request.config_json.as_ref())?),
            token_mappings_json: Set(json_to_string(request.token_mappings_json.as_ref())?),
            model_extraction_json: Set(json_to_string(request.model_extraction_json.as_ref())?),
            auth_configs_json: Set(json_to_string(request.auth_configs_json.as_ref())?),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let inserted = active
            .insert(self.db.as_ref())
            .await
            .context("创建服务商类型失败")?;
        Ok(inserted)
    }

    pub async fn update(
        &self,
        auth: &AuthContext,
        id: i32,
        request: &UpdateProviderTypeRequest,
    ) -> Result<provider_types::Model> {
        ensure_admin(auth)?;
        let existing = self.get(auth, id).await?;
        let existing_auth_type = existing.auth_type.clone();
        let mut active: provider_types::ActiveModel = existing.into();

        if let Some(name) = &request.name {
            validate_name(name)?;
            active.name = Set(name.trim().to_string());
        }
        if let Some(display_name) = &request.display_name {
            validate_display_name(display_name)?;
            active.display_name = Set(display_name.trim().to_string());
        }
        if let Some(auth_type) = &request.auth_type {
            validate_auth_type(auth_type)?;
            // auth_type 是分行粒度的“身份字段”，修改会引入歧义与唯一约束冲突
            ensure!(
                auth_type.trim() == existing_auth_type,
                error::conversion::ConversionError::message(
                    "不允许修改 auth_type；如需切换认证类型，请新建一条 provider_types 记录",
                )
            );
        }
        if let Some(base_url) = &request.base_url {
            validate_base_url(base_url)?;
            active.base_url = Set(base_url.trim().to_string());
        }
        if let Some(is_active) = request.is_active {
            active.is_active = Set(is_active);
        }

        if request.config_json.is_some() {
            active.config_json = Set(json_to_string(request.config_json.as_ref())?);
        }
        if request.token_mappings_json.is_some() {
            active.token_mappings_json = Set(json_to_string(request.token_mappings_json.as_ref())?);
        }
        if request.model_extraction_json.is_some() {
            active.model_extraction_json =
                Set(json_to_string(request.model_extraction_json.as_ref())?);
        }
        if request.auth_configs_json.is_some() {
            active.auth_configs_json = Set(json_to_string(request.auth_configs_json.as_ref())?);
        }

        active.updated_at = Set(chrono::Utc::now().naive_utc());

        let updated = active
            .update(self.db.as_ref())
            .await
            .context("更新服务商类型失败")?;
        Ok(updated)
    }

    pub async fn delete(&self, auth: &AuthContext, id: i32) -> Result<()> {
        ensure_admin(auth)?;
        let result = provider_types::Entity::delete_by_id(id)
            .exec(self.db.as_ref())
            .await
            .context("删除服务商类型失败")?;

        ensure!(
            result.rows_affected > 0,
            crate::error::auth::AuthError::Message("服务商类型不存在".to_string())
        );
        Ok(())
    }
}

fn ensure_admin(auth: &AuthContext) -> Result<()> {
    ensure!(
        auth.is_admin,
        crate::error::auth::AuthError::PermissionDenied {
            required: "admin".to_string(),
            actual: "user".to_string(),
        }
    );
    Ok(())
}

fn validate_name(name: &str) -> Result<()> {
    let trimmed = name.trim();
    ensure!(
        !trimmed.is_empty() && trimmed.len() <= 50,
        crate::error::auth::AuthError::Message("name 不能为空且长度不超过50".to_string())
    );
    Ok(())
}

fn validate_display_name(display_name: &str) -> Result<()> {
    let trimmed = display_name.trim();
    ensure!(
        !trimmed.is_empty() && trimmed.len() <= 100,
        crate::error::auth::AuthError::Message("display_name 不能为空且长度不超过100".to_string())
    );
    Ok(())
}

fn validate_auth_type(auth_type: &str) -> Result<()> {
    let v = auth_type.trim();
    ensure!(
        v == "api_key" || v == "oauth",
        crate::error::auth::AuthError::Message("auth_type 仅支持 api_key / oauth".to_string())
    );
    Ok(())
}

fn validate_base_url(base_url: &str) -> Result<()> {
    ensure!(
        !base_url.trim().is_empty(),
        crate::error::auth::AuthError::Message("base_url 不能为空".to_string())
    );
    Ok(())
}

fn json_to_string(value: Option<&serde_json::Value>) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(Some(
        serde_json::to_string(value).context("序列化 JSON 失败")?,
    ))
}
