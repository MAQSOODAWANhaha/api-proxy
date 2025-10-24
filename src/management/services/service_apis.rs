//! # 用户服务 API 服务
//!
//! 聚合用户服务 API 相关的业务逻辑，供管理端 Handler 复用。

use std::ops::Range;

use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use chrono_tz::Tz;
use entity::{
    provider_types::Entity as ProviderTypes, proxy_tracing, proxy_tracing::Entity as ProxyTracing,
    user_service_apis, user_service_apis::Entity as UserServiceApis,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    error::{ProxyError, Result},
    lerror,
    logging::{LogComponent, LogStage},
    management::response::Pagination,
    management::server::ManagementState,
    types::{ProviderTypeId, timezone_utils},
};

use super::shared::{
    metrics::ratio_as_percentage,
    pagination::{PaginationParams, build_page},
};

/// 用户服务 API 查询参数
#[derive(Debug, Deserialize)]
pub struct UserServiceKeyQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub provider_type_id: Option<ProviderTypeId>,
    pub is_active: Option<bool>,
}

/// 创建用户服务 API 请求
#[derive(Debug, Deserialize)]
pub struct CreateUserServiceKeyRequest {
    pub name: String,
    pub description: Option<String>,
    pub provider_type_id: ProviderTypeId,
    pub user_provider_keys_ids: Vec<i32>,
    pub scheduling_strategy: Option<String>,
    pub retry_count: Option<i32>,
    pub timeout_seconds: Option<i32>,
    pub max_request_per_min: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub max_tokens_per_day: Option<i64>,
    pub max_cost_per_day: Option<sea_orm::prelude::Decimal>,
    pub expires_at: Option<String>,
    pub is_active: Option<bool>,
}

/// 更新用户服务 API 请求
#[derive(Debug, Deserialize)]
pub struct UpdateUserServiceKeyRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub user_provider_keys_ids: Option<Vec<i32>>,
    pub scheduling_strategy: Option<String>,
    pub retry_count: Option<i32>,
    pub timeout_seconds: Option<i32>,
    pub max_request_per_min: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub max_tokens_per_day: Option<i64>,
    pub max_cost_per_day: Option<sea_orm::prelude::Decimal>,
    pub expires_at: Option<String>,
}

/// 使用统计查询
#[derive(Debug, Deserialize)]
pub struct UsageStatsQuery {
    pub time_range: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

/// 状态更新请求
#[derive(Debug, Deserialize)]
pub struct UpdateStatusRequest {
    pub is_active: bool,
}

/// 卡片指标
#[derive(Debug, Serialize)]
pub struct UserServiceCardsResponse {
    pub total_api_keys: i32,
    pub active_api_keys: i32,
    pub requests: i64,
}

/// 列表项
#[derive(Debug, Serialize)]
pub struct UserServiceKeyResponse {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub provider: String,
    pub provider_type_id: ProviderTypeId,
    pub api_key: String,
    pub usage: Option<Value>,
    pub is_active: bool,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub scheduling_strategy: Option<String>,
    pub retry_count: Option<i32>,
    pub timeout_seconds: Option<i32>,
    pub max_request_per_min: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub max_tokens_per_day: Option<i64>,
    pub max_cost_per_day: Option<sea_orm::prelude::Decimal>,
}

/// 列表响应
#[derive(Debug, Serialize)]
pub struct UserServiceKeyListResponse {
    pub service_api_keys: Vec<UserServiceKeyResponse>,
    pub pagination: Pagination,
}

/// 详情响应
#[derive(Debug, Serialize)]
pub struct UserServiceKeyDetailResponse {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub provider_type_id: ProviderTypeId,
    pub provider: String,
    pub api_key: String,
    pub user_provider_keys_ids: Vec<i32>,
    pub scheduling_strategy: Option<String>,
    pub retry_count: Option<i32>,
    pub timeout_seconds: Option<i32>,
    pub max_request_per_min: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub max_tokens_per_day: Option<i64>,
    pub max_cost_per_day: Option<sea_orm::prelude::Decimal>,
    pub expires_at: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建响应
#[derive(Debug, Serialize)]
pub struct CreateUserServiceKeyResponse {
    pub id: i32,
    pub api_key: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub provider_type_id: ProviderTypeId,
    pub is_active: bool,
    pub created_at: String,
}

/// 更新响应
#[derive(Debug, Serialize)]
pub struct UpdateUserServiceKeyResponse {
    pub id: i32,
    pub name: Option<String>,
    pub description: Option<String>,
    pub updated_at: String,
}

/// 使用统计响应
#[derive(Debug, Serialize)]
pub struct UserServiceKeyUsageResponse {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub success_rate: f64,
    pub total_tokens: i32,
    pub tokens_prompt: i32,
    pub tokens_completion: i32,
    pub cache_create_tokens: i32,
    pub cache_read_tokens: i32,
    pub total_cost: f64,
    pub cost_currency: &'static str,
    pub avg_response_time: i64,
    pub last_used: Option<String>,
    pub usage_trend: Vec<Value>,
}

/// 重新生成响应
#[derive(Debug, Serialize)]
pub struct RegenerateUserServiceKeyResponse {
    pub id: i32,
    pub api_key: String,
    pub regenerated_at: String,
}

/// 状态更新响应
#[derive(Debug, Serialize)]
pub struct UpdateUserServiceKeyStatusResponse {
    pub id: i32,
    pub is_active: bool,
    pub updated_at: String,
}

/// 用户服务 API 业务服务
pub struct ServiceApiService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> ServiceApiService<'a> {
    #[must_use]
    pub fn new(state: &'a ManagementState) -> Self {
        Self {
            db: state.database.as_ref(),
        }
    }

