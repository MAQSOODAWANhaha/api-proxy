//! # 提供商类型服务
//!
//! 提供对管理端提供商类型配置的查询与管理。

use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::collect::usage_model;
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

// =========================
// 数据结构定义 (DTOs)
// =========================

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

// =========================
// 核心逻辑实现
// =========================

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
        items.push(convert_model_to_dto(&provider, timezone)?);
    }
    Ok(items)
}

/// 将数据库模型转换为 API 响应对象
///
/// 管理端完整回显所有 JSON 字段（包括可能含敏感信息的 `auth_configs_json`）
pub fn convert_model_to_dto(
    provider: &provider_types::Model,
    timezone: Tz,
) -> Result<ProviderTypeItem> {
    Ok(ProviderTypeItem {
        id: provider.id,
        name: provider.name.clone(),
        display_name: provider.display_name.clone(),
        auth_type: provider.auth_type.clone(),
        base_url: provider.base_url.clone(),
        is_active: provider.is_active,
        // TODO: 从配置中提取支持的模型列表，目前留空
        supported_models: Vec::new(),
        auth_configs_json: deserialize_option_json(
            provider.auth_configs_json.as_deref(),
            "auth_configs_json",
        )?,
        config_json: deserialize_option_json(provider.config_json.as_deref(), "config_json")?,
        token_mappings_json: deserialize_option_json(
            provider.token_mappings_json.as_deref(),
            "token_mappings_json",
        )?,
        model_extraction_json: deserialize_option_json(
            provider.model_extraction_json.as_deref(),
            "model_extraction_json",
        )?,
        created_at: timezone_utils::format_naive_utc_for_response(&provider.created_at, &timezone),
        updated_at: timezone_utils::format_naive_utc_for_response(&provider.updated_at, &timezone),
    })
}

const STRATEGIES: &[(SchedulingStrategy, &str, &str, bool)] = &[
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
];

/// 获取调度策略枚举。
#[must_use]
pub fn list_scheduling_strategies() -> Vec<SchedulingStrategyItem> {
    STRATEGIES
        .iter()
        .map(
            |&(strategy, label, description, is_default)| SchedulingStrategyItem {
                value: strategy.as_str(),
                label,
                description,
                is_default,
            },
        )
        .collect()
}

// =========================
// CRUD 服务
// =========================

#[derive(Clone)]
pub struct ProviderTypeService {
    db: Arc<DatabaseConnection>,
}

