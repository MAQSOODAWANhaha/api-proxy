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
use crate::{bail, ensure, error};

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
    pub auth_configs: Option<serde_json::Value>,
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
    timezone: &Tz,
) -> Result<Vec<ProviderTypeItem>> {
    list_types(state, timezone, Some(true)).await
}

/// 列出提供商类型。
///
/// - `is_active` 为 Some(true/false) 时按状态过滤
/// - `is_active` 为 None 时返回全部
pub async fn list_types(
    state: &ManagementState,
    timezone: &Tz,
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

    Ok(rows
        .into_iter()
        .map(|provider| to_item(&provider, timezone))
        .collect())
}

/// 脱敏认证配置中的敏感字段（如 `client_secret`）
fn sanitize_auth_configs(value: &serde_json::Value) -> serde_json::Value {
    let mut sanitized = value.clone();
    let serde_json::Value::Object(ref mut map) = sanitized else {
        return sanitized;
    };

    map.remove("client_secret");
    map.remove("secret");
    sanitized
}

#[must_use]
pub fn to_item(provider: &provider_types::Model, timezone: &Tz) -> ProviderTypeItem {
    let auth_configs = provider
        .auth_configs_json
        .as_ref()
        .and_then(|config_json| serde_json::from_str::<serde_json::Value>(config_json).ok())
        .map(|value| sanitize_auth_configs(&value));

    let config_json = provider
        .config_json
        .as_ref()
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok());
    let token_mappings_json = provider
        .token_mappings_json
        .as_ref()
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok());
    let model_extraction_json = provider
        .model_extraction_json
        .as_ref()
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok());

    ProviderTypeItem {
        id: provider.id,
        name: provider.name.clone(),
        display_name: provider.display_name.clone(),
        auth_type: provider.auth_type.clone(),
        base_url: provider.base_url.clone(),
        is_active: provider.is_active,
        supported_models: Vec::new(),
        auth_configs,
        config_json,
        token_mappings_json,
        model_extraction_json,
        created_at: timezone_utils::format_naive_utc_for_response(&provider.created_at, timezone),
        updated_at: timezone_utils::format_naive_utc_for_response(&provider.updated_at, timezone),
    }
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
        validate_json_payload_for_create(request)?;

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

        validate_json_payload_for_update(&existing_auth_type, request)?;

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
    if value.is_null() {
        return Ok(None);
    }
    Ok(Some(
        serde_json::to_string(value).context("序列化 JSON 失败")?,
    ))
}

fn validate_json_payload_for_create(request: &CreateProviderTypeRequest) -> Result<()> {
    validate_common_json_fields(
        request.config_json.as_ref(),
        request.token_mappings_json.as_ref(),
        request.model_extraction_json.as_ref(),
    )?;
    validate_auth_configs_json(&request.auth_type, request.auth_configs_json.as_ref())
}

fn validate_json_payload_for_update(
    existing_auth_type: &str,
    request: &UpdateProviderTypeRequest,
) -> Result<()> {
    // 仅当字段被提交时做校验（避免对历史脏数据造成“无法编辑其它字段”的阻塞）
    if request.config_json.is_some() {
        validate_config_json(request.config_json.as_ref())?;
    }
    if request.token_mappings_json.is_some() {
        validate_token_mappings_json(request.token_mappings_json.as_ref())?;
    }
    if request.model_extraction_json.is_some() {
        validate_model_extraction_json(request.model_extraction_json.as_ref())?;
    }
    if request.auth_configs_json.is_some() {
        validate_auth_configs_json(existing_auth_type, request.auth_configs_json.as_ref())?;
    }
    Ok(())
}

fn validate_common_json_fields(
    config_json: Option<&serde_json::Value>,
    token_mappings_json: Option<&serde_json::Value>,
    model_extraction_json: Option<&serde_json::Value>,
) -> Result<()> {
    // 新建时：如果提供了字段就严格校验
    validate_config_json(config_json)?;
    validate_token_mappings_json(token_mappings_json)?;
    validate_model_extraction_json(model_extraction_json)?;
    Ok(())
}

fn validate_config_json(value: Option<&serde_json::Value>) -> Result<()> {
    let Some(v) = value else {
        return Ok(());
    };
    if v.is_null() {
        return Ok(());
    }
    ensure!(
        v.is_object(),
        error::conversion::ConversionError::message("config_json 必须是对象或 null")
    );
    Ok(())
}

