//! # 用户服务API管理处理器
//!
//! 处理用户API密钥管理功能，包括创建、编辑、统计等

#![allow(clippy::used_underscore_binding)]

use crate::lerror;
use crate::logging::{LogComponent, LogStage};
use crate::management::middleware::auth::AuthContext;
use crate::management::{response, server::AppState};
use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::QueryOrder; // for order_by()
use sea_orm::prelude::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

/// 用户服务API查询参数
#[derive(Debug, Deserialize)]
pub struct UserServiceKeyQuery {
    /// 页码
    pub page: Option<u32>,
    /// 每页大小
    pub limit: Option<u32>,
    /// 密钥名称筛选
    pub name: Option<String>,
    /// 描述筛选
    pub description: Option<String>,
    /// 服务类型筛选
    pub provider_type_id: Option<i32>,
    /// 状态筛选
    pub is_active: Option<bool>,
}

/// 创建用户服务API请求
#[derive(Debug, Deserialize)]
pub struct CreateUserServiceKeyRequest {
    /// API Key名称
    pub name: String,
    /// 描述信息
    pub description: Option<String>,
    /// 服务商类型ID
    pub provider_type_id: i32,
    /// 关联的提供商密钥ID列表
    pub user_provider_keys_ids: Vec<i32>,
    /// 调度策略
    pub scheduling_strategy: Option<String>,
    /// 重试次数
    pub retry_count: Option<i32>,
    /// 超时时间(秒)
    pub timeout_seconds: Option<i32>,
    /// 每分钟最大请求数
    pub max_request_per_min: Option<i32>,
    /// 每日最大请求数
    pub max_requests_per_day: Option<i32>,
    /// 每日最大Token数（单位：token，支持大整数）
    pub max_tokens_per_day: Option<i64>,
    /// 每日最大费用
    pub max_cost_per_day: Option<Decimal>,
    /// 过期时间(ISO 8601格式)
    pub expires_at: Option<String>,
    /// 是否启用
    pub is_active: Option<bool>,
}

/// 更新用户服务API请求
#[derive(Debug, Deserialize)]
pub struct UpdateUserServiceKeyRequest {
    /// API Key名称
    pub name: Option<String>,
    /// 描述信息
    pub description: Option<String>,
    /// 关联的提供商密钥ID列表
    pub user_provider_keys_ids: Option<Vec<i32>>,
    /// 调度策略
    pub scheduling_strategy: Option<String>,
    /// 重试次数
    pub retry_count: Option<i32>,
    /// 超时时间(秒)
    pub timeout_seconds: Option<i32>,
    /// 每分钟最大请求数
    pub max_request_per_min: Option<i32>,
    /// 每日最大请求数
    pub max_requests_per_day: Option<i32>,
    /// 每日最大Token数（单位：token，支持大整数）
    pub max_tokens_per_day: Option<i64>,
    /// 每日最大费用
    pub max_cost_per_day: Option<Decimal>,
    /// 过期时间(ISO 8601格式)
    pub expires_at: Option<String>,
}

/// 使用统计查询参数
#[derive(Debug, Deserialize)]
pub struct UsageStatsQuery {
    /// 时间范围
    pub time_range: Option<String>,
    /// 自定义开始日期
    pub start_date: Option<String>,
    /// 自定义结束日期
    pub end_date: Option<String>,
}

/// 状态更新请求
#[derive(Debug, Deserialize)]
pub struct UpdateStatusRequest {
    /// 启用状态
    pub is_active: bool,
}

/// 用户API Keys卡片响应
#[derive(Debug, Serialize)]
pub struct UserServiceCardsResponse {
    /// 总API Key数量
    pub total_api_keys: i32,
    /// 活跃API Key数量
    pub active_api_keys: i32,
    /// 总请求数
    pub requests: i64,
}

