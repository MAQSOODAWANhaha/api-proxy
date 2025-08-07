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
    let adapter_stats = state.adapter_manager.get_adapter_stats().await;
    
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
    let adapter_stats = state.adapter_manager.get_adapter_stats().await;
    
    let mut stats_by_type = std::collections::HashMap::new();
    let mut detailed_stats = std::collections::HashMap::new();
    
    for provider in &provider_types {
        let runtime_stats = adapter_stats.get(&provider.name);
        let usage_count = provider_usage_stats.get(&provider.id).unwrap_or(&0);
        
        
        // 按API格式分组统计
        let type_entry = stats_by_type
            .entry(provider.api_format.clone())
            .or_insert_with(|| json!({
                "adapters": 0,
                "active_configs": 0,
                "names": []
            }));
        
        if let Some(type_obj) = type_entry.as_object_mut() {
            type_obj["adapters"] = json!(type_obj["adapters"].as_u64().unwrap_or(0) + 1);
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
            "active_configurations": usage_count,
            "runtime_info": runtime_stats.map(|s| json!({
                "api_format": s.api_format
            })),
            "health_status": get_adapter_health_status(&state, &provider.name).await,
            "rate_limit": provider.rate_limit,
            "timeout_seconds": provider.timeout_seconds,
            "last_updated": provider.updated_at
        }));
    }

    let response = json!({
        "summary": {
            "total_adapters": provider_types.len(),
            "adapter_types": stats_by_type.len(),
            "total_active_configs": provider_usage_stats.values().sum::<u64>()
        },
        "by_type": stats_by_type,
        "detailed_stats": detailed_stats,
        "timestamp": chrono::Utc::now()
    });

    Ok(Json(response))
}

/// 获取适配器健康状态
async fn get_adapter_health_status(state: &AppState, provider_name: &str) -> Value {
    // 获取与该提供商相关的服务器健康状态
    let all_health_status = state.health_service.get_all_health_status().await;
    
    // 查找匹配的服务器（通过提供商名称）
    let mut matching_servers = Vec::new();
    let mut total_response_time = 0u64;
    let mut healthy_count = 0;
    let mut total_count = 0;
    
    for (server_address, health_status) in &all_health_status {
        // 简单匹配：如果服务器地址包含提供商名称，就认为是相关的
        if server_address.contains(provider_name) || server_address.contains(&provider_name.to_lowercase()) {
            total_count += 1;
            total_response_time += health_status.avg_response_time.as_millis() as u64;
            
            if health_status.is_healthy {
                healthy_count += 1;
            }
            
            matching_servers.push(json!({
                "server": server_address,
                "status": if health_status.is_healthy { "healthy" } else { "unhealthy" },
                "last_check": health_status.last_check.map(|t| chrono::Utc::now() - chrono::Duration::from_std(t.elapsed()).unwrap_or_default()),
                "response_time_ms": health_status.avg_response_time.as_millis(),
                "consecutive_failures": health_status.consecutive_failures,
                "is_healthy": health_status.is_healthy
            }));
        }
    }
    
    if total_count == 0 {
        // 没有找到相关服务器，返回未知状态
        return json!({
            "status": "no_servers",
            "last_check": null,
            "response_time_ms": null,
            "success_rate": 0.0,
            "healthy_servers": 0,
            "total_servers": 0,
            "is_healthy": false,
            "details": "No health check servers found for this provider",
            "servers": []
        });
    }
    
    let avg_response_time = if total_count > 0 { total_response_time / total_count as u64 } else { 0 };
    let success_rate = if total_count > 0 { (healthy_count as f64 / total_count as f64) * 100.0 } else { 0.0 };
    let overall_status = if healthy_count == total_count { "healthy" } else if healthy_count == 0 { "unhealthy" } else { "degraded" };
    
    json!({
        "status": overall_status,
        "last_check": matching_servers.first()
            .and_then(|s| s.get("last_check"))
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        "response_time_ms": avg_response_time,
        "success_rate": success_rate,
        "healthy_servers": healthy_count,
        "total_servers": total_count,
        "is_healthy": healthy_count > 0,
        "details": format!("{}/{} servers healthy", healthy_count, total_count),
        "servers": matching_servers
    })
}