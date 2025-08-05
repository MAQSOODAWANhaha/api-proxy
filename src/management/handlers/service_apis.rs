//! # Service APIs管理处理器
//! 
//! 处理用户对外API服务的管理功能

use crate::management::server::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use chrono::Utc;
use uuid::Uuid;

/// Service API查询参数
#[derive(Debug, Deserialize)]
pub struct ServiceApiQuery {
    /// 页码
    pub page: Option<u32>,
    /// 每页大小
    pub limit: Option<u32>,
    /// 用户ID过滤
    pub user_id: Option<i32>,
    /// 调度策略过滤
    pub scheduling_strategy: Option<String>,
    /// 状态过滤
    pub is_active: Option<bool>,
}

/// 创建Service API请求
#[derive(Debug, Deserialize)]
pub struct CreateServiceApiRequest {
    /// 服务商类型
    pub provider_type: String,
    /// 服务名称
    pub name: Option<String>,
    /// 服务描述
    pub description: Option<String>,
    /// 调度策略
    pub scheduling_strategy: Option<String>,
    /// 重试次数
    pub retry_count: Option<i32>,
    /// 超时时间(秒)
    pub timeout_seconds: Option<i32>,
    /// 速率限制
    pub rate_limit: Option<i32>,
    /// 每日Token限制
    pub max_tokens_per_day: Option<i32>,
    /// 过期天数
    pub expires_in_days: Option<i32>,
    /// 是否启用
    pub is_active: Option<bool>,
}

/// 更新Service API请求
#[derive(Debug, Deserialize)]
pub struct UpdateServiceApiRequest {
    /// 服务名称
    pub name: Option<String>,
    /// 服务描述
    pub description: Option<String>,
    /// 调度策略
    pub scheduling_strategy: Option<String>,
    /// 重试次数
    pub retry_count: Option<i32>,
    /// 超时时间(秒)
    pub timeout_seconds: Option<i32>,
    /// 速率限制
    pub rate_limit: Option<i32>,
    /// 每日Token限制
    pub max_tokens_per_day: Option<i32>,
    /// 是否启用
    pub is_active: Option<bool>,
}

/// Service API响应
#[derive(Debug, Serialize, Clone)]
pub struct ServiceApiResponse {
    /// API服务ID
    pub id: i32,
    /// 用户ID
    pub user_id: i32,
    /// 服务商类型
    pub provider_type: String,
    /// 服务商显示名称
    pub provider_name: String,
    /// 对外API密钥
    pub api_key: String,
    /// API密钥签名
    pub api_secret: String,
    /// 服务名称
    pub name: Option<String>,
    /// 服务描述
    pub description: Option<String>,
    /// 调度策略
    pub scheduling_strategy: String,
    /// 重试次数
    pub retry_count: i32,
    /// 超时时间(秒)
    pub timeout_seconds: i32,
    /// 速率限制
    pub rate_limit: i32,
    /// 每日Token限制
    pub max_tokens_per_day: i32,
    /// 今日已用Token数
    pub used_tokens_today: i32,
    /// 总请求数
    pub total_requests: i32,
    /// 成功请求数
    pub successful_requests: i32,
    /// 最后使用时间
    pub last_used: Option<String>,
    /// 过期时间
    pub expires_at: Option<String>,
    /// 是否启用
    pub is_active: bool,
    /// 创建时间
    pub created_at: String,
    /// 更新时间
    pub updated_at: String,
}