fn validate_token_mappings_json(value: Option<&serde_json::Value>) -> Result<()> {
    let Some(v) = value else {
        return Ok(());
    };
    if v.is_null() {
        return Ok(());
    }
    crate::collect::field_extractor::validate_token_mappings_value(v)
}

fn validate_model_extraction_json(value: Option<&serde_json::Value>) -> Result<()> {
    let Some(v) = value else {
        return Ok(());
    };
    if v.is_null() {
        return Ok(());
    }
    crate::collect::field_extractor::validate_model_extraction_value(v)
}

fn validate_auth_configs_json(auth_type: &str, value: Option<&serde_json::Value>) -> Result<()> {
    match auth_type {
        "api_key" => validate_auth_configs_api_key(value),
        "oauth" => validate_auth_configs_oauth(value),
        other => bail!(error::conversion::ConversionError::message(format!(
            "未知的 auth_type: {other}"
        ))),
    }
}

fn validate_auth_configs_api_key(value: Option<&serde_json::Value>) -> Result<()> {
    if let Some(v) = value
        && !v.is_null()
    {
        ensure!(
            v.is_object(),
            error::conversion::ConversionError::message(
                "auth_configs_json（api_key）必须是对象或 null"
            )
        );
    }
    Ok(())
}

fn validate_auth_configs_oauth(value: Option<&serde_json::Value>) -> Result<()> {
    let Some(v) = value else {
        bail!(error::conversion::ConversionError::message(
            "auth_configs_json（oauth）不能为空，请填写 OAuth 配置对象"
        ));
    };
    if v.is_null() {
        bail!(error::conversion::ConversionError::message(
            "auth_configs_json（oauth）不能为空，请填写 OAuth 配置对象"
        ));
    }

    let obj = v.as_object().ok_or_else(|| {
        error::conversion::ConversionError::message(
            "auth_configs_json（oauth）必须是对象（包含 client_id/authorize_url/token_url/scopes/pkce_required）",
        )
    })?;

    ensure_required_nonempty_string(obj, "client_id", "auth_configs_json（oauth）缺少 client_id")?;
    ensure_required_nonempty_string(
        obj,
        "authorize_url",
        "auth_configs_json（oauth）缺少 authorize_url",
    )?;
    ensure_required_nonempty_string(obj, "token_url", "auth_configs_json（oauth）缺少 token_url")?;
    ensure_required_nonempty_string(obj, "scopes", "auth_configs_json（oauth）缺少 scopes")?;

    ensure!(
        obj.get("pkce_required")
            .and_then(sea_orm::JsonValue::as_bool)
            .is_some(),
        error::conversion::ConversionError::message(
            "auth_configs_json（oauth）缺少 pkce_required（true/false）"
        )
    );

    ensure_optional_string_or_null(
        obj,
        "client_secret",
        "auth_configs_json（oauth）client_secret",
    )?;
    ensure_optional_string_or_null(
        obj,
        "redirect_uri",
        "auth_configs_json（oauth）redirect_uri",
    )?;
    ensure_optional_object_or_null(
        obj,
        "extra_params",
        "auth_configs_json（oauth）extra_params",
    )?;

    Ok(())
}

fn ensure_required_nonempty_string(
    obj: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    message: &str,
) -> Result<()> {
    let value = obj
        .get(key)
        .and_then(sea_orm::JsonValue::as_str)
        .unwrap_or("");
    ensure!(
        !value.trim().is_empty(),
        error::conversion::ConversionError::message(message)
    );
    Ok(())
}

fn ensure_optional_string_or_null(
    obj: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    label: &str,
) -> Result<()> {
    let Some(v) = obj.get(key) else {
        return Ok(());
    };
    if v.is_null() || v.is_string() {
        return Ok(());
    }
    bail!(error::conversion::ConversionError::message(format!(
        "{label} 必须是字符串或 null"
    )));
}

fn ensure_optional_object_or_null(
    obj: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    label: &str,
) -> Result<()> {
    let Some(v) = obj.get(key) else {
        return Ok(());
    };
    if v.is_null() || v.is_object() {
        return Ok(());
    }
    bail!(error::conversion::ConversionError::message(format!(
        "{label} 必须是对象或 null"
    )));
}
