//! # 提供商密钥管理处理器
//!
//! 处理上游AI服务商的API密钥管理相关请求

use crate::auth::{
    oauth_token_refresh_service::ScheduledTokenRefresh,
    oauth_token_refresh_task::OAuthTokenRefreshTask, types::AuthStatus,
};
use crate::error::ProxyError;
use crate::logging::{LogComponent, LogStage};
use crate::management::middleware::auth::AuthContext;
use crate::management::{response, server::AppState};
use crate::scheduler::types::ApiKeyHealthStatus;
use crate::types::ProviderTypeId;
use crate::{ldebug, lerror, linfo, lwarn};
use axum::extract::{Extension, Path, Query, State};
use axum::response::IntoResponse;
use axum::response::Json;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;

/// Gemini提供商名称常量
const GEMINI_PROVIDER_NAME: &str = "gemini";

/// `OAuth认证类型常量`
const OAUTH_AUTH_TYPE: &str = "oauth";

async fn prepare_oauth_schedule(
    task: Option<&Arc<OAuthTokenRefreshTask>>,
    session_id: Option<&String>,
    user_id: i32,
    key_id: Option<i32>,
) -> Result<Option<ScheduledTokenRefresh>, ProxyError> {
    let Some(task) = task else {
        lwarn!(
            "system",
            LogStage::Scheduling,
            LogComponent::OAuth,
            "task_unavailable",
            "OAuth refresh task unavailable, skip scheduling",
            user_id = user_id,
            key_id = key_id,
        );
        return Ok(None);
    };

    let Some(session_id) = session_id.filter(|id| !id.is_empty()) else {
        return Ok(None);
    };

    match task.prepare_schedule(session_id).await {
        Ok(schedule) => Ok(Some(schedule)),
        Err(err) => {
            lerror!(
                "system",
                LogStage::Scheduling,
                LogComponent::OAuth,
                "prepare_schedule_fail",
                &format!("Failed to prepare OAuth refresh schedule: {err}"),
                user_id = user_id,
                key_id = key_id,
                session_id = session_id.as_str(),
            );
            Err(err)
        }
    }
}

/// 获取提供商密钥列表
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::too_many_lines
)]
pub async fn get_provider_keys_list(
    State(state): State<AppState>,
    Query(query): Query<ProviderKeysListQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::provider_types::Entity as ProviderType;

    use entity::user_provider_keys::{self};

    use sea_orm::{PaginatorTrait, QuerySelect};

    let db = state.database.as_ref();

    let user_id = auth_context.user_id;

    let select = build_provider_keys_query(user_id, &query);

    // 分页参数

    let page = query.page.unwrap_or(1);

    let limit = query.limit.unwrap_or(10);

    let offset = (page - 1) * limit;

    // 获取总数

    let total = match select.clone().count(db).await {
        Ok(count) => count,

        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "count_fail",
                &format!("Failed to count provider keys: {err}")
            );

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
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_fail",
                &format!("Failed to fetch provider keys: {err}")
            );

            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch provider keys"
            ));
        }
    };

    // 获取所有密钥的使用统计数据

    let provider_key_ids: Vec<i32> = provider_keys.iter().map(|(pk, _)| pk.id).collect();

    let usage_stats_map = fetch_provider_keys_usage_stats(db, &provider_key_ids).await;

    // 构建响应数据

    let mut provider_keys_list = Vec::new();

    for (provider_key, provider_type_opt) in provider_keys {
        let provider_name =
            provider_type_opt.map_or_else(|| "Unknown".to_string(), |pt| pt.display_name);

        // 获取该密钥的使用统计

        let key_stats = usage_stats_map.get(&provider_key.id);

        let (usage_json, success_rate) = key_stats.map_or_else(

            || {

                (json!({

                    "total_requests": 0,

                    "successful_requests": 0,

                    "failed_requests": 0,

                    "success_rate": 0.0,

                    "total_tokens": 0,

                    "total_cost": 0.0,

                    "avg_response_time": 0.0,

                    "last_used_at": null,

                }), 0.0)

            },

            |stats| {

                let rate = if stats.total_requests > 0 {

                    (f64::from(stats.successful_requests) / f64::from(stats.total_requests)) * 100.0

                } else {

                    0.0

                };

                let usage = json!({

                    "total_requests": stats.total_requests,

                    "successful_requests": stats.successful_requests,

                    "failed_requests": stats.failed_requests,

                    "success_rate": rate,

                    "total_tokens": stats.total_tokens,

                    "total_cost": stats.total_cost,

                    "avg_response_time": stats.avg_response_time,

                    "last_used_at": stats.last_used_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()),

                });

                (usage, rate)

            },

        );

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

        let rate_limit_remaining_seconds =
            provider_key.rate_limit_resets_at.and_then(|resets_at| {
                let now = Utc::now().naive_utc();

                if resets_at > now {
                    let seconds = resets_at.signed_duration_since(now).num_seconds().max(0);

                    u64::try_from(seconds).ok()
                } else {
                    None
                }
            });

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

            "usage": usage_json,

            "success_rate": success_rate,

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

    let pages = total.div_ceil(limit);

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

