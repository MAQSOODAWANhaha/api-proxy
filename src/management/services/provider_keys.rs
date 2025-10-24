//! # 提供商密钥服务
//!
//! 聚合管理端提供商密钥的业务逻辑，逐步吸收 handler 中的实现以便复用。

use std::{collections::HashMap, convert::TryFrom, sync::Arc};

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use entity::{
    oauth_client_sessions, oauth_client_sessions::Entity as OAuthSession, provider_types,
    provider_types::Entity as ProviderType, proxy_tracing::Entity as ProxyTracing,
    user_provider_keys, user_provider_keys::Entity as UserProviderKey, user_service_apis,
    user_service_apis::Entity as UserServiceApi,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, FromQueryResult,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::spawn;

const GEMINI_PROVIDER_NAME: &str = "gemini";
const OAUTH_AUTH_TYPE: &str = "oauth";

use crate::{
    auth::{
        gemini_code_assist_client::GeminiCodeAssistClient,
        oauth_token_refresh_service::ScheduledTokenRefresh,
        oauth_token_refresh_task::OAuthTokenRefreshTask, types::AuthStatus,
    },
    error::{ProxyError, Result, auth::AuthError},
    key_pool::types::ApiKeyHealthStatus,
    ldebug, lerror, linfo,
    logging::{LogComponent, LogStage},
    lwarn,
    management::server::ManagementState,
    types::{ProviderTypeId, TimezoneContext, ratio_as_percentage, timezone_utils},
};

use super::shared::ServiceResponse;

/// 提供商密钥列表查询参数
#[derive(Debug, Deserialize)]
pub struct ProviderKeysListQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
    pub search: Option<String>,
    pub provider: Option<String>,
    pub status: Option<ApiKeyHealthStatus>,
}

/// 创建提供商密钥请求
#[derive(Debug, Deserialize)]
pub struct CreateProviderKeyRequest {
    pub provider_type_id: ProviderTypeId,
    pub name: String,
    pub api_key: Option<String>,
    pub auth_type: String,
    pub weight: Option<i32>,
    pub max_requests_per_minute: Option<i32>,
    pub max_tokens_prompt_per_minute: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub is_active: Option<bool>,
    pub project_id: Option<String>,
}

/// 更新提供商密钥请求
#[derive(Debug, Deserialize)]
pub struct UpdateProviderKeyRequest {
    pub provider_type_id: ProviderTypeId,
    pub name: String,
    pub api_key: Option<String>,
    pub auth_type: String,
    pub weight: Option<i32>,
    pub max_requests_per_minute: Option<i32>,
    pub max_tokens_prompt_per_minute: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub is_active: Option<bool>,
    pub project_id: Option<String>,
}

/// 密钥使用统计
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProviderKeyUsageStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub success_rate: f64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub avg_response_time: i64,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Default, FromQueryResult)]
struct ProviderKeyStatsRow {
    user_provider_key_id: Option<i32>,
    total_requests: Option<i64>,
    successful_requests: Option<i64>,
    total_tokens: Option<i64>,
    total_cost: Option<f64>,
    total_duration: Option<i64>,
    duration_count: Option<i64>,
    last_used_at: Option<chrono::NaiveDateTime>,
}

/// 用户提供商密钥查询参数
#[derive(Debug, Deserialize)]
pub struct UserProviderKeyQuery {
    pub provider_type_id: Option<ProviderTypeId>,
    pub is_active: Option<bool>,
}

/// 趋势查询参数
#[derive(Debug, Deserialize)]
pub struct TrendQuery {
    #[serde(default = "default_days")]
    pub days: u32,
}

const fn default_days() -> u32 {
    7
}

#[derive(Debug, Default, Serialize, Clone)]
struct TrendData {
    #[serde(rename = "trend_data")]
    points: Vec<TrendDataPoint>,
    total_requests: i64,
    total_cost: f64,
    total_tokens: i64,
    avg_response_time: i64,
    success_rate: f64,
    #[serde(skip_serializing)]
    total_successful_requests: i64,
}

#[derive(Debug, Default, Serialize, Clone)]
struct TrendDataPoint {
    date: String,
    requests: i64,
    successful_requests: i64,
    failed_requests: i64,
    success_rate: f64,
    avg_response_time: i64,
    tokens: i64,
    cost: f64,
}

#[derive(Debug, Default)]
struct DailyStats {
    total_requests: i64,
    successful_requests: i64,
    total_cost: f64,
    total_response_time: i64,
    total_tokens: i64,
}

fn aggregate_daily_stats(
    traces: &[entity::proxy_tracing::Model],
    timezone: &TimezoneContext,
) -> HashMap<String, DailyStats> {
    let mut daily_stats = HashMap::new();
    for trace in traces {
        let date_str = timezone_utils::local_date_label(&trace.created_at, &timezone.timezone);
        let entry = daily_stats
            .entry(date_str)
            .or_insert_with(DailyStats::default);
        entry.total_requests += 1;
        if trace.is_success {
            entry.successful_requests += 1;
        }
        entry.total_cost += trace.cost.unwrap_or(0.0);
        entry.total_response_time += trace.duration_ms.unwrap_or(0);
        entry.total_tokens += i64::from(trace.tokens_total.unwrap_or(0));
    }
    daily_stats
}

/// 提供商密钥服务入口
pub struct ProviderKeyService<'a> {
    state: &'a ManagementState,
    db: &'a DatabaseConnection,
    refresh_task: Option<Arc<OAuthTokenRefreshTask>>,
}

impl<'a> ProviderKeyService<'a> {
    #[must_use]
    pub fn new(state: &'a ManagementState) -> Self {
        Self {
            state,
            db: state.database.as_ref(),
            refresh_task: Some(state.oauth_token_refresh_task.clone()),
        }
    }

