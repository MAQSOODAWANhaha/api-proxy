//! # 提供商密钥管理处理器
//!
//! 处理上游AI服务商的API密钥管理相关请求

use crate::management::handlers::auth_utils::extract_user_id_from_headers;
use crate::management::{response, server::AppState};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::Deserialize;
use serde_json::json;

/// 获取提供商密钥列表
pub async fn get_provider_keys_list(
    State(state): State<AppState>,
    Query(query): Query<ProviderKeysListQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_provider_keys::{self, Entity as UserProviderKey};
    use sea_orm::{PaginatorTrait, QuerySelect};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response.into_response(),
    };

    // 构建查询条件
    let mut select = UserProviderKey::find().filter(user_provider_keys::Column::UserId.eq(user_id));

    // 应用搜索筛选
    if let Some(search) = &query.search {
        if !search.is_empty() {
            select = select.filter(user_provider_keys::Column::Name.contains(search));
        }
    }

    // 应用状态筛选
    if let Some(status) = &query.status {
        let is_active = match status.as_str() {
            "active" => true,
            "disabled" => false,
            _ => true, // 默认活跃
        };
        select = select.filter(user_provider_keys::Column::IsActive.eq(is_active));
    }

    // 分页参数
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(10);
    let offset = (page - 1) * limit;

    // 获取总数
    let total = match select.clone().count(db).await {
        Ok(count) => count,
        Err(err) => {
            tracing::error!("Failed to count provider keys: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to count provider keys",
            )
            .into_response();
        }
    };

    // 执行分页查询并关联 provider_types 表
    let provider_keys = match select
        .find_also_related(ProviderType)
        .offset(offset)
        .limit(limit)
        .order_by_desc(user_provider_keys::Column::CreatedAt)
        .all(db)
        .await
    {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to fetch provider keys: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch provider keys",
            )
            .into_response();
        }
    };

    // 构建响应数据
    let mut provider_keys_list = Vec::new();

    for (provider_key, provider_type_opt) in provider_keys {
        let provider_name = provider_type_opt
            .map(|pt| pt.display_name)
            .unwrap_or_else(|| "Unknown".to_string());

        let response_key = json!({
            "id": provider_key.id,
            "provider": provider_name,
            "name": provider_key.name,
            "api_key": provider_key.api_key,
            "weight": provider_key.weight,
            "max_requests_per_minute": provider_key.max_requests_per_minute,
            "max_tokens_prompt_per_minute": provider_key.max_tokens_prompt_per_minute,
            "max_requests_per_day": provider_key.max_requests_per_day,
            "is_active": provider_key.is_active,
            "usage": 0, // TODO: 从统计表获取
            "cost": 0.0, // TODO: 从统计表获取
            "created_at": provider_key.created_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            "health_status": provider_key.health_status
        });

        provider_keys_list.push(response_key);
    }

    let pages = (total + limit - 1) / limit;

    let data = json!({
        "provider_keys": provider_keys_list,
        "pagination": {
            "page": page,
            "limit": limit,
            "total": total,
            "pages": pages
        }
    });

    response::success(data).into_response()
}