/// 用户服务API响应
#[derive(Debug, Serialize)]
pub struct UserServiceKeyResponse {
    /// API Key ID
    pub id: i32,
    /// API Key名称
    pub name: String,
    /// 描述
    pub description: Option<String>,
    /// 服务商
    pub provider: String,
    /// 服务商类型ID
    pub provider_type_id: i32,
    /// API密钥(脱敏)
    pub api_key: String,
    /// 使用统计
    pub usage: Option<serde_json::Value>,
    /// 是否启用
    pub is_active: bool,
    /// 最后使用时间
    pub last_used_at: Option<String>,
    /// 创建时间
    pub created_at: String,
    /// 过期时间
    pub expires_at: Option<String>,
    /// 调度策略
    pub scheduling_strategy: Option<String>,
    /// 重试次数
    pub retry_count: Option<i32>,
    /// 超时时间(秒)
    pub timeout_seconds: Option<i32>,
    /// 每分钟最大请求数
    pub max_request_per_min: Option<i32>,
    /// 每日最大请求数
    pub max_requests_per_day: Option<i32>,
    /// 每日最大Token数
    pub max_tokens_per_day: Option<i64>,
    /// 每日最大费用
    pub max_cost_per_day: Option<Decimal>,
}

/// 详细的API Key响应
#[derive(Debug, Serialize)]
pub struct UserServiceKeyDetailResponse {
    /// API Key ID
    pub id: i32,
    /// API Key名称
    pub name: String,
    /// 描述
    pub description: Option<String>,
    /// 服务商类型ID
    pub provider_type_id: i32,
    /// 服务商
    pub provider: String,
    /// API密钥(脱敏)
    pub api_key: String,
    /// 关联的提供商密钥ID列表
    pub user_provider_keys_ids: Vec<i32>,
    /// 调度策略
    pub scheduling_strategy: Option<String>,
    /// 重试次数
    pub retry_count: Option<i32>,
    /// 超时时间(秒)
    pub timeout_seconds: Option<i32>,
    /// 每分钟最大请求数
    pub max_request_per_min: Option<i32>,
    /// 每日最大请求数
    pub max_requests_per_day: Option<i32>,
    /// 每日最大Token数（单位：token，支持大整数）
    pub max_tokens_per_day: Option<i64>,
    /// 每日最大费用
    pub max_cost_per_day: Option<Decimal>,
    /// 过期时间
    pub expires_at: Option<String>,
    /// 是否启用
    pub is_active: bool,
    /// 创建时间
    pub created_at: String,
    /// 更新时间
    pub updated_at: String,
}

/// 1. 用户API Keys卡片展示
pub async fn get_user_service_cards(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::proxy_tracing::{self, Entity as ProxyTracing};
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

    let db = state.database.as_ref();

    let user_id = auth_context.user_id;

    // 获取总API Key数量
    let total_api_keys = match UserServiceApi::find()
        .filter(user_service_apis::Column::UserId.eq(user_id))
        .count(db)
        .await
    {
        Ok(count) => i32::try_from(count).unwrap_or(i32::MAX),
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "count_service_apis_fail",
                &format!("Failed to count user service APIs: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to count user service APIs: {}",
                err
            ));
        }
    };

    // 获取活跃API Key数量
    let active_api_keys = match UserServiceApi::find()
        .filter(user_service_apis::Column::UserId.eq(user_id))
        .filter(user_service_apis::Column::IsActive.eq(true))
        .count(db)
        .await
    {
        Ok(count) => i32::try_from(count).unwrap_or(i32::MAX),
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "count_active_service_apis_fail",
                &format!("Failed to count active user service APIs: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to count active user service APIs: {}",
                err
            ));
        }
    };

    // 获取总请求数
    let total_requests = match ProxyTracing::find()
        .filter(proxy_tracing::Column::UserId.eq(user_id))
        .count(db)
        .await
    {
        #[allow(clippy::cast_possible_wrap)]
        Ok(count) => count as i64,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "count_user_requests_fail",
                &format!("Failed to count user requests: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to count user requests: {}",
                err
            ));
        }
    };

    let response = UserServiceCardsResponse {
        total_api_keys,
        active_api_keys,
        requests: total_requests,
    };

    response::success(response)
}

