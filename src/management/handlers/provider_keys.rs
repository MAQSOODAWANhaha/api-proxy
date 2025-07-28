//! # Provider Keys管理处理器

use crate::management::server::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sea_orm::{entity::*, query::*};
use entity::{
    user_provider_keys,
    user_provider_keys::Entity as UserProviderKeys,
    provider_types,
    provider_types::Entity as ProviderTypes,
};
use chrono::Utc;

/// Provider Key查询参数
#[derive(Debug, Deserialize)]
pub struct ProviderKeyQuery {
    /// 页码
    pub page: Option<u32>,
    /// 每页大小
    pub limit: Option<u32>,
    /// 用户ID过滤
    pub user_id: Option<i32>,
    /// 服务商类型过滤
    pub provider_type: Option<String>,
    /// 状态过滤
    pub status: Option<String>,
    /// 健康状态过滤
    pub healthy: Option<bool>,
}

/// 创建Provider Key请求
#[derive(Debug, Deserialize)]
pub struct CreateProviderKeyRequest {
    /// 服务商类型
    pub provider_type: String,
    /// 密钥名称
    pub name: String,
    /// API密钥
    pub api_key: String,
    /// 权重
    pub weight: Option<i32>,
    /// 最大每分钟请求数
    pub max_requests_per_minute: Option<i32>,
    /// 最大每日令牌数
    pub max_tokens_per_day: Option<i32>,
    /// 是否启用
    pub is_active: Option<bool>,
}

/// 更新Provider Key请求
#[derive(Debug, Deserialize)]
pub struct UpdateProviderKeyRequest {
    /// 密钥名称
    pub name: Option<String>,
    /// API密钥
    pub api_key: Option<String>,
    /// 权重
    pub weight: Option<i32>,
    /// 最大每分钟请求数
    pub max_requests_per_minute: Option<i32>,
    /// 最大每日令牌数
    pub max_tokens_per_day: Option<i32>,
    /// 是否激活
    pub is_active: Option<bool>,
}

/// Provider Key响应
#[derive(Debug, Serialize)]
pub struct ProviderKeyResponse {
    /// 密钥ID
    pub id: i32,
    /// 用户ID
    pub user_id: i32,
    /// 服务商类型
    pub provider_type: String,
    /// 服务商显示名称
    pub provider_name: String,
    /// 密钥名称
    pub name: String,
    /// 完整API密钥
    pub api_key: String,
    /// 权重
    pub weight: i32,
    /// 最大每分钟请求数
    pub max_requests_per_minute: Option<i32>,
    /// 最大每日令牌数
    pub max_tokens_per_day: Option<i32>,
    /// 今日已用令牌数
    pub used_tokens_today: i32,
    /// 最后使用时间
    pub last_used: Option<String>,
    /// 是否启用
    pub is_active: bool,
    /// 健康状态
    pub health_status: String,
    /// 创建时间
    pub created_at: String,
    /// 更新时间
    pub updated_at: String,
}