impl ProviderTypeService {
    #[must_use]
    pub const fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn get(&self, auth: &AuthContext, id: i32) -> Result<provider_types::Model> {
        Self::ensure_admin(auth)?;
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
        Self::ensure_admin(auth)?;

        // 验证输入
        let name = request.name.trim();
        ensure!(
            !name.is_empty() && name.len() <= 50,
            crate::error::auth::AuthError::Message("name 不能为空且长度不超过50".to_string())
        );

        let display_name = request.display_name.trim();
        ensure!(
            !display_name.is_empty() && display_name.len() <= 100,
            crate::error::auth::AuthError::Message(
                "display_name 不能为空且长度不超过100".to_string()
            )
        );

        let auth_type = request.auth_type.trim();
        ensure!(
            auth_type == "api_key" || auth_type == "oauth",
            crate::error::auth::AuthError::Message("auth_type 仅支持 api_key / oauth".to_string())
        );

        ensure!(
            !request.base_url.trim().is_empty(),
            crate::error::auth::AuthError::Message("base_url 不能为空".to_string())
        );

        let now = chrono::Utc::now().naive_utc();
        let active = provider_types::ActiveModel {
            name: Set(name.to_string()),
            display_name: Set(display_name.to_string()),
            auth_type: Set(auth_type.to_string()),
            base_url: Set(request.base_url.trim().to_string()),
            is_active: Set(request.is_active.unwrap_or(true)),
            config_json: Set(serialize_option_json(request.config_json.as_ref())?),
            token_mappings_json: Set(serialize_option_json(request.token_mappings_json.as_ref())?),
            model_extraction_json: Set(serialize_option_json(
                request.model_extraction_json.as_ref(),
            )?),
            auth_configs_json: Set(serialize_option_json(request.auth_configs_json.as_ref())?),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let inserted = active
            .insert(self.db.as_ref())
            .await
            .context("创建服务商类型失败")?;
        usage_model::invalidate_token_extractor_cache(inserted.id);
        Ok(inserted)
    }

    pub async fn update(
        &self,
        auth: &AuthContext,
        id: i32,
        request: &UpdateProviderTypeRequest,
    ) -> Result<provider_types::Model> {
        Self::ensure_admin(auth)?;
        let existing = self.get(auth, id).await?;
        let existing_auth_type = existing.auth_type.clone();
        let mut active: provider_types::ActiveModel = existing.into();

        if let Some(name) = &request.name {
            let name = name.trim();
            ensure!(
                !name.is_empty() && name.len() <= 50,
                crate::error::auth::AuthError::Message("name 不能为空且长度不超过50".to_string())
            );
            active.name = Set(name.to_string());
        }

        if let Some(display_name) = &request.display_name {
            let display_name = display_name.trim();
            ensure!(
                !display_name.is_empty() && display_name.len() <= 100,
                crate::error::auth::AuthError::Message(
                    "display_name 不能为空且长度不超过100".to_string()
                )
            );
            active.display_name = Set(display_name.to_string());
        }

        if let Some(auth_type) = &request.auth_type {
            let auth_type = auth_type.trim();
            ensure!(
                auth_type == "api_key" || auth_type == "oauth",
                crate::error::auth::AuthError::Message(
                    "auth_type 仅支持 api_key / oauth".to_string()
                )
            );
            // auth_type 是分行粒度的“身份字段”，修改会引入歧义与唯一约束冲突
            ensure!(
                auth_type == existing_auth_type,
                error::conversion::ConversionError::message(
                    "不允许修改 auth_type；如需切换认证类型，请新建一条 provider_types 记录",
                )
            );
        }

        if let Some(base_url) = &request.base_url {
            ensure!(
                !base_url.trim().is_empty(),
                crate::error::auth::AuthError::Message("base_url 不能为空".to_string())
            );
            active.base_url = Set(base_url.trim().to_string());
        }

        if let Some(is_active) = request.is_active {
            active.is_active = Set(is_active);
        }

        if request.config_json.is_some() {
            active.config_json = Set(serialize_option_json(request.config_json.as_ref())?);
        }
        if request.token_mappings_json.is_some() {
            active.token_mappings_json =
                Set(serialize_option_json(request.token_mappings_json.as_ref())?);
        }
        if request.model_extraction_json.is_some() {
            active.model_extraction_json = Set(serialize_option_json(
                request.model_extraction_json.as_ref(),
            )?);
        }
        if request.auth_configs_json.is_some() {
            active.auth_configs_json =
                Set(serialize_option_json(request.auth_configs_json.as_ref())?);
        }

        active.updated_at = Set(chrono::Utc::now().naive_utc());

        let updated = active
            .update(self.db.as_ref())
            .await
            .context("更新服务商类型失败")?;
        if request.token_mappings_json.is_some() {
            usage_model::invalidate_token_extractor_cache(updated.id);
        }
        Ok(updated)
    }

    pub async fn delete(&self, auth: &AuthContext, id: i32) -> Result<()> {
        Self::ensure_admin(auth)?;
        let result = provider_types::Entity::delete_by_id(id)
            .exec(self.db.as_ref())
            .await
            .context("删除服务商类型失败")?;

        ensure!(
            result.rows_affected > 0,
            crate::error::auth::AuthError::Message("服务商类型不存在".to_string())
        );
        usage_model::invalidate_token_extractor_cache(id);
        Ok(())
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
}

// 兼容别名
pub type ProviderTypesCrudService = ProviderTypeService;

// =========================
// 私有辅助函数
// =========================

fn deserialize_option_json(
    raw: Option<&str>,
    field: &'static str,
) -> Result<Option<serde_json::Value>> {
    raw.map(|s| {
        serde_json::from_str::<serde_json::Value>(s).map_err(|e| {
            error::conversion::ConversionError::message(format!("{field} 不是合法 JSON: {e}"))
                .into()
        })
    })
    .transpose()
}

fn serialize_option_json(value: Option<&serde_json::Value>) -> Result<Option<String>> {
    value
        .map(|v| serde_json::to_string(v).context("序列化 JSON 失败"))
        .transpose()
}