/// 2. 用户API Keys列表
pub async fn list_user_service_keys(
    State(state): State<AppState>,
    Query(query): Query<UserServiceKeyQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::provider_types::Entity as ProviderType;
    use entity::proxy_tracing::{self, Entity as ProxyTracing};
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, QueryTrait};

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(10);
    let db = state.database.as_ref();

    let user_id = auth_context.user_id;

    // 构建基础查询
    let mut select = UserServiceApi::find().filter(user_service_apis::Column::UserId.eq(user_id));

    // 应用筛选条件
    if let Some(name) = &query.name {
        select = select.filter(user_service_apis::Column::Name.like(format!("%{name}%")));
    }

    if let Some(description) = &query.description {
        select =
            select.filter(user_service_apis::Column::Description.like(format!("%{description}%")));
    }

    if let Some(provider_type_id) = query.provider_type_id {
        select = select.filter(user_service_apis::Column::ProviderTypeId.eq(provider_type_id));
    }

    if let Some(is_active) = query.is_active {
        select = select.filter(user_service_apis::Column::IsActive.eq(is_active));
    }

    // 获取总数
    let total = match select.clone().count(db).await {
        Ok(count) => count,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "count_service_apis_fail",
                &format!("Failed to count user service APIs: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to count user service APIs: {}",
                err
            ));
        }
    };

    // 分页查询，使用手动JOIN避免重复
    let apis = match UserServiceApi::find()
        .filter(user_service_apis::Column::UserId.eq(user_id))
        .apply_if(query.name, |query, name| {
            query.filter(user_service_apis::Column::Name.like(format!("%{name}%")))
        })
        .apply_if(query.description, |query, description| {
            query.filter(user_service_apis::Column::Description.like(format!("%{description}%")))
        })
        .apply_if(query.provider_type_id, |query, provider_type_id| {
            query.filter(user_service_apis::Column::ProviderTypeId.eq(provider_type_id))
        })
        .apply_if(query.is_active, |query, is_active| {
            query.filter(user_service_apis::Column::IsActive.eq(is_active))
        })
        .find_also_related(ProviderType)
        .offset(u64::from((page - 1) * limit))
        .limit(u64::from(limit))
        .all(db)
        .await
    {
        Ok(data) => data,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_service_apis_fail",
                &format!("Failed to fetch user service APIs: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch user service APIs: {}",
                err
            ));
        }
    };

    // 构建响应数据
    let mut service_api_keys = Vec::new();

    for (api, provider_type) in apis {
        // 获取完整的使用统计
        let tracings = ProxyTracing::find()
            .filter(proxy_tracing::Column::UserServiceApiId.eq(api.id))
            .all(db)
            .await
            .unwrap_or_default();

        let success_count = i32::try_from(tracings.iter().filter(|t| t.is_success).count()).unwrap_or(i32::MAX);
        let failure_count = i32::try_from(tracings.len()).unwrap_or(i32::MAX) - success_count;
        let total_requests = success_count + failure_count;

        // 计算成功率
        let success_rate = if total_requests > 0 {
            (f64::from(success_count) / f64::from(total_requests)) * 100.0
        } else {
            0.0
        };

        // 计算平均响应时间
        let total_response_time: i64 = tracings.iter().filter_map(|t| t.duration_ms).sum();
        let avg_response_time = if success_count > 0 {
            total_response_time / i64::from(success_count)
        } else {
            0
        };

        // 计算总成本
        let total_cost: f64 = tracings.iter().filter_map(|t| t.cost).sum();

        // 计算总token数
        let total_tokens: i32 = tracings.iter().filter_map(|t| t.tokens_total).sum();

        // 获取最后使用时间
        let last_used_at = match ProxyTracing::find()
            .filter(proxy_tracing::Column::UserServiceApiId.eq(api.id))
            .order_by(proxy_tracing::Column::CreatedAt, sea_orm::Order::Desc)
            .one(db)
            .await
        {
            Ok(Some(tracing)) => Some(
                DateTime::<Utc>::from_naive_utc_and_offset(tracing.created_at, Utc).to_rfc3339(),
            ),
            _ => None,
        };

        let usage = json!({
            "successful_requests": success_count,
            "failed_requests": failure_count,
            "total_requests": total_requests,
            "success_rate": success_rate,
            "avg_response_time": avg_response_time,
            "total_cost": total_cost,
            "total_tokens": total_tokens,
            "last_used_at": last_used_at
        });

        let provider_name = provider_type
            .as_ref()
            .map_or("Unknown".to_string(), |pt| pt.display_name.clone());

        // API Key脱敏处理
        /* let masked_api_key = if api.api_key.len() > 8 {
            format!(
                "{}****{}",
                &api.api_key[..4],
                &api.api_key[api.api_key.len() - 4..]
            )
        } else {
            "****".to_string()
        }; */

        let response_api = UserServiceKeyResponse {
            id: api.id,
            name: api.name.unwrap_or(String::new()),
            description: api.description,
            provider: provider_name,
            provider_type_id: api.provider_type_id,
            api_key: api.api_key,
            usage: Some(usage),
            is_active: api.is_active,
            last_used_at,
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(api.created_at, Utc)
                .to_rfc3339(),
            expires_at: api
                .expires_at
                .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339()),
            scheduling_strategy: api.scheduling_strategy,
            retry_count: api.retry_count,
            timeout_seconds: api.timeout_seconds,
            max_request_per_min: api.max_request_per_min,
            max_requests_per_day: api.max_requests_per_day,
            max_tokens_per_day: api.max_tokens_per_day,
            max_cost_per_day: api.max_cost_per_day,
        };

        service_api_keys.push(response_api);
    }

    let pagination = response::Pagination {
        page: u64::from(page),
        limit: u64::from(limit),
        total,
        pages: ((f64::from(total.try_into().unwrap_or(u32::MAX))) / f64::from(limit)).ceil() as u64,
    };

    let data = json!({
        "service_api_keys": service_api_keys,
        "pagination": pagination
    });

    response::success(data)
}