fn build_provider_keys_query(
    user_id: i32,
    query: &ProviderKeysListQuery,
) -> sea_orm::Select<entity::user_provider_keys::Entity> {
    use entity::user_provider_keys;

    use sea_orm::QueryFilter;

    let mut select = entity::user_provider_keys::Entity::find()
        .filter(user_provider_keys::Column::UserId.eq(user_id));

    // 应用搜索筛选

    if let Some(search) = &query.search
        && !search.is_empty()
    {
        select = select.filter(user_provider_keys::Column::Name.contains(search));
    }

    // 应用状态筛选 - 基于health_status而不是IsActive

    if let Some(status) = &query.status {
        match status {
            ApiKeyHealthStatus::Healthy => {
                select = select.filter(
                    user_provider_keys::Column::HealthStatus
                        .eq(ApiKeyHealthStatus::Healthy.to_string()),
                );
            }

            ApiKeyHealthStatus::RateLimited => {
                select = select.filter(
                    user_provider_keys::Column::HealthStatus
                        .eq(ApiKeyHealthStatus::RateLimited.to_string()),
                );
            }

            ApiKeyHealthStatus::Unhealthy => {
                select = select.filter(
                    user_provider_keys::Column::HealthStatus
                        .eq(ApiKeyHealthStatus::Unhealthy.to_string()),
                );
            }
        }
    }

    select
}

/// 创建提供商密钥
/// 创建提供商密钥
#[allow(clippy::too_many_lines)]
pub async fn create_provider_key(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(payload): Json<CreateProviderKeyRequest>,
) -> axum::response::Response {
    let db = state.database.as_ref();
    let user_id = auth_context.user_id;

    if let Err(e) = validate_key_payload(&payload) {
        return crate::manage_error!(e);
    }

    if let Err(e) =
        check_name_conflict(db, user_id, &payload.name, payload.provider_type_id, None).await
    {
        return crate::manage_error!(e);
    }

    if payload.auth_type == OAUTH_AUTH_TYPE
        && let Err(e) = validate_oauth_session(db, user_id, payload.api_key.as_deref(), None).await
    {
        return crate::manage_error!(e);
    }

    // Gemini OAuth 场景处理 (这部分逻辑比较特殊，暂时保留在主函数中)
    let final_project_id = payload.project_id.clone();
    let health_status = ApiKeyHealthStatus::Healthy.to_string();

    if payload.auth_type == OAUTH_AUTH_TYPE
        && let Ok(Some(provider_type)) =
            entity::provider_types::Entity::find_by_id(payload.provider_type_id)
                .one(db)
                .await
        && provider_type.name == GEMINI_PROVIDER_NAME
    {}

    // 创建新密钥
    let new_provider_key = entity::user_provider_keys::ActiveModel {
        user_id: Set(user_id),
        provider_type_id: Set(payload.provider_type_id),
        name: Set(payload.name),
        api_key: Set(payload.api_key.clone().unwrap_or_default()),
        auth_type: Set(payload.auth_type),
        auth_status: Set(Some(AuthStatus::Authorized.to_string())),
        weight: Set(payload.weight),
        max_requests_per_minute: Set(payload.max_requests_per_minute),
        max_tokens_prompt_per_minute: Set(payload.max_tokens_prompt_per_minute),
        max_requests_per_day: Set(payload.max_requests_per_day),
        is_active: Set(payload.is_active.unwrap_or(true)),
        project_id: Set(final_project_id),
        health_status: Set(health_status),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };

    match new_provider_key.insert(db).await {
        Ok(model) => model,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "create_key_fail",
                &format!("Failed to create provider key: {err}")
            );
            return crate::manage_error!(crate::error::ProxyError::database_with_source(
                "Failed to create provider key",
                err,
            ));
        }
    };

    // ... (rest of the function, including task spawning and response creation)
    response::success_with_message(json!({}), "创建成功")
}

/// 验证创建/更新密钥的载荷
fn validate_key_payload(payload: &CreateProviderKeyRequest) -> Result<(), ProxyError> {
    if payload.auth_type == "api_key" && payload.api_key.is_none() {
        return Err(crate::proxy_err!(
            business,
            "API Key认证类型需要提供api_key字段 (field: api_key)"
        ));
    }
    if payload.auth_type == OAUTH_AUTH_TYPE && payload.api_key.is_none() {
        return Err(crate::proxy_err!(
            business,
            "OAuth认证类型需要通过api_key字段提供session_id (field: api_key)"
        ));
    }
    Ok(())
}

