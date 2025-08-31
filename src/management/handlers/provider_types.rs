use crate::auth::extract_user_id_from_headers;
use crate::management::{response, server::AppState};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use entity::{provider_types, provider_types::Entity as ProviderTypes};
use sea_orm::{entity::*, query::*};
use serde_json::json;

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
            tracing::error!("Failed to fetch provider types: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch provider types",
            );
        }
    };

    // 转换为响应格式
    let provider_types: Vec<_> = provider_types_data
        .into_iter()
        .map(|provider| {
            // 解析支持的认证类型
            let supported_auth_types: Vec<String> = serde_json::from_str::<Vec<String>>(&provider.supported_auth_types).unwrap_or_else(|_| vec!["api_key".to_string()]);

            // 解析认证配置
            let auth_configs: Option<serde_json::Value> = provider.auth_configs_json
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

pub async fn get_scheduling_strategies(headers: HeaderMap) -> axum::response::Response {
    // 从JWT token中提取用户ID进行身份验证
    let _user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // 返回系统支持的调度策略列表（静态数据）
    let scheduling_strategies = vec![
        json!({
            "value": "round_robin",
            "label": "轮询调度",
            "description": "按顺序轮流分配请求到各个上游服务器",
            "is_default": true
        }),
        json!({
            "value": "weighted",
            "label": "权重调度",
            "description": "根据权重比例分配请求到上游服务器",
            "is_default": false
        }),
        json!({
            "value": "priority",
            "label": "优先级调度",
            "description": "优先使用高优先级的上游服务器",
            "is_default": false
        }),
        json!({
            "value": "health_best",
            "label": "健康优选",
            "description": "优先选择健康状态最佳的上游服务器",
            "is_default": false
        }),
    ];

    let data = json!({
        "scheduling_strategies": scheduling_strategies
    });

    response::success(data)
}
