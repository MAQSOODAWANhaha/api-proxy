//! # 适配器管理处理器

use crate::management::server::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use serde_json::{json, Value};
use sea_orm::{entity::*, query::*};
use entity::{
    provider_types,
    provider_types::Entity as ProviderTypes,
    user_service_apis,
    user_service_apis::Entity as UserServiceApis,
};

/// 列出所有适配器
pub async fn list_adapters(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // 从数据库获取提供商类型信息
    let provider_types = match ProviderTypes::find()
        .filter(provider_types::Column::IsActive.eq(true))
        .all(state.database.as_ref())
        .await
    {
        Ok(types) => types,
        Err(err) => {
            tracing::error!("Failed to fetch provider types: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // 获取运行时适配器统计信息
    let adapter_stats = state.adapter_manager.get_adapter_stats();
    
    let mut adapters = Vec::new();
    
    // 合并数据库配置和运行时统计
    for provider in provider_types {
        let runtime_stats = adapter_stats.get(&provider.name);
        
        let adapter_info = json!({
            "id": provider.id,
            "name": provider.name,
            "display_name": provider.display_name,
            "upstream_type": provider.api_format,
            "base_url": provider.base_url,
            "default_model": provider.default_model,
            "max_tokens": provider.max_tokens,
            "rate_limit": provider.rate_limit,
            "timeout_seconds": provider.timeout_seconds,
            "health_check_path": provider.health_check_path,
            "auth_header_format": provider.auth_header_format,
            "status": if provider.is_active { "active" } else { "inactive" },
            "supported_endpoints": runtime_stats.map(|s| s.supported_endpoints).unwrap_or(0),
            "endpoints": runtime_stats.map(|s| s.endpoints.clone()).unwrap_or_default(),
            "version": "1.0.0",
            "created_at": provider.created_at,
            "updated_at": provider.updated_at
        });
        
        adapters.push(adapter_info);
    }

    let response = json!({
        "adapters": adapters,
        "total": adapters.len(),
        "timestamp": chrono::Utc::now()
    });

    Ok(Json(response))
}

/// 获取适配器统计信息
pub async fn get_adapter_stats(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // 从数据库获取提供商类型信息
    let provider_types = match ProviderTypes::find()
        .filter(provider_types::Column::IsActive.eq(true))
        .all(state.database.as_ref())
        .await
    {
        Ok(types) => types,
        Err(err) => {
            tracing::error!("Failed to fetch provider types: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // 获取每个提供商的使用统计
    let mut provider_usage_stats = std::collections::HashMap::new();
    for provider in &provider_types {
        let usage_count = match UserServiceApis::find()
            .filter(user_service_apis::Column::ProviderTypeId.eq(provider.id))
            .filter(user_service_apis::Column::IsActive.eq(true))
            .count(state.database.as_ref())
            .await
        {
            Ok(count) => count,
            Err(err) => {
                tracing::warn!("Failed to get usage count for provider {}: {}", provider.name, err);
                0
            }
        };
        provider_usage_stats.insert(provider.id, usage_count);
    }
    
    // 获取运行时适配器统计信息
    let adapter_stats = state.adapter_manager.get_adapter_stats();
    
    let mut total_endpoints = 0;
    let mut stats_by_type = std::collections::HashMap::new();
    let mut detailed_stats = std::collections::HashMap::new();
    
    for provider in &provider_types {
        let runtime_stats = adapter_stats.get(&provider.name);
        let usage_count = provider_usage_stats.get(&provider.id).unwrap_or(&0);
        
        let endpoints = runtime_stats.map(|s| s.supported_endpoints).unwrap_or(0);
        total_endpoints += endpoints;
        
        // 按API格式分组统计
        let type_entry = stats_by_type
            .entry(provider.api_format.clone())
            .or_insert_with(|| json!({
                "adapters": 0,
                "endpoints": 0,
                "active_configs": 0,
                "names": []
            }));
        
        if let Some(type_obj) = type_entry.as_object_mut() {
            type_obj["adapters"] = json!(type_obj["adapters"].as_u64().unwrap_or(0) + 1);
            type_obj["endpoints"] = json!(type_obj["endpoints"].as_u64().unwrap_or(0) + endpoints as u64);
            type_obj["active_configs"] = json!(type_obj["active_configs"].as_u64().unwrap_or(0) + *usage_count);
            
            if let Some(names_array) = type_obj["names"].as_array_mut() {
                names_array.push(json!(provider.name));
            }
        }
        
        // 详细统计信息
        detailed_stats.insert(provider.name.clone(), json!({
            "id": provider.id,
            "display_name": provider.display_name,
            "api_format": provider.api_format,
            "base_url": provider.base_url,
            "supported_endpoints": endpoints,
            "active_configurations": usage_count,
            "runtime_info": runtime_stats.map(|s| json!({
                "upstream_type": s.upstream_type,
                "endpoints": s.endpoints
            })),
            "health_status": "unknown", // TODO: 从健康检查服务获取
            "rate_limit": provider.rate_limit,
            "timeout_seconds": provider.timeout_seconds,
            "last_updated": provider.updated_at
        }));
    }

    let response = json!({
        "summary": {
            "total_adapters": provider_types.len(),
            "total_endpoints": total_endpoints,
            "adapter_types": stats_by_type.len(),
            "total_active_configs": provider_usage_stats.values().sum::<u64>()
        },
        "by_type": stats_by_type,
        "detailed_stats": detailed_stats,
        "timestamp": chrono::Utc::now()
    });

    Ok(Json(response))
}