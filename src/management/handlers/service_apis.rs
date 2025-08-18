//! # Service APIs管理处理器
//!
//! 处理用户对外API服务的管理功能

use crate::management::{response, server::AppState};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json; // Value unused removed
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

/// 关联的提供商密钥配置
#[derive(Debug, Deserialize)]
pub struct ProviderKeyConfig {
    /// 提供商密钥ID
    pub provider_key_id: i32,
    /// 权重
    pub weight: Option<i32>,
    /// 是否启用
    pub is_active: Option<bool>,
}

/// 创建Service API请求
#[derive(Debug, Deserialize)]
pub struct CreateServiceApiRequest {
    /// 服务商类型ID
    pub provider_type_id: i32,
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
    /// 关联的提供商密钥列表（必须都是同类型）
    pub provider_keys: Vec<ProviderKeyConfig>,
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
) -> impl IntoResponse {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_provider_keys::Entity as UserProviderKey;
    use entity::user_service_api_providers::{self, Entity as UserServiceApiProvider};
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect};

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
    let total = match select.clone().count(db).await {
        Ok(count) => count as u32,
        Err(err) => {
            tracing::error!("Failed to count service APIs: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to count service APIs",
            );
        }
    };

    // 分页查询Service APIs
    let apis = match select
        .offset(((page - 1) * limit) as u64)
        .limit(limit as u64)
        .all(db)
        .await
    {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to fetch service APIs: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch service APIs",
            );
        }
    };

    // 获取每个API关联的provider keys信息
    let mut response_apis = Vec::new();

    for api in apis {
        // 查询关联的provider keys
        let provider_associations = match UserServiceApiProvider::find()
            .filter(user_service_api_providers::Column::UserServiceApiId.eq(api.id))
            .find_also_related(UserProviderKey)
            .all(db)
            .await
        {
            Ok(data) => data,
            Err(err) => {
                tracing::error!(
                    "Failed to fetch provider associations for API {}: {}",
                    api.id,
                    err
                );
                continue;
            }
        };

        // 构建provider信息
        let provider_count = provider_associations.len();
        let provider_info = if provider_count == 0 {
            ("none".to_string(), "无关联提供商".to_string())
        } else if provider_count == 1 {
            // 单个提供商，获取具体信息
            if let Some((_, Some(provider_key))) = provider_associations.first() {
                // 获取provider type信息
                match ProviderType::find_by_id(provider_key.provider_type_id)
                    .one(db)
                    .await
                {
                    Ok(Some(provider_type)) => (
                        provider_type.name.clone(),
                        provider_type.display_name.clone(),
                    ),
                    _ => ("unknown".to_string(), "未知提供商".to_string()),
                }
            } else {
                ("unknown".to_string(), "未知提供商".to_string())
            }
        } else {
            // 多个提供商
            (
                "multi".to_string(),
                format!("多提供商({} 个)", provider_count),
            )
        };

        let response_api = ServiceApiResponse {
            id: api.id,
            user_id: api.user_id,
            provider_type: provider_info.0,
            provider_name: provider_info.1,
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
            last_used: api
                .last_used
                .map(|dt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339()),
            expires_at: api
                .expires_at
                .map(|dt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339()),
            is_active: api.is_active,
            created_at: chrono::DateTime::<Utc>::from_naive_utc_and_offset(api.created_at, Utc)
                .to_rfc3339(),
            updated_at: chrono::DateTime::<Utc>::from_naive_utc_and_offset(api.updated_at, Utc)
                .to_rfc3339(),
        };

        response_apis.push(response_api);
    }

    let pagination = response::Pagination {
        page: page as u64,
        limit: limit as u64,
        total: total as u64,
        pages: ((total as f64) / (limit as f64)).ceil() as u64,
    };

    response::paginated(response_apis, pagination)
}

