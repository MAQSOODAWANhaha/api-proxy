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
}

/// 创建Provider Key请求
#[derive(Debug, Deserialize)]
pub struct CreateProviderKeyRequest {
    /// 用户ID
    pub user_id: i32,
    /// 服务商类型ID
    pub provider_type_id: i32,
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
    pub provider_display_name: String,
    /// 密钥名称
    pub name: String,
    /// 密钥前缀（用于显示）
    pub api_key_prefix: String,
    /// 权重
    pub weight: Option<i32>,
    /// 最大每分钟请求数
    pub max_requests_per_minute: Option<i32>,
    /// 最大每日令牌数
    pub max_tokens_per_day: Option<i32>,
    /// 今日已用令牌数
    pub used_tokens_today: Option<i32>,
    /// 状态
    pub status: String,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// 最后使用时间
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
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
    
    // 状态过滤
    if let Some(status) = &query.status {
        match status.as_str() {
            "active" => select = select.filter(user_provider_keys::Column::IsActive.eq(true)),
            "inactive" => select = select.filter(user_provider_keys::Column::IsActive.eq(false)),
            _ => {}
        }
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
    if let Some(status) = &query.status {
        match status.as_str() {
            "active" => count_select = count_select.filter(user_provider_keys::Column::IsActive.eq(true)),
            "inactive" => count_select = count_select.filter(user_provider_keys::Column::IsActive.eq(false)),
            _ => {}
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
        
        let api_key_prefix = if provider_key.api_key.len() > 10 {
            format!("{}***{}", &provider_key.api_key[..4], &provider_key.api_key[provider_key.api_key.len() - 4..])
        } else {
            "***".to_string()
        };
        
        provider_keys.push(ProviderKeyResponse {
            id: provider_key.id,
            user_id: provider_key.user_id,
            provider_type: provider.name.clone(),
            provider_display_name: provider.display_name,
            name: provider_key.name,
            api_key_prefix,
            weight: provider_key.weight,
            max_requests_per_minute: provider_key.max_requests_per_minute,
            max_tokens_per_day: provider_key.max_tokens_per_day,
            used_tokens_today: provider_key.used_tokens_today,
            status: if provider_key.is_active { "active".to_string() } else { "inactive".to_string() },
            created_at: provider_key.created_at.and_utc(),
            updated_at: provider_key.updated_at.and_utc(),
            last_used: provider_key.last_used.map(|dt| dt.and_utc()),
        });
    }

    let response = json!({
        "provider_keys": provider_keys,
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
        return Err(StatusCode::BAD_REQUEST);
    }

    if request.api_key.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    if request.user_id <= 0 || request.provider_type_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 验证服务商类型是否存在
    let provider_type_exists = match ProviderTypes::find_by_id(request.provider_type_id)
        .one(state.database.as_ref()).await {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(err) => {
            tracing::error!("Failed to check provider type existence: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    if !provider_type_exists {
        return Ok(Json(json!({
            "success": false,
            "message": "Provider type not found"
        })));
    }

    let now = Utc::now().naive_utc();

    // 创建Provider Key记录
    let new_provider_key = user_provider_keys::ActiveModel {
        user_id: Set(request.user_id),
        provider_type_id: Set(request.provider_type_id),
        name: Set(request.name.clone()),
        api_key: Set(request.api_key.clone()),
        weight: Set(request.weight),
        max_requests_per_minute: Set(request.max_requests_per_minute),
        max_tokens_per_day: Set(request.max_tokens_per_day),
        used_tokens_today: Set(Some(0)),
        is_active: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        last_used: Set(None),
        ..Default::default()
    };

    let insert_result = UserProviderKeys::insert(new_provider_key).exec(state.database.as_ref()).await;
    
    match insert_result {
        Ok(result) => {
            let response = json!({
                "success": true,
                "provider_key_id": result.last_insert_id,
                "message": "Provider key created successfully"
            });
            Ok(Json(response))
        }
        Err(err) => {
            tracing::error!("Failed to create provider key: {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
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

    let api_key_prefix = if provider_key.api_key.len() > 10 {
        format!("{}***{}", &provider_key.api_key[..4], &provider_key.api_key[provider_key.api_key.len() - 4..])
    } else {
        "***".to_string()
    };

    let provider_key_response = ProviderKeyResponse {
        id: provider_key.id,
        user_id: provider_key.user_id,
        provider_type: provider.name,
        provider_display_name: provider.display_name,
        name: provider_key.name,
        api_key_prefix,
        weight: provider_key.weight,
        max_requests_per_minute: provider_key.max_requests_per_minute,
        max_tokens_per_day: provider_key.max_tokens_per_day,
        used_tokens_today: provider_key.used_tokens_today,
        status: if provider_key.is_active { "active".to_string() } else { "inactive".to_string() },
        created_at: provider_key.created_at.and_utc(),
        updated_at: provider_key.updated_at.and_utc(),
        last_used: provider_key.last_used.map(|dt| dt.and_utc()),
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

    // 软删除：设置为非活跃状态
    let now = Utc::now().naive_utc();
    let mut provider_key: user_provider_keys::ActiveModel = existing_key.into();
    provider_key.is_active = Set(false);
    provider_key.updated_at = Set(now);

    match provider_key.update(state.database.as_ref()).await {
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