/// 创建提供商密钥
pub async fn create_provider_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateProviderKeyRequest>,
) -> impl IntoResponse {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response.into_response(),
    };

    // 检查同名密钥是否已存在
    let existing = UserProviderKey::find()
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .filter(user_provider_keys::Column::Name.eq(&payload.name))
        .filter(user_provider_keys::Column::ProviderTypeId.eq(payload.provider_type_id))
        .one(db)
        .await;

    match existing {
        Ok(Some(_)) => {
            return response::error::<serde_json::Value>(
                StatusCode::CONFLICT,
                "DUPLICATE_NAME",
                "密钥名称已存在",
            )
            .into_response();
        }
        Err(err) => {
            tracing::error!("Failed to check existing provider key: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to check existing provider key",
            )
            .into_response();
        }
        _ => {}
    }

    // 创建新密钥
    let new_provider_key = user_provider_keys::ActiveModel {
        user_id: Set(user_id),
        provider_type_id: Set(payload.provider_type_id),
        name: Set(payload.name),
        api_key: Set(payload.api_key),
        weight: Set(payload.weight),
        max_requests_per_minute: Set(payload.max_requests_per_minute),
        max_tokens_prompt_per_minute: Set(payload.max_tokens_prompt_per_minute),
        max_requests_per_day: Set(payload.max_requests_per_day),
        is_active: Set(payload.is_active.unwrap_or(true)),
        health_status: Set("healthy".to_string()),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };

    let result = match new_provider_key.insert(db).await {
        Ok(model) => model,
        Err(err) => {
            tracing::error!("Failed to create provider key: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to create provider key",
            )
            .into_response();
        }
    };

    // 获取provider类型信息
    let provider_name = match entity::provider_types::Entity::find_by_id(payload.provider_type_id)
        .one(db)
        .await
    {
        Ok(Some(provider_type)) => provider_type.display_name,
        _ => "Unknown".to_string(),
    };

    let data = json!({
        "id": result.id,
        "provider": provider_name,
        "name": result.name,
        "created_at": result.created_at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    });

    response::success_with_message(data, "创建成功").into_response()
}

/// 获取提供商密钥详情
pub async fn get_provider_key_detail(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    headers: HeaderMap,
) -> impl IntoResponse {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response.into_response(),
    };

    // 查找密钥详情
    let provider_key = match UserProviderKey::find()
        .filter(user_provider_keys::Column::Id.eq(key_id))
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .find_also_related(ProviderType)
        .one(db)
        .await
    {
        Ok(Some((key, provider_type_opt))) => (key, provider_type_opt),
        Ok(None) => {
            return response::error::<serde_json::Value>(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "密钥不存在",
            )
            .into_response();
        }
        Err(err) => {
            tracing::error!("Failed to fetch provider key detail: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch provider key detail",
            )
            .into_response();
        }
    };

    let provider_name = provider_key
        .1
        .map(|pt| pt.display_name)
        .unwrap_or_else(|| "Unknown".to_string());

    let data = json!({
        "id": provider_key.0.id,
        "provider": provider_name,
        "name": provider_key.0.name,
        "api_key": provider_key.0.api_key,
        "weight": provider_key.0.weight,
        "max_requests_per_minute": provider_key.0.max_requests_per_minute,
        "max_tokens_prompt_per_minute": provider_key.0.max_tokens_prompt_per_minute,
        "max_requests_per_day": provider_key.0.max_requests_per_day,
        "is_active": provider_key.0.is_active,
        "usage": 0, // TODO: 从统计表获取
        "cost": 0.0, // TODO: 从统计表获取
        "created_at": provider_key.0.created_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "updated_at": provider_key.0.updated_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "health_status": provider_key.0.health_status
    });

    response::success(data).into_response()
}