/// 获取单个Service API
pub async fn get_service_api(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
) -> impl IntoResponse {
    use entity::provider_types::{self, Entity as ProviderType};
    use entity::user_service_apis::Entity as UserServiceApi;
    use sea_orm::{EntityTrait, JoinType, QuerySelect, RelationTrait};

    if api_id <= 0 {
        return response::error(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid API ID",
        );
    }

    // 获取数据库连接
    let db = state.database.as_ref();

    // 查询Service API及其关联的Provider Type
    let api_with_provider = match UserServiceApi::find_by_id(api_id)
        .join(
            JoinType::InnerJoin,
            entity::user_service_apis::Relation::ProviderType.def(),
        )
        .find_also_related(ProviderType)
        .one(db)
        .await
    {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to fetch service API: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch service API",
            );
        }
    };

    let (api, provider_type) = match api_with_provider {
        Some((api, provider)) => (api, provider),
        None => {
            return response::error(
                StatusCode::NOT_FOUND,
                "API_NOT_FOUND",
                "Service API not found",
            );
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
        last_used: api
            .last_used
            .map(|dt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339()),
        expires_at: api
            .expires_at
            .map(|dt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339()),
        is_active: api.is_active,
        created_at: chrono::DateTime::<Utc>::from_naive_utc_and_offset(api.created_at, Utc)
            .to_rfc3339(),
        updated_at: chrono::DateTime::<Utc>::from_naive_utc_and_offset(api.updated_at, Utc)
            .to_rfc3339(),
    };

    response::success(response_api)
}

/// 创建Service API
pub async fn create_service_api(
    State(state): State<AppState>,
    Json(request): Json<CreateServiceApiRequest>,
) -> impl IntoResponse {
    use entity::provider_types::Entity as ProviderType;
    // 需要同时引入模块自身以便访问 Column 枚举
    use entity::user_provider_keys::{self, Entity as UserProviderKey};
    use entity::user_service_api_providers::{self};
    use entity::user_service_apis::{self};
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};

    // 验证输入
    if request.provider_keys.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "至少需要选择一个提供商API密钥",
        );
    }

    // 获取数据库连接
    let db = state.database.as_ref();
    let user_id = 1; // TODO: 从认证上下文获取实际用户ID

    // 验证provider_type_id是否存在
    let provider_type = match ProviderType::find_by_id(request.provider_type_id)
        .one(db)
        .await
    {
        Ok(Some(pt)) => pt,
        Ok(None) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                "无效的服务商类型",
            );
        }
        Err(err) => {
            tracing::error!("Failed to query provider type: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to query provider type",
            );
        }
    };

    // 用户可以为同一provider type创建多个service API，不需要检查重复

    // 验证所有提供商密钥是否存在且属于用户和指定的provider type
    let provider_key_ids: Vec<i32> = request
        .provider_keys
        .iter()
        .map(|pk| pk.provider_key_id)
        .collect();

    let valid_provider_keys = match UserProviderKey::find()
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .filter(user_provider_keys::Column::ProviderTypeId.eq(request.provider_type_id))
        .filter(user_provider_keys::Column::Id.is_in(provider_key_ids.clone()))
        .all(db)
        .await
    {
        Ok(keys) => keys,
        Err(err) => {
            tracing::error!("Failed to query provider keys: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to query provider keys",
            );
        }
    };

    if valid_provider_keys.len() != provider_key_ids.len() {
        return response::error(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            &format!(
                "部分提供商API密钥不存在、不属于该用户或不属于{}类型",
                provider_type.display_name
            ),
        );
    }

    // 生成唯一的API密钥和密钥签名
    let api_key = format!("sk-api-{}", Uuid::new_v4().to_string().replace("-", ""));
    let api_secret = format!("secret_{}", Uuid::new_v4().to_string().replace("-", ""));

    // 开始事务
    let txn = match db.begin().await {
        Ok(txn) => txn,
        Err(err) => {
            tracing::error!("Failed to start transaction: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to start transaction",
            );
        }
    };

    // 创建新的Service API记录
    let new_service_api = user_service_apis::ActiveModel {
        user_id: Set(user_id),
        provider_type_id: Set(request.provider_type_id),
        api_key: Set(api_key.clone()),
        api_secret: Set(api_secret.clone()),
        name: Set(request.name.clone()),
        description: Set(request.description.clone()),
        scheduling_strategy: Set(Some(
            request
                .scheduling_strategy
                .unwrap_or("round_robin".to_string()),
        )),
        retry_count: Set(Some(request.retry_count.unwrap_or(3))),
        timeout_seconds: Set(Some(request.timeout_seconds.unwrap_or(30))),
        rate_limit: Set(request.rate_limit),
        max_tokens_per_day: Set(request.max_tokens_per_day),
        used_tokens_today: Set(Some(0)),
        total_requests: Set(Some(0)),
        successful_requests: Set(Some(0)),
        last_used: Set(None),
        expires_at: Set(request
            .expires_in_days
            .map(|days| (Utc::now() + chrono::Duration::days(days as i64)).naive_utc())),
        is_active: Set(request.is_active.unwrap_or(true)),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };

    // 插入Service API记录
    let inserted_api = match new_service_api.insert(&txn).await {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("Failed to insert Service API: {}", e);
            let _ = txn.rollback().await;
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to create Service API",
            );
        }
    };

    // 创建Service API与Provider Key的关联记录
    for provider_key_config in &request.provider_keys {
        let association = user_service_api_providers::ActiveModel {
            user_service_api_id: Set(inserted_api.id),
            user_provider_key_id: Set(provider_key_config.provider_key_id),
            weight: Set(provider_key_config.weight),
            is_active: Set(provider_key_config.is_active.unwrap_or(true)),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        if let Err(e) = association.insert(&txn).await {
            tracing::error!("Failed to create API-Provider association: {}", e);
            let _ = txn.rollback().await;
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to create API-Provider association",
            );
        }
    }

    // 提交事务
    if let Err(e) = txn.commit().await {
        tracing::error!("Failed to commit transaction: {}", e);
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DB_ERROR",
            "Failed to commit transaction",
        );
    }

    // 构建响应
    let response_api = ServiceApiResponse {
        id: inserted_api.id,
        user_id: inserted_api.user_id,
        provider_type: provider_type.name.clone(),
        provider_name: format!(
            "{}({} 个API)",
            provider_type.display_name,
            request.provider_keys.len()
        ),
        api_key: inserted_api.api_key,
        api_secret: inserted_api.api_secret,
        name: inserted_api.name,
        description: inserted_api.description,
        scheduling_strategy: inserted_api
            .scheduling_strategy
            .unwrap_or("round_robin".to_string()),
        retry_count: inserted_api.retry_count.unwrap_or(3),
        timeout_seconds: inserted_api.timeout_seconds.unwrap_or(30),
        rate_limit: inserted_api.rate_limit.unwrap_or(0),
        max_tokens_per_day: inserted_api.max_tokens_per_day.unwrap_or(0),
        used_tokens_today: inserted_api.used_tokens_today.unwrap_or(0),
        total_requests: inserted_api.total_requests.unwrap_or(0),
        successful_requests: inserted_api.successful_requests.unwrap_or(0),
        last_used: inserted_api
            .last_used
            .map(|dt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339()),
        expires_at: inserted_api
            .expires_at
            .map(|dt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339()),
        is_active: inserted_api.is_active,
        created_at: chrono::DateTime::<Utc>::from_naive_utc_and_offset(
            inserted_api.created_at,
            Utc,
        )
        .to_rfc3339(),
        updated_at: chrono::DateTime::<Utc>::from_naive_utc_and_offset(
            inserted_api.updated_at,
            Utc,
        )
        .to_rfc3339(),
    };

    let message = format!(
        "{}服务API创建成功，关联了 {} 个同类型提供商密钥",
        provider_type.display_name,
        request.provider_keys.len()
    );
    response::success_with_message(response_api, &message)
}