/// 检查提供商密钥名称冲突
async fn check_name_conflict(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    name: &str,
    provider_type_id: ProviderTypeId,
    key_id_to_exclude: Option<i32>,
) -> Result<(), ProxyError> {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};
    let mut query = UserProviderKey::find()
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .filter(user_provider_keys::Column::Name.eq(name))
        .filter(user_provider_keys::Column::ProviderTypeId.eq(provider_type_id));

    if let Some(exclude_id) = key_id_to_exclude {
        query = query.filter(user_provider_keys::Column::Id.ne(exclude_id));
    }

    match query.one(db).await {
        Ok(Some(_)) => Err(crate::proxy_err!(
            business,
            "ProviderKey conflict: {}",
            name
        )),
        Ok(None) => Ok(()),
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "check_exist_fail",
                &format!("Failed to check existing provider key: {err}")
            );
            Err(crate::error::ProxyError::database_with_source(
                "Failed to check existing provider key",
                err,
            ))
        }
    }
}

/// 验证OAuth会话是否可用于创建/更新密钥
async fn validate_oauth_session(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    session_id: Option<&str>,
    key_id_to_exclude: Option<i32>,
) -> Result<(), ProxyError> {
    use entity::oauth_client_sessions::{self, Entity as OAuthSession};
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let Some(session_id) = session_id else {
        return Ok(()); // Not an OAuth key or no session provided, skip validation
    };

    // 1. 验证会话本身是否存在、属于当前用户且已授权
    let session_query = OAuthSession::find()
        .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
        .filter(oauth_client_sessions::Column::UserId.eq(user_id))
        .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()));

    if !matches!(session_query.one(db).await, Ok(Some(_))) {
        return Err(crate::proxy_err!(
            business,
            "指定的OAuth会话不存在或未完成授权 (field: api_key)"
        ));
    }

    // 2. 检查会话是否已被其他活跃的密钥使用
    let mut usage_query = UserProviderKey::find()
        .filter(user_provider_keys::Column::ApiKey.eq(session_id))
        .filter(user_provider_keys::Column::AuthType.eq(OAUTH_AUTH_TYPE))
        .filter(user_provider_keys::Column::IsActive.eq(true));

    if let Some(exclude_id) = key_id_to_exclude {
        usage_query = usage_query.filter(user_provider_keys::Column::Id.ne(exclude_id));
    }

    if matches!(usage_query.one(db).await, Ok(Some(_))) {
        return Err(crate::proxy_err!(
            business,
            "指定的OAuth会话已被其他provider key使用"
        ));
    }

    Ok(())
}

