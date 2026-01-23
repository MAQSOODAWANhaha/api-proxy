//! # 提供商密钥服务
//!
//! 核心服务编排逻辑，协调各个子模块完成业务功能。

use chrono::{Duration, Utc};
use entity::{user_provider_keys, user_service_apis, user_service_apis::Entity as UserServiceApi};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde_json::{Value, json};

use crate::{
    ProxyError,
    error::{Context, Result, auth::AuthError},
    lerror, linfo,
    logging::{LogComponent, LogStage},
    lwarn,
    management::server::ManagementState,
    types::{TimezoneContext, timezone_utils},
};

use super::{
    crud::{
        delete_key, ensure_unique_provider_key, insert_provider_key_record, load_existing_key,
        load_key_with_provider, load_provider_type_or_error, persist_updated_key,
    },
    gemini::{prepare_gemini_context, spawn_gemini_project_task},
    models::{
        CreateProviderKeyRequest, PrepareGeminiContext, ProviderKeysListQuery, TrendQuery,
        UpdateProviderKeyRequest, UserProviderKeyQuery,
    },
    oauth::{OAuthHelper, needs_oauth_schedule},
    statistics::{
        build_provider_key_json, build_update_response, fetch_key_trends_data,
        fetch_provider_keys_usage_stats, mask_api_key, rate_limit_remaining_seconds,
    },
    validation::{
        ensure_unique_name, validate_create_payload, validate_oauth_session_for_creation,
        validate_oauth_session_for_update, validate_update_requirements,
    },
};

use crate::management::services::shared::ServiceResponse;

const OAUTH_AUTH_TYPE: &str = "oauth";

/// 提供商密钥服务入口
pub struct ProviderKeyService<'a> {
    state: &'a ManagementState,
    db: &'a DatabaseConnection,
    oauth_helper: OAuthHelper,
}

impl<'a> ProviderKeyService<'a> {
    #[must_use]
    pub fn new(state: &'a ManagementState) -> Self {
        let db = state.database.as_ref();
        let refresh_task = Some(state.oauth_token_refresh_task());
        Self {
            state,
            db,
            oauth_helper: OAuthHelper {
                db: db.clone(),
                refresh_task,
            },
        }
    }

