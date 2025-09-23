//! # 提供商密钥管理处理器
//!
//! 处理上游AI服务商的API密钥管理相关请求

use crate::auth::{extract_user_id_from_headers, gemini_code_assist_client::GeminiCodeAssistClient};

/// Gemini提供商名称常量
const GEMINI_PROVIDER_NAME: &str = "gemini";

/// OAuth认证类型常量
const OAUTH_AUTH_TYPE: &str = "oauth";

/// OAuth会话完成状态常量
const OAUTH_SESSION_STATUS_COMPLETED: &str = "completed";
use crate::management::{response, server::AppState};
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Json;
use chrono::Utc;
use tracing::info;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

/// 获取提供商密钥列表
pub async fn get_provider_keys_list(
    State(state): State<AppState>,
    Query(query): Query<ProviderKeysListQuery>,
    headers: HeaderMap,
) -> axum::response::Response {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_provider_keys::{self, Entity as UserProviderKey};
    use sea_orm::{PaginatorTrait, QuerySelect};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
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
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to count provider keys"
            ));
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
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch provider keys"
            ));
        }
    };

    // 获取所有密钥的使用统计数据
    let provider_key_ids: Vec<i32> = provider_keys.iter().map(|(pk, _)| pk.id).collect();
    let usage_stats = fetch_provider_keys_usage_stats(db, &provider_key_ids).await;

    // 构建响应数据
    let mut provider_keys_list = Vec::new();

    for (provider_key, provider_type_opt) in provider_keys {
        let provider_name = provider_type_opt
            .map(|pt| pt.display_name)
            .unwrap_or_else(|| "Unknown".to_string());

        // 获取该密钥的使用统计
        let key_stats = usage_stats
            .get(&provider_key.id)
            .cloned()
            .unwrap_or_default();

        // 隐藏API Key敏感信息
        let masked_api_key = if provider_key.api_key.len() > 8 {
            format!(
                "{}****{}",
                &provider_key.api_key[..4],
                &provider_key.api_key[provider_key.api_key.len() - 4..]
            )
        } else {
            "****".to_string()
        };

        // 计算限流剩余时间（秒）
        let rate_limit_remaining_seconds = if let Some(resets_at) = provider_key.rate_limit_resets_at {
            let now = Utc::now().naive_utc();
            if resets_at > now {
                let duration = resets_at.signed_duration_since(now);
                Some(duration.num_seconds().max(0) as u64)
            } else {
                None
            }
        } else {
            None
        };

        let response_key = json!({
            "id": provider_key.id,
            "provider": provider_name,
            "name": provider_key.name,
            "api_key": if provider_key.auth_type == "api_key" { masked_api_key } else { provider_key.api_key.clone() },
            "project_id": provider_key.project_id,
            "auth_type": provider_key.auth_type,
            "auth_status": provider_key.auth_status,
            "expires_at": provider_key.expires_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
            "weight": provider_key.weight,
            "max_requests_per_minute": provider_key.max_requests_per_minute,
            "max_tokens_prompt_per_minute": provider_key.max_tokens_prompt_per_minute,
            "max_requests_per_day": provider_key.max_requests_per_day,
            "is_active": provider_key.is_active,
            "usage": {
                "total_requests": key_stats.total_requests,
                "successful_requests": key_stats.successful_requests,
                "failed_requests": key_stats.failed_requests,
                "success_rate": key_stats.success_rate,
                "total_tokens": key_stats.total_tokens,
                "total_cost": key_stats.total_cost,
                "avg_response_time": key_stats.avg_response_time,
                "last_used_at": key_stats.last_used_at
            },
            "limits": {
                "max_requests_per_minute": provider_key.max_requests_per_minute,
                "max_tokens_prompt_per_minute": provider_key.max_tokens_prompt_per_minute,
                "max_requests_per_day": provider_key.max_requests_per_day
            },
            "status": {
                "is_active": provider_key.is_active,
                "health_status": provider_key.health_status,
                "rate_limit_remaining_seconds": rate_limit_remaining_seconds
            },
            "created_at": provider_key.created_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            "updated_at": provider_key.updated_at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
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

    response::success(data)
}

/// 创建提供商密钥
pub async fn create_provider_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateProviderKeyRequest>,
) -> axum::response::Response {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
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
            return crate::manage_error!(crate::proxy_err!(
                business,
                "ProviderKey conflict: {}",
                &payload.name
            ));
        }
        Err(err) => {
            tracing::error!("Failed to check existing provider key: {}", err);
            return crate::manage_error!(crate::error::ProxyError::database_with_source(
                "Failed to check existing provider key",
                err,
            ));
        }
        _ => {}
    }

    // 验证认证类型和相应参数
    if payload.auth_type == "api_key" && payload.api_key.is_none() {
        return crate::manage_error!(crate::proxy_err!(
            business,
            "API Key认证类型需要提供api_key字段 (field: api_key)"
        ));
    }

    // OAuth类型需要通过api_key字段提供session_id
    if payload.auth_type == "oauth" && payload.api_key.is_none() {
        return crate::manage_error!(crate::proxy_err!(
            business,
            "OAuth认证类型需要通过api_key字段提供session_id (field: api_key)"
        ));
    }

    // 验证OAuth会话存在性和所有权
    if payload.auth_type == "oauth" {
        if let Some(session_id) = &payload.api_key {
            use entity::oauth_client_sessions::{self, Entity as OAuthSession};

            match OAuthSession::find()
                .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
                .filter(oauth_client_sessions::Column::UserId.eq(user_id))
                .filter(oauth_client_sessions::Column::Status.eq("completed"))
                .one(db)
                .await
            {
                Ok(Some(_)) => {
                    // OAuth会话存在且属于当前用户，检查是否已被其他provider key使用
                    let existing_usage = UserProviderKey::find()
                        .filter(user_provider_keys::Column::ApiKey.eq(session_id))
                        .filter(user_provider_keys::Column::AuthType.eq("oauth"))
                        .filter(user_provider_keys::Column::IsActive.eq(true))
                        .one(db)
                        .await;

                    match existing_usage {
                        Ok(Some(_)) => {
                            return crate::manage_error!(crate::proxy_err!(
                                business,
                                "指定的OAuth会话已被其他provider key使用"
                            ));
                        }
                        Err(err) => {
                            tracing::error!("Failed to check OAuth session usage: {}", err);
                            return crate::manage_error!(
                                crate::error::ProxyError::database_with_source(
                                    "Failed to check OAuth session usage",
                                    err,
                                )
                            );
                        }
                        _ => {} // 会话可用
                    }
                }
                Ok(None) => {
                    return crate::manage_error!(crate::proxy_err!(
                        business,
                        "指定的OAuth会话不存在或未完成授权 (field: api_key)"
                    ));
                }
                Err(err) => {
                    tracing::error!("Failed to validate OAuth session: {}", err);
                    return crate::manage_error!(crate::error::ProxyError::database_with_source(
                        "Failed to validate OAuth session",
                        err,
                    ));
                }
            }
        }
    }

    // 自动获取Gemini OAuth场景的project_id
    let mut final_project_id = payload.project_id.clone();

    // 检查是否是Gemini OAuth场景且需要自动获取project_id
    if payload.auth_type == OAUTH_AUTH_TYPE && final_project_id.is_none() {
        // 查询provider类型信息，判断是否是Gemini
        if let Ok(Some(provider_type)) = entity::provider_types::Entity::find_by_id(payload.provider_type_id)
            .one(db)
            .await
        {
            if provider_type.name == GEMINI_PROVIDER_NAME {
                tracing::info!(
                    user_id = user_id,
                    provider_type_id = payload.provider_type_id,
                    "检测到Gemini OAuth且无project_id，开始自动获取"
                );

                // 获取OAuth会话的access_token
                if let Some(session_id) = &payload.api_key {
                    use entity::oauth_client_sessions::{self, Entity as OAuthSession};

                    if let Ok(Some(oauth_session)) = OAuthSession::find()
                        .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
                        .filter(oauth_client_sessions::Column::UserId.eq(user_id))
                        .filter(oauth_client_sessions::Column::Status.eq(OAUTH_SESSION_STATUS_COMPLETED))
                        .one(db)
                        .await
                    {
                        // 使用Gemini Code Assist API客户端自动获取project_id
                        let gemini_client = GeminiCodeAssistClient::new();
                        match gemini_client.auto_get_project_id(&oauth_session.access_token.as_deref().unwrap_or("")).await {
                            Ok(Some(auto_project_id)) => {
                                final_project_id = Some(auto_project_id.clone());
                                tracing::info!(
                                    user_id = user_id,
                                    session_id = session_id,
                                    auto_project_id = auto_project_id,
                                    "成功自动获取Gemini project_id"
                                );
                            }
                            Ok(None) => {
                                tracing::warn!(
                                    user_id = user_id,
                                    session_id = session_id,
                                    "Gemini auto_get_project_id返回None，将使用空project_id创建"
                                );
                            }
                            Err(e) => {
                                tracing::error!(
                                    user_id = user_id,
                                    session_id = session_id,
                                    error = %e,
                                    "自动获取Gemini project_id失败，将使用空project_id创建"
                                );
                                // 不阻断创建流程，继续使用空project_id
                            }
                        }
                    } else {
                        tracing::warn!(
                            user_id = user_id,
                            session_id = session_id,
                            "无法找到OAuth会话，跳过自动获取project_id"
                        );
                    }
                }
            }
        }
    }

    // 创建新密钥
    let new_provider_key = user_provider_keys::ActiveModel {
        user_id: Set(user_id),
        provider_type_id: Set(payload.provider_type_id),
        name: Set(payload.name),
        api_key: Set(payload.api_key.unwrap_or_else(|| "".to_string())),
        auth_type: Set(payload.auth_type),
        auth_status: Set(Some("active".to_string())),
        weight: Set(payload.weight),
        max_requests_per_minute: Set(payload.max_requests_per_minute),
        max_tokens_prompt_per_minute: Set(payload.max_tokens_prompt_per_minute),
        max_requests_per_day: Set(payload.max_requests_per_day),
        is_active: Set(payload.is_active.unwrap_or(true)),
        // 保存最终确定的project_id到数据库（可能是传入的，也可能是自动获取的）
        project_id: Set(final_project_id),
        health_status: Set("healthy".to_string()),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };

    let result = match new_provider_key.insert(db).await {
        Ok(model) => model,
        Err(err) => {
            tracing::error!("Failed to create provider key: {}", err);
            return crate::manage_error!(crate::error::ProxyError::database_with_source(
                "Failed to create provider key",
                err,
            ));
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
        "auth_type": result.auth_type,
        "auth_status": result.auth_status,
        "created_at": result.created_at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    });

    response::success_with_message(data, "创建成功")
}