    #[must_use]
    const fn db(&self) -> &'a DatabaseConnection {
        self.db
    }

    #[must_use]
    const fn refresh_task(&self) -> Option<&Arc<OAuthTokenRefreshTask>> {
        self.refresh_task.as_ref()
    }

    /// 获取提供商密钥列表
    pub async fn list(
        &self,
        user_id: i32,
        timezone_context: &TimezoneContext,
        query: &ProviderKeysListQuery,
    ) -> Result<ServiceResponse<Value>> {
        let mut select =
            UserProviderKey::find().filter(user_provider_keys::Column::UserId.eq(user_id));

        if let Some(search) = query.search.as_ref().filter(|s| !s.is_empty()) {
            select = select.filter(user_provider_keys::Column::Name.contains(search));
        }

        if let Some(status) = &query.status {
            select = select.filter(user_provider_keys::Column::HealthStatus.eq(status.to_string()));
        }

        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(10).max(1);
        let offset = (page - 1) * limit;

        let total = select.clone().count(self.db()).await.map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "count_fail",
                &format!("Failed to count provider keys: {err}")
            );
            crate::error!(Database, format!("Failed to count provider keys: {err}"))
        })?;

        let provider_keys = select
            .find_also_related(ProviderType)
            .offset(offset)
            .limit(limit)
            .order_by_desc(user_provider_keys::Column::CreatedAt)
            .all(self.db())
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "fetch_fail",
                    &format!("Failed to fetch provider keys: {err}")
                );
                crate::error!(Database, format!("Failed to fetch provider keys: {err}"))
            })?;

        let provider_key_ids: Vec<i32> = provider_keys.iter().map(|(pk, _)| pk.id).collect();
        let usage_stats =
            fetch_provider_keys_usage_stats(self.db(), &provider_key_ids, timezone_context).await;

        let provider_keys_list = provider_keys
            .into_iter()
            .map(|(provider_key, provider_type_opt)| {
                build_provider_key_json(
                    &provider_key,
                    provider_type_opt,
                    &usage_stats,
                    timezone_context,
                )
            })
            .collect::<Vec<_>>();

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

        Ok(ServiceResponse::new(data))
    }

    /// 创建提供商密钥
    pub async fn create(
        &self,
        user_id: i32,
        timezone_context: &TimezoneContext,
        payload: &CreateProviderKeyRequest,
    ) -> Result<ServiceResponse<Value>> {
        ensure_unique_provider_key(self.db(), user_id, payload).await?;
        validate_create_payload(payload)?;
        validate_oauth_session_for_creation(self.db(), user_id, payload).await?;

        let PrepareGeminiContext {
            final_project_id,
            health_status,
            needs_auto_get_project_id_async,
        } = prepare_gemini_context(self.db(), user_id, payload).await?;

        let pending_schedule =
            schedule_oauth_if_needed(self.refresh_task(), payload, user_id, None).await?;

        let record = insert_provider_key_record(
            self.db(),
            user_id,
            payload,
            final_project_id,
            health_status,
        )
        .await?;

        enqueue_oauth_schedule(
            self.refresh_task(),
            pending_schedule,
            self.db(),
            user_id,
            &record,
        )
        .await?;

        spawn_gemini_project_task(
            needs_auto_get_project_id_async,
            self.db().clone(),
            user_id,
            record.id,
        );

        self.state
            .key_pool_service
            .register_new_key(record.id)
            .await?;

        let provider_name = ProviderType::find_by_id(payload.provider_type_id)
            .one(self.db())
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "fetch_provider_type_fail",
                    &format!("Failed to fetch provider type: {err}")
                );
                crate::error!(Database, format!("Failed to fetch provider type: {err}"))
            })?
            .map_or_else(|| "Unknown".to_string(), |provider| provider.display_name);

        let mut message = "创建成功".to_string();
        if needs_auto_get_project_id_async {
            message.push_str("，正在后台自动获取 project_id，请稍后查看 key 状态");
        }

        let data = json!({
            "id": record.id,
            "provider": provider_name,
            "name": record.name,
            "auth_type": record.auth_type,
            "auth_status": record.auth_status,
            "health_status": record.health_status,
            "project_id": record.project_id,
            "has_background_tasks": needs_auto_get_project_id_async,
            "background_tasks": {
                "auto_get_project_id_pending": needs_auto_get_project_id_async
            },
            "created_at": timezone_utils::format_naive_utc_for_response(
                &record.created_at,
                &timezone_context.timezone
            )
        });

        Ok(ServiceResponse::with_message(data, message))
    }

    /// 更新提供商密钥
    pub async fn update(
        &self,
        key_id: i32,
        user_id: i32,
        timezone_context: &TimezoneContext,
        payload: &UpdateProviderKeyRequest,
    ) -> Result<ServiceResponse<Value>> {
        let db = self.db();
        let refresh_task = self.refresh_task();

        let existing_key = self.load_existing_key(key_id, user_id).await?;
        self.ensure_unique_name(user_id, key_id, &existing_key, payload)
            .await?;
        validate_update_requirements(payload)?;
        if payload.auth_type == OAUTH_AUTH_TYPE {
            validate_oauth_session_for_update(db, user_id, key_id, payload).await?;
        }

        let pending_schedule = self
            .prepare_pending_schedule(refresh_task, payload, user_id, key_id)
            .await?;

        let original_key = existing_key.clone();
        let old_session_id = extract_oauth_session_id(&existing_key);
        let updated_key = self.persist_updated_key(existing_key, payload).await?;

        self.enqueue_pending_schedule(
            refresh_task,
            pending_schedule,
            original_key,
            &updated_key,
            user_id,
            key_id,
        )
        .await?;

        self.cleanup_obsolete_session(refresh_task, old_session_id, &updated_key, user_id, key_id)
            .await;

        self.state.key_pool_service.refresh_key(key_id).await?;

        let payload = build_update_response(&updated_key, timezone_context);
        Ok(ServiceResponse::with_message(payload, "更新成功"))
    }

    /// 获取提供商密钥详情
    pub async fn detail(
        &self,
        user_id: i32,
        timezone_context: &TimezoneContext,
        key_id: i32,
    ) -> Result<ServiceResponse<Value>> {
        let (key, provider_type_opt) = self.load_key_with_provider(key_id, user_id).await?;
        let provider_name =
            provider_type_opt.map_or_else(|| "Unknown".to_string(), |pt| pt.display_name);

        let usage_stats =
            fetch_provider_keys_usage_stats(self.db(), &[key.id], timezone_context).await;
        let key_stats = usage_stats.get(&key.id).cloned().unwrap_or_default();
        let api_key_value = if key.auth_type == "api_key" {
            mask_api_key(&key)
        } else {
            key.api_key.clone()
        };
        let rate_limit_remaining_seconds = rate_limit_remaining_seconds(&key, user_id);

        let data = json!({
            "id": key.id,
            "provider": provider_name,
            "name": key.name,
            "api_key": api_key_value,
            "auth_type": key.auth_type,
            "auth_status": key.auth_status,
            "expires_at": key.expires_at.map(|dt|
                timezone_utils::format_naive_utc_for_response(&dt, &timezone_context.timezone)
            ),
            "last_auth_check": key.last_auth_check.map(|dt|
                timezone_utils::format_naive_utc_for_response(&dt, &timezone_context.timezone)
            ),
            "weight": key.weight,
            "max_requests_per_minute": key.max_requests_per_minute,
            "max_tokens_prompt_per_minute": key.max_tokens_prompt_per_minute,
            "max_requests_per_day": key.max_requests_per_day,
            "is_active": key.is_active,
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
                "max_requests_per_minute": key.max_requests_per_minute,
                "max_tokens_prompt_per_minute": key.max_tokens_prompt_per_minute,
                "max_requests_per_day": key.max_requests_per_day
            },
            "status": {
                "is_active": key.is_active,
                "health_status": key.health_status,
                "rate_limit_remaining_seconds": rate_limit_remaining_seconds
            },
            "created_at": timezone_utils::format_naive_utc_for_response(
                &key.created_at,
                &timezone_context.timezone
            ),
            "updated_at": timezone_utils::format_naive_utc_for_response(
                &key.updated_at,
                &timezone_context.timezone
            )
        });

        Ok(ServiceResponse::new(data))
    }

    /// 删除提供商密钥
    pub async fn delete(
        &self,
        user_id: i32,
        timezone_context: &TimezoneContext,
        key_id: i32,
    ) -> Result<ServiceResponse<Value>> {
        let refresh_task = self.refresh_task();
        let existing_key = self.load_existing_key(key_id, user_id).await?;

        let session_to_remove =
            if existing_key.auth_type == OAUTH_AUTH_TYPE && !existing_key.api_key.is_empty() {
                Some(existing_key.api_key.clone())
            } else {
                None
            };

        let active_model: user_provider_keys::ActiveModel = existing_key.into();
        active_model.delete(self.db()).await.map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "delete_key_fail",
                &format!("Failed to delete provider key: {err}")
            );
            crate::error!(Database, format!("Failed to delete provider key: {err}"))
        })?;

        if let (Some(session_id), Some(task)) = (session_to_remove.as_ref(), refresh_task)
            && let Err(err) = task.remove_session(session_id.as_str()).await
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

        self.state.key_pool_service.remove_key(key_id).await?;

        let data = json!({
            "id": key_id,
            "deleted_at": timezone_utils::format_utc_for_response(
                &Utc::now(),
                &timezone_context.timezone
            )
        });

        Ok(ServiceResponse::with_message(data, "删除成功"))
    }

    /// 获取密钥统计信息
    pub async fn stats(
        &self,
        user_id: i32,
        timezone_context: &TimezoneContext,
        key_id: i32,
    ) -> Result<ServiceResponse<Value>> {
        let (provider_key, provider_type_opt) =
            self.load_key_with_provider(key_id, user_id).await?;

        let provider_name =
            provider_type_opt.map_or_else(|| "Unknown".to_string(), |pt| pt.display_name);

        let now = Utc::now();
        let (_, today_end_utc) = timezone_utils::local_day_bounds(&now, &timezone_context.timezone)
            .unwrap_or((now - Duration::days(1), now));
        let start_utc = today_end_utc - Duration::days(7);

        let trends = fetch_key_trends_data(
            self.db(),
            key_id,
            &start_utc,
            &today_end_utc,
            "provider",
            timezone_context,
        )
        .await
        .map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_trends_fail",
                &format!("Failed to fetch provider key trends: {err}")
            );
            crate::error!(Database, format!("Failed to fetch trends data: {err}"))
        })?;

        let usage_series: Vec<i64> = trends.points.iter().map(|point| point.requests).collect();
        let cost_series: Vec<f64> = trends.points.iter().map(|point| point.cost).collect();
        let response_time_series: Vec<i64> = trends
            .points
            .iter()
            .map(|point| point.avg_response_time)
            .collect();

        let data = json!({
            "basic_info": {
                "provider": provider_name,
                "name": provider_key.name,
                "weight": provider_key.weight
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
                "max_requests_per_minute": provider_key.max_requests_per_minute,
                "max_tokens_prompt_per_minute": provider_key.max_tokens_prompt_per_minute,
                "max_requests_per_day": provider_key.max_requests_per_day
            }
        });

        Ok(ServiceResponse::new(data))
    }

    /// 获取密钥总览统计
    pub async fn dashboard(&self, user_id: i32) -> Result<ServiceResponse<Value>> {
        use entity::user_provider_keys::{self, Entity as UserProviderKey};

        let total_keys = UserProviderKey::find()
            .filter(user_provider_keys::Column::UserId.eq(user_id))
            .count(self.db())
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "count_total_keys_fail",
                    &format!("Failed to count total keys: {err}")
                );
                crate::error!(Database, format!("Failed to count total keys: {err}"))
            })?;

        let active_keys = UserProviderKey::find()
            .filter(user_provider_keys::Column::UserId.eq(user_id))
            .filter(user_provider_keys::Column::IsActive.eq(true))
            .count(self.db())
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "count_active_keys_fail",
                    &format!("Failed to count active keys: {err}")
                );
                crate::error!(Database, format!("Failed to count active keys: {err}"))
            })?;

        let user_provider_key_ids: Vec<i32> = UserProviderKey::find()
            .filter(user_provider_keys::Column::UserId.eq(user_id))
            .all(self.db())
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "fetch_user_keys_fail",
                    &format!("Failed to fetch user provider keys: {err}")
                );
                crate::error!(
                    Database,
                    format!("Failed to fetch user provider keys: {err}")
                )
            })?
            .into_iter()
            .map(|k| k.id)
            .collect();

        let (total_usage, total_cost) = if user_provider_key_ids.is_empty() {
            (0u64, 0.0f64)
        } else {
            use entity::proxy_tracing::{Column, Entity as ProxyTracing};

            ProxyTracing::find()
                .filter(Column::UserProviderKeyId.is_in(user_provider_key_ids))
                .filter(Column::IsSuccess.eq(true))
                .all(self.db())
                .await
                .map(|records| {
                    let usage_count = records.len() as u64;
                    let cost_sum: f64 = records.iter().filter_map(|record| record.cost).sum();
                    (usage_count, cost_sum)
                })
                .map_err(|err| {
                    lerror!(
                        "system",
                        LogStage::Db,
                        LogComponent::Database,
                        "fetch_tracing_fail",
                        &format!("Failed to fetch proxy tracing records: {err}")
                    );
                    crate::error!(Database, format!("Failed to fetch usage statistics: {err}"))
                })?
        };

        let data = json!({
            "total_keys": total_keys,
            "active_keys": active_keys,
            "total_usage": total_usage,
            "total_cost": total_cost
        });

        Ok(ServiceResponse::new(data))
    }

    /// 获取简单提供商密钥列表
    pub async fn simple_list(
        &self,
        user_id: i32,
        query: &UserProviderKeyQuery,
    ) -> Result<ServiceResponse<Value>> {
        use entity::provider_types::Entity as ProviderType;
        use entity::user_provider_keys::{self, Entity as UserProviderKey};

        let mut select =
            UserProviderKey::find().filter(user_provider_keys::Column::UserId.eq(user_id));

        if let Some(provider_type_id) = query.provider_type_id {
            select = select.filter(user_provider_keys::Column::ProviderTypeId.eq(provider_type_id));
        }

        if let Some(is_active) = query.is_active {
            select = select.filter(user_provider_keys::Column::IsActive.eq(is_active));
        }

        let provider_keys = select
            .find_also_related(ProviderType)
            .order_by_desc(user_provider_keys::Column::CreatedAt)
            .all(self.db())
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "fetch_simple_keys_fail",
                    &format!("Failed to fetch simple provider keys: {err}")
                );
                crate::error!(Database, format!("Failed to fetch provider keys: {err}"))
            })?;

        let provider_keys_list = provider_keys
            .into_iter()
            .map(|(provider_key, provider_type_opt)| {
                let provider_name = provider_type_opt
                    .as_ref()
                    .map_or_else(|| "Unknown".to_string(), |pt| pt.display_name.clone());

                let display_name = format!("{} ({})", provider_key.name, provider_name);

                json!({
                    "id": provider_key.id,
                    "name": provider_key.name,
                    "display_name": display_name,
                    "provider": provider_name,
                    "provider_type_id": provider_key.provider_type_id,
                    "is_active": provider_key.is_active
                })
            })
            .collect::<Vec<_>>();

        Ok(ServiceResponse::new(
            json!({ "provider_keys": provider_keys_list }),
        ))
    }

    /// 执行健康检查（占位实现）
    pub async fn health_check(
        &self,
        user_id: i32,
        timezone_context: &TimezoneContext,
        key_id: i32,
    ) -> Result<ServiceResponse<Value>> {
        use entity::user_provider_keys::{self, Entity as UserProviderKey};

        let existing_key = UserProviderKey::find()
            .filter(user_provider_keys::Column::Id.eq(key_id))
            .filter(user_provider_keys::Column::UserId.eq(user_id))
            .one(self.db())
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "find_key_fail",
                    &format!("Failed to find provider key: {err}")
                );
                crate::error!(Database, format!("Failed to find provider key: {err}"))
            })?
            .ok_or_else(|| {
                ProxyError::Authentication(AuthError::Message(format!(
                    "ProviderKey not found: {key_id}"
                )))
            })?;

        let health_status = "healthy";
        let response_time = 245;
        let check_time = Utc::now();

        let mut active_model: user_provider_keys::ActiveModel = existing_key.into();
        active_model.health_status = Set(health_status.to_string());
        active_model.updated_at = Set(check_time.naive_utc());

        if let Err(err) = active_model.update(self.db()).await {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::HealthChecker,
                "update_health_fail",
                &format!("Failed to update health status: {err}")
            );
        }

        let data = json!({
            "id": key_id,
            "health_status": health_status,
            "check_time": timezone_utils::format_utc_for_response(
                &check_time,
                &timezone_context.timezone
            ),
            "response_time": response_time,
            "details": {
                "status_code": 200,
                "latency": response_time,
                "error_message": null
            }
        });

        Ok(ServiceResponse::with_message(data, "健康检查完成"))
    }

    /// 获取提供商密钥趋势数据
    pub async fn trends(
        &self,
        user_id: i32,
        key_id: i32,
        query: &TrendQuery,
        timezone_context: &TimezoneContext,
    ) -> Result<ServiceResponse<Value>> {
        self.load_existing_key(key_id, user_id).await?;

        let days = query.days.min(30);
        let now = Utc::now();
        let (_, today_end_utc) = timezone_utils::local_day_bounds(&now, &timezone_context.timezone)
            .unwrap_or((now - Duration::days(1), now));
        let start_utc = today_end_utc - Duration::days(i64::from(days));

        let trends = fetch_key_trends_data(
            self.db(),
            key_id,
            &start_utc,
            &today_end_utc,
            "provider",
            timezone_context,
        )
        .await
        .map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_provider_key_trends_fail",
                &format!("Failed to fetch provider key trends: {err}")
            );
            crate::error!(Database, format!("Failed to fetch trends data: {err}"))
        })?;

        let trend_points = trends.points.clone();
        let data = json!({
            "trend_data": trend_points,
            "summary": {
                "total_requests": trends.total_requests,
                "total_cost": trends.total_cost,
                "avg_response_time": trends.avg_response_time,
                "success_rate": trends.success_rate,
                "total_tokens": trends.total_tokens,
            }
        });

        Ok(ServiceResponse::new(data))
    }

    /// 获取用户服务 API 趋势数据
    pub async fn user_service_trends(
        &self,
        user_id: i32,
        api_id: i32,
        query: &TrendQuery,
        timezone_context: &TimezoneContext,
    ) -> Result<ServiceResponse<Value>> {
        UserServiceApi::find()
            .filter(user_service_apis::Column::Id.eq(api_id))
            .filter(user_service_apis::Column::UserId.eq(user_id))
            .one(self.db())
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "fetch_service_api_fail",
                    &format!("Failed to fetch user service api: {err}")
                );
                crate::error!(Database, format!("Failed to fetch user service api: {err}"))
            })?
            .ok_or_else(|| {
                ProxyError::Authentication(AuthError::Message(format!(
                    "UserServiceApi not found: {api_id}"
                )))
            })?;

        let days = query.days.min(30);
        let now = Utc::now();
        let (_, today_end_utc) = timezone_utils::local_day_bounds(&now, &timezone_context.timezone)
            .unwrap_or((now - Duration::days(1), now));
        let start_utc = today_end_utc - Duration::days(i64::from(days));

        let trends = fetch_key_trends_data(
            self.db(),
            api_id,
            &start_utc,
            &today_end_utc,
            "user_service",
            timezone_context,
        )
        .await
        .map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "fetch_service_api_trends_fail",
                &format!("Failed to fetch user service api trends: {err}")
            );
            crate::error!(Database, format!("Failed to fetch trends data: {err}"))
        })?;

        let trend_points = trends.points.clone();
        let data = json!({
            "trend_data": trend_points,
            "summary": {
                "total_requests": trends.total_requests,
                "total_cost": trends.total_cost,
                "avg_response_time": trends.avg_response_time,
                "success_rate": trends.success_rate,
                "total_tokens": trends.total_tokens,
            }
        });

        Ok(ServiceResponse::new(data))
    }

    async fn load_existing_key(
        &self,
        key_id: i32,
        user_id: i32,
    ) -> Result<user_provider_keys::Model> {
        UserProviderKey::find()
            .filter(user_provider_keys::Column::Id.eq(key_id))
            .filter(user_provider_keys::Column::UserId.eq(user_id))
            .one(self.db())
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "find_key_fail",
                    &format!("Failed to find provider key: {err}")
                );
                crate::error!(Database, format!("Failed to find provider key: {err}"))
            })?
            .ok_or_else(|| {
                ProxyError::Authentication(AuthError::Message(format!(
                    "ProviderKey not found: {key_id}"
                )))
            })
    }

    async fn load_key_with_provider(
        &self,
        key_id: i32,
        user_id: i32,
    ) -> Result<(user_provider_keys::Model, Option<provider_types::Model>)> {
        use entity::provider_types::Entity as ProviderType;
        use entity::user_provider_keys::{self, Entity as UserProviderKey};

        UserProviderKey::find()
            .filter(user_provider_keys::Column::Id.eq(key_id))
            .filter(user_provider_keys::Column::UserId.eq(user_id))
            .find_also_related(ProviderType)
            .one(self.db())
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "fetch_key_with_provider_fail",
                    &format!("Failed to fetch provider key detail: {err}")
                );
                crate::error!(
                    Database,
                    format!("Failed to fetch provider key detail: {err}")
                )
            })?
            .ok_or_else(|| {
                ProxyError::Authentication(AuthError::Message(format!(
                    "ProviderKey not found: {key_id}"
                )))
            })
    }

    async fn ensure_unique_name(
        &self,
        user_id: i32,
        key_id: i32,
        existing_key: &user_provider_keys::Model,
        payload: &UpdateProviderKeyRequest,
    ) -> Result<()> {
        if existing_key.name == payload.name {
            return Ok(());
        }

        let duplicate = UserProviderKey::find()
            .filter(user_provider_keys::Column::UserId.eq(user_id))
            .filter(user_provider_keys::Column::Name.eq(&payload.name))
            .filter(user_provider_keys::Column::ProviderTypeId.eq(payload.provider_type_id))
            .filter(user_provider_keys::Column::Id.ne(key_id))
            .one(self.db())
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "check_duplicate_fail",
                    &format!("Failed to check duplicate name: {err}")
                );
                crate::error!(Database, format!("Failed to check duplicate name: {err}"))
            })?;

        if duplicate.is_some() {
            return Err(ProxyError::Authentication(AuthError::Message(format!(
                "ProviderKey conflict: {}",
                payload.name
            ))));
        }

        Ok(())
    }

    async fn prepare_pending_schedule(
        &self,
        refresh_task: Option<&Arc<OAuthTokenRefreshTask>>,
        payload: &UpdateProviderKeyRequest,
        user_id: i32,
        key_id: i32,
    ) -> Result<Option<ScheduledTokenRefresh>> {
        if payload.auth_type != OAUTH_AUTH_TYPE {
            return Ok(None);
        }

        prepare_oauth_schedule(
            refresh_task,
            payload.api_key.as_ref(),
            user_id,
            Some(key_id),
        )
        .await
    }

    async fn persist_updated_key(
        &self,
        existing_key: user_provider_keys::Model,
        payload: &UpdateProviderKeyRequest,
    ) -> Result<user_provider_keys::Model> {
        let mut active_model: user_provider_keys::ActiveModel = existing_key.into();
        active_model.provider_type_id = Set(payload.provider_type_id);
        active_model.name = Set(payload.name.clone());
        active_model.api_key = Set(payload.api_key.clone().unwrap_or_default());
        active_model.auth_type = Set(payload.auth_type.clone());
        active_model.weight = Set(payload.weight);
        active_model.max_requests_per_minute = Set(payload.max_requests_per_minute);
        active_model.max_tokens_prompt_per_minute = Set(payload.max_tokens_prompt_per_minute);
        active_model.max_requests_per_day = Set(payload.max_requests_per_day);
        active_model.is_active = Set(payload.is_active.unwrap_or(true));
        active_model.project_id = Set(payload.project_id.clone());
        active_model.updated_at = Set(Utc::now().naive_utc());

        active_model.update(self.db()).await.map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "update_key_fail",
                &format!("Failed to update provider key: {err}")
            );
            crate::error!(Database, format!("Failed to update provider key: {err}"))
        })
    }

    async fn enqueue_pending_schedule(
        &self,
        refresh_task: Option<&Arc<OAuthTokenRefreshTask>>,
        pending_schedule: Option<ScheduledTokenRefresh>,
        original_key: user_provider_keys::Model,
        updated_key: &user_provider_keys::Model,
        user_id: i32,
        key_id: i32,
    ) -> Result<()> {
        let Some(schedule) = pending_schedule else {
            return Ok(());
        };

        let Some(task) = refresh_task else {
            return Ok(());
        };

        if let Err(err) = task.enqueue_schedule(schedule).await {
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
            if let Err(revert_err) = revert_model.update(self.db()).await {
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

            return Err(err);
        }

        if updated_key.auth_type == OAUTH_AUTH_TYPE && !updated_key.api_key.is_empty() {
            linfo!(
                "system",
                LogStage::Scheduling,
                LogComponent::OAuth,
                "schedule_enqueued",
                &format!("OAuth refresh schedule updated for key {key_id}"),
                user_id = user_id
            );
        }

        Ok(())
    }

    async fn cleanup_obsolete_session(
        &self,
        refresh_task: Option<&Arc<OAuthTokenRefreshTask>>,
        old_session_id: Option<String>,
        updated_key: &user_provider_keys::Model,
        user_id: i32,
        key_id: i32,
    ) {
        let Some(old_id) = old_session_id else {
            return;
        };

        let Some(task) = refresh_task else {
            return;
        };

        let updated_session_id = extract_oauth_session_id(updated_key);
        if updated_session_id.as_deref() == Some(old_id.as_str()) {
            return;
        }

        if let Err(err) = task.remove_session(&old_id).await {
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
}

async fn fetch_provider_keys_usage_stats(
    db: &DatabaseConnection,
    provider_key_ids: &[i32],
    timezone_ctx: &TimezoneContext,
) -> HashMap<i32, ProviderKeyUsageStats> {
    use entity::proxy_tracing::Column;
    use sea_orm::sea_query::Expr;

    if provider_key_ids.is_empty() {
        return HashMap::new();
    }

    let select = ProxyTracing::find()
        .select_only()
        .column(Column::UserProviderKeyId)
        .column_as(Column::Id.count(), "total_requests")
        .column_as(
            Expr::cust("SUM(CASE WHEN is_success = true THEN 1 ELSE 0 END)"),
            "successful_requests",
        )
        .column_as(Column::TokensTotal.sum(), "total_tokens")
        .column_as(Column::Cost.sum(), "total_cost")
        .column_as(Column::DurationMs.sum(), "total_duration")
        .column_as(Column::DurationMs.count(), "duration_count")
        .column_as(Column::CreatedAt.max(), "last_used_at")
        .filter(Column::UserProviderKeyId.is_in(provider_key_ids.to_vec()))
        .group_by(Column::UserProviderKeyId);

    let stats_rows = match select.into_model::<ProviderKeyStatsRow>().all(db).await {
        Ok(rows) => rows,
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

    let mut usage_stats = HashMap::with_capacity(stats_rows.len());
    for row in stats_rows {
        if let Some(key_id) = row.user_provider_key_id {
            let total_requests = u64::try_from(row.total_requests.unwrap_or(0)).unwrap_or_default();
            let successful_requests =
                u64::try_from(row.successful_requests.unwrap_or(0)).unwrap_or_default();
            let failed_requests = total_requests.saturating_sub(successful_requests);

            let success_rate = if total_requests > 0 {
                ratio_as_percentage(successful_requests, total_requests)
            } else {
                0.0
            };

            let total_tokens = row.total_tokens.unwrap_or(0);
            let total_cost = row.total_cost.unwrap_or(0.0);

            let avg_response_time = if let (Some(total_duration), Some(count)) =
                (row.total_duration, row.duration_count)
            {
                if count > 0 { total_duration / count } else { 0 }
            } else {
                0
            };

            let last_used_at = row.last_used_at.map(|dt| {
                let utc_time = DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc);
                timezone_utils::format_utc_for_response(&utc_time, &timezone_ctx.timezone)
            });

            usage_stats.insert(
                key_id,
                ProviderKeyUsageStats {
                    total_requests,
                    successful_requests,
                    failed_requests,
                    success_rate,
                    total_tokens,
                    total_cost,
                    avg_response_time,
                    last_used_at,
                },
            );
        }
    }

    usage_stats
}

fn round_two_decimal(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[allow(clippy::too_many_arguments)]
async fn fetch_key_trends_data(
    db: &DatabaseConnection,
    key_id: i32,
    start_utc: &DateTime<Utc>,
    end_utc: &DateTime<Utc>,
    key_type: &str,
    timezone: &TimezoneContext,
) -> std::result::Result<TrendData, DbErr> {
    use entity::proxy_tracing::{Column, Entity as ProxyTracing};

    let mut trend_data = TrendData::default();

    let mut select = ProxyTracing::find()
        .filter(Column::CreatedAt.gte(start_utc.naive_utc()))
        .filter(Column::CreatedAt.lt(end_utc.naive_utc()));

    if key_type == "provider" {
        select = select.filter(Column::UserProviderKeyId.eq(key_id));
    } else {
        select = select.filter(Column::UserServiceApiId.eq(key_id));
    }

    let traces = select.all(db).await?;
    let daily_stats = aggregate_daily_stats(&traces, timezone);

    let local_start_date = start_utc.with_timezone(&timezone.timezone).date_naive();
    let local_end_exclusive = end_utc.with_timezone(&timezone.timezone).date_naive();

    let mut current_local_date = local_start_date;

    while current_local_date < local_end_exclusive {
        let label = current_local_date.format("%Y-%m-%d").to_string();
        let stats = daily_stats.get(&label);

        let (requests, successful_requests, total_cost, total_response_time, total_tokens) = stats
            .map_or((0, 0, 0.0, 0, 0), |stats| {
                (
                    stats.total_requests,
                    stats.successful_requests,
                    stats.total_cost,
                    stats.total_response_time,
                    stats.total_tokens,
                )
            });

        let avg_response_time = if successful_requests > 0 {
            total_response_time / successful_requests
        } else {
            0
        };

        let success_rate = match (u64::try_from(successful_requests), u64::try_from(requests)) {
            (Ok(success), Ok(total)) if total > 0 => ratio_as_percentage(success, total),
            _ => 0.0,
        };

        let date_display =
            timezone_utils::local_date_window(current_local_date, 1, &timezone.timezone)
                .map_or_else(
                    || format!("{label} 00:00:00"),
                    |(start, _)| {
                        start
                            .with_timezone(&timezone.timezone)
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string()
                    },
                );

        trend_data.points.push(TrendDataPoint {
            date: date_display,
            requests,
            successful_requests,
            failed_requests: requests - successful_requests,
            success_rate: round_two_decimal(success_rate),
            avg_response_time,
            tokens: total_tokens,
            cost: round_two_decimal(total_cost),
        });

        trend_data.total_requests += requests;
        trend_data.total_cost += total_cost;
        trend_data.total_tokens += total_tokens;
        trend_data.total_successful_requests += successful_requests;

        current_local_date += chrono::Duration::days(1);
    }

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

        trend_data.success_rate = match (
            u64::try_from(trend_data.total_successful_requests),
            u64::try_from(trend_data.total_requests),
        ) {
            (Ok(success), Ok(total)) => round_two_decimal(ratio_as_percentage(success, total)),
            _ => 0.0,
        };
    }

    trend_data.total_cost = round_two_decimal(trend_data.total_cost);

    Ok(trend_data)
}

fn validate_update_requirements(payload: &UpdateProviderKeyRequest) -> Result<()> {
    if payload.auth_type == "api_key" && payload.api_key.is_none() {
        return Err(ProxyError::Authentication(AuthError::Message(
            "API Key认证类型需要提供api_key字段 (field: api_key)".to_string(),
        )));
    }

    if payload.auth_type == "oauth" && payload.api_key.is_none() {
        return Err(ProxyError::Authentication(AuthError::Message(
            "OAuth认证类型需要通过api_key字段提供session_id (field: api_key)".to_string(),
        )));
    }

    Ok(())
}

fn extract_oauth_session_id(key: &user_provider_keys::Model) -> Option<String> {
    if key.auth_type == OAUTH_AUTH_TYPE && !key.api_key.is_empty() {
        Some(key.api_key.clone())
    } else {
        None
    }
}

fn build_update_response(
    updated_key: &user_provider_keys::Model,
    timezone_context: &TimezoneContext,
) -> Value {
    json!({
        "id": updated_key.id,
        "name": updated_key.name,
        "auth_type": updated_key.auth_type,
        "auth_status": updated_key.auth_status,
        "updated_at": timezone_utils::format_naive_utc_for_response(
            &updated_key.updated_at,
            &timezone_context.timezone
        )
    })
}

fn build_provider_key_json(
    provider_key: &user_provider_keys::Model,
    provider_type_opt: Option<provider_types::Model>,
    usage_stats: &HashMap<i32, ProviderKeyUsageStats>,
    timezone_context: &TimezoneContext,
) -> Value {
    let provider_name =
        provider_type_opt.map_or_else(|| "Unknown".to_string(), |pt| pt.display_name);

    let stats = usage_stats
        .get(&provider_key.id)
        .cloned()
        .unwrap_or_default();

    json!({
        "id": provider_key.id,
        "provider": provider_name,
        "name": provider_key.name,
        "api_key": provider_key.api_key,
        "auth_type": provider_key.auth_type,
        "auth_status": provider_key.auth_status,
        "health_status": provider_key.health_status,
        "weight": provider_key.weight,
        "max_requests_per_minute": provider_key.max_requests_per_minute,
        "max_tokens_prompt_per_minute": provider_key.max_tokens_prompt_per_minute,
        "max_requests_per_day": provider_key.max_requests_per_day,
        "is_active": provider_key.is_active,
        "project_id": provider_key.project_id,
        "usage": {
            "total_requests": stats.total_requests,
            "successful_requests": stats.successful_requests,
            "failed_requests": stats.failed_requests,
            "success_rate": stats.success_rate,
            "total_tokens": stats.total_tokens,
            "total_cost": stats.total_cost,
            "avg_response_time": stats.avg_response_time,
            "last_used_at": stats.last_used_at
        },
        "created_at": timezone_utils::format_naive_utc_for_response(
                &provider_key.created_at,
                &timezone_context.timezone
            ),
        "updated_at": timezone_utils::format_naive_utc_for_response(
                &provider_key.updated_at,
                &timezone_context.timezone
            )
    })
}

fn mask_api_key(key: &user_provider_keys::Model) -> String {
    if key.api_key.len() > 8 {
        format!(
            "{}****{}",
            &key.api_key[..4],
            &key.api_key[key.api_key.len().saturating_sub(4)..]
        )
    } else {
        "****".to_string()
    }
}

fn rate_limit_remaining_seconds(key: &user_provider_keys::Model, user_id: i32) -> Option<u64> {
    let Some(resets_at) = key.rate_limit_resets_at else {
        log_no_rate_limit_reset(key, user_id);
        return None;
    };
    let now = Utc::now().naive_utc();

    log_rate_limit_start(key.id, user_id, resets_at, now);
    if resets_at <= now {
        log_rate_limit_expired(key.id, user_id, resets_at, now);
        return None;
    }

    remaining_seconds_from_duration(key.id, user_id, resets_at, now)
}

fn log_no_rate_limit_reset(key: &user_provider_keys::Model, user_id: i32) {
    linfo!(
        "system",
        LogStage::Scheduling,
        LogComponent::KeyPool,
        "no_rate_limit_reset_time",
        "No rate limit reset time in DB, returning None",
        key_id = key.id,
        user_id = user_id,
        health_status = %key.health_status,
    );
}

fn log_rate_limit_start(key_id: i32, user_id: i32, resets_at: NaiveDateTime, now: NaiveDateTime) {
    linfo!(
        "system",
        LogStage::Scheduling,
        LogComponent::KeyPool,
        "calc_rate_limit_remaining",
        "Calculating rate limit remaining time - reset time found in DB",
        key_id = key_id,
        user_id = user_id,
        rate_limit_resets_at = ?resets_at,
        current_time = ?now,
    );
}

fn log_rate_limit_expired(key_id: i32, user_id: i32, resets_at: NaiveDateTime, now: NaiveDateTime) {
    linfo!(
        "system",
        LogStage::Scheduling,
        LogComponent::KeyPool,
        "rate_limit_expired",
        "Rate limit expired, returning None",
        key_id = key_id,
        user_id = user_id,
        rate_limit_resets_at = ?resets_at,
        current_time = ?now,
    );
}

fn remaining_seconds_from_duration(
    key_id: i32,
    user_id: i32,
    resets_at: NaiveDateTime,
    now: NaiveDateTime,
) -> Option<u64> {
    let seconds = resets_at.signed_duration_since(now).num_seconds().max(0);
    u64::try_from(seconds).map_or_else(
        |err| {
            linfo!(
                "system",
                LogStage::Scheduling,
                LogComponent::KeyPool,
                "rate_limit_conversion_fail",
                &format!("Failed to convert rate limit duration: {err}"),
                key_id = key_id,
                user_id = user_id,
                duration_seconds = seconds,
            );
            None
        },
        |remaining_seconds| {
            linfo!(
                "system",
                LogStage::Scheduling,
                LogComponent::KeyPool,
                "rate_limit_not_lifted",
                "Rate limit not lifted, calculating remaining seconds",
                key_id = key_id,
                user_id = user_id,
                remaining_seconds = remaining_seconds,
                duration_seconds = seconds,
            );
            Some(remaining_seconds)
        },
    )
}

struct PrepareGeminiContext {
    final_project_id: Option<String>,
    health_status: String,
    needs_auto_get_project_id_async: bool,
}

fn validate_create_payload(payload: &CreateProviderKeyRequest) -> Result<()> {
    if payload.auth_type == "api_key" && payload.api_key.is_none() {
        return Err(ProxyError::Authentication(AuthError::Message(
            "API Key认证类型需要提供api_key字段 (field: api_key)".to_string(),
        )));
    }

    if payload.auth_type == "oauth" && payload.api_key.is_none() {
        return Err(ProxyError::Authentication(AuthError::Message(
            "OAuth认证类型需要通过api_key字段提供session_id (field: api_key)".to_string(),
        )));
    }

    Ok(())
}

async fn ensure_unique_provider_key(
    db: &DatabaseConnection,
    user_id: i32,
    payload: &CreateProviderKeyRequest,
) -> Result<()> {
    let existing = UserProviderKey::find()
        .filter(user_provider_keys::Column::UserId.eq(user_id))
        .filter(user_provider_keys::Column::Name.eq(&payload.name))
        .filter(user_provider_keys::Column::ProviderTypeId.eq(payload.provider_type_id))
        .one(db)
        .await;

    match existing {
        Ok(Some(_)) => Err(ProxyError::Authentication(AuthError::Message(format!(
            "ProviderKey conflict: {}",
            &payload.name
        )))),
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "check_exist_fail",
                &format!("Failed to check existing provider key: {err}")
            );
            Err(crate::error!(
                Database,
                format!("Failed to check existing provider key: {err}")
            ))
        }
        _ => Ok(()),
    }
}