/// 3. 新增API Key
pub async fn create_user_service_key(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<CreateUserServiceKeyRequest>,
) -> axum::response::Response {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_provider_keys::{self, Entity as UserProviderKey};
    use entity::user_service_apis::{self};
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

    // 验证输入
    if request.user_provider_keys_ids.is_empty() {
        return crate::manage_error!(crate::proxy_err!(business, "至少需要选择一个提供商API密钥"));
    }

    let db = state.database.as_ref();

    let user_id = auth_context.user_id;

    // 验证provider_type_id是否存在
    let provider_type = match ProviderType::find_by_id(request.provider_type_id)
        .one(db)
        .await
    {
        Ok(Some(pt)) => pt,
        Ok(None) => {
            return crate::manage_error!(crate::proxy_err!(business, "无效的服务商类型"));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "query_provider_type_fail",
                &format!("Failed to query provider type: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to query provider type: {}",
                err
            ));
        }
    };

    // 验证所有提供商密钥是否存在且属于用户和指定的provider type
    let valid_provider_keys = match UserProviderKey::find()
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .filter(user_provider_keys::Column::ProviderTypeId.eq(request.provider_type_id))
        .filter(user_provider_keys::Column::Id.is_in(request.user_provider_keys_ids.clone()))
        .all(db)
        .await
    {
        Ok(keys) => keys,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "query_provider_keys_fail",
                &format!("Failed to query provider keys: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to query provider keys: {}",
                err
            ));
        }
    };

    if valid_provider_keys.len() != request.user_provider_keys_ids.len() {
        return crate::manage_error!(crate::proxy_err!(
            business,
            "部分提供商API密钥不存在、不属于该用户或不属于{}类型",
            provider_type.display_name
        ));
    }

    // 生成唯一的API密钥
    let api_key = format!("sk-usr-{}", Uuid::new_v4().to_string().replace('-', ""));

    // 解析过期时间
    let expires_at = if let Some(expires_str) = &request.expires_at {
        match chrono::DateTime::parse_from_rfc3339(expires_str) {
            Ok(dt) => Some(dt.naive_utc()),
            Err(_) => {
                return crate::manage_error!(crate::proxy_err!(
                    business,
                    "过期时间格式错误，请使用ISO 8601格式"
                ));
            }
        }
    } else {
        None
    };

    // 创建新的用户服务API记录
    let new_service_api = user_service_apis::ActiveModel {
        user_id: Set(user_id),
        provider_type_id: Set(request.provider_type_id),
        api_key: Set(api_key.clone()),
        name: Set(Some(request.name.clone())),
        description: Set(request.description.clone()),
        user_provider_keys_ids: Set(serde_json::to_value(&request.user_provider_keys_ids)
            .map_err(|e| {
                lerror!(
                    "system",
                    LogStage::Internal,
                    LogComponent::Database,
                    "serialize_ids_fail",
                    &format!("Failed to serialize user_provider_keys_ids: {e}")
                );
                e
            })
            .unwrap_or(serde_json::Value::Array(vec![]))),
        scheduling_strategy: Set(request.scheduling_strategy.clone()),
        retry_count: Set(request.retry_count),
        timeout_seconds: Set(request.timeout_seconds),
        max_request_per_min: Set(request.max_request_per_min),
        max_requests_per_day: Set(request.max_requests_per_day),
        max_tokens_per_day: Set(request.max_tokens_per_day),
        max_cost_per_day: Set(request.max_cost_per_day),
        expires_at: Set(expires_at),
        is_active: Set(request.is_active.unwrap_or(true)),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };

    // 插入记录
    let inserted_api = match new_service_api.insert(db).await {
        Ok(data) => data,
        Err(e) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "insert_service_api_fail",
                &format!("Failed to insert user service API: {e}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to create API Key: {}",
                e
            ));
        }
    };

    // 构建响应
    let data = json!({
        "id": inserted_api.id,
        "api_key": inserted_api.api_key,
        "name": inserted_api.name,
        "description": inserted_api.description,
        "provider_type_id": inserted_api.provider_type_id,
        "is_active": inserted_api.is_active,
        "created_at": DateTime::<Utc>::from_naive_utc_and_offset(inserted_api.created_at, Utc).to_rfc3339()
    });

    response::success_with_message(data, "API Key创建成功")
}