/// 获取提供商密钥详情
pub async fn get_provider_key_detail(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    headers: HeaderMap,
) -> axum::response::Response {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
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
            return crate::manage_error!(crate::proxy_err!(
                business,
                "ProviderKey not found: {}",
                key_id
            ));
        }
        Err(err) => {
            tracing::error!("Failed to fetch provider key detail: {}", err);
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch provider key detail: {}",
                err
            ));
        }
    };

    let provider_name = provider_key
        .1
        .map(|pt| pt.display_name)
        .unwrap_or_else(|| "Unknown".to_string());

    // 获取使用统计
    let usage_stats = fetch_provider_keys_usage_stats(db, &[provider_key.0.id]).await;
    let key_stats = usage_stats
        .get(&provider_key.0.id)
        .cloned()
        .unwrap_or_default();

    // 隐藏API Key敏感信息
    let masked_api_key = if provider_key.0.api_key.len() > 8 {
        format!(
            "{}****{}",
            &provider_key.0.api_key[..4],
            &provider_key.0.api_key[provider_key.0.api_key.len() - 4..]
        )
    } else {
        "****".to_string()
    };

    // 计算限流剩余时间（秒）
    let rate_limit_remaining_seconds = if let Some(resets_at) = provider_key.0.rate_limit_resets_at {
        let now = Utc::now().naive_utc();
        info!(
            key_id = provider_key.0.id,
            rate_limit_resets_at = ?resets_at,
            current_time = ?now,
            "计算限流剩余时间 - 数据库中有重置时间"
        );

        if resets_at > now {
            let duration = resets_at.signed_duration_since(now);
            let remaining_seconds = duration.num_seconds().max(0) as u64;
            info!(
                key_id = provider_key.0.id,
                remaining_seconds = remaining_seconds,
                duration_seconds = duration.num_seconds(),
                "限流尚未解除，计算剩余秒数"
            );
            Some(remaining_seconds)
        } else {
            info!(
                key_id = provider_key.0.id,
                rate_limit_resets_at = ?resets_at,
                current_time = ?now,
                "限流已过期，返回None"
            );
            None
        }
    } else {
        info!(
            key_id = provider_key.0.id,
            health_status = %provider_key.0.health_status,
            "数据库中无限流重置时间，返回None"
        );
        None
    };

    let data = json!({
        "id": provider_key.0.id,
        "provider": provider_name,
        "name": provider_key.0.name,
        "api_key": if provider_key.0.auth_type == "api_key" { masked_api_key } else { provider_key.0.api_key.clone() },
        "auth_type": provider_key.0.auth_type,
        "auth_status": provider_key.0.auth_status,
        "expires_at": provider_key.0.expires_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
        "last_auth_check": provider_key.0.last_auth_check.map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
        "weight": provider_key.0.weight,
        "max_requests_per_minute": provider_key.0.max_requests_per_minute,
        "max_tokens_prompt_per_minute": provider_key.0.max_tokens_prompt_per_minute,
        "max_requests_per_day": provider_key.0.max_requests_per_day,
        "is_active": provider_key.0.is_active,
        "usage": {
            "total_requests": key_stats.total_requests,
            "successful_requests": key_stats.successful_requests,
            "failed_requests": key_stats.failed_requests,
            "success_rate": key_stats.success_rate,
            "total_tokens": key_stats.total_tokens,
            "total_cost": key_stats.total_cost,
            "avg_response_time": key_stats.avg_response_time,
            "last_used_at": key_stats.last_used_at
        },
        "limits": {
            "max_requests_per_minute": provider_key.0.max_requests_per_minute,
            "max_tokens_prompt_per_minute": provider_key.0.max_tokens_prompt_per_minute,
            "max_requests_per_day": provider_key.0.max_requests_per_day
        },
        "status": {
            "is_active": provider_key.0.is_active,
            "health_status": provider_key.0.health_status,
            "rate_limit_remaining_seconds": rate_limit_remaining_seconds
        },
        "created_at": provider_key.0.created_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "updated_at": provider_key.0.updated_at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    });

    response::success(data)
}