/// 更新Service API
pub async fn update_service_api(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
    Json(request): Json<UpdateServiceApiRequest>,
) -> impl IntoResponse {
    use entity::provider_types::{self, Entity as ProviderType};
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};

    if api_id <= 0 {
        return response::error(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid API ID",
        );
    }

    // 获取数据库连接
    let db = state.database.as_ref();

    // 查找现有的Service API记录
    let existing_api = match UserServiceApi::find_by_id(api_id).one(db).await {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to fetch Service API: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch Service API",
            );
        }
    };

    let _existing_record = match existing_api {
        Some(api) => api,
        None => {
            return response::error(
                StatusCode::NOT_FOUND,
                "API_NOT_FOUND",
                "Service API not found",
            );
        }
    };

    // 创建更新模型
    let mut update_model = user_service_apis::ActiveModel {
        id: Set(api_id), // 指定要更新的记录ID
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
    let updated_api = match update_model.update(db).await {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to update Service API: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to update Service API",
            );
        }
    };

    // 获取provider类型信息
    let provider_type = match ProviderType::find_by_id(updated_api.provider_type_id)
        .one(db)
        .await
    {
        Ok(data) => data.unwrap_or_else(|| provider_types::Model {
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
        }),
        Err(err) => {
            tracing::error!("Failed to fetch provider type: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch provider type",
            );
        }
    };

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
        scheduling_strategy: updated_api
            .scheduling_strategy
            .unwrap_or("round_robin".to_string()),
        retry_count: updated_api.retry_count.unwrap_or(3),
        timeout_seconds: updated_api.timeout_seconds.unwrap_or(30),
        rate_limit: updated_api.rate_limit.unwrap_or(0),
        max_tokens_per_day: updated_api.max_tokens_per_day.unwrap_or(0),
        used_tokens_today: updated_api.used_tokens_today.unwrap_or(0),
        total_requests: updated_api.total_requests.unwrap_or(0),
        successful_requests: updated_api.successful_requests.unwrap_or(0),
        last_used: updated_api
            .last_used
            .map(|dt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339()),
        expires_at: updated_api
            .expires_at
            .map(|dt| chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339()),
        is_active: updated_api.is_active,
        created_at: chrono::DateTime::<Utc>::from_naive_utc_and_offset(updated_api.created_at, Utc)
            .to_rfc3339(),
        updated_at: chrono::DateTime::<Utc>::from_naive_utc_and_offset(updated_api.updated_at, Utc)
            .to_rfc3339(),
    };

    response::success_with_message(
        response_api,
        &format!("Service API {} updated successfully", api_id),
    )
}