async fn validate_oauth_session_for_creation(
    db: &DatabaseConnection,
    user_id: i32,
    payload: &CreateProviderKeyRequest,
) -> Result<()> {
    if payload.auth_type != "oauth" {
        return Ok(());
    }

    let Some(session_id) = &payload.api_key else {
        return Ok(());
    };

    match OAuthSession::find()
        .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
        .filter(oauth_client_sessions::Column::UserId.eq(user_id))
        .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
        .one(db)
        .await
    {
        Ok(Some(_)) => {
            let existing_usage = UserProviderKey::find()
                .filter(user_provider_keys::Column::ApiKey.eq(session_id))
                .filter(user_provider_keys::Column::AuthType.eq("oauth"))
                .filter(user_provider_keys::Column::IsActive.eq(true))
                .one(db)
                .await;

            match existing_usage {
                Ok(Some(_)) => Err(ProxyError::Authentication(AuthError::Message(
                    "指定的OAuth会话已被其他provider key使用".to_string(),
                ))),
                Err(err) => {
                    lerror!(
                        "system",
                        LogStage::Db,
                        LogComponent::OAuth,
                        "check_session_usage_fail",
                        &format!("Failed to check OAuth session usage: {err}")
                    );
                    Err(crate::error!(
                        Database,
                        format!("Failed to check OAuth session usage: {err}")
                    ))
                }
                _ => Ok(()),
            }
        }
        Ok(None) => Err(ProxyError::Authentication(AuthError::Message(
            "指定的OAuth会话不存在或未完成授权 (field: api_key)".to_string(),
        ))),
        Err(err) => {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::OAuth,
                "validate_session_fail",
                &format!("Failed to validate OAuth session: {err}")
            );
            Err(crate::error!(
                Database,
                format!("Failed to validate OAuth session: {err}")
            ))
        }
    }
}