/// 更新提供商密钥
pub async fn update_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    headers: HeaderMap,
    Json(payload): Json<UpdateProviderKeyRequest>,
) -> axum::response::Response {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
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
            return crate::manage_error!(crate::proxy_err!(
                business,
                "ProviderKey not found: {}",
                key_id
            ));
        }
        Err(err) => {
            tracing::error!("Failed to find provider key: {}", err);
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to find provider key: {}",
                err
            ));
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
                return crate::manage_error!(crate::proxy_err!(
                    business,
                    "ProviderKey conflict: {}",
                    payload.name.clone()
                ));
            }
            Err(err) => {
                tracing::error!("Failed to check duplicate name: {}", err);
                return crate::manage_error!(crate::proxy_err!(
                    database,
                    "Failed to check duplicate name: {}",
                    err
                ));
            }
            _ => {}
        }
    }

    // 验证认证类型和相应参数
    if payload.auth_type == "api_key" && payload.api_key.is_none() {
        return crate::manage_error!(crate::proxy_err!(
            business,
            "API Key认证类型需要提供api_key字段 (field: api_key)"
        ));
    }

    // OAuth类型需要通过api_key字段提供session_id
    if payload.auth_type == "oauth" && payload.api_key.is_none() {
        return crate::manage_error!(crate::proxy_err!(
            business,
            "OAuth认证类型需要通过api_key字段提供session_id (field: api_key)"
        ));
    }

    // 验证OAuth会话存在性和所有权
    if payload.auth_type == "oauth" {
        if let Some(session_id) = &payload.api_key {
            use entity::oauth_client_sessions::{self, Entity as OAuthSession};

            // 检查会话是否有效
            match OAuthSession::find()
                .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
                .filter(oauth_client_sessions::Column::UserId.eq(user_id))
                .filter(oauth_client_sessions::Column::Status.eq("completed"))
                .one(db)
                .await
            {
                Ok(Some(_)) => {
                    // OAuth会话存在且属于当前用户，检查是否已被其他provider key使用
                    // (排除当前正在更新的key)
                    let existing_usage = UserProviderKey::find()
                        .filter(user_provider_keys::Column::ApiKey.eq(session_id))
                        .filter(user_provider_keys::Column::AuthType.eq("oauth"))
                        .filter(user_provider_keys::Column::IsActive.eq(true))
                        .filter(user_provider_keys::Column::Id.ne(key_id)) // 排除当前key
                        .one(db)
                        .await;

                    match existing_usage {
                        Ok(Some(_)) => {
                            return crate::manage_error!(crate::proxy_err!(
                                business,
                                "指定的OAuth会话已被其他provider key使用"
                            ));
                        }
                        Err(err) => {
                            tracing::error!("Failed to check OAuth session usage: {}", err);
                            return crate::manage_error!(
                                crate::error::ProxyError::database_with_source(
                                    "Failed to check OAuth session usage",
                                    err,
                                )
                            );
                        }
                        _ => {} // 会话可用
                    }
                }
                Ok(None) => {
                    return crate::manage_error!(crate::proxy_err!(
                        business,
                        "指定的OAuth会话不存在或未完成授权 (field: api_key)"
                    ));
                }
                Err(err) => {
                    tracing::error!("Failed to validate OAuth session: {}", err);
                    return crate::manage_error!(crate::error::ProxyError::database_with_source(
                        "Failed to validate OAuth session",
                        err,
                    ));
                }
            }
        }
    }

    // 更新密钥
    let mut active_model: user_provider_keys::ActiveModel = existing_key.into();
    active_model.provider_type_id = Set(payload.provider_type_id);
    active_model.name = Set(payload.name);
    active_model.api_key = Set(payload.api_key.unwrap_or_else(|| "".to_string()));
    active_model.auth_type = Set(payload.auth_type);
    active_model.weight = Set(payload.weight);
    active_model.max_requests_per_minute = Set(payload.max_requests_per_minute);
    active_model.max_tokens_prompt_per_minute = Set(payload.max_tokens_prompt_per_minute);
    active_model.max_requests_per_day = Set(payload.max_requests_per_day);
    active_model.is_active = Set(payload.is_active.unwrap_or(true));
    // 简单更新project_id到数据库
    active_model.project_id = Set(payload.project_id);
    active_model.updated_at = Set(Utc::now().naive_utc());

    let updated_key = match active_model.update(db).await {
        Ok(model) => model,
        Err(err) => {
            tracing::error!("Failed to update provider key: {}", err);
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to update provider key: {}",
                err
            ));
        }
    };

    let data = json!({
        "id": updated_key.id,
        "name": updated_key.name,
        "auth_type": updated_key.auth_type,
        "auth_status": updated_key.auth_status,
        "updated_at": updated_key.updated_at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    });

    response::success_with_message(data, "更新成功")
}