    /// 获取卡片指标
    pub async fn cards(&self, user_id: i32) -> Result<UserServiceCardsResponse> {
        let total_api_keys = self.count_user_service_keys(user_id, None).await?;
        let active_api_keys = self.count_user_service_keys(user_id, Some(true)).await?;
        let requests = ProxyTracing::find()
            .filter(proxy_tracing::Column::UserId.eq(user_id))
            .count(self.db)
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "count_user_requests_fail",
                    &format!("Failed to count user requests: {err}")
                );
                crate::error!(Database, format!("Failed to count user requests: {err}"))
            })?;

        Ok(UserServiceCardsResponse {
            total_api_keys: to_i32(total_api_keys),
            active_api_keys: to_i32(active_api_keys),
            requests: to_i64(requests),
        })
    }

    /// 查询列表
    pub async fn list(
        &self,
        user_id: i32,
        query: &UserServiceKeyQuery,
        timezone: &Tz,
    ) -> Result<UserServiceKeyListResponse> {
        let page = query.page.map(u64::from);
        let limit = query.limit.map(u64::from);
        let pagination_params = PaginationParams::new(page, limit, 10, 100);

        let mut selector =
            UserServiceApis::find().filter(user_service_apis::Column::UserId.eq(user_id));

        if let Some(name) = &query.name {
            selector = selector.filter(user_service_apis::Column::Name.like(format!("%{name}%")));
        }
        if let Some(description) = &query.description {
            selector = selector
                .filter(user_service_apis::Column::Description.like(format!("%{description}%")));
        }
        if let Some(provider_type_id) = query.provider_type_id {
            selector =
                selector.filter(user_service_apis::Column::ProviderTypeId.eq(provider_type_id));
        }
        if let Some(is_active) = query.is_active {
            selector = selector.filter(user_service_apis::Column::IsActive.eq(is_active));
        }

        let total = selector.clone().count(self.db).await.map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "count_service_apis_fail",
                &format!("Failed to count user service APIs: {err}")
            );
            crate::error!(
                Database,
                format!("Failed to count user service APIs: {err}")
            )
        })?;

        let rows = selector
            .find_also_related(ProviderTypes)
            .offset(pagination_params.offset())
            .limit(pagination_params.limit)
            .all(self.db)
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "fetch_service_apis_fail",
                    &format!("Failed to fetch user service APIs: {err}")
                );
                crate::error!(
                    Database,
                    format!("Failed to fetch user service APIs: {err}")
                )
            })?;

        let mut service_api_keys = Vec::with_capacity(rows.len());
        for (api, provider_type) in rows {
            let response = self
                .build_user_service_key_response(api, provider_type, timezone)
                .await?;
            service_api_keys.push(response);
        }

        let pagination = build_page(total, pagination_params);
        let pagination = Pagination {
            page: pagination.page,
            limit: pagination.limit,
            total: pagination.total,
            pages: pagination.pages,
        };

        Ok(UserServiceKeyListResponse {
            service_api_keys,
            pagination,
        })
    }

    /// 创建新的用户服务 API
    pub async fn create(
        &self,
        user_id: i32,
        request: &CreateUserServiceKeyRequest,
        timezone: &Tz,
    ) -> Result<CreateUserServiceKeyResponse> {
        let api_key = format!("sk-usr-{}", Uuid::new_v4().to_string().replace('-', ""));
        let expires_at = parse_optional_rfc3339(request.expires_at.as_deref())?;
        let now = Utc::now().naive_utc();

        let user_provider_keys_ids = serde_json::to_value(&request.user_provider_keys_ids)
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Internal,
                    LogComponent::Database,
                    "serialize_ids_fail",
                    &format!("Failed to serialize user_provider_keys_ids: {err}")
                );
                crate::error!(
                    Internal,
                    format!("Failed to serialize user provider key ids: {err}")
                )
            })?;

        let model = user_service_apis::ActiveModel {
            user_id: Set(user_id),
            provider_type_id: Set(request.provider_type_id),
            api_key: Set(api_key.clone()),
            name: Set(Some(request.name.clone())),
            description: Set(request.description.clone()),
            user_provider_keys_ids: Set(user_provider_keys_ids),
            scheduling_strategy: Set(request.scheduling_strategy.clone()),
            retry_count: Set(request.retry_count),
            timeout_seconds: Set(request.timeout_seconds),
            max_request_per_min: Set(request.max_request_per_min),
            max_requests_per_day: Set(request.max_requests_per_day),
            max_tokens_per_day: Set(request.max_tokens_per_day),
            max_cost_per_day: Set(request.max_cost_per_day),
            expires_at: Set(expires_at),
            is_active: Set(request.is_active.unwrap_or(true)),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let inserted = model.insert(self.db).await.map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "insert_service_api_fail",
                &format!("Failed to insert user service API: {err}")
            );
            crate::error!(Database, format!("Failed to create API Key: {err}"))
        })?;

        Ok(CreateUserServiceKeyResponse {
            id: inserted.id,
            api_key: inserted.api_key,
            name: inserted.name,
            description: inserted.description,
            provider_type_id: inserted.provider_type_id,
            is_active: inserted.is_active,
            created_at: format_naive_utc(&inserted.created_at, *timezone),
        })
    }

    /// 获取详情
    pub async fn detail(
        &self,
        api_id: i32,
        user_id: i32,
        timezone: &Tz,
    ) -> Result<UserServiceKeyDetailResponse> {
        ensure_positive(api_id)?;
        let api = self.find_user_api(api_id, user_id).await?;

        let provider = ProviderTypes::find_by_id(api.provider_type_id)
            .one(self.db)
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "query_provider_type_fail",
                    &format!("Failed to fetch provider type: {err}")
                );
                crate::error!(Database, format!("Failed to fetch provider type: {err}"))
            })?
            .ok_or_else(|| {
                business_error(format!("Provider type not found: {}", api.provider_type_id))
            })?;

        let user_provider_keys_ids =
            serde_json::from_value::<Vec<i32>>(api.user_provider_keys_ids.clone())
                .unwrap_or_default();

        Ok(UserServiceKeyDetailResponse {
            id: api.id,
            name: api.name.unwrap_or_default(),
            description: api.description,
            provider_type_id: api.provider_type_id,
            provider: provider.display_name,
            api_key: api.api_key,
            user_provider_keys_ids,
            scheduling_strategy: api.scheduling_strategy,
            retry_count: api.retry_count,
            timeout_seconds: api.timeout_seconds,
            max_request_per_min: api.max_request_per_min,
            max_requests_per_day: api.max_requests_per_day,
            max_tokens_per_day: api.max_tokens_per_day,
            max_cost_per_day: api.max_cost_per_day,
            expires_at: api.expires_at.map(|dt| format_naive_utc(&dt, *timezone)),
            is_active: api.is_active,
            created_at: format_naive_utc(&api.created_at, *timezone),
            updated_at: format_naive_utc(&api.updated_at, *timezone),
        })
    }

    /// 更新
    pub async fn update(
        &self,
        api_id: i32,
        user_id: i32,
        request: &UpdateUserServiceKeyRequest,
    ) -> Result<UpdateUserServiceKeyResponse> {
        ensure_positive(api_id)?;
        let existing = self.find_user_api(api_id, user_id).await?;

        let expires_at = match &request.expires_at {
            Some(value) => Some(parse_rfc3339(value)?),
            None => existing.expires_at,
        };

        let mut model = user_service_apis::ActiveModel {
            id: Set(api_id),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        if let Some(name) = &request.name {
            model.name = Set(Some(name.clone()));
        }
        if let Some(description) = &request.description {
            model.description = Set(Some(description.clone()));
        }
        if let Some(user_provider_keys_ids) = &request.user_provider_keys_ids {
            let value =
                serde_json::to_value(user_provider_keys_ids).unwrap_or(Value::Array(vec![]));
            model.user_provider_keys_ids = Set(value);
        }
        if let Some(strategy) = &request.scheduling_strategy {
            model.scheduling_strategy = Set(Some(strategy.clone()));
        }
        model.retry_count = Set(request.retry_count);
        model.timeout_seconds = Set(request.timeout_seconds);
        model.max_request_per_min = Set(request.max_request_per_min);
        model.max_requests_per_day = Set(request.max_requests_per_day);
        model.max_tokens_per_day = Set(request.max_tokens_per_day);
        model.max_cost_per_day = Set(request.max_cost_per_day);
        model.expires_at = Set(expires_at);

        let updated = model.update(self.db).await.map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "update_service_api_fail",
                &format!("Failed to update user service API: {err}")
            );
            crate::error!(Database, format!("Failed to update API Key: {err}"))
        })?;

        Ok(UpdateUserServiceKeyResponse {
            id: updated.id,
            name: updated.name,
            description: updated.description,
            updated_at: DateTime::<Utc>::from_naive_utc_and_offset(updated.updated_at, Utc)
                .to_rfc3339(),
        })
    }

    /// 删除
    pub async fn delete(&self, api_id: i32, user_id: i32) -> Result<()> {
        ensure_positive(api_id)?;
        self.find_user_api(api_id, user_id).await?;

        let result = UserServiceApis::delete_by_id(api_id)
            .exec(self.db)
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "delete_service_api_fail",
                    &format!("Failed to delete user service API: {err}")
                );
                crate::error!(Database, format!("Failed to delete API Key: {err}"))
            })?;

        if result.rows_affected == 0 {
            return Err(ProxyError::internal("Failed to delete API Key"));
        }

        Ok(())
    }

    /// 使用统计
    pub async fn usage_stats(
        &self,
        api_id: i32,
        user_id: i32,
        query: &UsageStatsQuery,
        timezone: &Tz,
    ) -> Result<UserServiceKeyUsageResponse> {
        ensure_positive(api_id)?;
        self.find_user_api(api_id, user_id).await?;

        let range = resolve_usage_range(query, *timezone)?;
        let tracings = ProxyTracing::find()
            .filter(proxy_tracing::Column::UserServiceApiId.eq(api_id))
            .filter(proxy_tracing::Column::CreatedAt.gte(range.start.naive_utc()))
            .filter(proxy_tracing::Column::CreatedAt.lt(range.end.naive_utc()))
            .all(self.db)
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Tracing,
                    "fetch_tracings_fail",
                    &format!("Failed to fetch proxy tracings: {err}")
                );
                crate::error!(Database, format!("Failed to fetch usage statistics: {err}"))
            })?;

        let total_requests = u64::try_from(tracings.len()).unwrap_or(0);
        let successful_requests =
            u64::try_from(tracings.iter().filter(|t| t.is_success).count()).unwrap_or(0);
        let failed_requests = total_requests.saturating_sub(successful_requests);
        let success_rate = ratio_as_percentage(successful_requests, total_requests);

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

        let avg_response_time = if total_requests > 0 {
            let total_duration: i64 = tracings.iter().map(|t| t.duration_ms.unwrap_or(0)).sum();
            total_duration / i64::try_from(total_requests).unwrap_or(1)
        } else {
            0
        };

        let last_used = tracings
            .iter()
            .max_by_key(|t| t.created_at)
            .map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t.created_at, Utc).to_rfc3339());

        Ok(UserServiceKeyUsageResponse {
            total_requests,
            successful_requests,
            failed_requests,
            success_rate,
            total_tokens,
            tokens_prompt,
            tokens_completion,
            cache_create_tokens,
            cache_read_tokens,
            total_cost,
            cost_currency: "USD",
            avg_response_time,
            last_used,
            usage_trend: Vec::new(),
        })
    }

    /// 重新生成
    pub async fn regenerate(
        &self,
        api_id: i32,
        user_id: i32,
    ) -> Result<RegenerateUserServiceKeyResponse> {
        ensure_positive(api_id)?;
        self.find_user_api(api_id, user_id).await?;

        let new_api_key = format!("sk-usr-{}", Uuid::new_v4().to_string().replace('-', ""));
        let now = Utc::now().naive_utc();

        let model = user_service_apis::ActiveModel {
            id: Set(api_id),
            api_key: Set(new_api_key.clone()),
            updated_at: Set(now),
            ..Default::default()
        };

        let updated = model.update(self.db).await.map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "regenerate_key_fail",
                &format!("Failed to regenerate API key: {err}")
            );
            crate::error!(Database, format!("Failed to regenerate API key: {err}"))
        })?;

        Ok(RegenerateUserServiceKeyResponse {
            id: updated.id,
            api_key: new_api_key,
            regenerated_at: DateTime::<Utc>::from_naive_utc_and_offset(updated.updated_at, Utc)
                .to_rfc3339(),
        })
    }

    /// 更新启用状态
    pub async fn update_status(
        &self,
        api_id: i32,
        user_id: i32,
        request: &UpdateStatusRequest,
    ) -> Result<UpdateUserServiceKeyStatusResponse> {
        ensure_positive(api_id)?;
        self.find_user_api(api_id, user_id).await?;

        let model = user_service_apis::ActiveModel {
            id: Set(api_id),
            is_active: Set(request.is_active),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        let updated = model.update(self.db).await.map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "update_key_status_fail",
                &format!("Failed to update API key status: {err}")
            );
            crate::error!(Database, format!("Failed to update API key status: {err}"))
        })?;

        Ok(UpdateUserServiceKeyStatusResponse {
            id: updated.id,
            is_active: updated.is_active,
            updated_at: DateTime::<Utc>::from_naive_utc_and_offset(updated.updated_at, Utc)
                .to_rfc3339(),
        })
    }

    async fn build_user_service_key_response(
        &self,
        api: user_service_apis::Model,
        provider_type: Option<entity::provider_types::Model>,
        timezone: &Tz,
    ) -> Result<UserServiceKeyResponse> {
        let tracings = ProxyTracing::find()
            .filter(proxy_tracing::Column::UserServiceApiId.eq(api.id))
            .all(self.db)
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Tracing,
                    "fetch_tracings_fail",
                    &format!("Failed to fetch proxy tracings: {err}")
                );
                crate::error!(Database, format!("Failed to fetch proxy tracings: {err}"))
            })?;

        let success_count =
            u64::try_from(tracings.iter().filter(|t| t.is_success).count()).unwrap_or(0);
        let total_requests = u64::try_from(tracings.len()).unwrap_or(success_count);
        let failure_count = total_requests.saturating_sub(success_count);
        let success_rate = ratio_as_percentage(success_count, total_requests);
        let total_response_time: i64 = tracings.iter().map(|t| t.duration_ms.unwrap_or(0)).sum();
        let avg_response_time = if success_count > 0 {
            total_response_time / i64::try_from(success_count).unwrap_or(1)
        } else {
            0
        };
        let total_cost: f64 = tracings.iter().map(|t| t.cost.unwrap_or(0.0)).sum();
        let total_tokens: i32 = tracings.iter().map(|t| t.tokens_total.unwrap_or(0)).sum();

        let last_used_at = ProxyTracing::find()
            .filter(proxy_tracing::Column::UserServiceApiId.eq(api.id))
            .order_by(proxy_tracing::Column::CreatedAt, Order::Desc)
            .one(self.db)
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Tracing,
                    "fetch_last_trace_fail",
                    &format!("Failed to fetch last proxy tracing: {err}")
                );
                crate::error!(
                    Database,
                    format!("Failed to fetch last proxy tracing: {err}")
                )
            })?
            .map(|tracing| {
                let utc_dt = DateTime::<Utc>::from_naive_utc_and_offset(tracing.created_at, Utc);
                timezone_utils::format_utc_for_response(&utc_dt, timezone)
            });

        let usage = serde_json::json!({
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
            .map_or_else(|| "Unknown".to_string(), |pt| pt.display_name.clone());

        Ok(UserServiceKeyResponse {
            id: api.id,
            name: api.name.unwrap_or_default(),
            description: api.description,
            provider: provider_name,
            provider_type_id: api.provider_type_id,
            api_key: api.api_key,
            usage: Some(usage),
            is_active: api.is_active,
            last_used_at,
            created_at: format_naive_utc(&api.created_at, *timezone),
            expires_at: api.expires_at.map(|dt| format_naive_utc(&dt, *timezone)),
            scheduling_strategy: api.scheduling_strategy,
            retry_count: api.retry_count,
            timeout_seconds: api.timeout_seconds,
            max_request_per_min: api.max_request_per_min,
            max_requests_per_day: api.max_requests_per_day,
            max_tokens_per_day: api.max_tokens_per_day,
            max_cost_per_day: api.max_cost_per_day,
        })
    }

    async fn count_user_service_keys(&self, user_id: i32, is_active: Option<bool>) -> Result<u64> {
        let mut query =
            UserServiceApis::find().filter(user_service_apis::Column::UserId.eq(user_id));
        if let Some(active) = is_active {
            query = query.filter(user_service_apis::Column::IsActive.eq(active));
        }
        query.count(self.db).await.map_err(|err| {
            lerror!(
                "system",
                LogStage::Db,
                LogComponent::Database,
                "count_service_apis_fail",
                &format!("Failed to count user service APIs: {err}")
            );
            crate::error!(
                Database,
                format!("Failed to count user service APIs: {err}")
            )
        })
    }

    async fn find_user_api(&self, api_id: i32, user_id: i32) -> Result<user_service_apis::Model> {
        UserServiceApis::find_by_id(api_id)
            .filter(user_service_apis::Column::UserId.eq(user_id))
            .one(self.db)
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Database,
                    "fetch_service_apis_fail",
                    &format!("Failed to fetch user service API: {err}")
                );
                crate::error!(Database, format!("Failed to fetch user service API: {err}"))
            })?
            .ok_or_else(|| business_error(format!("API Key not found: {api_id}")))
    }
}