async fn validate_oauth_session_for_update(
    db: &DatabaseConnection,
    user_id: i32,
    key_id: i32,
    payload: &UpdateProviderKeyRequest,
) -> Result<()> {
    if payload.auth_type != "oauth" {
        return Ok(());
    }

    let Some(session_id) = &payload.api_key else {
        return Ok(());
    };

    let session_exists = OAuthSession::find()
        .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
        .filter(oauth_client_sessions::Column::UserId.eq(user_id))
        .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
        .one(db)
        .await
        .map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::OAuth,
                "validate_session_fail",
                &format!("Failed to validate OAuth session: {err}")
            );
            crate::error!(Database, format!("Failed to validate OAuth session: {err}"))
        })?;

    if session_exists.is_none() {
        return Err(ProxyError::Authentication(AuthError::Message(
            "指定的OAuth会话不存在或未完成授权 (field: api_key)".to_string(),
        )));
    }

    let existing_usage = UserProviderKey::find()
        .filter(user_provider_keys::Column::ApiKey.eq(session_id))
        .filter(user_provider_keys::Column::AuthType.eq("oauth"))
        .filter(user_provider_keys::Column::IsActive.eq(true))
        .filter(user_provider_keys::Column::Id.ne(key_id))
        .one(db)
        .await
        .map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::OAuth,
                "check_session_usage_fail",
                &format!("Failed to check OAuth session usage: {err}")
            );
            crate::error!(
                Database,
                format!("Failed to check OAuth session usage: {err}")
            )
        })?;

    if existing_usage.is_some() {
        return Err(ProxyError::Authentication(AuthError::Message(
            "指定的OAuth会话已被其他provider key使用".to_string(),
        )));
    }

    Ok(())
}