/// 删除提供商密钥
pub async fn delete_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    headers: HeaderMap,
) -> axum::response::Response {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
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
            return crate::manage_error!(crate::proxy_err!(
                business,
                "ProviderKey not found: {}",
                key_id
            ));
        }
        Err(err) => {
            tracing::error!("Failed to find provider key: {}", err);
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to find provider key: {}",
                err
            ));
        }
    };

    // 删除密钥
    let active_model: user_provider_keys::ActiveModel = existing_key.into();
    match active_model.delete(db).await {
        Ok(_) => {}
        Err(err) => {
            tracing::error!("Failed to delete provider key: {}", err);
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to delete provider key: {}",
                err
            ));
        }
    };

    let data = json!({
        "id": key_id,
        "deleted_at": Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
    });

    response::success_with_message(data, "删除成功")
}

/// 获取密钥统计信息
pub async fn get_provider_key_stats(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    headers: HeaderMap,
) -> axum::response::Response {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
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
            return crate::manage_error!(crate::proxy_err!(
                business,
                "ProviderKey not found: {}",
                key_id
            ));
        }
        Err(err) => {
            tracing::error!("Failed to fetch provider key: {}", err);
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch provider key: {}",
                err
            ));
        }
    };

    let provider_name = provider_key
        .1
        .map(|pt| pt.display_name)
        .unwrap_or_else(|| "Unknown".to_string());

    // 获取真实的统计数据
    let end_date = Utc::now().naive_utc();
    let start_date = end_date - chrono::Duration::days(7); // 默认查询7天数据

    let trends = match fetch_key_trends_data(db, key_id, &start_date, &end_date, "provider").await {
        Ok(trends) => trends,
        Err(err) => {
            tracing::error!("Failed to fetch provider key trends: {}", err);
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch trends data: {}",
                err
            ));
        }
    };

    let data = json!({
        "basic_info": {
            "provider": provider_name,
            "name": provider_key.0.name,
            "weight": provider_key.0.weight
        },
        "usage_stats": {
            "total_usage": trends.total_requests,
            "monthly_cost": trends.total_cost,
            "success_rate": trends.success_rate,
            "avg_response_time": trends.avg_response_time
        },
        "daily_trends": {
            "usage": trends.daily_usage,
            "cost": trends.daily_cost,
            "response_time": trends.daily_response_time
        },
        "limits": {
            "max_requests_per_minute": provider_key.0.max_requests_per_minute,
            "max_tokens_prompt_per_minute": provider_key.0.max_tokens_prompt_per_minute,
            "max_requests_per_day": provider_key.0.max_requests_per_day
        }
    });

    response::success(data)
}