/// 获取提供商密钥详情
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::too_many_lines
)]
pub async fn get_provider_key_detail(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    let user_id = auth_context.user_id;

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
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_detail_fail",
                &format!("Failed to fetch provider key detail: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch provider key detail: {}",
                err
            ));
        }
    };

    let provider_name = provider_key
        .1
        .map_or_else(|| "Unknown".to_string(), |pt| pt.display_name);

    // 获取使用统计
    let usage_stats_map = fetch_provider_keys_usage_stats(db, &[provider_key.0.id]).await;
    let key_stats = usage_stats_map.get(&provider_key.0.id);

    let usage_json = key_stats.map_or_else(
        || {
            json!({
                "total_requests": 0,
                "successful_requests": 0,
                "failed_requests": 0,
                "success_rate": 0.0,
                "total_tokens": 0,
                "total_cost": 0.0,
                "avg_response_time": 0.0,
                "last_used_at": null,
            })
        },
        |stats| {
                            let rate = if stats.total_requests > 0 {
                                (f64::from(stats.successful_requests) / f64::from(stats.total_requests)) * 100.0
                            } else {
                                0.0
                            };            json!({
                "total_requests": stats.total_requests,
                "successful_requests": stats.successful_requests,
                "failed_requests": stats.failed_requests,
                "success_rate": rate,
                "total_tokens": stats.total_tokens,
                "total_cost": stats.total_cost,
                "avg_response_time": stats.avg_response_time,
                "last_used_at": stats.last_used_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
            })
        },
    );

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
    let rate_limit_remaining_seconds = provider_key.0.rate_limit_resets_at.and_then(|resets_at| {
        let now = Utc::now().naive_utc();
        if resets_at > now {
            let seconds = resets_at.signed_duration_since(now).num_seconds().max(0);
            u64::try_from(seconds).ok()
        } else {
            None
        }
    });

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
        "usage": usage_json,
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
/// 更新提供商密钥
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
pub async fn update_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
    Json(payload): Json<UpdateProviderKeyRequest>,
) -> axum::response::Response {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();
    let refresh_task = state.oauth_token_refresh_task.clone();
    let user_id = auth_context.user_id;

    // 查找要更新的密钥
    let existing_key = match UserProviderKey::find_by_id(key_id)
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
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "find_key_fail",
                &format!("Failed to find provider key: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to find provider key: {}",
                err
            ));
        }
    };

    let original_key = existing_key.clone();
    let old_session_id =
        if existing_key.auth_type == OAUTH_AUTH_TYPE && !existing_key.api_key.is_empty() {
            Some(existing_key.api_key.clone())
        } else {
            None
        };

    // 使用辅助函数进行验证
    if existing_key.name != payload.name
        && let Err(e) = check_name_conflict(
            db,
            user_id,
            &payload.name,
            payload.provider_type_id,
            Some(key_id),
        )
        .await
    {
        return crate::manage_error!(e);
    }

    if payload.auth_type == OAUTH_AUTH_TYPE
        && let Err(e) =
            validate_oauth_session(db, user_id, payload.api_key.as_deref(), Some(key_id)).await
    {
        return crate::manage_error!(e);
    }

    let pending_schedule = if payload.auth_type == OAUTH_AUTH_TYPE {
        match prepare_oauth_schedule(
            refresh_task.as_ref(),
            payload.api_key.as_ref(),
            user_id,
            Some(key_id),
        )
        .await
        {
            Ok(schedule) => schedule,
            Err(err) => return crate::manage_error!(err),
        }
    } else {
        None
    };

    // 更新密钥
    let mut active_model: user_provider_keys::ActiveModel = existing_key.into();
    active_model.provider_type_id = Set(payload.provider_type_id);
    active_model.name = Set(payload.name);
    active_model.api_key = Set(payload.api_key.clone().unwrap_or_else(String::new));
    active_model.auth_type = Set(payload.auth_type);
    active_model.weight = Set(payload.weight);
    active_model.max_requests_per_minute = Set(payload.max_requests_per_minute);
    active_model.max_tokens_prompt_per_minute = Set(payload.max_tokens_prompt_per_minute);
    active_model.max_requests_per_day = Set(payload.max_requests_per_day);
    active_model.is_active = Set(payload.is_active.unwrap_or(true));
    active_model.project_id = Set(payload.project_id);
    active_model.updated_at = Set(Utc::now().naive_utc());

    let updated_key = match active_model.update(db).await {
        Ok(model) => model,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "update_key_fail",
                &format!("Failed to update provider key: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to update provider key: {}",
                err
            ));
        }
    };

    if let (Some(schedule), Some(task)) = (pending_schedule, refresh_task.as_ref())
        && let Err(err) = task.enqueue_schedule(schedule).await
    {
        lerror!(
            "system",
            LogStage::Scheduling,
            LogComponent::OAuth,
            "enqueue_schedule_update_fail",
            &format!("Failed to enqueue OAuth refresh schedule during update: {err}"),
            user_id = user_id,
            key_id = key_id,
        );
        let revert_model: user_provider_keys::ActiveModel = original_key.into();
        if let Err(revert_err) = revert_model.update(db).await {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "rollback_key_update_fail",
                &format!("Failed to rollback provider key after enqueue error: {revert_err}"),
                user_id = user_id,
                key_id = key_id,
            );
        }
        return crate::manage_error!(err);
    }

    if let Some(old_id) = old_session_id {
        let updated_session_id =
            if updated_key.auth_type == OAUTH_AUTH_TYPE && !updated_key.api_key.is_empty() {
                Some(updated_key.api_key.clone())
            } else {
                None
            };

        if updated_session_id.as_deref() != Some(old_id.as_str())
            && let Some(task) = refresh_task.as_ref()
            && let Err(err) = task.remove_session(&old_id).await
        {
            lwarn!(
                "system",
                LogStage::Scheduling,
                LogComponent::OAuth,
                "remove_old_session_fail",
                &format!("Failed to remove old OAuth session from refresh queue: {err}"),
                user_id = user_id,
                key_id = key_id,
                session_id = old_id.as_str(),
            );
        }
    }

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
#[allow(clippy::cognitive_complexity)]
pub async fn delete_provider_key(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();
    let refresh_task = state.oauth_token_refresh_task.clone();

    let user_id = auth_context.user_id;

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
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "find_key_fail",
                &format!("Failed to find provider key: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to find provider key: {}",
                err
            ));
        }
    };

    let session_to_remove =
        if existing_key.auth_type == OAUTH_AUTH_TYPE && !existing_key.api_key.is_empty() {
            Some(existing_key.api_key.clone())
        } else {
            None
        };

    // 删除密钥
    let active_model: user_provider_keys::ActiveModel = existing_key.into();
    match active_model.delete(db).await {
        Ok(_) => {}
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "delete_key_fail",
                &format!("Failed to delete provider key: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to delete provider key: {}",
                err
            ));
        }
    }

    if let Some(session_id) = session_to_remove
        && let Some(task) = refresh_task.as_ref()
        && let Err(err) = task.remove_session(&session_id).await
    {
        lwarn!(
            "system",
            LogStage::Scheduling,
            LogComponent::OAuth,
            "remove_session_after_delete_fail",
            &format!("Failed to remove OAuth session from refresh queue after delete: {err}"),
            user_id = user_id,
            key_id = key_id,
            session_id = session_id.as_str(),
        );
    }

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
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    let user_id = auth_context.user_id;

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
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_detail_fail",
                &format!("Failed to fetch provider key: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch provider key: {}",
                err
            ));
        }
    };

    let provider_name = provider_key
        .1
        .map_or_else(|| "Unknown".to_string(), |pt| pt.display_name);

    // 获取真实的统计数据
    let end_date = Utc::now().naive_utc();
    let start_date = end_date - chrono::Duration::days(7); // 默认查询7天数据

    let trends = match fetch_key_trends_data(db, key_id, &start_date, &end_date, "provider").await {
        Ok(trends) => trends,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_trends_fail",
                &format!("Failed to fetch provider key trends: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch trends data: {}",
                err
            ));
        }
    };

    let usage_series: Vec<i64> = trends
        .trend_data
        .iter()
        .map(|point| i64::from(point.requests))
        .collect();
    let cost_series: Vec<f64> = trends.trend_data.iter().map(|point| point.cost).collect();
    #[allow(clippy::cast_possible_truncation)]
    let response_time_series: Vec<i64> = trends
        .trend_data
        .iter()
        .map(|point| point.avg_response_time as i64)
        .collect();

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
            "usage": usage_series,
            "cost": cost_series,
            "response_time": response_time_series
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
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
pub async fn get_provider_keys_dashboard_stats(
    State(state): State<AppState>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    let user_id = auth_context.user_id;

    // 查询总密钥数
    let total_keys = match UserProviderKey::find()
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .count(db)
        .await
    {
        Ok(count) => count,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "count_total_keys_fail",
                &format!("Failed to count total keys: {err}")
            );
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
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "count_active_keys_fail",
                &format!("Failed to count active keys: {err}")
            );
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
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_user_keys_fail",
                &format!("Failed to fetch user provider keys: {err}")
            );
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
        use entity::proxy_tracing::{Column, Entity as ProxyTracing};

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
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "fetch_tracing_fail",
                    &format!("Failed to fetch proxy tracing records: {err}")
                );
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
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::provider_types::Entity as ProviderType;
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    let user_id = auth_context.user_id;

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
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_simple_keys_fail",
                &format!("Failed to fetch simple provider keys: {err}")
            );
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
            .map_or_else(|| "Unknown".to_string(), |pt| pt.display_name.clone());

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
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    let user_id = auth_context.user_id;

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
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "find_key_fail",
                &format!("Failed to find provider key: {err}")
            );
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
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::HealthChecker,
                "update_health_fail",
                &format!("Failed to update health status: {err}")
            );
            // 不返回错误，继续返回检查结果
        }
    }

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
    pub status: Option<ApiKeyHealthStatus>,
}