/// 更新提供商密钥
pub async fn update_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    headers: HeaderMap,
    Json(payload): Json<UpdateProviderKeyRequest>,
) -> impl IntoResponse {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response.into_response(),
    };

    // 查找要更新的密钥
    let existing_key = match UserProviderKey::find()
        .filter(user_provider_keys::Column::Id.eq(key_id))
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .one(db)
        .await
    {
        Ok(Some(key)) => key,
        Ok(None) => {
            return response::error::<serde_json::Value>(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "密钥不存在",
            )
            .into_response();
        }
        Err(err) => {
            tracing::error!("Failed to find provider key: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to find provider key",
            )
            .into_response();
        }
    };

    // 检查名称是否与其他密钥冲突
    if existing_key.name != payload.name {
        let duplicate = UserProviderKey::find()
            .filter(user_provider_keys::Column::UserId.eq(user_id))
            .filter(user_provider_keys::Column::Name.eq(&payload.name))
            .filter(user_provider_keys::Column::ProviderTypeId.eq(payload.provider_type_id))
            .filter(user_provider_keys::Column::Id.ne(key_id))
            .one(db)
            .await;

        match duplicate {
            Ok(Some(_)) => {
                return response::error::<serde_json::Value>(
                    StatusCode::CONFLICT,
                    "DUPLICATE_NAME",
                    "密钥名称已存在",
                )
                .into_response();
            }
            Err(err) => {
                tracing::error!("Failed to check duplicate name: {}", err);
                return response::error::<serde_json::Value>(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "DB_ERROR",
                    "Failed to check duplicate name",
                )
                .into_response();
            }
            _ => {}
        }
    }

    // 更新密钥
    let mut active_model: user_provider_keys::ActiveModel = existing_key.into();
    active_model.provider_type_id = Set(payload.provider_type_id);
    active_model.name = Set(payload.name);
    active_model.api_key = Set(payload.api_key);
    active_model.weight = Set(payload.weight);
    active_model.max_requests_per_minute = Set(payload.max_requests_per_minute);
    active_model.max_tokens_prompt_per_minute = Set(payload.max_tokens_prompt_per_minute);
    active_model.max_requests_per_day = Set(payload.max_requests_per_day);
    active_model.is_active = Set(payload.is_active.unwrap_or(true));
    active_model.updated_at = Set(Utc::now().naive_utc());

    let updated_key = match active_model.update(db).await {
        Ok(model) => model,
        Err(err) => {
            tracing::error!("Failed to update provider key: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to update provider key",
            )
            .into_response();
        }
    };

    let data = json!({
        "id": updated_key.id,
        "name": updated_key.name,
        "updated_at": updated_key.updated_at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    });

    response::success_with_message(data, "更新成功").into_response()
}

/// 删除提供商密钥
pub async fn delete_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    headers: HeaderMap,
) -> impl IntoResponse {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response.into_response(),
    };

    // 查找要删除的密钥
    let existing_key = match UserProviderKey::find()
        .filter(user_provider_keys::Column::Id.eq(key_id))
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .one(db)
        .await
    {
        Ok(Some(key)) => key,
        Ok(None) => {
            return response::error::<serde_json::Value>(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "密钥不存在",
            )
            .into_response();
        }
        Err(err) => {
            tracing::error!("Failed to find provider key: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to find provider key",
            )
            .into_response();
        }
    };

    // 删除密钥
    let active_model: user_provider_keys::ActiveModel = existing_key.into();
    match active_model.delete(db).await {
        Ok(_) => {}
        Err(err) => {
            tracing::error!("Failed to delete provider key: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to delete provider key",
            )
            .into_response();
        }
    };

    let data = json!({
        "id": key_id,
        "deleted_at": Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
    });

    response::success_with_message(data, "删除成功").into_response()
}

/// 获取密钥统计信息
pub async fn get_provider_key_stats(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    headers: HeaderMap,
) -> impl IntoResponse {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response.into_response(),
    };

    // 查找密钥详情
    let provider_key = match UserProviderKey::find()
        .filter(user_provider_keys::Column::Id.eq(key_id))
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .find_also_related(ProviderType)
        .one(db)
        .await
    {
        Ok(Some((key, provider_type_opt))) => (key, provider_type_opt),
        Ok(None) => {
            return response::error::<serde_json::Value>(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "密钥不存在",
            )
            .into_response();
        }
        Err(err) => {
            tracing::error!("Failed to fetch provider key: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch provider key",
            )
            .into_response();
        }
    };

    let provider_name = provider_key
        .1
        .map(|pt| pt.display_name)
        .unwrap_or_else(|| "Unknown".to_string());

    // TODO: 实际统计数据应该从统计表获取，这里使用模拟数据
    let data = json!({
        "basic_info": {
            "provider": provider_name,
            "name": provider_key.0.name,
            "weight": provider_key.0.weight
        },
        "usage_stats": {
            "total_usage": 8520,
            "monthly_cost": 125.50,
            "success_rate": 99.2,
            "avg_response_time": 850
        },
        "daily_trends": {
            "usage": [320, 450, 289, 645, 378, 534, 489],
            "cost": [12.5, 18.2, 11.3, 25.8, 15.1, 21.4, 19.6]
        },
        "limits": {
            "max_requests_per_minute": provider_key.0.max_requests_per_minute,
            "max_tokens_prompt_per_minute": provider_key.0.max_tokens_prompt_per_minute,
            "max_requests_per_day": provider_key.0.max_requests_per_day
        }
    });

    response::success(data).into_response()
}