/// 获取提供商密钥卡片统计数据
pub async fn get_provider_keys_dashboard_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> axum::response::Response {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
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
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to count total keys: {}",
                err
            ));
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
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to count active keys: {}",
                err
            ));
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
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch user provider keys: {}",
                err
            ));
        }
    };

    // 统计使用次数和费用
    let (total_usage, total_cost) = if user_provider_key_ids.is_empty() {
        (0u64, 0.0f64)
    } else {
        use entity::proxy_tracing::{Entity as ProxyTracing, Column};

        match ProxyTracing::find()
            .filter(Column::UserProviderKeyId.is_in(user_provider_key_ids))
            .filter(Column::IsSuccess.eq(true))
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
                return crate::manage_error!(crate::proxy_err!(
                    database,
                    "Failed to fetch usage statistics: {}",
                    err
                ));
            }
        }
    };

    let data = json!({
        "total_keys": total_keys,
        "active_keys": active_keys,
        "total_usage": total_usage,
        "total_cost": total_cost
    });

    response::success(data)
}

/// 获取简单提供商密钥列表（用于下拉选择）
pub async fn get_simple_provider_keys_list(
    State(state): State<AppState>,
    Query(query): Query<UserProviderKeyQuery>,
    headers: HeaderMap,
) -> axum::response::Response {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
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
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch provider keys: {}",
                err
            ));
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

    response::success(data)
}