async fn prepare_gemini_context(
    db: &DatabaseConnection,
    user_id: i32,
    payload: &CreateProviderKeyRequest,
) -> Result<PrepareGeminiContext> {
    let mut context = PrepareGeminiContext {
        final_project_id: payload.project_id.clone(),
        health_status: ApiKeyHealthStatus::Healthy.to_string(),
        needs_auto_get_project_id_async: false,
    };

    if !is_gemini_oauth_flow(db, payload).await? {
        return Ok(context);
    }

    let Some(session_id) = payload.api_key.as_deref() else {
        log_missing_session_id(user_id);
        context.health_status = ApiKeyHealthStatus::Unhealthy.to_string();
        return Ok(context);
    };

    let Some(oauth_session) = fetch_authorized_session(db, user_id, session_id).await? else {
        log_missing_authorized_session(user_id);
        context.health_status = ApiKeyHealthStatus::Unhealthy.to_string();
        return Ok(context);
    };

    let access_token = oauth_session.access_token.as_deref().unwrap_or("");
    let gemini_client = GeminiCodeAssistClient::new();

    if let Some(provided_pid) = context.final_project_id.clone() {
        process_provided_project_id(
            &gemini_client,
            access_token,
            provided_pid,
            user_id,
            &mut context,
        )
        .await;
    } else {
        mark_project_id_pending(user_id, &mut context);
    }

    Ok(context)
}