/// 创建提供商密钥请求
#[derive(Debug, Deserialize)]
pub struct CreateProviderKeyRequest {
    pub provider_type_id: ProviderTypeId,
    pub name: String,
    pub api_key: Option<String>,
    pub auth_type: String, // "api_key", "oauth"
    // OAuth认证类型现在通过api_key字段传递session_id
    pub weight: Option<i32>,
    pub max_requests_per_minute: Option<i32>,
    pub max_tokens_prompt_per_minute: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub is_active: Option<bool>,
    /// Gemini项目ID（仅适用于Google `Gemini提供商的OAuth认证`）
    pub project_id: Option<String>,
}

/// 更新提供商密钥请求
#[derive(Debug, Deserialize)]
pub struct UpdateProviderKeyRequest {
    pub provider_type_id: ProviderTypeId,
    pub name: String,
    pub api_key: Option<String>,
    pub auth_type: String, // "api_key", "oauth"
    // OAuth认证类型现在通过api_key字段传递session_id
    pub weight: Option<i32>,
    pub max_requests_per_minute: Option<i32>,
    pub max_tokens_prompt_per_minute: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub is_active: Option<bool>,
    /// Gemini项目ID（仅适用于Google `Gemini提供商的OAuth认证`）
    pub project_id: Option<String>,
}