/// 执行健康检查
pub async fn health_check_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    headers: HeaderMap,
) -> axum::response::Response {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
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
            return crate::manage_error!(crate::proxy_err!(
                business,
                "ProviderKey not found: {}",
                key_id
            ));
        }
        Err(err) => {
            tracing::error!("Failed to find provider key: {}", err);
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to find provider key: {}",
                err
            ));
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

    response::success_with_message(data, "健康检查完成")
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
    pub api_key: Option<String>,
    pub auth_type: String, // "api_key", "oauth", "service_account", "adc"
    // OAuth认证类型现在通过api_key字段传递session_id
    pub weight: Option<i32>,
    pub max_requests_per_minute: Option<i32>,
    pub max_tokens_prompt_per_minute: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub is_active: Option<bool>,
    /// Gemini项目ID（仅适用于Google Gemini提供商的OAuth认证）
    pub project_id: Option<String>,
}

/// 更新提供商密钥请求
#[derive(Debug, Deserialize)]
pub struct UpdateProviderKeyRequest {
    pub provider_type_id: i32,
    pub name: String,
    pub api_key: Option<String>,
    pub auth_type: String, // "api_key", "oauth", "service_account", "adc"
    // OAuth认证类型现在通过api_key字段传递session_id
    pub weight: Option<i32>,
    pub max_requests_per_minute: Option<i32>,
    pub max_tokens_prompt_per_minute: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub is_active: Option<bool>,
    /// Gemini项目ID（仅适用于Google Gemini提供商的OAuth认证）
    pub project_id: Option<String>,
}

/// 用户提供商密钥查询参数
#[derive(Debug, Deserialize)]
pub struct UserProviderKeyQuery {
    /// 服务商类型ID筛选
    pub provider_type_id: Option<i32>,
    /// 是否启用筛选
    pub is_active: Option<bool>,
}

/// 趋势数据查询参数
#[derive(Debug, Deserialize)]
pub struct TrendQuery {
    /// 查询天数，默认7天
    #[serde(default = "default_days")]
    pub days: u32,
}

/// 默认查询天数
fn default_days() -> u32 {
    7
}

/// 密钥使用统计
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProviderKeyUsageStats {
    pub total_requests: i64,
    pub successful_requests: i64,
    pub failed_requests: i64,
    pub success_rate: f64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub avg_response_time: i64,
    pub last_used_at: Option<String>,
}

/// 获取提供商密钥的使用统计数据
async fn fetch_provider_keys_usage_stats(
    db: &sea_orm::DatabaseConnection,
    provider_key_ids: &[i32],
) -> HashMap<i32, ProviderKeyUsageStats> {
    use entity::proxy_tracing::{Entity as ProxyTracing, Column};

    let mut stats_map = HashMap::new();

    if provider_key_ids.is_empty() {
        return stats_map;
    }

    // 批量查询所有密钥的使用记录
    let traces = match ProxyTracing::find()
        .filter(Column::UserProviderKeyId.is_in(provider_key_ids.to_vec()))
        .all(db)
        .await
    {
        Ok(records) => records,
        Err(err) => {
            tracing::error!("Failed to fetch proxy tracing records: {}", err);
            return stats_map;
        }
    };

    // 按密钥ID分组统计
    for trace in &traces {
        let key_id = match trace.user_provider_key_id {
            Some(id) => id,
            None => continue,
        };

        let entry = stats_map
            .entry(key_id)
            .or_insert_with(ProviderKeyUsageStats::default);

        // 统计请求数
        entry.total_requests += 1;
        if trace.is_success {
            entry.successful_requests += 1;
        } else {
            entry.failed_requests += 1;
        }

        // 统计token数
        if let Some(tokens) = trace.tokens_total {
            entry.total_tokens += tokens as i64;
        }

        // 统计费用
        if let Some(cost) = trace.cost {
            entry.total_cost += cost;
        }

        // 统计响应时间
        if let Some(duration) = trace.duration_ms {
            // 简单平均，实际应该加权平均
            entry.avg_response_time = (entry.avg_response_time + duration) / 2;
        }

        // 更新最后使用时间
        let created_at = trace
            .created_at
            .and_utc()
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        if entry.last_used_at.is_none()
            || entry
                .last_used_at
                .as_ref()
                .map_or(true, |last| last < &created_at)
        {
            entry.last_used_at = Some(created_at);
        }
    }

    // 计算成功率
    for stats in stats_map.values_mut() {
        if stats.total_requests > 0 {
            stats.success_rate =
                (stats.successful_requests as f64 / stats.total_requests as f64) * 100.0;
            stats.success_rate = (stats.success_rate * 100.0).round() / 100.0; // 保留两位小数
        }

        // 格式化费用
        stats.total_cost = (stats.total_cost * 100.0).round() / 100.0;
    }

    stats_map
}

