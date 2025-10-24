//! # 提供商类型服务
//!
//! 提供对管理端提供商类型配置的查询。

use chrono_tz::Tz;
use serde::Serialize;

use crate::error::Result;
use crate::key_pool::types::SchedulingStrategy;
use crate::lerror;
use crate::logging::{LogComponent, LogStage};
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
    pub api_format: String,
    pub default_model: Option<String>,
    pub is_active: bool,
    pub supported_models: Vec<String>,
    pub supported_auth_types: Vec<String>,
    pub auth_configs: Option<serde_json::Value>,
    pub created_at: String,
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
    let rows = ProviderTypes::find()
        .filter(provider_types::Column::IsActive.eq(true))
        .order_by_asc(provider_types::Column::Id)
        .all(state.database.as_ref())
        .await
        .map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_provider_types_fail",
                &format!("Failed to fetch provider types: {err}")
            );
            crate::error!(Database, format!("Failed to fetch provider types: {err}"))
        })?;

    Ok(rows
        .into_iter()
        .map(|provider| {
            let supported_auth_types: Vec<String> =
                serde_json::from_str::<Vec<String>>(&provider.supported_auth_types)
                    .unwrap_or_else(|_| vec!["api_key".to_string()]);

            let auth_configs = provider
                .auth_configs_json
                .as_ref()
                .and_then(|config_json| serde_json::from_str(config_json).ok());

            ProviderTypeItem {
                id: provider.id,
                name: provider.name,
                display_name: provider.display_name,
                base_url: provider.base_url,
                api_format: provider.api_format,
                default_model: provider.default_model,
                is_active: provider.is_active,
                supported_models: Vec::new(),
                supported_auth_types,
                auth_configs,
                created_at: timezone_utils::format_naive_utc_for_response(
                    &provider.created_at,
                    timezone,
                ),
            }
        })
        .collect())
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