async fn is_gemini_oauth_flow(
    db: &DatabaseConnection,
    payload: &CreateProviderKeyRequest,
) -> Result<bool> {
    if payload.auth_type != OAUTH_AUTH_TYPE {
        return Ok(false);
    }

    let provider = ProviderType::find_by_id(payload.provider_type_id)
        .one(db)
        .await
        .map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "gemini_provider_query_fail",
                &format!("Failed to query provider type for Gemini validation: {err}"),
            );
            crate::error!(Database, format!("Failed to query provider type: {err}"))
        })?;

    Ok(matches!(provider.map(|p| p.name), Some(name) if name == GEMINI_PROVIDER_NAME))
}

async fn fetch_authorized_session(
    db: &DatabaseConnection,
    user_id: i32,
    session_id: &str,
) -> Result<Option<oauth_client_sessions::Model>> {
    OAuthSession::find()
        .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
        .filter(oauth_client_sessions::Column::UserId.eq(user_id))
        .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
        .one(db)
        .await
        .map_err(|err| {
            lerror!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "gemini_session_query_fail",
                &format!(
                    "Gemini OAuth: Failed to query OAuth session while validating project_id: {err}"
                ),
                user_id = user_id,
            );
            crate::error!(Database, format!("Failed to validate OAuth session: {err}"))
        })
}