/// 4. 获取API Key详情
pub async fn get_user_service_key(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    if api_id <= 0 {
        return crate::manage_error!(crate::proxy_err!(business, "Invalid API ID"));
    }

    let db = state.database.as_ref();

    let user_id = auth_context.user_id;

    // 查询API Key（确保属于当前用户）
    let api = match UserServiceApi::find_by_id(api_id)
        .filter(user_service_apis::Column::UserId.eq(user_id))
        .one(db)
        .await
    {
        Ok(Some(api)) => api,
        Ok(None) => {
            return crate::manage_error!(crate::proxy_err!(
                business,
                "API Key not found: {}",
                api_id
            ));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_service_apis_fail",
                &format!("Failed to fetch user service API: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch API Key: {}",
                err
            ));
        }
    };

    // 获取provider type信息
    let provider_type = match ProviderType::find_by_id(api.provider_type_id).one(db).await {
        Ok(Some(pt)) => pt,
        Ok(None) => {
            return crate::manage_error!(crate::proxy_err!(
                business,
                "Provider type not found: {}",
                api.provider_type_id
            ));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "query_provider_type_fail",
                &format!("Failed to fetch provider type: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch provider type: {}",
                err
            ));
        }
    };

    /* // API Key脱敏处理
    let masked_api_key = if api.api_key.len() > 8 {
        format!(
            "{}****{}",
            &api.api_key[..4],
            &api.api_key[api.api_key.len() - 4..]
        )
    } else {
        "****".to_string()
    }; */

    let response = UserServiceKeyDetailResponse {
        id: api.id,
        name: api.name.unwrap_or(String::new()),
        description: api.description,
        provider_type_id: api.provider_type_id,
        provider: provider_type.display_name,
        api_key: api.api_key,
        user_provider_keys_ids: serde_json::from_value::<Vec<i32>>(
            api.user_provider_keys_ids.clone(),
        )
        .unwrap_or_else(|_| vec![]),
        scheduling_strategy: api.scheduling_strategy,
        retry_count: api.retry_count,
        timeout_seconds: api.timeout_seconds,
        max_request_per_min: api.max_request_per_min,
        max_requests_per_day: api.max_requests_per_day,
        max_tokens_per_day: api.max_tokens_per_day,
        max_cost_per_day: api.max_cost_per_day,
        expires_at: api
            .expires_at
            .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339()),
        is_active: api.is_active,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(api.created_at, Utc).to_rfc3339(),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(api.updated_at, Utc).to_rfc3339(),
    };

    response::success(response)
}