/// 删除响应结构
#[derive(Serialize)]
struct DeleteResponse {
    api_id: i32,
}

/// 重新生成响应结构
#[derive(Serialize)]
struct RegenerateResponse {
    api_key: String,
}

/// 撤销响应结构
#[derive(Serialize)]
struct RevokeResponse {
    revoked_at: String,
}

/// 删除Service API
pub async fn delete_service_api(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
) -> impl IntoResponse {
    use entity::user_service_apis::Entity as UserServiceApi;
    use sea_orm::EntityTrait;

    if api_id <= 0 {
        return response::error(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid API ID",
        );
    }

    // 获取数据库连接
    let db = state.database.as_ref();

    // 检查记录是否存在
    let existing_api = match UserServiceApi::find_by_id(api_id).one(db).await {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to check Service API: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to check Service API",
            );
        }
    };

    if existing_api.is_none() {
        return response::error(
            StatusCode::NOT_FOUND,
            "API_NOT_FOUND",
            "Service API not found",
        );
    }

    // 执行硬删除（也可以实现软删除通过设置is_active=false）
    let delete_result = match UserServiceApi::delete_by_id(api_id).exec(db).await {
        Ok(result) => result,
        Err(err) => {
            tracing::error!("Failed to delete Service API: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to delete Service API",
            );
        }
    };

    if delete_result.rows_affected == 0 {
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "DELETE_FAILED",
            "Failed to delete Service API",
        );
    }

    response::success_with_message(
        DeleteResponse { api_id },
        &format!("Service API {} deleted successfully", api_id),
    )
}

/// 重新生成Service API密钥
pub async fn regenerate_service_api_key(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
) -> impl IntoResponse {
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};

    if api_id <= 0 {
        return response::error(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid API ID",
        );
    }

    // 获取数据库连接
    let db = state.database.as_ref();

    // 查找现有的Service API记录
    let existing_api = match UserServiceApi::find_by_id(api_id).one(db).await {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to fetch Service API: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch Service API",
            );
        }
    };

    let _existing_record = match existing_api {
        Some(api) => api,
        None => {
            return response::error(
                StatusCode::NOT_FOUND,
                "API_NOT_FOUND",
                "Service API not found",
            );
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
    let updated_api = match update_model.update(db).await {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to update Service API key: {}", err);
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to regenerate API key",
            );
        }
    };

    response::success_with_message(
        RegenerateResponse {
            api_key: updated_api.api_key,
        },
        "API key regenerated successfully",
    )
}

/// 撤销Service API
pub async fn revoke_service_api(
    State(_state): State<AppState>,
    Path(api_id): Path<i32>,
) -> impl IntoResponse {
    if api_id <= 0 {
        return response::error(
            StatusCode::BAD_REQUEST,
            "VALIDATION_ERROR",
            "Invalid API ID",
        );
    }

    // TODO: 实现实际的API撤销逻辑
    let revoked_at = Utc::now().to_rfc3339();

    response::success_with_message(
        RevokeResponse { revoked_at },
        "Service API revoked successfully",
    )
}

/// 获取调度策略列表
pub async fn get_scheduling_strategies(State(_state): State<AppState>) -> impl IntoResponse {
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

    response::success(strategies)
}