/// 获取提供商密钥卡片统计数据
pub async fn get_provider_keys_dashboard_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response.into_response(),
    };

    // 查询总密钥数
    let total_keys = match UserProviderKey::find()
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .count(db)
        .await
    {
        Ok(count) => count,
        Err(err) => {
            tracing::error!("Failed to count total keys: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to count total keys",
            )
            .into_response();
        }
    };

    // 查询活跃密钥数
    let active_keys = match UserProviderKey::find()
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .filter(user_provider_keys::Column::IsActive.eq(true))
        .count(db)
        .await
    {
        Ok(count) => count,
        Err(err) => {
            tracing::error!("Failed to count active keys: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to count active keys",
            )
            .into_response();
        }
    };

    // 查询总使用次数和总花费 - 从 proxy_tracing 表中统计
    // 使用子查询来获取该用户的provider_key_ids
    let user_provider_key_ids: Vec<i32> = match UserProviderKey::find()
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .all(db)
        .await
    {
        Ok(keys) => keys.iter().map(|k| k.id).collect(),
        Err(err) => {
            tracing::error!("Failed to fetch user provider keys: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch user provider keys",
            )
            .into_response();
        }
    };

    // 统计使用次数和费用
    let (total_usage, total_cost) = if user_provider_key_ids.is_empty() {
        (0u64, 0.0f64)
    } else {
        use entity::proxy_tracing::{self, Entity as ProxyTracing};

        match ProxyTracing::find()
            .filter(proxy_tracing::Column::UserProviderKeyId.is_in(user_provider_key_ids))
            .filter(proxy_tracing::Column::IsSuccess.eq(true))
            .all(db)
            .await
        {
            Ok(records) => {
                let usage_count = records.len() as u64;
                let cost_sum: f64 = records.iter().filter_map(|record| record.cost).sum();
                (usage_count, cost_sum)
            }
            Err(err) => {
                tracing::error!("Failed to fetch proxy tracing records: {}", err);
                return response::error::<serde_json::Value>(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "DB_ERROR",
                    "Failed to fetch usage statistics",
                )
                .into_response();
            }
        }
    };

    let data = json!({
        "total_keys": total_keys,
        "active_keys": active_keys,
        "total_usage": total_usage,
        "total_cost": total_cost
    });

    response::success(data).into_response()
}

/// 获取简单提供商密钥列表（用于下拉选择）
pub async fn get_simple_provider_keys_list(
    State(state): State<AppState>,
    Query(query): Query<UserProviderKeyQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response.into_response(),
    };

    // 构建查询条件
    let mut select = UserProviderKey::find().filter(user_provider_keys::Column::UserId.eq(user_id));

    // 应用服务商类型筛选
    if let Some(provider_type_id) = query.provider_type_id {
        select = select.filter(user_provider_keys::Column::ProviderTypeId.eq(provider_type_id));
    }

    // 应用状态筛选
    if let Some(is_active) = query.is_active {
        select = select.filter(user_provider_keys::Column::IsActive.eq(is_active));
    }

    // 执行查询并关联 provider_types 表
    let provider_keys = match select
        .find_also_related(ProviderType)
        .order_by_desc(user_provider_keys::Column::CreatedAt)
        .all(db)
        .await
    {
        Ok(data) => data,
        Err(err) => {
            tracing::error!("Failed to fetch simple provider keys: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to fetch provider keys",
            )
            .into_response();
        }
    };

    // 构建响应数据
    let mut provider_keys_list = Vec::new();

    for (provider_key, provider_type_opt) in provider_keys {
        let provider_name = provider_type_opt
            .as_ref()
            .map(|pt| pt.display_name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let display_name = format!("{} ({})", provider_key.name, provider_name);

        let response_key = json!({
            "id": provider_key.id,
            "name": provider_key.name,
            "display_name": display_name,
            "provider": provider_name,
            "provider_type_id": provider_key.provider_type_id,
            "is_active": provider_key.is_active
        });

        provider_keys_list.push(response_key);
    }

    let data = json!({
        "provider_keys": provider_keys_list
    });

    response::success(data).into_response()
}