fn log_missing_session_id(user_id: i32) {
    lerror!(
        "system",
        LogStage::Authentication,
        LogComponent::OAuth,
        "gemini_missing_session_id",
        "Gemini OAuth: Missing session_id (api_key field), cannot complete validation",
        user_id = user_id,
    );
}

fn log_missing_authorized_session(user_id: i32) {
    lerror!(
        "system",
        LogStage::Authentication,
        LogComponent::OAuth,
        "gemini_no_auth_session",
        "Gemini OAuth: Authorized OAuth session not found, cannot validate project_id",
        user_id = user_id,
    );
}

async fn process_provided_project_id(
    gemini_client: &GeminiCodeAssistClient,
    access_token: &str,
    provided_pid: String,
    user_id: i32,
    context: &mut PrepareGeminiContext,
) {
    linfo!(
        "system",
        LogStage::Authentication,
        LogComponent::OAuth,
        "gemini_load_assist_with_project",
        "Gemini OAuth: Using user-provided project_id to call loadCodeAssist",
        user_id = user_id,
        project_id = %provided_pid,
    );

    match gemini_client
        .load_code_assist(access_token, Some(&provided_pid), None)
        .await
    {
        Ok(resp) => {
            if let Some(server_pid) = resp.cloudaicompanionProject {
                context.final_project_id = Some(server_pid);
            } else {
                linfo!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::OAuth,
                    "gemini_invalid_project_id",
                    "loadCodeAssist did not return cloudaicompanionProject, user-provided project_id is invalid",
                    user_id = user_id,
                    provided_project_id = %provided_pid,
                );
                context.health_status = ApiKeyHealthStatus::Unhealthy.to_string();
                context.needs_auto_get_project_id_async = true;
                context.final_project_id = None;
            }
        }
        Err(e) => {
            context.health_status = ApiKeyHealthStatus::Unhealthy.to_string();
            lerror!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "gemini_load_assist_fail",
                "Gemini OAuth: loadCodeAssist call failed (with project_id)",
                user_id = user_id,
                error = %e
            );
        }
    }
}

