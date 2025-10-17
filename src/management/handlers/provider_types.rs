use crate::key_pool::types::SchedulingStrategy;
use crate::lerror;
use crate::logging::{LogComponent, LogStage};
use crate::management::middleware::auth::AuthContext;
use crate::management::{response, server::AppState};
use axum::extract::{Extension, State};
use entity::{provider_types, provider_types::Entity as ProviderTypes};
use sea_orm::{
    entity::{ColumnTrait, EntityTrait},
    query::{QueryFilter, QueryOrder},
};
use serde_json::json;
use std::sync::Arc;

/// 获取服务提供商类型列表
pub async fn list_provider_types(State(state): State<AppState>) -> axum::response::Response {
    // changed
    // 获取所有活跃的服务提供商类型
    let provider_types_result = ProviderTypes::find()
        .filter(provider_types::Column::IsActive.eq(true))
        .order_by_asc(provider_types::Column::Id)
        .all(state.database.as_ref())
        .await;

    let provider_types_data = match provider_types_result {
        Ok(data) => data,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_provider_types_fail",
                &format!("Failed to fetch provider types: {err}")
            );
            return crate::management::response::app_error(crate::error!(
                Database,
                format!("Failed to fetch provider types: {}", err)
            ));
        }
    };

    // 转换为响应格式
    let provider_types: Vec<_> = provider_types_data
        .into_iter()
        .map(|provider| {
            // 解析支持的认证类型
            let supported_auth_types: Vec<String> =
                serde_json::from_str::<Vec<String>>(&provider.supported_auth_types)
                    .unwrap_or_else(|_| vec!["api_key".to_string()]);

            // 解析认证配置
            let auth_configs: Option<serde_json::Value> = provider
                .auth_configs_json
                .as_ref()
                .and_then(|config_json| serde_json::from_str(config_json).ok());

            json!({
                "id": provider.id,
                "name": provider.name,
                "display_name": provider.display_name,
                "base_url": provider.base_url,
                "api_format": provider.api_format,
                "default_model": provider.default_model,
                "is_active": provider.is_active,
                "supported_models": [], // 暂时为空数组，可以根据需要添加
                "supported_auth_types": supported_auth_types,
                "auth_configs": auth_configs,
                "created_at": provider.created_at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
            })
        })
        .collect();

    // 按照API文档格式包装数据
    let data = json!({
        "provider_types": provider_types
    });

    response::success(data)
}

pub async fn get_scheduling_strategies(
    Extension(_auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    // 使用枚举动态生成调度策略列表
    let scheduling_strategies: Vec<serde_json::Value> = [
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
        (
            SchedulingStrategy::HealthBest,
            "健康优选",
            "优先选择健康状态最佳的上游服务器",
            false,
        ),
    ]
    .iter()
    .map(|(strategy, label, description, is_default)| {
        json!({
            "value": strategy.as_str(),
            "label": label,
            "description": description,
            "is_default": is_default
        })
    })
    .collect();

    let data = json!({
        "scheduling_strategies": scheduling_strategies
    });

    response::success(data)
}