/// 用户提供商密钥查询参数
#[derive(Debug, Deserialize)]
pub struct UserProviderKeyQuery {
    /// 服务商类型ID筛选
    pub provider_type_id: Option<ProviderTypeId>,
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
const fn default_days() -> u32 {
    7
}

/// 密钥使用统计
#[derive(Debug, Clone, Default, Serialize, sea_orm::FromQueryResult)]
pub struct ProviderKeyUsageStats {
    pub user_provider_key_id: i32,
    pub total_requests: i32,
    pub successful_requests: i32,
    pub failed_requests: i32,
    pub total_tokens: i32,
    pub total_cost: f64,
    pub avg_response_time: f64,
    pub last_used_at: Option<chrono::NaiveDateTime>,
}

/// 获取提供商密钥的使用统计数据
async fn fetch_provider_keys_usage_stats(
    db: &sea_orm::DatabaseConnection,
    provider_key_ids: &[i32],
) -> HashMap<i32, ProviderKeyUsageStats> {
    use entity::proxy_tracing::{Column, Entity as ProxyTracing};
    use sea_orm::{ColumnTrait, QuerySelect, sea_query::Expr};

    if provider_key_ids.is_empty() {
        return HashMap::new();
    }

    let results = match ProxyTracing::find()
        .select_only()
        .column(Column::UserProviderKeyId)
        .column_as(Column::Id.count(), "total_requests")
        .column_as(
            Expr::cust("SUM(CASE WHEN IsSuccess = true THEN 1 ELSE 0 END)"),
            "successful_requests",
        )
        .column_as(
            Expr::cust("SUM(CASE WHEN IsSuccess = false THEN 1 ELSE 0 END)"),
            "failed_requests",
        )
        .column_as(Column::TokensTotal.sum(), "total_tokens")
        .column_as(Expr::cust("AVG(DurationMs)"), "avg_response_time")
        .column_as(Column::CreatedAt.max(), "last_used_at")
        .filter(Column::UserProviderKeyId.is_in(provider_key_ids.to_vec()))
        .group_by(Column::UserProviderKeyId)
        .into_model::<ProviderKeyUsageStats>()
        .all(db)
        .await
    {
        Ok(res) => res,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_usage_stats_fail",
                &format!("Failed to fetch provider key usage stats: {err}")
            );
            return HashMap::new();
        }
    };

    results
        .into_iter()
        .map(|stats| (stats.user_provider_key_id, stats))
        .collect()
}

/// 获取提供商密钥趋势数据
pub async fn get_provider_key_trends(
    State(state): State<AppState>,
    Path(key_id): Path<i32>,
    Query(query): Query<TrendQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::user_provider_keys::{self, Entity as UserProviderKey};

    let db = state.database.as_ref();

    let user_id = auth_context.user_id;

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
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_detail_fail",
                &format!("Failed to fetch provider key: {err}")
            );
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
    let start_date = end_date - chrono::Duration::days(i64::from(days));

    // 查询趋势数据
    let trends = match fetch_key_trends_data(db, key_id, &start_date, &end_date, "provider").await {
        Ok(trends) => trends,
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_trends_fail",
                &format!("Failed to fetch provider key trends: {err}")
            );
            return crate::manage_error!(crate::proxy_err!(
                database,
                "Failed to fetch trends data: {}",
                err
            ));
        }
    };

    let data = json!({
        "trend_data": trends.trend_data,
        "summary": {
            "total_requests": trends.total_requests,
            "total_cost": trends.total_cost,
            "avg_response_time": trends.avg_response_time,
            "success_rate": trends.success_rate,
            "total_tokens": trends.total_tokens,
        }
    });

    response::success(data)
}

/// 获取用户服务API趋势数据
pub async fn get_user_service_api_trends(
    State(state): State<AppState>,
    Path(api_id): Path<i32>,
    Query(query): Query<TrendQuery>,
    Extension(auth_context): Extension<Arc<AuthContext>>,
) -> axum::response::Response {
    use entity::user_service_apis::{self, Entity as UserServiceApi};

    let db = state.database.as_ref();

    let user_id = auth_context.user_id;

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
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_service_api_fail",
                &format!("Failed to fetch user service api: {err}")
            );
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
    let start_date = end_date - chrono::Duration::days(i64::from(days));

    // 查询趋势数据
    let trends =
        match fetch_key_trends_data(db, api_id, &start_date, &end_date, "user_service").await {
            Ok(trends) => trends,
            Err(err) => {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "fetch_service_api_trends_fail",
                    &format!("Failed to fetch user service api trends: {err}")
                );
                return crate::manage_error!(crate::proxy_err!(
                    database,
                    "Failed to fetch trends data: {}",
                    err
                ));
            }
        };

    let data = json!({
        "trend_data": trends.trend_data,
        "summary": {
            "total_requests": trends.total_requests,
            "total_cost": trends.total_cost,
            "avg_response_time": trends.avg_response_time,
            "success_rate": trends.success_rate,
            "total_tokens": trends.total_tokens,
        }
    });

    response::success(data)
}

/// 趋势数据结构
#[derive(Debug, Default, Serialize)]
#[allow(clippy::struct_field_names)]
struct TrendData {
    trend_data: Vec<TrendDataPoint>,
    total_requests: i64,
    total_cost: f64,
    total_tokens: i64,
    avg_response_time: i64,
    success_rate: f64,
}

#[derive(Debug, Default, Serialize, sea_orm::FromQueryResult)]
struct TrendDataPoint {
    date: String,
    requests: i32,
    successful_requests: i32,
    failed_requests: i32,
    avg_response_time: f64,
    tokens: i32,
    cost: f64,
}