fn business_error(message: impl Into<String>) -> ProxyError {
    crate::error!(Authentication, message.into())
}

fn ensure_positive(id: i32) -> Result<()> {
    if id <= 0 {
        return Err(business_error("Invalid API ID"));
    }
    Ok(())
}

fn to_i32(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn parse_optional_rfc3339(value: Option<&str>) -> Result<Option<NaiveDateTime>> {
    value.map(parse_rfc3339).transpose()
}

fn parse_rfc3339(value: &str) -> Result<NaiveDateTime> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.naive_utc())
        .map_err(|_| business_error("过期时间格式错误，请使用ISO 8601格式"))
}

fn format_naive_utc(naive: &NaiveDateTime, timezone: Tz) -> String {
    let utc_dt = DateTime::<Utc>::from_naive_utc_and_offset(*naive, Utc);
    timezone_utils::format_utc_for_response(&utc_dt, &timezone)
}

fn resolve_usage_range(query: &UsageStatsQuery, timezone: Tz) -> Result<Range<DateTime<Utc>>> {
    let now = Utc::now();
    let (today_start, today_end) =
        timezone_utils::local_day_bounds(&now, &timezone).ok_or_else(|| {
            crate::error!(
                Conversion,
                "Failed to calculate local day bounds for timezone {}",
                timezone
            )
        })?;

    let (start, end) = match query.time_range.as_deref() {
        Some("today") => (today_start, today_end),
        Some("7days") => (today_end - Duration::days(7), today_end),
        Some("30days" | _) => (today_end - Duration::days(30), today_end),
        None => {
            if let (Some(start_str), Some(end_str)) = (&query.start_date, &query.end_date) {
                let start_date = NaiveDate::parse_from_str(start_str, "%Y-%m-%d")
                    .map_err(|_| business_error("Invalid start date format"))?;
                let end_date = NaiveDate::parse_from_str(end_str, "%Y-%m-%d")
                    .map_err(|_| business_error("Invalid end date format"))?;
                if start_date > end_date {
                    (today_end - Duration::days(30), today_end)
                } else {
                    let start_window = timezone_utils::local_date_window(start_date, 1, &timezone)
                        .unwrap_or((today_end - Duration::days(30), today_end));
                    let end_window = timezone_utils::local_date_window(end_date, 1, &timezone)
                        .unwrap_or((today_end - Duration::days(30), today_end));
                    (start_window.0, end_window.1)
                }
            } else {
                (today_end - Duration::days(30), today_end)
            }
        }
    };

    Ok(start..end)
}