/// 5. 编辑API Key
pub async fn update_user_service_key(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<UpdateUserServiceKeyRequest>,
) -> axum::response::Response {
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

    if api_id <= 0 {
        return crate::manage_error!(crate::proxy_err!(business, "Invalid API ID"));
    }

    let db = state.database.as_ref();

    let user_id = auth_context.user_id;

    // 验证API Key存在且属于当前用户
    let _existing_api = match UserServiceApi::find_by_id(api_id)
        .filter(user_service_apis::Column::UserId.eq(user_id))
        .one(db)
        .await
    {
        Ok(Some(api)) => api,
        Ok(None) => {
            return crate::manage_error!(crate::proxy_err!(
                business,
                "API Key not found: {}",
                api_id
            ));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_service_apis_fail",
                &format!("Failed to fetch user service API: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch API Key: {}",
                err
            ));
        }
    };

    // 解析过期时间
    let expires_at = if let Some(expires_str) = &request.expires_at {
        match chrono::DateTime::parse_from_rfc3339(expires_str) {
            Ok(dt) => Some(dt.naive_utc()),
            Err(_) => {
                return crate::manage_error!(crate::proxy_err!(
                    business,
                    "过期时间格式错误，请使用ISO 8601格式"
                ));
            }
        }
    } else {
        _existing_api.expires_at
    };

    // 创建更新模型
    let mut update_model = user_service_apis::ActiveModel {
        id: Set(api_id),
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

    if let Some(user_provider_keys_ids) = request.user_provider_keys_ids {
        update_model.user_provider_keys_ids = Set(serde_json::to_value(&user_provider_keys_ids)
            .unwrap_or(serde_json::Value::Array(vec![])));
    }

    if let Some(scheduling_strategy) = request.scheduling_strategy {
        update_model.scheduling_strategy = Set(Some(scheduling_strategy));
    }

    if let Some(retry_count) = request.retry_count {
        update_model.retry_count = Set(Some(retry_count));
    }

    if let Some(timeout_seconds) = request.timeout_seconds {
        update_model.timeout_seconds = Set(Some(timeout_seconds));
    }

    if let Some(max_request_per_min) = request.max_request_per_min {
        update_model.max_request_per_min = Set(Some(max_request_per_min));
    }

    if let Some(max_requests_per_day) = request.max_requests_per_day {
        update_model.max_requests_per_day = Set(Some(max_requests_per_day));
    }

    if let Some(max_tokens_per_day) = request.max_tokens_per_day {
        update_model.max_tokens_per_day = Set(Some(max_tokens_per_day));
    }

    if let Some(max_cost_per_day) = request.max_cost_per_day {
        update_model.max_cost_per_day = Set(Some(max_cost_per_day));
    }

    update_model.expires_at = Set(expires_at);

    // 执行更新
    let updated_api = match update_model.update(db).await {
        Ok(data) => data,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "update_service_api_fail",
                &format!("Failed to update user service API: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to update API Key: {}",
                err
            ));
        }
    };

    let data = json!({
        "id": updated_api.id,
        "name": updated_api.name,
        "description": updated_api.description,
        "updated_at": DateTime::<Utc>::from_naive_utc_and_offset(updated_api.updated_at, Utc).to_rfc3339()
    });

    response::success_with_message(data, "API Key更新成功")
}

/// 6. 删除API Key
pub async fn delete_user_service_key(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    if api_id <= 0 {
        return crate::manage_error!(crate::proxy_err!(business, "Invalid API ID"));
    }

    let db = state.database.as_ref();
    let user_id = auth_context.user_id;

    // 验证API Key存在且属于当前用户
    let _existing_api = match UserServiceApi::find_by_id(api_id)
        .filter(user_service_apis::Column::UserId.eq(user_id))
        .one(db)
        .await
    {
        Ok(Some(api)) => api,
        Ok(None) => {
            return crate::manage_error!(crate::proxy_err!(
                business,
                "API Key not found: {}",
                api_id
            ));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_service_apis_fail",
                &format!("Failed to fetch user service API: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch API Key: {}",
                err
            ));
        }
    };

    // 执行硬删除
    let delete_result = match UserServiceApi::delete_by_id(api_id).exec(db).await {
        Ok(result) => result,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "delete_service_api_fail",
                &format!("Failed to delete user service API: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to delete API Key: {}",
                err
            ));
        }
    };

    if delete_result.rows_affected == 0 {
        return crate::manage_error!(crate::proxy_err!(internal, "Failed to delete API Key"));
    }

    response::success_with_message(serde_json::Value::Null, "API Key删除成功")
}