fn mark_project_id_pending(user_id: i32, context: &mut PrepareGeminiContext) {
    linfo!(
        "system",
        LogStage::Authentication,
        LogComponent::OAuth,
        "gemini_auto_get_project_id_async",
        "Gemini OAuth: No project_id provided, will auto-get asynchronously (loadCodeAssist / onboardUser)",
        user_id = user_id,
    );
    context.health_status = ApiKeyHealthStatus::Unhealthy.to_string();
    context.needs_auto_get_project_id_async = true;
}

async fn schedule_oauth_if_needed(
    refresh_task: Option<&Arc<OAuthTokenRefreshTask>>,
    payload: &CreateProviderKeyRequest,
    user_id: i32,
    key_id: Option<i32>,
) -> Result<Option<ScheduledTokenRefresh>> {
    if payload.auth_type != OAUTH_AUTH_TYPE {
        return Ok(None);
    }

    prepare_oauth_schedule(refresh_task, payload.api_key.as_ref(), user_id, key_id).await
}

async fn prepare_oauth_schedule(
    task: Option<&Arc<OAuthTokenRefreshTask>>,
    session_id: Option<&String>,
    user_id: i32,
    key_id: Option<i32>,
) -> Result<Option<ScheduledTokenRefresh>> {
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

async fn insert_provider_key_record(
    db: &DatabaseConnection,
    user_id: i32,
    payload: &CreateProviderKeyRequest,
    final_project_id: Option<String>,
    health_status: String,
) -> Result<user_provider_keys::Model> {
    let new_provider_key = user_provider_keys::ActiveModel {
        user_id: Set(user_id),
        provider_type_id: Set(payload.provider_type_id),
        name: Set(payload.name.clone()),
        api_key: Set(payload.api_key.clone().unwrap_or_default()),
        auth_type: Set(payload.auth_type.clone()),
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

    new_provider_key.insert(db).await.map_err(|err| {
        lerror!(
            "system",
            LogStage::Db,
            LogComponent::Database,
            "create_key_fail",
            &format!("Failed to create provider key: {err}")
        );
        crate::error!(Database, format!("Failed to create provider key: {err}"))
    })
}

async fn enqueue_oauth_schedule(
    refresh_task: Option<&Arc<OAuthTokenRefreshTask>>,
    pending_schedule: Option<ScheduledTokenRefresh>,
    db: &DatabaseConnection,
    user_id: i32,
    inserted_key: &user_provider_keys::Model,
) -> Result<()> {
    let Some(schedule) = pending_schedule else {
        return Ok(());
    };

    let Some(task) = refresh_task else {
        lwarn!(
            "system",
            LogStage::Scheduling,
            LogComponent::OAuth,
            "task_unavailable_no_enqueue",
            "OAuth refresh task unavailable, schedule not enqueued",
            user_id = user_id,
            key_id = inserted_key.id,
        );
        return Ok(());
    };

    if let Err(err) = task.enqueue_schedule(schedule).await {
        lerror!(
            "system",
            LogStage::Scheduling,
            LogComponent::OAuth,
            "enqueue_schedule_fail",
            &format!("Failed to enqueue OAuth refresh schedule: {err}"),
            user_id = user_id,
            key_id = inserted_key.id,
        );

        if let Err(delete_err) = user_provider_keys::Entity::delete_by_id(inserted_key.id)
            .exec(db)
            .await
        {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "rollback_key_fail",
                &format!("Failed to rollback provider key after enqueue error: {delete_err}"),
                user_id = user_id,
                key_id = inserted_key.id,
            );
        }

        return Err(err);
    }

    Ok(())
}

fn spawn_gemini_project_task(
    needs_auto_get_project_id_async: bool,
    db: DatabaseConnection,
    user_id: i32,
    key_id: i32,
) {
    if !needs_auto_get_project_id_async {
        return;
    }

    let user_id_string = user_id.to_string();
    spawn(async move {
        linfo!(
            "system",
            LogStage::BackgroundTask,
            LogComponent::OAuth,
            "start_auto_get_project_id_task",
            "Starting async auto-get project_id task",
            user_id = user_id_string,
            key_id = %key_id,
        );

        if let Err(e) = execute_auto_get_project_id_async(&db, key_id, &user_id_string).await {
            lerror!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "auto_get_project_id_task_fail",
                "Async auto-get project_id task failed",
                user_id = user_id_string,
                key_id = %key_id,
                error = %e,
            );
        }
    });
}

async fn execute_auto_get_project_id_async(
    db: &DatabaseConnection,
    key_id: i32,
    user_id: &str,
) -> Result<()> {
    let gemini_client = GeminiCodeAssistClient::new();
    let access_token = get_access_token_for_key(db, key_id, user_id).await?;

    match gemini_client
        .auto_get_project_id_with_retry(&access_token)
        .await
    {
        Ok(Some(pid)) => {
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

            if let Some(key_model) = UserProviderKey::find_by_id(key_id).one(db).await? {
                let mut active_key: user_provider_keys::ActiveModel = key_model.into();
                active_key.project_id = Set(Some(pid.clone()));
                active_key.health_status = Set(ApiKeyHealthStatus::Healthy.to_string());
                active_key.updated_at = Set(Utc::now().naive_utc());
                active_key.update(db).await?;
            }
            Ok(())
        }
        Ok(None) => {
            lwarn!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "auto_get_project_id_empty",
                "Async auto-get project_id returned empty",
                user_id = user_id,
                key_id = %key_id,
            );
            Ok(())
        }
        Err(err) => Err(err),
    }
}

async fn get_access_token_for_key(
    db: &DatabaseConnection,
    key_id: i32,
    user_id: &str,
) -> Result<String> {
    let key_record = UserProviderKey::find_by_id(key_id)
        .one(db)
        .await
        .map_err(|err| crate::error!(Database, format!("查询key记录失败: {err}")))?
        .ok_or_else(|| {
            ProxyError::internal(format!("未找到key记录: key_id={key_id}, user_id={user_id}"))
        })?;

    if key_record.auth_type != OAUTH_AUTH_TYPE {
        return Err(ProxyError::internal(format!(
            "key不是OAuth类型: auth_type={}",
            key_record.auth_type
        )));
    }

    let session_id = key_record.api_key;
    if session_id.is_empty() {
        return Err(ProxyError::internal("OAuth key的session_id为空"));
    }

    let oauth_session = OAuthSession::find()
        .filter(oauth_client_sessions::Column::SessionId.eq(&session_id))
        .filter(oauth_client_sessions::Column::UserId.eq(user_id))
        .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
        .one(db)
        .await
        .map_err(|err| crate::error!(Database, format!("查询OAuth会话失败: {err}")))?
        .ok_or_else(|| {
            ProxyError::internal(format!(
                "未找到授权的OAuth会话: session_id={session_id}, user_id={user_id}"
            ))
        })?;

    let access_token = oauth_session
        .access_token
        .filter(|token| !token.is_empty())
        .ok_or_else(|| ProxyError::internal("OAuth会话中没有access_token"))?;

    ldebug!(
        "system",
        LogStage::Authentication,
        LogComponent::OAuth,
        "load_access_token_success",
        "Loaded access token for auto-get project_id task",
        session_id = session_id.as_str(),
    );

    Ok(access_token)
}