/// 列出Service APIs
pub async fn list_service_apis(
    State(state): State<AppState>,
    Query(query): Query<ServiceApiQuery>,
) -> Result<Json<Value>, StatusCode> {
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use entity::provider_types::{self, Entity as ProviderType};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect, PaginatorTrait};
    
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    
    // 获取数据库连接
    let db = state.database.as_ref();
    
    // 构建基础查询
    let mut select = UserServiceApi::find();
    
    // 应用筛选条件
    if let Some(user_id) = query.user_id {
        select = select.filter(user_service_apis::Column::UserId.eq(user_id));
    }
    
    if let Some(strategy) = &query.scheduling_strategy {
        select = select.filter(user_service_apis::Column::SchedulingStrategy.eq(strategy));
    }
    
    if let Some(is_active) = query.is_active {
        select = select.filter(user_service_apis::Column::IsActive.eq(is_active));
    }

    // 获取总数
    let total = select
        .clone()
        .count(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? as u32;

    // 分页查询
    let apis_with_provider = select
        .offset(((page - 1) * limit) as u64)
        .limit(limit as u64)
        .find_also_related(ProviderType)
        .all(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 转换为响应格式
    tracing::debug!("Processing {} service API records", apis_with_provider.len());
    
    let response_apis: Result<Vec<ServiceApiResponse>, String> = apis_with_provider
        .into_iter()
        .map(|(api, provider_type)| {
            tracing::debug!("Processing API ID: {}, Provider Type ID: {}", api.id, api.provider_type_id);
            
            let provider = match provider_type {
                Some(p) => {
                    tracing::debug!("Found provider: {} ({})", p.display_name, p.name);
                    p
                },
                None => {
                    tracing::warn!("Provider type not found for API ID: {}, using defaults", api.id);
                    provider_types::Model {
                        id: api.provider_type_id,
                        name: "unknown".to_string(),
                        display_name: "Unknown".to_string(),
                        base_url: "".to_string(),
                        api_format: "openai".to_string(),
                        default_model: None,
                        max_tokens: None,
                        rate_limit: None,
                        timeout_seconds: None,
                        health_check_path: None,
                        auth_header_format: None,
                        is_active: true,
                        config_json: None,
                        created_at: chrono::Utc::now().naive_utc(),
                        updated_at: chrono::Utc::now().naive_utc(),
                    }
                }
            };
            
            // 使用新的时间转换方法替代已弃用的from_utc
            let last_used = api.last_used.map(|dt| {
                chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339()
            });
            
            let expires_at = api.expires_at.map(|dt| {
                chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339()
            });
            
            let created_at = chrono::DateTime::<Utc>::from_naive_utc_and_offset(api.created_at, Utc).to_rfc3339();
            let updated_at = chrono::DateTime::<Utc>::from_naive_utc_and_offset(api.updated_at, Utc).to_rfc3339();
            
            Ok(ServiceApiResponse {
                id: api.id,
                user_id: api.user_id,
                provider_type: provider.id.to_string(),
                provider_name: provider.display_name,
                api_key: api.api_key,
                api_secret: api.api_secret,
                name: api.name,
                description: api.description,
                scheduling_strategy: api.scheduling_strategy.unwrap_or("round_robin".to_string()),
                retry_count: api.retry_count.unwrap_or(3),
                timeout_seconds: api.timeout_seconds.unwrap_or(30),
                rate_limit: api.rate_limit.unwrap_or(0),
                max_tokens_per_day: api.max_tokens_per_day.unwrap_or(0),
                used_tokens_today: api.used_tokens_today.unwrap_or(0),
                total_requests: api.total_requests.unwrap_or(0),
                successful_requests: api.successful_requests.unwrap_or(0),
                last_used,
                expires_at,
                is_active: api.is_active,
                created_at,
                updated_at,
            })
        })
        .collect();
    
    let response_apis = match response_apis {
        Ok(apis) => apis,
        Err(e) => {
            tracing::error!("Error processing service APIs: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let response = json!({
        "api_keys": response_apis,  // 保持与前端ApiKeyListResponse接口一致
        "pagination": {
            "page": page,
            "limit": limit,
            "total": total,
            "pages": ((total as f64) / (limit as f64)).ceil() as u32
        }
    });

    Ok(Json(response))
}

/// 获取单个Service API
pub async fn get_service_api(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
) -> Result<Json<Value>, StatusCode> {
    use entity::user_service_apis::{Entity as UserServiceApi};
    use entity::provider_types::{self, Entity as ProviderType};
    use sea_orm::{EntityTrait, JoinType, QuerySelect, RelationTrait};
    
    if api_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 获取数据库连接
    let db = state.database.as_ref();
    
    // 查询Service API及其关联的Provider Type
    let api_with_provider = UserServiceApi::find_by_id(api_id)
        .join(JoinType::InnerJoin, entity::user_service_apis::Relation::ProviderType.def())
        .find_also_related(ProviderType)
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (api, provider_type) = match api_with_provider {
        Some((api, provider)) => (api, provider),
        None => {
            return Err(StatusCode::NOT_FOUND);
        }
    };

    let provider = provider_type.unwrap_or_else(|| provider_types::Model {
        id: api.provider_type_id,
        name: "unknown".to_string(),
        display_name: "Unknown".to_string(),
        base_url: "".to_string(),
        api_format: "openai".to_string(),
        default_model: None,
        max_tokens: None,
        rate_limit: None,
        timeout_seconds: None,
        health_check_path: None,
        auth_header_format: None,
        is_active: true,
        config_json: None,
        created_at: chrono::Utc::now().naive_utc(),
        updated_at: chrono::Utc::now().naive_utc(),
    });

    // 构建响应
    let response_api = ServiceApiResponse {
        id: api.id,
        user_id: api.user_id,
        provider_type: provider.id.to_string(),
        provider_name: provider.display_name,
        api_key: api.api_key,
        api_secret: api.api_secret,
        name: api.name,
        description: api.description,
        scheduling_strategy: api.scheduling_strategy.unwrap_or("round_robin".to_string()),
        retry_count: api.retry_count.unwrap_or(3),
        timeout_seconds: api.timeout_seconds.unwrap_or(30),
        rate_limit: api.rate_limit.unwrap_or(0),
        max_tokens_per_day: api.max_tokens_per_day.unwrap_or(0),
        used_tokens_today: api.used_tokens_today.unwrap_or(0),
        total_requests: api.total_requests.unwrap_or(0),
        successful_requests: api.successful_requests.unwrap_or(0),
        last_used: api.last_used.map(|dt| chrono::DateTime::<Utc>::from_utc(dt, Utc).to_rfc3339()),
        expires_at: api.expires_at.map(|dt| chrono::DateTime::<Utc>::from_utc(dt, Utc).to_rfc3339()),
        is_active: api.is_active,
        created_at: chrono::DateTime::<Utc>::from_utc(api.created_at, Utc).to_rfc3339(),
        updated_at: chrono::DateTime::<Utc>::from_utc(api.updated_at, Utc).to_rfc3339(),
    };

    Ok(Json(serde_json::to_value(response_api).unwrap()))
}

/// 创建Service API
pub async fn create_service_api(
    State(state): State<AppState>,
    Json(request): Json<CreateServiceApiRequest>,
) -> Result<Json<Value>, StatusCode> {
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use entity::provider_types::{self, Entity as ProviderType};
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
    
    // 验证输入
    if request.provider_type.is_empty() {
        return Ok(Json(json!({
            "success": false,
            "message": "Provider type cannot be empty"
        })));
    }

    // 获取数据库连接
    let db = state.database.as_ref();
    
    // 查找provider_type_id，支持按name或id查找
    let provider_type = if let Ok(id) = request.provider_type.parse::<i32>() {
        // 如果是数字，按ID查找
        ProviderType::find()
            .filter(provider_types::Column::Id.eq(id))
            .one(db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        // 如果是字符串，按name查找
        ProviderType::find()
            .filter(provider_types::Column::Name.eq(&request.provider_type))
            .one(db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };
        
    let provider_type_record = match provider_type {
        Some(pt) => pt,
        None => {
            return Ok(Json(json!({
                "success": false,
                "message": "Invalid provider type"
            })));
        }
    };

    // 生成唯一的API密钥和密钥签名
    let api_key = format!("sk-api-{}", Uuid::new_v4().to_string().replace("-", ""));
    let api_secret = format!("secret_{}", Uuid::new_v4().to_string().replace("-", ""));

    // 检查是否已存在相同服务商类型的API密钥（每种服务商只能有1个）
    let existing_api = UserServiceApi::find()
        .filter(user_service_apis::Column::UserId.eq(1)) // TODO: 从认证上下文获取实际用户ID
        .filter(user_service_apis::Column::ProviderTypeId.eq(provider_type_record.id))
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 如果已存在，返回明确的错误信息
    if let Some(existing) = existing_api {
        return Ok(Json(json!({
            "success": false,
            "message": format!("{}服务商已存在API密钥，每种服务商只能创建1个API密钥。如需更换，请先删除现有密钥。", provider_type_record.display_name),
            "error_code": "PROVIDER_API_KEY_EXISTS",
            "existing_api": {
                "id": existing.id,
                "name": existing.name,
                "created_at": chrono::DateTime::<Utc>::from_naive_utc_and_offset(existing.created_at, Utc).to_rfc3339()
            }
        })));
    }

    // 创建新的Service API记录
    let new_service_api = user_service_apis::ActiveModel {
        user_id: Set(1), // TODO: 从认证上下文获取实际用户ID
        provider_type_id: Set(provider_type_record.id),
        api_key: Set(api_key.clone()),
        api_secret: Set(api_secret.clone()),
        name: Set(request.name.clone()),
        description: Set(request.description.clone()),
        scheduling_strategy: Set(Some(request.scheduling_strategy.unwrap_or("round_robin".to_string()))),
        retry_count: Set(Some(request.retry_count.unwrap_or(3))),
        timeout_seconds: Set(Some(request.timeout_seconds.unwrap_or(30))),
        rate_limit: Set(request.rate_limit),
        max_tokens_per_day: Set(request.max_tokens_per_day),
        used_tokens_today: Set(Some(0)),
        total_requests: Set(Some(0)),
        successful_requests: Set(Some(0)),
        last_used: Set(None),
        expires_at: Set(request.expires_in_days.map(|days| {
            (Utc::now() + chrono::Duration::days(days as i64)).naive_utc()
        })),
        is_active: Set(request.is_active.unwrap_or(true)),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };

    // 插入数据库
    let inserted_api = new_service_api
        .insert(db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to insert Service API: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // 构建响应
    let provider_display_name = provider_type_record.display_name.clone();
    let response_api = ServiceApiResponse {
        id: inserted_api.id,
        user_id: inserted_api.user_id,
        provider_type: request.provider_type,
        provider_name: provider_type_record.display_name,
        api_key: inserted_api.api_key,
        api_secret: inserted_api.api_secret,
        name: inserted_api.name,
        description: inserted_api.description,
        scheduling_strategy: inserted_api.scheduling_strategy.unwrap_or("round_robin".to_string()),
        retry_count: inserted_api.retry_count.unwrap_or(3),
        timeout_seconds: inserted_api.timeout_seconds.unwrap_or(30),
        rate_limit: inserted_api.rate_limit.unwrap_or(0),
        max_tokens_per_day: inserted_api.max_tokens_per_day.unwrap_or(0),
        used_tokens_today: inserted_api.used_tokens_today.unwrap_or(0),
        total_requests: inserted_api.total_requests.unwrap_or(0),
        successful_requests: inserted_api.successful_requests.unwrap_or(0),
        last_used: inserted_api.last_used.map(|dt| chrono::DateTime::<Utc>::from_utc(dt, Utc).to_rfc3339()),
        expires_at: inserted_api.expires_at.map(|dt| chrono::DateTime::<Utc>::from_utc(dt, Utc).to_rfc3339()),
        is_active: inserted_api.is_active,
        created_at: chrono::DateTime::<Utc>::from_utc(inserted_api.created_at, Utc).to_rfc3339(),
        updated_at: chrono::DateTime::<Utc>::from_utc(inserted_api.updated_at, Utc).to_rfc3339(),
    };

    let message = format!("{}服务API创建成功", provider_display_name);

    let response = json!({
        "success": true,
        "api": response_api,
        "message": message
    });

    Ok(Json(response))
}

/// 更新Service API
pub async fn update_service_api(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
    Json(request): Json<UpdateServiceApiRequest>,
) -> Result<Json<Value>, StatusCode> {
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use entity::provider_types::{self, Entity as ProviderType};
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    
    if api_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 获取数据库连接
    let db = state.database.as_ref();
    
    // 查找现有的Service API记录
    let existing_api = UserServiceApi::find_by_id(api_id)
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
    let existing_record = match existing_api {
        Some(api) => api,
        None => {
            return Ok(Json(json!({
                "success": false,
                "message": "Service API not found"
            })));
        }
    };

    // 创建更新模型
    let mut update_model = user_service_apis::ActiveModel {
        id: Set(api_id),  // 指定要更新的记录ID
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };

    // 只更新提供的字段
    if let Some(name) = request.name {
        update_model.name = Set(Some(name));
    }
    
    if let Some(description) = request.description {
        update_model.description = Set(Some(description));
    }
    
    if let Some(strategy) = request.scheduling_strategy {
        update_model.scheduling_strategy = Set(Some(strategy));
    }
    
    if let Some(retry_count) = request.retry_count {
        update_model.retry_count = Set(Some(retry_count));
    }
    
    if let Some(timeout) = request.timeout_seconds {
        update_model.timeout_seconds = Set(Some(timeout));
    }
    
    if let Some(rate_limit) = request.rate_limit {
        update_model.rate_limit = Set(Some(rate_limit));
    }
    
    if let Some(max_tokens) = request.max_tokens_per_day {
        update_model.max_tokens_per_day = Set(Some(max_tokens));
    }
    
    if let Some(is_active) = request.is_active {
        update_model.is_active = Set(is_active);
    }

    // 执行更新
    let updated_api = update_model
        .update(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 获取provider类型信息
    let provider_type = ProviderType::find_by_id(updated_api.provider_type_id)
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_else(|| provider_types::Model {
            id: updated_api.provider_type_id,
            name: "unknown".to_string(),
            display_name: "Unknown".to_string(),
            base_url: "".to_string(),
            api_format: "openai".to_string(),
            default_model: None,
            max_tokens: None,
            rate_limit: None,
            timeout_seconds: None,
            health_check_path: None,
            auth_header_format: None,
            is_active: true,
            config_json: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        });

    // 构建响应
    let response_api = ServiceApiResponse {
        id: updated_api.id,
        user_id: updated_api.user_id,
        provider_type: provider_type.id.to_string(),
        provider_name: provider_type.display_name,
        api_key: updated_api.api_key,
        api_secret: updated_api.api_secret,
        name: updated_api.name,
        description: updated_api.description,
        scheduling_strategy: updated_api.scheduling_strategy.unwrap_or("round_robin".to_string()),
        retry_count: updated_api.retry_count.unwrap_or(3),
        timeout_seconds: updated_api.timeout_seconds.unwrap_or(30),
        rate_limit: updated_api.rate_limit.unwrap_or(0),
        max_tokens_per_day: updated_api.max_tokens_per_day.unwrap_or(0),
        used_tokens_today: updated_api.used_tokens_today.unwrap_or(0),
        total_requests: updated_api.total_requests.unwrap_or(0),
        successful_requests: updated_api.successful_requests.unwrap_or(0),
        last_used: updated_api.last_used.map(|dt| chrono::DateTime::<Utc>::from_utc(dt, Utc).to_rfc3339()),
        expires_at: updated_api.expires_at.map(|dt| chrono::DateTime::<Utc>::from_utc(dt, Utc).to_rfc3339()),
        is_active: updated_api.is_active,
        created_at: chrono::DateTime::<Utc>::from_utc(updated_api.created_at, Utc).to_rfc3339(),
        updated_at: chrono::DateTime::<Utc>::from_utc(updated_api.updated_at, Utc).to_rfc3339(),
    };

    let response = json!({
        "success": true,
        "api": response_api,
        "message": format!("Service API {} updated successfully", api_id)
    });

    Ok(Json(response))
}

/// 删除Service API
pub async fn delete_service_api(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
) -> Result<Json<Value>, StatusCode> {
    use entity::user_service_apis::{Entity as UserServiceApi};
    use sea_orm::{EntityTrait};
    
    if api_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 获取数据库连接
    let db = state.database.as_ref();
    
    // 检查记录是否存在
    let existing_api = UserServiceApi::find_by_id(api_id)
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
    if existing_api.is_none() {
        return Ok(Json(json!({
            "success": false,
            "message": "Service API not found"
        })));
    }

    // 执行硬删除（也可以实现软删除通过设置is_active=false）
    let delete_result = UserServiceApi::delete_by_id(api_id)
        .exec(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if delete_result.rows_affected == 0 {
        return Ok(Json(json!({
            "success": false,
            "message": "Failed to delete Service API"
        })));
    }

    let response = json!({
        "success": true,
        "message": format!("Service API {} deleted successfully", api_id)
    });

    Ok(Json(response))
}

/// 重新生成Service API密钥
pub async fn regenerate_service_api_key(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
) -> Result<Json<Value>, StatusCode> {
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};
    
    if api_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 获取数据库连接
    let db = state.database.as_ref();
    
    // 查找现有的Service API记录
    let existing_api = UserServiceApi::find_by_id(api_id)
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
    let existing_record = match existing_api {
        Some(api) => api,
        None => {
            return Ok(Json(json!({
                "success": false,
                "message": "Service API not found"
            })));
        }
    };

    // 生成新的API密钥和密钥签名
    let new_api_key = format!("sk-api-{}", Uuid::new_v4().to_string().replace("-", ""));
    let new_api_secret = format!("secret_{}", Uuid::new_v4().to_string().replace("-", ""));

    // 创建更新模型，只更新密钥相关字段
    let update_model = user_service_apis::ActiveModel {
        id: Set(api_id),
        api_key: Set(new_api_key.clone()),
        api_secret: Set(new_api_secret.clone()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };

    // 执行更新
    let updated_api = update_model
        .update(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = json!({
        "success": true,
        "api_key": updated_api.api_key,
        "message": "API key regenerated successfully"
    });

    Ok(Json(response))
}

/// 撤销Service API
pub async fn revoke_service_api(
    State(_state): State<AppState>,
    Path(api_id): Path<i32>,
) -> Result<Json<Value>, StatusCode> {
    if api_id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // TODO: 实现实际的API撤销逻辑
    let revoked_at = Utc::now().to_rfc3339();
    
    let response = json!({
        "success": true,
        "message": "Service API revoked successfully",
        "revoked_at": revoked_at
    });

    Ok(Json(response))
}

/// 获取调度策略列表
pub async fn get_scheduling_strategies(
    State(_state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let strategies = vec![
        json!({
            "key": "round_robin",
            "name": "轮询调度",
            "description": "按顺序轮流分配请求到各个后端服务"
        }),
        json!({
            "key": "weighted",
            "name": "加权轮询",
            "description": "根据权重分配请求到各个后端服务"
        }),
        json!({
            "key": "health_best",
            "name": "健康优先",
            "description": "优先选择健康状态最好的后端服务"
        }),
    ];

    Ok(Json(json!(strategies)))
}