/// 7. API Key使用统计
pub async fn get_user_service_key_usage(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
    Query(query): Query<UsageStatsQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::proxy_tracing::{self, Entity as ProxyTracing};
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    if api_id <= 0 {
        return crate::manage_error!(crate::proxy_err!(business, "Invalid API ID"));
    }

    let db = state.database.as_ref();
    let user_id = auth_context.user_id;

    // 验证API Key存在且属于当前用户
    let _existing_api = match UserServiceApi::find_by_id(api_id)
        .filter(user_service_apis::Column::UserId.eq(user_id))
        .one(db)
        .await
    {
        Ok(Some(api)) => api,
        Ok(None) => {
            return crate::manage_error!(crate::proxy_err!(
                business,
                "API Key not found: {}",
                api_id
            ));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_service_apis_fail",
                &format!("Failed to fetch user service API: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch API Key: {}",
                err
            ));
        }
    };

    // 确定时间范围
    let (start_time, end_time) = match &query.time_range {
        Some(range) => match range.as_str() {
            "today" => {
                let today = Utc::now().date_naive();
                (
                    today.and_hms_opt(0, 0, 0).unwrap(),
                    today.and_hms_opt(23, 59, 59).unwrap(),
                )
            }
            "7days" => {
                let end = Utc::now().naive_utc();
                let start = end - chrono::Duration::days(7);
                (start, end)
            }
            _ => {
                let end = Utc::now().naive_utc();
                let start = end - chrono::Duration::days(30);
                (start, end)
            }
        },
        None => {
            // 使用自定义日期范围
            if let (Some(start_str), Some(end_str)) = (&query.start_date, &query.end_date) {
                if let (Ok(start_time), Ok(end_time)) = (
                    NaiveDate::parse_from_str(start_str, "%Y-%m-%d"),
                    NaiveDate::parse_from_str(end_str, "%Y-%m-%d"),
                ) {
                    (
                        start_time.and_hms_opt(0, 0, 0).unwrap(),
                        end_time.and_hms_opt(23, 59, 59).unwrap(),
                    )
                } else {
                    // 自定义日期解析失败，使用默认30天
                    let end = Utc::now().naive_utc();
                    let start = end - chrono::Duration::days(30);
                    (start, end)
                }
            } else {
                // 没有提供自定义日期，使用默认30天
                let end = Utc::now().naive_utc();
                let start = end - chrono::Duration::days(30);
                (start, end)
            }
        }
    };

    // 查询统计数据
    let tracings = match ProxyTracing::find()
        .filter(proxy_tracing::Column::UserServiceApiId.eq(api_id))
        .filter(proxy_tracing::Column::CreatedAt.between(start_time, end_time))
        .all(db)
        .await
    {
        Ok(data) => data,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Tracing,
                "fetch_tracings_fail",
                &format!("Failed to fetch proxy tracings: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch usage statistics: {}",
                err
            ));
        }
    };

    // 计算统计数据
    let total_requests = tracings.len() as i64;
    let successful_requests = tracings.iter().filter(|t| t.is_success).count() as i64;
    let failed_requests = total_requests - successful_requests;
    #[allow(clippy::cast_precision_loss)]
    let success_rate = if total_requests > 0 {
        (successful_requests as f64 / total_requests as f64) * 100.0
    } else {
        0.0
    };

    let total_tokens = tracings
        .iter()
        .map(|t| t.tokens_total.unwrap_or(0))
        .sum::<i32>();
    let tokens_prompt = tracings
        .iter()
        .map(|t| t.tokens_prompt.unwrap_or(0))
        .sum::<i32>();
    let tokens_completion = tracings
        .iter()
        .map(|t| t.tokens_completion.unwrap_or(0))
        .sum::<i32>();
    let cache_create_tokens = tracings
        .iter()
        .map(|t| t.cache_create_tokens.unwrap_or(0))
        .sum::<i32>();
    let cache_read_tokens = tracings
        .iter()
        .map(|t| t.cache_read_tokens.unwrap_or(0))
        .sum::<i32>();

    let total_cost = tracings.iter().map(|t| t.cost.unwrap_or(0.0)).sum::<f64>();

    let avg_response_time = if tracings.is_empty() {
        0
    } else {
        tracings
            .iter()
            .map(|t| t.duration_ms.unwrap_or(0))
            .sum::<i64>()
            / tracings.len() as i64
    };

    let last_used = tracings
        .iter()
        .max_by_key(|t| t.created_at)
        .map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t.created_at, Utc).to_rfc3339());

    // 构建响应
    let data = json!({
        "total_requests": total_requests,
        "successful_requests": successful_requests,
        "failed_requests": failed_requests,
        "success_rate": success_rate,
        "total_tokens": total_tokens,
        "tokens_prompt": tokens_prompt,
        "tokens_completion": tokens_completion,
        "cache_create_tokens": cache_create_tokens,
        "cache_read_tokens": cache_read_tokens,
        "total_cost": total_cost,
        "cost_currency": "USD",
        "avg_response_time": avg_response_time,
        "last_used": last_used,
        "usage_trend": [] // TODO: 实现按日期分组的趋势数据
    });

    response::success(data)
}