/// 获取提供商密钥趋势数据
pub async fn get_provider_key_trends(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    Query(query): Query<TrendQuery>,
    headers: HeaderMap,
) -> axum::response::Response {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};
    

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // 验证密钥存在且属于当前用户
    match UserProviderKey::find()
        .filter(user_provider_keys::Column::Id.eq(key_id))
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .one(db)
        .await
    {
        Ok(Some(_)) => {
            // 密钥验证成功，继续查询趋势数据
        }
        Ok(None) => {
            return crate::manage_error!(crate::proxy_err!(
                business,
                "ProviderKey not found: {}",
                key_id
            ));
        }
        Err(err) => {
            tracing::error!("Failed to fetch provider key: {}", err);
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch provider key: {}",
                err
            ));
        }
    }

    // 计算时间范围
    let days = query.days.min(30); // 最多查询30天
    let end_date = Utc::now().naive_utc();
    let start_date = end_date - chrono::Duration::days(days as i64);

    // 查询趋势数据
    let trends = match fetch_key_trends_data(db, key_id, &start_date, &end_date, "provider").await {
        Ok(trends) => trends,
        Err(err) => {
            tracing::error!("Failed to fetch provider key trends: {}", err);
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch trends data: {}",
                err
            ));
        }
    };

    let data = json!({
        "daily_usage": trends.daily_usage,
        "daily_cost": trends.daily_cost,
        "daily_response_time": trends.daily_response_time,
        "dates": trends.dates,
        "summary": {
            "total_requests": trends.total_requests,
            "total_cost": trends.total_cost,
            "avg_response_time": trends.avg_response_time,
            "success_rate": trends.success_rate
        }
    });

    response::success(data)
}

/// 获取用户服务API趋势数据
pub async fn get_user_service_api_trends(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
    Query(query): Query<TrendQuery>,
    headers: HeaderMap,
) -> axum::response::Response {
    use entity::user_service_apis::{self, Entity as UserServiceApi};
    

    let db = state.database.as_ref();

    // 从JWT token中提取用户ID
    let user_id = match extract_user_id_from_headers(&headers) {
        Ok(id) => id,
        Err(error_response) => return error_response,
    };

    // 验证API存在且属于当前用户
    match UserServiceApi::find()
        .filter(user_service_apis::Column::Id.eq(api_id))
        .filter(user_service_apis::Column::UserId.eq(user_id))
        .one(db)
        .await
    {
        Ok(Some(_)) => {
            // API验证成功，继续查询趋势数据
        }
        Ok(None) => {
            return crate::manage_error!(crate::proxy_err!(
                business,
                "UserServiceApi not found: {}",
                api_id
            ));
        }
        Err(err) => {
            tracing::error!("Failed to fetch user service api: {}", err);
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch user service api: {}",
                err
            ));
        }
    }

    // 计算时间范围
    let days = query.days.min(30); // 最多查询30天
    let end_date = Utc::now().naive_utc();
    let start_date = end_date - chrono::Duration::days(days as i64);

    // 查询趋势数据
    let trends = match fetch_key_trends_data(db, api_id, &start_date, &end_date, "user_service").await {
        Ok(trends) => trends,
        Err(err) => {
            tracing::error!("Failed to fetch user service api trends: {}", err);
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch trends data: {}",
                err
            ));
        }
    };

    let data = json!({
        "daily_usage": trends.daily_usage,
        "daily_cost": trends.daily_cost,
        "daily_response_time": trends.daily_response_time,
        "dates": trends.dates,
        "summary": {
            "total_requests": trends.total_requests,
            "total_cost": trends.total_cost,
            "avg_response_time": trends.avg_response_time,
            "success_rate": trends.success_rate
        }
    });

    response::success(data)
}