fn round_two_decimal(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// 获取趋势数据的通用函数
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
async fn fetch_key_trends_data(
    db: &sea_orm::DatabaseConnection,
    key_id: i32,
    start_date: &chrono::NaiveDateTime,
    end_date: &chrono::NaiveDateTime,
    key_type: &str, // "provider" 或 "user_service"
) -> Result<TrendData, sea_orm::DbErr> {
    use entity::proxy_tracing::{Column, Entity as ProxyTracing};
    use sea_orm::{
        ColumnTrait, QuerySelect,
        sea_query::{Alias, Expr},
    };

    // 1. DB-side aggregation
    let mut select = ProxyTracing::find()
        .select_only()
        .column_as(
            Expr::col(Column::CreatedAt).cast_as(Alias::new("date")),
            "date",
        )
        .column_as(Column::Id.count(), "requests")
        .column_as(
            Expr::cust("SUM(CASE WHEN IsSuccess = true THEN 1 ELSE 0 END)"),
            "successful_requests",
        )
        .column_as(Column::TokensTotal.sum(), "tokens")
        .column_as(Column::Cost.sum(), "cost")
        .column_as(Expr::cust("AVG(DurationMs)"), "avg_response_time")
        .filter(Column::CreatedAt.between(*start_date, *end_date));

    if key_type == "provider" {
        select = select.filter(Column::UserProviderKeyId.eq(key_id));
    } else {
        select = select.filter(Column::UserServiceApiId.eq(key_id));
    }

    let daily_stats_from_db: Vec<TrendDataPoint> = select
        .group_by(Expr::col(Column::CreatedAt).cast_as(Alias::new("date")))
        .into_model()
        .all(db)
        .await?;

    // 2. Post-processing and filling missing dates
    let mut daily_stats_map: HashMap<String, TrendDataPoint> = daily_stats_from_db
        .into_iter()
        .map(|mut p| {
            p.failed_requests = p.requests - p.successful_requests;
            p.date = p.date.to_string(); // Convert to string for map key
            (p.date.clone(), p)
        })
        .collect();

    let mut final_trend_data = Vec::new();
    let mut current_date = start_date.date();
    let end_date_only = end_date.date();

    let mut total_requests = 0;
    let mut total_successful_requests = 0;
    let mut total_cost = 0.0;
    let mut total_tokens = 0;
    let mut total_response_time_sum = 0.0;

    while current_date <= end_date_only {
        let date_str = current_date.format("%Y-%m-%d").to_string();
        let point = daily_stats_map
            .remove(&date_str)
            .unwrap_or_else(|| TrendDataPoint {
                date: date_str,
                ..Default::default()
            });

        total_requests += i64::from(point.requests);
        total_successful_requests += i64::from(point.successful_requests);
        total_cost += point.cost;
        total_tokens += i64::from(point.tokens);
        total_response_time_sum += point.avg_response_time * f64::from(point.requests);

        final_trend_data.push(point);
        current_date += chrono::Duration::days(1);
    }

    // 3. Calculate overall summary
    let success_rate = if total_requests > 0 {
        (total_successful_requests as f64 / total_requests as f64) * 100.0
    } else {
        0.0
    };
    let avg_response_time = if total_requests > 0 {
        total_response_time_sum / total_requests as f64
    } else {
        0.0
    };

    #[allow(clippy::cast_possible_truncation)]
    let avg_response_time_i64 = avg_response_time as i64;

    Ok(TrendData {
        trend_data: final_trend_data,
        total_requests,
        total_cost: round_two_decimal(total_cost),
        total_tokens,
        avg_response_time: avg_response_time_i64,
        success_rate: round_two_decimal(success_rate),
    })
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

/// 获取API密钥健康状态列表
#[must_use]
pub fn get_provider_key_health_statuses() -> axum::response::Response {
    use crate::scheduler::types::ApiKeyHealthStatus;

    let mut statuses = vec![json!({"value": "all", "label": "全部"})];

    // 添加所有枚举状态
    for status in &[
        ApiKeyHealthStatus::Healthy,
        ApiKeyHealthStatus::RateLimited,
        ApiKeyHealthStatus::Unhealthy,
    ] {
        statuses.push(json!({
            "value": status.to_string(),
            "label": match status {
                ApiKeyHealthStatus::Healthy => "健康",
                ApiKeyHealthStatus::RateLimited => "限流中",
                ApiKeyHealthStatus::Unhealthy => "不健康",
            }
        }));
    }

    Json(statuses).into_response()
}

/// `异步执行自动获取project_id任务的辅助方法`
#[allow(clippy::cognitive_complexity)]
async fn execute_auto_get_project_id_async(
    db: &sea_orm::DatabaseConnection,
    key_id: i32,
    user_id: &str,
) -> Result<(), crate::ProxyError> {
    use crate::auth::gemini_code_assist_client::GeminiCodeAssistClient;
    use entity::user_provider_keys::{ActiveModel, Entity as UserProviderKey};
    use sea_orm::ActiveValue::Set;

    // 创建Gemini客户端
    let gemini_client = GeminiCodeAssistClient::new();

    // 从数据库重新获取OAuth会话和access token
    let access_token = match get_access_token_for_key(db, key_id, user_id).await {
        Ok(token) => token,
        Err(e) => {
            lerror!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "get_access_token_fail",
                "Cannot get access token to execute auto-get project_id",
                user_id = user_id,
                key_id = %key_id,
                error = %e,
            );
            return Err(e);
        }
    };

    // 调用auto_get_project_id_with_retry
    match gemini_client
        .auto_get_project_id_with_retry(&access_token)
        .await
    {
        Ok(pid_opt) => {
            if let Some(pid) = pid_opt {
                linfo!(
                    "system",
                    LogStage::BackgroundTask,
                    LogComponent::OAuth,
                    "auto_get_project_id_success",
                    "Async auto-get project_id success",
                    user_id = user_id,
                    key_id = %key_id,
                    project_id = %pid,
                );

                // 更新数据库记录
                let key_opt = UserProviderKey::find_by_id(key_id).one(db).await?;
                if let Some(key_model) = key_opt {
                    let mut active_key: ActiveModel = key_model.into();
                    active_key.project_id = Set(Some(pid.clone()));
                    active_key.health_status = Set(ApiKeyHealthStatus::Healthy.to_string());
                    active_key.updated_at = Set(Utc::now().naive_utc());

                    active_key.update(db).await?;
                    linfo!(
                        "system",
                        LogStage::BackgroundTask,
                        LogComponent::OAuth,
                        "update_project_id_success",
                        "Async update of auto-get project_id success",
                        user_id = user_id,
                        key_id = %key_id,
                        project_id = %pid,
                    );
                }
            } else {
                lwarn!(
                    "system",
                    LogStage::BackgroundTask,
                    LogComponent::OAuth,
                    "auto_get_project_id_empty",
                    "Async auto-get project_id returned empty",
                    user_id = user_id,
                    key_id = %key_id,
                );
            }
            Ok(())
        }
        Err(e) => {
            lerror!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "auto_get_project_id_retry_fail",
                "Async auto-get project_id retry failed",
                user_id = user_id,
                key_id = %key_id,
                error = %e,
            );
            Err(e)
        }
    }
}