/// 8. 重新生成API Key
pub async fn regenerate_user_service_key(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

    if api_id <= 0 {
        return crate::manage_error!(crate::proxy_err!(business, "Invalid API ID"));
    }

    let db = state.database.as_ref();
    let user_id = auth_context.user_id;

    // 验证API Key存在且属于当前用户
    let _existing_api = match UserServiceApi::find_by_id(api_id)
        .filter(user_service_apis::Column::UserId.eq(user_id))
        .one(db)
        .await
    {
        Ok(Some(api)) => api,
        Ok(None) => {
            return crate::manage_error!(crate::proxy_err!(
                business,
                "API Key not found: {}",
                api_id
            ));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_service_apis_fail",
                &format!("Failed to fetch user service API: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch API Key: {}",
                err
            ));
        }
    };

    // 生成新的API Key
    let new_api_key = format!("sk-usr-{}", Uuid::new_v4().to_string().replace('-', ""));

    // 更新API Key
    let update_model = user_service_apis::ActiveModel {
        id: Set(api_id),
        api_key: Set(new_api_key.clone()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };

    let updated_api = match update_model.update(db).await {
        Ok(data) => data,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "regenerate_key_fail",
                &format!("Failed to regenerate API key: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to regenerate API key: {}",
                err
            ));
        }
    };

    let data = json!({
        "id": updated_api.id,
        "api_key": updated_api.api_key,
        "regenerated_at": DateTime::<Utc>::from_naive_utc_and_offset(updated_api.updated_at, Utc).to_rfc3339()
    });

    response::success_with_message(data, "API Key重新生成成功")
}

/// 9. 启用/禁用API Key
pub async fn update_user_service_key_status(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(request): Json<UpdateStatusRequest>,
) -> axum::response::Response {
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

    if api_id <= 0 {
        return crate::manage_error!(crate::proxy_err!(business, "Invalid API ID"));
    }

    let db = state.database.as_ref();
    let user_id = auth_context.user_id;

    // 验证API Key存在且属于当前用户
    let _existing_api = match UserServiceApi::find_by_id(api_id)
        .filter(user_service_apis::Column::UserId.eq(user_id))
        .one(db)
        .await
    {
        Ok(Some(api)) => api,
        Ok(None) => {
            return crate::manage_error!(crate::proxy_err!(
                business,
                "API Key not found: {}",
                api_id
            ));
        }
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_service_apis_fail",
                &format!("Failed to fetch user service API: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch API Key: {}",
                err
            ));
        }
    };

    // 更新状态
    let update_model = user_service_apis::ActiveModel {
        id: Set(api_id),
        is_active: Set(request.is_active),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };

    let updated_api = match update_model.update(db).await {
        Ok(data) => data,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "update_key_status_fail",
                &format!("Failed to update API key status: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to update API key status: {}",
                err
            ));
        }
    };

    let data = json!({
        "id": updated_api.id,
        "is_active": updated_api.is_active,
        "updated_at": DateTime::<Utc>::from_naive_utc_and_offset(updated_api.updated_at, Utc).to_rfc3339()
    });

    response::success_with_message(data, "API Key状态更新成功")
}