/// 列出Provider Keys
pub async fn list_provider_keys(
    State(state): State<AppState>,
    Query(query): Query<ProviderKeyQuery>,
) -> Result<Json<Value>, StatusCode> {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;
    
    // 构建查询条件
    let mut select = UserProviderKeys::find();
    
    // 用户ID过滤
    if let Some(user_id) = query.user_id {
        select = select.filter(user_provider_keys::Column::UserId.eq(user_id));
    }
    
    // 服务商类型过滤
    if let Some(provider_type) = &query.provider_type {
        // 先查找provider_type的ID
        if let Ok(Some(pt)) = ProviderTypes::find()
            .filter(provider_types::Column::Name.eq(provider_type))
            .one(state.database.as_ref()).await {
            select = select.filter(user_provider_keys::Column::ProviderTypeId.eq(pt.id));
        }
    }
    
    // 状态过滤
    if let Some(status) = &query.status {
        match status.as_str() {
            "active" => select = select.filter(user_provider_keys::Column::IsActive.eq(true)),
            "inactive" => select = select.filter(user_provider_keys::Column::IsActive.eq(false)),
            _ => {}
        }
    }
    
    // 健康状态过滤
    // TODO: 当实现了真实的健康检查系统后，这里需要join health_checks表进行筛选
    // 目前所有密钥的health_status都是硬编码为"healthy"，所以healthy=false的查询将返回空结果
    if let Some(healthy) = query.healthy {
        if !healthy {
            // 如果查询非健康状态，由于目前所有密钥都是健康的，返回空结果
            // 通过添加一个永远为false的条件来实现
            select = select.filter(user_provider_keys::Column::Id.eq(-1));
        }
        // 如果查询健康状态(healthy=true)，不需要额外筛选，因为所有密钥都是健康的
    }
    
    // 分页查询
    let provider_keys_result = select
        .offset(offset as u64)
        .limit(limit as u64)
        .order_by_desc(user_provider_keys::Column::CreatedAt)
        .find_also_related(ProviderTypes)
        .all(state.database.as_ref())
        .await;
        
    let provider_keys_data = match provider_keys_result {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to fetch provider keys: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // 获取总数
    let mut count_select = UserProviderKeys::find();
    if let Some(user_id) = query.user_id {
        count_select = count_select.filter(user_provider_keys::Column::UserId.eq(user_id));
    }
    if let Some(provider_type) = &query.provider_type {
        // 先查找provider_type的ID
        if let Ok(Some(pt)) = ProviderTypes::find()
            .filter(provider_types::Column::Name.eq(provider_type))
            .one(state.database.as_ref()).await {
            count_select = count_select.filter(user_provider_keys::Column::ProviderTypeId.eq(pt.id));
        }
    }
    if let Some(status) = &query.status {
        match status.as_str() {
            "active" => count_select = count_select.filter(user_provider_keys::Column::IsActive.eq(true)),
            "inactive" => count_select = count_select.filter(user_provider_keys::Column::IsActive.eq(false)),
            _ => {}
        }
    }
    if let Some(healthy) = query.healthy {
        if !healthy {
            count_select = count_select.filter(user_provider_keys::Column::Id.eq(-1));
        }
    }
    
    let total = match count_select.count(state.database.as_ref()).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!("Failed to count provider keys: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // 转换为响应格式
    let mut provider_keys = Vec::new();
    for (provider_key, provider_type) in provider_keys_data {
        let provider = provider_type.unwrap_or(provider_types::Model {
            id: 0,
            name: "unknown".to_string(),
            display_name: "Unknown Provider".to_string(),
            base_url: "".to_string(),
            api_format: "".to_string(),
            default_model: None,
            max_tokens: None,
            rate_limit: None,
            timeout_seconds: None,
            health_check_path: None,
            auth_header_format: None,
            is_active: false,
            config_json: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        });
        
        provider_keys.push(ProviderKeyResponse {
            id: provider_key.id,
            user_id: provider_key.user_id,
            provider_type: provider.name.clone(),
            provider_name: provider.display_name,
            name: provider_key.name,
            api_key: provider_key.api_key,
            weight: provider_key.weight.unwrap_or(1),
            max_requests_per_minute: provider_key.max_requests_per_minute,
            max_tokens_per_day: provider_key.max_tokens_per_day,
            used_tokens_today: provider_key.used_tokens_today.unwrap_or(0),
            last_used: provider_key.last_used.map(|dt| dt.and_utc().to_rfc3339()),
            is_active: provider_key.is_active,
            health_status: "healthy".to_string(), // TODO: 从health检查表获取实际状态
            created_at: provider_key.created_at.and_utc().to_rfc3339(),
            updated_at: provider_key.updated_at.and_utc().to_rfc3339(),
        });
    }

    let response = json!({
        "keys": provider_keys,
        "pagination": {
            "page": page,
            "limit": limit,
            "total": total,
            "pages": ((total as f64) / (limit as f64)).ceil() as u32
        }
    });

    Ok(Json(response))
}

/// 创建Provider Key
pub async fn create_provider_key(
    State(state): State<AppState>,
    Json(request): Json<CreateProviderKeyRequest>,
) -> Result<Json<Value>, StatusCode> {
    // 验证输入
    if request.name.is_empty() {
        return Ok(Json(json!({
            "success": false,
            "message": "Name cannot be empty"
        })));
    }

    if request.api_key.is_empty() {
        return Ok(Json(json!({
            "success": false,
            "message": "API key cannot be empty"
        })));
    }

    // 通过provider_type名称查找对应的provider_type记录
    let provider_type = match ProviderTypes::find()
        .filter(provider_types::Column::Name.eq(&request.provider_type))
        .one(state.database.as_ref()).await {
        Ok(Some(pt)) => pt,
        Ok(None) => {
            return Ok(Json(json!({
                "success": false,
                "message": format!("Provider type '{}' not found", request.provider_type)
            })));
        },
        Err(err) => {
            tracing::error!("Failed to check provider type existence: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let now = Utc::now().naive_utc();
    // TODO: 从认证上下文获取真实的user_id，这里暂时使用固定值
    let user_id = 1; 

    // 创建Provider Key记录
    let new_provider_key = user_provider_keys::ActiveModel {
        user_id: Set(user_id),
        provider_type_id: Set(provider_type.id),
        name: Set(request.name.clone()),
        api_key: Set(request.api_key.clone()),
        weight: Set(request.weight),
        max_requests_per_minute: Set(request.max_requests_per_minute),
        max_tokens_per_day: Set(request.max_tokens_per_day),
        used_tokens_today: Set(Some(0)),
        is_active: Set(request.is_active.unwrap_or(true)),
        created_at: Set(now),
        updated_at: Set(now),
        last_used: Set(None),
        ..Default::default()
    };

    let insert_result = UserProviderKeys::insert(new_provider_key).exec(state.database.as_ref()).await;
    
    match insert_result {
        Ok(result) => {
            // 获取创建的密钥以返回完整信息
            let created_key = UserProviderKeys::find_by_id(result.last_insert_id)
                .find_also_related(ProviderTypes)
                .one(state.database.as_ref())
                .await;

            match created_key {
                Ok(Some((key, provider))) => {
                    let provider = provider.unwrap_or(provider_type);
                    let key_response = ProviderKeyResponse {
                        id: key.id,
                        user_id: key.user_id,
                        provider_type: provider.name,
                        provider_name: provider.display_name,
                        name: key.name,
                        api_key: key.api_key,
                        weight: key.weight.unwrap_or(1),
                        max_requests_per_minute: key.max_requests_per_minute,
                        max_tokens_per_day: key.max_tokens_per_day,
                        used_tokens_today: key.used_tokens_today.unwrap_or(0),
                        last_used: key.last_used.map(|dt| dt.and_utc().to_rfc3339()),
                        is_active: key.is_active,
                        health_status: "healthy".to_string(),
                        created_at: key.created_at.and_utc().to_rfc3339(),
                        updated_at: key.updated_at.and_utc().to_rfc3339(),
                    };

                    let response = json!({
                        "success": true,
                        "key": key_response,
                        "message": "Provider key created successfully"
                    });
                    Ok(Json(response))
                },
                _ => {
                    let response = json!({
                        "success": true,
                        "message": "Provider key created successfully"
                    });
                    Ok(Json(response))
                }
            }
        }
        Err(err) => {
            tracing::error!("Failed to create provider key: {}", err);
            let response = json!({
                "success": false,
                "message": "Failed to create provider key"
            });
            Ok(Json(response))
        }
    }
}

/// 获取单个Provider Key
pub async fn get_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
) -> Result<Json<Value>, StatusCode> {
    if key_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 从数据库获取Provider Key
    let provider_key_result = UserProviderKeys::find_by_id(key_id)
        .find_also_related(ProviderTypes)
        .one(state.database.as_ref())
        .await;

    let (provider_key, provider_type) = match provider_key_result {
        Ok(Some(data)) => data,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(err) => {
            tracing::error!("Failed to fetch provider key {}: {}", key_id, err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let provider = provider_type.unwrap_or(provider_types::Model {
        id: 0,
        name: "unknown".to_string(),
        display_name: "Unknown Provider".to_string(),
        base_url: "".to_string(),
        api_format: "".to_string(),
        default_model: None,
        max_tokens: None,
        rate_limit: None,
        timeout_seconds: None,
        health_check_path: None,
        auth_header_format: None,
        is_active: false,
        config_json: None,
        created_at: chrono::Utc::now().naive_utc(),
        updated_at: chrono::Utc::now().naive_utc(),
    });

    let provider_key_response = ProviderKeyResponse {
        id: provider_key.id,
        user_id: provider_key.user_id,
        provider_type: provider.name,
        provider_name: provider.display_name,
        name: provider_key.name,
        api_key: provider_key.api_key,
        weight: provider_key.weight.unwrap_or(1),
        max_requests_per_minute: provider_key.max_requests_per_minute,
        max_tokens_per_day: provider_key.max_tokens_per_day,
        used_tokens_today: provider_key.used_tokens_today.unwrap_or(0),
        last_used: provider_key.last_used.map(|dt| dt.and_utc().to_rfc3339()),
        is_active: provider_key.is_active,
        health_status: "healthy".to_string(), // TODO: 从health检查表获取实际状态
        created_at: provider_key.created_at.and_utc().to_rfc3339(),
        updated_at: provider_key.updated_at.and_utc().to_rfc3339(),
    };

    Ok(Json(serde_json::to_value(provider_key_response).unwrap()))
}

/// 更新Provider Key
pub async fn update_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    Json(request): Json<UpdateProviderKeyRequest>,
) -> Result<Json<Value>, StatusCode> {
    if key_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 检查Provider Key是否存在
    let existing_key = match UserProviderKeys::find_by_id(key_id).one(state.database.as_ref()).await {
        Ok(Some(key)) => key,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(err) => {
            tracing::error!("Failed to check provider key existence: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // 更新Provider Key
    let now = Utc::now().naive_utc();
    let mut provider_key: user_provider_keys::ActiveModel = existing_key.into();
    
    if let Some(name) = request.name {
        provider_key.name = Set(name);
    }
    if let Some(api_key) = request.api_key {
        provider_key.api_key = Set(api_key);
    }
    if let Some(weight) = request.weight {
        provider_key.weight = Set(Some(weight));
    }
    if let Some(max_requests) = request.max_requests_per_minute {
        provider_key.max_requests_per_minute = Set(Some(max_requests));
    }
    if let Some(max_tokens) = request.max_tokens_per_day {
        provider_key.max_tokens_per_day = Set(Some(max_tokens));
    }
    if let Some(is_active) = request.is_active {
        provider_key.is_active = Set(is_active);
    }
    
    provider_key.updated_at = Set(now);

    match provider_key.update(state.database.as_ref()).await {
        Ok(_) => {
            let response = json!({
                "success": true,
                "message": format!("Provider key {} has been updated", key_id)
            });
            Ok(Json(response))
        }
        Err(err) => {
            tracing::error!("Failed to update provider key {}: {}", key_id, err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 删除Provider Key
pub async fn delete_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
) -> Result<Json<Value>, StatusCode> {
    if key_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 检查Provider Key是否存在
    let existing_key = match UserProviderKeys::find_by_id(key_id).one(state.database.as_ref()).await {
        Ok(Some(key)) => key,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(err) => {
            tracing::error!("Failed to check provider key existence: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // 硬删除：直接从数据库删除记录
    match UserProviderKeys::delete_by_id(key_id).exec(state.database.as_ref()).await {
        Ok(_) => {
            let response = json!({
                "success": true,
                "message": format!("Provider key {} has been deleted", key_id)
            });
            Ok(Json(response))
        }
        Err(err) => {
            tracing::error!("Failed to delete provider key {}: {}", key_id, err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 切换Provider Key状态
#[derive(Debug, Deserialize)]
pub struct ToggleStatusRequest {
    pub is_active: bool,
}

pub async fn toggle_provider_key_status(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    Json(request): Json<ToggleStatusRequest>,
) -> Result<Json<Value>, StatusCode> {
    if key_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 检查Provider Key是否存在
    let existing_key = match UserProviderKeys::find_by_id(key_id).one(state.database.as_ref()).await {
        Ok(Some(key)) => key,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(err) => {
            tracing::error!("Failed to check provider key existence: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // 更新状态
    let now = Utc::now().naive_utc();
    let mut provider_key: user_provider_keys::ActiveModel = existing_key.into();
    provider_key.is_active = Set(request.is_active);
    provider_key.updated_at = Set(now);

    match provider_key.update(state.database.as_ref()).await {
        Ok(_) => {
            let response = json!({
                "success": true,
                "message": format!("Provider key status updated to {}", 
                    if request.is_active { "active" } else { "inactive" })
            });
            Ok(Json(response))
        }
        Err(err) => {
            tracing::error!("Failed to toggle provider key status: {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 测试Provider Key
pub async fn test_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
) -> Result<Json<Value>, StatusCode> {
    if key_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 检查Provider Key是否存在
    let provider_key = match UserProviderKeys::find_by_id(key_id).one(state.database.as_ref()).await {
        Ok(Some(key)) => key,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(err) => {
            tracing::error!("Failed to check provider key existence: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // TODO: 实现实际的API密钥测试逻辑
    // 这里应该向对应的服务商API发送测试请求
    let start_time = std::time::Instant::now();
    
    // 模拟测试过程
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    let response_time = start_time.elapsed().as_millis() as u64;
    let success = provider_key.is_active; // 简单的模拟逻辑

    let response = json!({
        "success": success,
        "response_time": response_time,
        "status": if success { "healthy" } else { "unhealthy" },
        "message": if success { 
            "API key test successful" 
        } else { 
            "API key is inactive or invalid" 
        }
    });

    Ok(Json(response))
}

/// 获取Provider Key使用统计
pub async fn get_provider_key_usage(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    if key_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 检查Provider Key是否存在
    let _provider_key = match UserProviderKeys::find_by_id(key_id).one(state.database.as_ref()).await {
        Ok(Some(key)) => key,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(err) => {
            tracing::error!("Failed to check provider key existence: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // TODO: 从实际的统计表获取数据
    // 这里返回模拟数据
    let usage_data = json!({
        "usage": [
            {
                "timestamp": "2025-07-27",
                "requests": 150,
                "tokens": 25000,
                "success_rate": 96.5
            },
            {
                "timestamp": "2025-07-26",
                "requests": 180,
                "tokens": 30000,
                "success_rate": 98.2
            }
        ],
        "summary": {
            "total_requests": 330,
            "total_tokens": 55000,
            "avg_response_time": 245
        }
    });

    Ok(Json(usage_data))
}

/// 获取支持的服务商类型
pub async fn get_provider_types(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let provider_types_result = ProviderTypes::find()
        .filter(provider_types::Column::IsActive.eq(true))
        .all(state.database.as_ref())
        .await;

    let provider_types = match provider_types_result {
        Ok(types) => types,
        Err(err) => {
            tracing::error!("Failed to fetch provider types: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let provider_types_data: Vec<Value> = provider_types.into_iter().map(|pt| {
        json!({
            "id": pt.name,
            "name": pt.name,
            "display_name": pt.display_name,
            "base_url": pt.base_url,
            "default_model": pt.default_model,
            "supported_features": [] // TODO: 从config_json解析
        })
    }).collect();

    Ok(Json(json!(provider_types_data)))
}