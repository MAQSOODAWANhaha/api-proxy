//! # 提供商类型服务
//!
//! 提供对管理端提供商类型配置的查询。

use chrono_tz::Tz;
use serde::Serialize;

use crate::error::{Context, Result};
use crate::key_pool::types::SchedulingStrategy;
use crate::management::server::ManagementState;
use crate::types::timezone_utils;

use entity::{provider_types, provider_types::Entity as ProviderTypes};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

#[derive(Debug, Serialize)]
pub struct ProviderTypeItem {
    pub id: i32,
    pub name: String,
    pub display_name: String,
    pub base_url: String,
    pub timeout_seconds: Option<i32>,
    pub is_active: bool,
    pub supported_models: Vec<String>,
    pub supported_auth_types: Vec<String>,
    pub auth_configs: Option<serde_json::Value>,
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
        .map(|provider| {
            let supported_auth_types: Vec<String> =
                serde_json::from_str::<Vec<String>>(&provider.supported_auth_types)
                    .unwrap_or_else(|_| vec!["api_key".to_string()]);

            let auth_configs = provider
                .auth_configs_json
                .as_ref()
                .and_then(|config_json| serde_json::from_str::<serde_json::Value>(config_json).ok())
                .map(|value| sanitize_auth_configs(&value));

            ProviderTypeItem {
                id: provider.id,
                name: provider.name,
                display_name: provider.display_name,
                base_url: provider.base_url,
                timeout_seconds: provider.timeout_seconds,
                is_active: provider.is_active,
                supported_models: Vec::new(),
                supported_auth_types,
                auth_configs,
                created_at: timezone_utils::format_naive_utc_for_response(
                    &provider.created_at,
                    timezone,
                ),
                updated_at: timezone_utils::format_naive_utc_for_response(
                    &provider.updated_at,
                    timezone,
                ),
            }
        })
        .collect())
}

/// 脱敏认证配置中的敏感字段（如 `client_secret`）
fn sanitize_auth_configs(value: &serde_json::Value) -> serde_json::Value {
    let mut sanitized = value.clone();
    if let serde_json::Value::Object(ref mut map) = sanitized {
        for (_auth_type, cfg) in map.iter_mut() {
            if let serde_json::Value::Object(cfg_map) = cfg {
                cfg_map.remove("client_secret");
                cfg_map.remove("secret");
            }
        }
    }
    sanitized
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