    #[must_use]
    const fn db(&self) -> &'a DatabaseConnection {
        self.db
    }

    /// 获取提供商密钥列表
    pub async fn list(
        &self,
        user_id: i32,
        timezone_context: &TimezoneContext,
        query: &ProviderKeysListQuery,
    ) -> Result<ServiceResponse<Value>> {
        let mut select = entity::user_provider_keys::Entity::find()
            .filter(entity::user_provider_keys::Column::UserId.eq(user_id));

        if let Some(search) = query.search.as_ref().filter(|s| !s.is_empty()) {
            select = select.filter(entity::user_provider_keys::Column::Name.contains(search));
        }

        if let Some(status) = &query.status {
            select = select
                .filter(entity::user_provider_keys::Column::HealthStatus.eq(status.to_string()));
        }

        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(10).max(1);
        let offset = (page - 1) * limit;

        let total = select
            .clone()
            .count(self.db())
            .await
            .context("Failed to count provider keys")?;

        let provider_keys = select
            .find_also_related(entity::provider_types::Entity)
            .offset(offset)
            .limit(limit)
            .order_by_desc(entity::user_provider_keys::Column::CreatedAt)
            .all(self.db())
            .await
            .context("Failed to fetch provider keys")?;

        let provider_key_ids: Vec<i32> = provider_keys.iter().map(|(pk, _)| pk.id).collect();
        let usage_stats =
            fetch_provider_keys_usage_stats(&provider_key_ids, timezone_context, self.db()).await;

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
        let provider_type =
            load_provider_type_or_error(self.db(), payload.provider_type_id).await?;
        let effective_auth_type = provider_type.auth_type.clone();
        if payload.auth_type != effective_auth_type {
            lwarn!(
                "system",
                LogStage::Authentication,
                LogComponent::Auth,
                "provider_key_auth_type_mismatch",
                "Payload auth_type does not match provider type auth_type, using provider type value",
                user_id = user_id,
                provider_type_id = payload.provider_type_id,
                payload_auth_type = payload.auth_type.as_str(),
                provider_auth_type = effective_auth_type.as_str(),
            );
        }

        ensure_unique_provider_key(self.db(), user_id, payload).await?;
        validate_create_payload(payload, effective_auth_type.as_str())?;
        validate_oauth_session_for_creation(
            self.db(),
            user_id,
            payload,
            effective_auth_type.as_str(),
        )
        .await?;

        let PrepareGeminiContext {
            final_project_id,
            health_status,
            needs_auto_get_project_id_async,
        } = prepare_gemini_context(
            self.db(),
            user_id,
            payload.api_key.as_ref(),
            payload.project_id.clone(),
            provider_type.name.as_str(),
        )
        .await?;

        let pending_schedule = if needs_oauth_schedule(effective_auth_type.as_str()) {
            self.oauth_helper
                .prepare_schedule(payload.api_key.as_ref(), user_id, None)
                .await?
        } else {
            None
        };

        let record = insert_provider_key_record(
            self.db(),
            user_id,
            payload,
            final_project_id,
            health_status,
            effective_auth_type.as_str(),
        )
        .await?;

        self.oauth_helper
            .enqueue_schedule(pending_schedule, user_id, &record)
            .await?;

        spawn_gemini_project_task(
            needs_auto_get_project_id_async,
            self.db().clone(),
            user_id,
            record.id,
        );

        let provider_name = provider_type.display_name.clone();

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
        let existing_key = load_existing_key(self.db(), key_id, user_id).await?;
        let provider_type =
            load_provider_type_or_error(self.db(), payload.provider_type_id).await?;
        let effective_auth_type = provider_type.auth_type.clone();
        if payload.auth_type != effective_auth_type {
            lwarn!(
                "system",
                LogStage::Authentication,
                LogComponent::Auth,
                "provider_key_auth_type_mismatch",
                "Payload auth_type does not match provider type auth_type, using provider type value",
                user_id = user_id,
                provider_type_id = payload.provider_type_id,
                payload_auth_type = payload.auth_type.as_str(),
                provider_auth_type = effective_auth_type.as_str(),
            );
        }

        ensure_unique_name(self.db(), user_id, key_id, &existing_key, payload).await?;
        validate_update_requirements(payload, effective_auth_type.as_str())?;
        if effective_auth_type == OAUTH_AUTH_TYPE {
            validate_oauth_session_for_update(
                self.db(),
                user_id,
                key_id,
                payload,
                effective_auth_type.as_str(),
            )
            .await?;
        }

        let pending_schedule = if needs_oauth_schedule(effective_auth_type.as_str()) {
            self.oauth_helper
                .prepare_schedule(payload.api_key.as_ref(), user_id, Some(key_id))
                .await?
        } else {
            None
        };

        let original_key = existing_key.clone();
        let old_session_id =
            crate::management::services::provider_keys::oauth::OAuthHelper::extract_session_id(
                &existing_key,
            );
        let updated_key = persist_updated_key(
            self.db(),
            existing_key,
            payload,
            effective_auth_type.as_str(),
        )
        .await?;

        if let Some(schedule) = pending_schedule {
            if let Err(err) = self
                .state
                .oauth_token_refresh_task()
                .enqueue_schedule(schedule)
                .await
            {
                let revert_model: user_provider_keys::ActiveModel = original_key.into();
                if let Err(revert_err) = revert_model.update(self.db()).await {
                    lerror!(
                        "system",
                        LogStage::Db,
                        LogComponent::Database,
                        "rollback_key_update_fail",
                        &format!(
                            "Failed to rollback provider key after enqueue error: {revert_err}"
                        ),
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
        }

        self.oauth_helper
            .cleanup_obsolete_session(old_session_id, &updated_key, user_id, key_id)
            .await;

        let response_payload = build_update_response(&updated_key, timezone_context);
        Ok(ServiceResponse::with_message(response_payload, "更新成功"))
    }

    /// 获取提供商密钥详情
    pub async fn detail(
        &self,
        user_id: i32,
        timezone_context: &TimezoneContext,
        key_id: i32,
    ) -> Result<ServiceResponse<Value>> {
        let (key, provider_type_opt) = load_key_with_provider(self.db(), key_id, user_id).await?;
        let provider_name =
            provider_type_opt.map_or_else(|| "Unknown".to_string(), |pt| pt.display_name);

        let usage_stats =
            fetch_provider_keys_usage_stats(&[key.id], timezone_context, self.db()).await;
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
            "provider_type_id": key.provider_type_id,
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
            "project_id": key.project_id,
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
        let existing_key = load_existing_key(self.db(), key_id, user_id).await?;

        let session_to_remove =
            if existing_key.auth_type == OAUTH_AUTH_TYPE && !existing_key.api_key.is_empty() {
                Some(existing_key.api_key.clone())
            } else {
                None
            };

        delete_key(self.db(), existing_key).await?;

        if let (Some(session_id), Some(task)) = (
            session_to_remove.as_ref(),
            self.oauth_helper.refresh_task.as_deref(),
        ) && let Err(err) = task.remove_session(session_id.as_str()).await
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
            load_key_with_provider(self.db(), key_id, user_id).await?;

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
        .context("Failed to fetch trends data")?;

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
        let total_keys = entity::user_provider_keys::Entity::find()
            .filter(entity::user_provider_keys::Column::UserId.eq(user_id))
            .count(self.db())
            .await
            .context("Failed to count total keys")?;

        let active_keys = entity::user_provider_keys::Entity::find()
            .filter(entity::user_provider_keys::Column::UserId.eq(user_id))
            .filter(entity::user_provider_keys::Column::IsActive.eq(true))
            .count(self.db())
            .await
            .context("Failed to count active keys")?;

        let user_provider_key_ids: Vec<i32> = entity::user_provider_keys::Entity::find()
            .filter(entity::user_provider_keys::Column::UserId.eq(user_id))
            .all(self.db())
            .await
            .context("Failed to fetch user provider keys")?
            .into_iter()
            .map(|k| k.id)
            .collect();

        let (total_usage, total_cost) = if user_provider_key_ids.is_empty() {
            (0u64, 0.0f64)
        } else {
            entity::proxy_tracing::Entity::find()
                .filter(
                    entity::proxy_tracing::Column::UserProviderKeyId.is_in(user_provider_key_ids),
                )
                .filter(entity::proxy_tracing::Column::IsSuccess.eq(true))
                .all(self.db())
                .await
                .map(|records| {
                    let usage_count = records.len() as u64;
                    let cost_sum: f64 = records.iter().filter_map(|record| record.cost).sum();
                    (usage_count, cost_sum)
                })
                .context("Failed to fetch usage statistics")?
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
        let mut select = entity::user_provider_keys::Entity::find()
            .filter(entity::user_provider_keys::Column::UserId.eq(user_id));

        if let Some(provider_type_id) = query.provider_type_id {
            select = select
                .filter(entity::user_provider_keys::Column::ProviderTypeId.eq(provider_type_id));
        }

        if let Some(is_active) = query.is_active {
            select = select.filter(entity::user_provider_keys::Column::IsActive.eq(is_active));
        }

        let provider_keys = select
            .find_also_related(entity::provider_types::Entity)
            .order_by_desc(entity::user_provider_keys::Column::CreatedAt)
            .all(self.db())
            .await
            .context("Failed to fetch simple provider keys")?;

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
        let existing_key = load_existing_key(self.db(), key_id, user_id).await?;

        let health_status = "healthy";
        let response_time = 245;
        let check_time = Utc::now();

        let mut active_model: user_provider_keys::ActiveModel = existing_key.into();
        active_model.health_status = Set(health_status.to_string());
        active_model.updated_at = Set(check_time.naive_utc());

        if let Err(err) = active_model.update(self.db()).await {
            use crate::{
                lerror,
                logging::{LogComponent, LogStage},
            };

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
        load_existing_key(self.db(), key_id, user_id).await?;

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
        .context("Failed to fetch provider key trends")?;

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
            .context("Failed to fetch user service api")?
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
        .context("Failed to fetch user service api trends")?;

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
}