/// 趋势数据结构
#[derive(Debug, Default, Serialize)]
struct TrendData {
    daily_usage: Vec<i64>,
    daily_cost: Vec<f64>,
    daily_response_time: Vec<i64>,
    dates: Vec<String>,
    total_requests: i64,
    total_cost: f64,
    avg_response_time: i64,
    success_rate: f64,
}

/// 获取趋势数据的通用函数
async fn fetch_key_trends_data(
    db: &sea_orm::DatabaseConnection,
    key_id: i32,
    start_date: &chrono::NaiveDateTime,
    end_date: &chrono::NaiveDateTime,
    key_type: &str, // "provider" 或 "user_service"
) -> Result<TrendData, sea_orm::DbErr> {
    use entity::proxy_tracing::{Entity as ProxyTracing, Column};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let mut trend_data = TrendData::default();

    // 构建查询条件
    let mut select = ProxyTracing::find()
        .filter(Column::CreatedAt.gte(*start_date))
        .filter(Column::CreatedAt.lte(*end_date));

    // 根据密钥类型选择过滤字段
    if key_type == "provider" {
        select = select.filter(Column::UserProviderKeyId.eq(key_id));
    } else {
        select = select.filter(Column::UserServiceApiId.eq(key_id));
    }

    // 获取所有追踪记录
    let traces = select.all(db).await?;

    // 按日期分组统计
    let mut daily_stats = std::collections::HashMap::new();

    for trace in &traces {
        let date_str = trace.created_at.format("%Y-%m-%d").to_string();
        let entry = daily_stats.entry(date_str).or_insert_with(|| DailyStats::default());

        entry.total_requests += 1;
        if trace.is_success {
            entry.successful_requests += 1;
        }
        entry.total_cost += trace.cost.unwrap_or(0.0);
        entry.total_response_time += trace.duration_ms.unwrap_or(0);
        entry.total_tokens += trace.tokens_total.unwrap_or(0) as i64;
    }

    // 生成日期序列和趋势数据
    let mut current_date = start_date.date();
    let end_date_only = end_date.date();

    while current_date <= end_date_only {
        let date_str = current_date.format("%Y-%m-%d").to_string();
        trend_data.dates.push(date_str.clone());

        if let Some(stats) = daily_stats.get(&date_str) {
            trend_data.daily_usage.push(stats.total_requests);
            trend_data.daily_cost.push(stats.total_cost);
            trend_data.daily_response_time.push(
                if stats.successful_requests > 0 {
                    stats.total_response_time / stats.successful_requests
                } else {
                    0
                }
            );

            // 累计汇总数据
            trend_data.total_requests += stats.total_requests;
            trend_data.total_cost += stats.total_cost;
            trend_data.success_rate += if stats.total_requests > 0 {
                (stats.successful_requests as f64 / stats.total_requests as f64) * 100.0
            } else {
                0.0
            };
        } else {
            // 没有数据的日期填充0
            trend_data.daily_usage.push(0);
            trend_data.daily_cost.push(0.0);
            trend_data.daily_response_time.push(0);
        }

        current_date += chrono::Duration::days(1);
    }

    // 计算平均响应时间和成功率
    if trend_data.total_requests > 0 {
        let mut total_response_time = 0i64;
        let mut successful_requests = 0i64;

        for trace in &traces {
            if let Some(duration) = trace.duration_ms {
                total_response_time += duration;
            }
            if trace.is_success {
                successful_requests += 1;
            }
        }

        trend_data.avg_response_time = if successful_requests > 0 {
            total_response_time / successful_requests
        } else {
            0
        };

        trend_data.success_rate = if !trend_data.dates.is_empty() {
            trend_data.success_rate / trend_data.dates.len() as f64
        } else {
            0.0
        };
    }

    // 四舍五入保留两位小数
    trend_data.success_rate = (trend_data.success_rate * 100.0).round() / 100.0;
    trend_data.total_cost = (trend_data.total_cost * 100.0).round() / 100.0;

    Ok(trend_data)
}

/// 每日统计结构
#[derive(Debug, Default)]
struct DailyStats {
    total_requests: i64,
    successful_requests: i64,
    total_cost: f64,
    total_response_time: i64,
    total_tokens: i64,
}