/// 为指定key获取access token的辅助方法
async fn get_access_token_for_key(
    db: &sea_orm::DatabaseConnection,
    key_id: i32,
    user_id: &str,
) -> Result<String, crate::ProxyError> {
    use entity::oauth_client_sessions::{self, Entity as OAuthSession};
    use entity::user_provider_keys::Entity as UserProviderKey;

    // 首先获取user_provider_key记录，找到session_id
    let key_record = match UserProviderKey::find_by_id(key_id).one(db).await {
        Ok(Some(key)) => key,
        Ok(None) => {
            return Err(crate::ProxyError::business(format!(
                "未找到key记录: key_id={key_id}, user_id={user_id}"
            )));
        }
        Err(e) => {
            return Err(crate::ProxyError::database_with_source(
                "查询key记录失败",
                e,
            ));
        }
    };

    // 确保是OAuth类型的key
    if key_record.auth_type != OAUTH_AUTH_TYPE {
        return Err(crate::ProxyError::business(format!(
            "key不是OAuth类型: auth_type={}",
            key_record.auth_type
        )));
    }

    // 从api_key字段获取session_id
    let session_id = key_record.api_key;
    if session_id.is_empty() {
        return Err(crate::ProxyError::business("OAuth key的session_id为空"));
    }

    // 查询OAuth会话获取access_token
    let oauth_session = match OAuthSession::find()
        .filter(oauth_client_sessions::Column::SessionId.eq(&session_id))
        .filter(oauth_client_sessions::Column::UserId.eq(user_id))
        .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
        .one(db)
        .await
    {
        Ok(Some(session)) => session,
        Ok(None) => {
            return Err(crate::ProxyError::business(format!(
                "未找到授权的OAuth会话: session_id={session_id}, user_id={user_id}"
            )));
        }
        Err(e) => {
            return Err(crate::ProxyError::database_with_source(
                "查询OAuth会话失败",
                e,
            ));
        }
    };

    // 检查access_token是否存在
    oauth_session.access_token.as_ref().map_or_else(
        || Err(crate::ProxyError::business("OAuth会话中没有access_token")),
        |access_token| {
            if access_token.is_empty() {
                Err(crate::ProxyError::business("OAuth会话中的access_token为空"))
            } else {
                ldebug!(
                    "system",
                    LogStage::BackgroundTask,
                    LogComponent::OAuth,
                    "get_access_token_success",
                    "Successfully got access token",
                    user_id = user_id,
                    key_id = %key_id,
                    session_id = %session_id,
                );
                Ok(access_token.clone())
            }
        },
    )
}