/// 执行健康检查
pub async fn health_check_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    headers: HeaderMap,
) -> impl IntoResponse {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response.into_response(),
    };

    // 查找要检查的密钥
    let existing_key = match UserProviderKey::find()
        .filter(user_provider_keys::Column::Id.eq(key_id))
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .one(db)
        .await
    {
        Ok(Some(key)) => key,
        Ok(None) => {
            return response::error::<serde_json::Value>(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "密钥不存在",
            )
            .into_response();
        }
        Err(err) => {
            tracing::error!("Failed to find provider key: {}", err);
            return response::error::<serde_json::Value>(
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                "Failed to find provider key",
            )
            .into_response();
        }
    };

    // TODO: 实际执行健康检查逻辑，这里使用模拟结果
    let health_status = "healthy";
    let response_time = 245;
    let check_time = Utc::now();

    // 更新健康状态
    let mut active_model: user_provider_keys::ActiveModel = existing_key.into();
    active_model.health_status = Set(health_status.to_string());
    active_model.updated_at = Set(check_time.naive_utc());

    match active_model.update(db).await {
        Ok(_) => {}
        Err(err) => {
            tracing::error!("Failed to update health status: {}", err);
            // 不返回错误，继续返回检查结果
        }
    };

    let data = json!({
        "id": key_id,
        "health_status": health_status,
        "check_time": check_time.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "response_time": response_time,
        "details": {
            "status_code": 200,
            "latency": response_time,
            "error_message": null
        }
    });

    response::success_with_message(data, "健康检查完成").into_response()
}

/// 提供商密钥列表查询参数
#[derive(Debug, Deserialize)]
pub struct ProviderKeysListQuery {
    /// 页码（从1开始）
    pub page: Option<u64>,
    /// 每页数量
    pub limit: Option<u64>,
    /// 搜索关键词
    pub search: Option<String>,
    /// 筛选指定服务商
    pub provider: Option<String>,
    /// 筛选状态
    pub status: Option<String>,
}

/// 创建提供商密钥请求
#[derive(Debug, Deserialize)]
pub struct CreateProviderKeyRequest {
    pub provider_type_id: i32,
    pub name: String,
    pub api_key: String,
    pub weight: Option<i32>,
    pub max_requests_per_minute: Option<i32>,
    pub max_tokens_prompt_per_minute: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub is_active: Option<bool>,
}

/// 更新提供商密钥请求
#[derive(Debug, Deserialize)]
pub struct UpdateProviderKeyRequest {
    pub provider_type_id: i32,
    pub name: String,
    pub api_key: String,
    pub weight: Option<i32>,
    pub max_requests_per_minute: Option<i32>,
    pub max_tokens_prompt_per_minute: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub is_active: Option<bool>,
}

/// 用户提供商密钥查询参数
#[derive(Debug, Deserialize)]
pub struct UserProviderKeyQuery {
    /// 服务商类型ID筛选
    pub provider_type_id: Option<i32>,
    /// 是否启用筛选
    pub is_active: Option<bool>,
}
