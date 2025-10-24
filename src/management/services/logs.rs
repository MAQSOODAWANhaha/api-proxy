//! # 日志管理服务
//!
//! 将原先 handler 中的复杂查询逻辑集中在服务层，便于复用与测试。

use crate::{
    error::{ProxyError, Result},
    lerror, linfo,
    logging::{LogComponent, LogStage},
    management::{middleware::auth::AuthContext, server::ManagementState},
    types::{ConvertToUtc, ProviderTypeId, TimezoneContext, timezone_utils},
};
use chrono::{DateTime, Utc};
use entity::{
    ProviderTypes, ProxyTracing, UserProviderKeys, UserServiceApis, proxy_tracing,
    user_provider_keys, user_service_apis,
};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Select,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::shared::{PaginationInfo, PaginationParams, build_page};

/// 日志仪表板统计响应
#[derive(Debug, Serialize)]
pub struct LogsDashboardStatsResponse {
    pub total_requests: i64,
    pub successful_requests: i64,
    pub failed_requests: i64,
    pub success_rate: f64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub avg_response_time: i64,
}

/// 日志列表条目（省略 `request_id` 字段）
#[derive(Debug, Serialize)]
pub struct ProxyTraceListEntry {
    pub id: i32,
    pub user_service_api_id: i32,
    pub user_provider_key_id: Option<i32>,
    pub user_id: Option<i32>,
    pub method: String,
    pub path: Option<String>,
    pub status_code: Option<i32>,
    pub tokens_prompt: i32,
    pub tokens_completion: i32,
    pub tokens_total: i32,
    pub token_efficiency_ratio: Option<f64>,
    pub cache_create_tokens: i32,
    pub cache_read_tokens: i32,
    pub cost: Option<f64>,
    pub cost_currency: String,
    pub model_used: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub provider_type_id: Option<ProviderTypeId>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub duration_ms: Option<i64>,
    pub is_success: bool,
    pub created_at: String,
    pub provider_name: Option<String>,
    pub user_service_api_name: Option<String>,
    pub user_provider_key_name: Option<String>,
}

/// 代理跟踪日志详情条目
#[derive(Debug, Serialize)]
pub struct ProxyTraceEntry {
    pub id: i32,
    pub request_id: String,
    pub user_service_api_id: i32,
    pub user_provider_key_id: Option<i32>,
    pub user_id: Option<i32>,
    pub method: String,
    pub path: Option<String>,
    pub status_code: Option<i32>,
    pub tokens_prompt: i32,
    pub tokens_completion: i32,
    pub tokens_total: i32,
    pub token_efficiency_ratio: Option<f64>,
    pub cache_create_tokens: i32,
    pub cache_read_tokens: i32,
    pub cost: Option<f64>,
    pub cost_currency: String,
    pub model_used: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub provider_type_id: Option<ProviderTypeId>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub duration_ms: Option<i64>,
    pub is_success: bool,
    pub created_at: String,
    pub provider_name: Option<String>,
    pub user_service_api_name: Option<String>,
    pub user_provider_key_name: Option<String>,
}

/// 日志列表响应
#[derive(Debug, Serialize)]
pub struct LogsListResponse {
    pub traces: Vec<ProxyTraceListEntry>,
    pub pagination: PaginationInfo,
}

/// 日志分析响应
#[derive(Debug, Serialize)]
pub struct LogsAnalyticsResponse {
    pub time_series: Vec<TimeSeriesData>,
    pub model_distribution: Vec<ModelDistribution>,
    pub provider_distribution: Vec<ProviderDistribution>,
    pub status_distribution: Vec<StatusDistribution>,
}

#[derive(Debug, Serialize)]
pub struct TimeSeriesData {
    pub timestamp: String,
    pub total_requests: i64,
    pub successful_requests: i64,
    pub failed_requests: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub avg_response_time: i64,
}

#[derive(Debug, Serialize)]
pub struct ModelDistribution {
    pub model: String,
    pub request_count: i64,
    pub token_count: i64,
    pub cost: f64,
    pub percentage: f64,
}

#[derive(Debug, Serialize)]
pub struct ProviderDistribution {
    pub provider_name: String,
    pub request_count: i64,
    pub success_rate: f64,
    pub avg_response_time: i64,
}

#[derive(Debug, Serialize)]
pub struct StatusDistribution {
    pub status_code: i32,
    pub count: i64,
    pub percentage: f64,
}

/// 日志列表查询参数
#[derive(Debug, Deserialize)]
pub struct LogsListQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
    pub search: Option<String>,
    pub method: Option<String>,
    pub status_code: Option<i32>,
    pub is_success: Option<bool>,
    pub model_used: Option<String>,
    pub provider_type_id: Option<ProviderTypeId>,
    pub user_service_api_id: Option<i32>,
    pub user_service_api_name: Option<String>,
    pub user_provider_key_name: Option<String>,
    pub start_time: Option<chrono::NaiveDateTime>,
    pub end_time: Option<chrono::NaiveDateTime>,
}

/// 日志分析查询参数
#[derive(Debug, Deserialize)]
pub struct LogsAnalyticsQuery {
    pub time_range: Option<String>, // 1h, 6h, 24h, 7d, 30d
    pub group_by: Option<String>,   // hour, day, model, provider, status
}

/// 日志服务
pub struct LogsService<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> LogsService<'a> {
    #[must_use]
    pub fn new(state: &'a ManagementState) -> Self {
        Self {
            db: state.database.as_ref(),
        }
    }

    const fn db(&self) -> &'a DatabaseConnection {
        self.db
    }

    /// 获取仪表板统计信息
    pub async fn dashboard_stats(&self) -> Result<LogsDashboardStatsResponse> {
        self.calculate_dashboard_stats().await
    }

    /// 分页获取日志列表
    pub async fn traces_list(
        &self,
        auth: &AuthContext,
        timezone: &TimezoneContext,
        query: &LogsListQuery,
    ) -> Result<LogsListResponse> {
        let params = PaginationParams::new(query.page, query.limit, 20, 100);
        self.fetch_traces_list(auth, timezone, query, params).await
    }

    /// 获取日志详情
    pub async fn trace_detail(
        &self,
        id: i32,
        timezone: &TimezoneContext,
    ) -> Result<Option<ProxyTraceEntry>> {
        self.fetch_trace_detail(id, timezone).await
    }

    /// 获取日志分析数据
    pub async fn analytics(
        &self,
        query: &LogsAnalyticsQuery,
        timezone: &TimezoneContext,
    ) -> Result<LogsAnalyticsResponse> {
        let time_range = query.time_range.as_deref().unwrap_or("24h");
        let group_by = query.group_by.as_deref().unwrap_or("hour");
        self.fetch_logs_analytics(time_range, group_by, timezone)
            .await
    }

    async fn calculate_dashboard_stats(&self) -> Result<LogsDashboardStatsResponse> {
        let total_requests = ProxyTracing::find()
            .count(self.db())
            .await
            .map_err(|err| db_error("统计总请求数失败", &err))?;

        let total_requests = i64::try_from(total_requests).unwrap_or(0);

        let successful_requests = ProxyTracing::find()
            .filter(proxy_tracing::Column::IsSuccess.eq(true))
            .count(self.db())
            .await
            .map_err(|err| db_error("统计成功请求数失败", &err))?;
        let successful_requests = i64::try_from(successful_requests).unwrap_or(0);

        let failed_requests = total_requests.saturating_sub(successful_requests);
        let success_rate = if total_requests > 0 {
            let successful = f64::from(u32::try_from(successful_requests).unwrap_or(0));
            let total = f64::from(u32::try_from(total_requests).unwrap_or(1));
            (successful / total) * 100.0
        } else {
            0.0
        };

        let total_tokens = ProxyTracing::find()
            .select_only()
            .column_as(proxy_tracing::Column::TokensTotal.sum(), "total_tokens")
            .into_tuple::<Option<i64>>()
            .one(self.db())
            .await
            .map_err(|err| db_error("统计总 Token 数失败", &err))?
            .flatten()
            .unwrap_or(0);

        let total_cost = ProxyTracing::find()
            .select_only()
            .column_as(proxy_tracing::Column::Cost.sum(), "total_cost")
            .into_tuple::<Option<f64>>()
            .one(self.db())
            .await
            .map_err(|err| db_error("统计总费用失败", &err))?
            .flatten()
            .unwrap_or(0.0);

        let duration_result = ProxyTracing::find()
            .filter(proxy_tracing::Column::DurationMs.is_not_null())
            .select_only()
            .column_as(proxy_tracing::Column::DurationMs.sum(), "total_duration")
            .column_as(proxy_tracing::Column::Id.count(), "request_count")
            .into_tuple::<(Option<i64>, i64)>()
            .one(self.db())
            .await
            .map_err(|err| db_error("统计平均响应时间失败", &err))?;

        let avg_response_time = match duration_result {
            Some((Some(total_duration), count)) if count > 0 => total_duration / count,
            _ => 0,
        };

        Ok(LogsDashboardStatsResponse {
            total_requests,
            successful_requests,
            failed_requests,
            success_rate: (success_rate * 100.0).round() / 100.0,
            total_tokens,
            total_cost: (total_cost * 100.0).round() / 100.0,
            avg_response_time,
        })
    }

    async fn fetch_traces_list(
        &self,
        auth: &AuthContext,
        timezone: &TimezoneContext,
        query: &LogsListQuery,
        params: PaginationParams,
    ) -> Result<LogsListResponse> {
        let mut select = Self::base_trace_select(auth, query, timezone);
        select = self.filter_by_service_api_name(select, query).await?;
        select = self.filter_by_provider_key_name(select, query).await?;

        let total = self.count_traces(select.clone()).await?;
        let records = self.load_trace_records(select, params).await?;
        let lookups = self.collect_trace_lookups(&records).await?;
        let traces = build_trace_entries(records, lookups, timezone);
        let pagination = build_page(total, params);

        Ok(LogsListResponse { traces, pagination })
    }

    fn base_trace_select(
        auth: &AuthContext,
        query: &LogsListQuery,
        timezone: &TimezoneContext,
    ) -> Select<ProxyTracing> {
        let mut select = ProxyTracing::find();

        if auth.is_admin {
            linfo!(
                "system",
                LogStage::Internal,
                LogComponent::Tracing,
                "admin_access",
                &format!("Admin user {} accessing all traces", auth.user_id)
            );
        } else {
            select = select.filter(proxy_tracing::Column::UserId.eq(auth.user_id));
            linfo!(
                "system",
                LogStage::Internal,
                LogComponent::Tracing,
                "non_admin_access",
                &format!(
                    "Non-admin user {} accessing traces - filtering by user_id",
                    auth.user_id
                )
            );
        }

        if let Some(search) = &query.search
            && !search.trim().is_empty()
        {
            let search_pattern = format!("%{}%", search.trim());
            select = select.filter(
                Condition::any()
                    .add(proxy_tracing::Column::RequestId.like(&search_pattern))
                    .add(proxy_tracing::Column::Path.like(&search_pattern))
                    .add(proxy_tracing::Column::ModelUsed.like(&search_pattern)),
            );
        }

        if let Some(method) = &query.method {
            select = select.filter(proxy_tracing::Column::Method.eq(method));
        }

        if let Some(status_code) = query.status_code {
            select = select.filter(proxy_tracing::Column::StatusCode.eq(status_code));
        }

        if let Some(is_success) = query.is_success {
            select = select.filter(proxy_tracing::Column::IsSuccess.eq(is_success));
        }

        if let Some(model_used) = &query.model_used {
            select = select.filter(proxy_tracing::Column::ModelUsed.eq(model_used));
        }

        if let Some(provider_type_id) = query.provider_type_id {
            select = select.filter(proxy_tracing::Column::ProviderTypeId.eq(provider_type_id));
        }

        if let Some(user_service_api_id) = query.user_service_api_id {
            select = select.filter(proxy_tracing::Column::UserServiceApiId.eq(user_service_api_id));
        }

        if let Some(start_naive) = query.start_time
            && let Some(start_utc) = start_naive.to_utc(&timezone.timezone)
        {
            select = select.filter(proxy_tracing::Column::CreatedAt.gte(start_utc));
        }
        if let Some(end_naive) = query.end_time
            && let Some(end_utc) = end_naive.to_utc(&timezone.timezone)
        {
            select = select.filter(proxy_tracing::Column::CreatedAt.lte(end_utc));
        }

        select
    }

    async fn filter_by_service_api_name(
        &self,
        select: Select<ProxyTracing>,
        query: &LogsListQuery,
    ) -> Result<Select<ProxyTracing>> {
        let Some(name) = query
            .user_service_api_name
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        else {
            return Ok(select);
        };

        let pattern = format!("%{name}%");
        let matched_ids: Vec<i32> = UserServiceApis::find()
            .filter(user_service_apis::Column::Name.like(&pattern))
            .all(self.db())
            .await
            .map_err(|err| db_error("查询用户服务 API 名称失败", &err))?
            .into_iter()
            .map(|api| api.id)
            .collect();

        if matched_ids.is_empty() {
            Ok(select.filter(proxy_tracing::Column::UserServiceApiId.eq(-1)))
        } else {
            Ok(select.filter(proxy_tracing::Column::UserServiceApiId.is_in(matched_ids)))
        }
    }

    async fn filter_by_provider_key_name(
        &self,
        select: Select<ProxyTracing>,
        query: &LogsListQuery,
    ) -> Result<Select<ProxyTracing>> {
        let Some(name) = query
            .user_provider_key_name
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        else {
            return Ok(select);
        };

        let pattern = format!("%{name}%");
        let matched_ids: Vec<i32> = UserProviderKeys::find()
            .filter(user_provider_keys::Column::Name.like(&pattern))
            .all(self.db())
            .await
            .map_err(|err| db_error("查询用户密钥名称失败", &err))?
            .into_iter()
            .map(|key| key.id)
            .collect();

        if matched_ids.is_empty() {
            Ok(select.filter(proxy_tracing::Column::UserProviderKeyId.eq(-1)))
        } else {
            Ok(select.filter(proxy_tracing::Column::UserProviderKeyId.is_in(matched_ids)))
        }
    }

    async fn count_traces(&self, select: Select<ProxyTracing>) -> Result<u64> {
        select
            .count(self.db())
            .await
            .map_err(|err| db_error("统计日志总数失败", &err))
    }

    async fn load_trace_records(
        &self,
        select: Select<ProxyTracing>,
        params: PaginationParams,
    ) -> Result<Vec<TraceRecord>> {
        let records = select
            .order_by_desc(proxy_tracing::Column::CreatedAt)
            .offset(params.offset())
            .limit(params.limit)
            .find_with_related(ProviderTypes)
            .all(self.db())
            .await
            .map_err(|err| db_error("查询日志列表失败", &err))?;

        Ok(records
            .into_iter()
            .map(|(trace, providers)| TraceRecord {
                trace,
                provider_name: providers.first().map(|pt| pt.display_name.clone()),
            })
            .collect())
    }

    async fn collect_trace_lookups(&self, records: &[TraceRecord]) -> Result<TraceLookups> {
        let mut service_api_ids = HashSet::new();
        let mut provider_key_ids = HashSet::new();

        for record in records {
            service_api_ids.insert(record.trace.user_service_api_id);
            if let Some(key_id) = record.trace.user_provider_key_id {
                provider_key_ids.insert(key_id);
            }
        }

        let service_api_names = if service_api_ids.is_empty() {
            HashMap::new()
        } else {
            let ids: Vec<i32> = service_api_ids.into_iter().collect();
            UserServiceApis::find()
                .filter(user_service_apis::Column::Id.is_in(ids))
                .all(self.db())
                .await
                .map_err(|err| db_error("批量查询用户服务 API 名称失败", &err))?
                .into_iter()
                .filter_map(|model| model.name.map(|name| (model.id, name)))
                .collect()
        };

        let provider_key_names = if provider_key_ids.is_empty() {
            HashMap::new()
        } else {
            let ids: Vec<i32> = provider_key_ids.into_iter().collect();
            UserProviderKeys::find()
                .filter(user_provider_keys::Column::Id.is_in(ids))
                .all(self.db())
                .await
                .map_err(|err| db_error("批量查询用户密钥名称失败", &err))?
                .into_iter()
                .map(|model| (model.id, model.name))
                .collect()
        };

        Ok(TraceLookups {
            service_api_names,
            provider_key_names,
        })
    }

    async fn count_requests_since(&self, start_time: DateTime<Utc>) -> Result<i64> {
        ProxyTracing::find()
            .filter(proxy_tracing::Column::CreatedAt.gte(start_time))
            .count(self.db())
            .await
            .map(|count| i64::try_from(count).unwrap_or(0))
            .map_err(|err| db_error("统计总请求数失败", &err))
    }

    async fn model_distribution(
        &self,
        start_time: DateTime<Utc>,
        total_requests: i64,
    ) -> Result<Vec<ModelDistribution>> {
        let stats = ProxyTracing::find()
            .filter(proxy_tracing::Column::CreatedAt.gte(start_time))
            .filter(proxy_tracing::Column::ModelUsed.is_not_null())
            .select_only()
            .column(proxy_tracing::Column::ModelUsed)
            .column_as(proxy_tracing::Column::Id.count(), "request_count")
            .column_as(proxy_tracing::Column::TokensTotal.sum(), "token_count")
            .column_as(proxy_tracing::Column::Cost.sum(), "cost")
            .group_by(proxy_tracing::Column::ModelUsed)
            .into_tuple::<(Option<String>, i64, Option<i64>, Option<f64>)>()
            .all(self.db())
            .await
            .map_err(|err| db_error("统计模型分布失败", &err))?;

        Ok(stats
            .into_iter()
            .map(|(model, requests, tokens, cost)| {
                let percentage = calculate_percentage(requests, total_requests);
                ModelDistribution {
                    model: model.unwrap_or_else(|| "Unknown".to_string()),
                    request_count: requests,
                    token_count: tokens.unwrap_or(0),
                    cost: cost.unwrap_or(0.0),
                    percentage: (percentage * 100.0).round() / 100.0,
                }
            })
            .collect())
    }

    async fn provider_distribution(
        &self,
        start_time: DateTime<Utc>,
    ) -> Result<Vec<ProviderDistribution>> {
        let stats = ProxyTracing::find()
            .filter(proxy_tracing::Column::CreatedAt.gte(start_time))
            .find_with_related(ProviderTypes)
            .all(self.db())
            .await
            .map_err(|err| db_error("统计服务商分布失败", &err))?;

        let mut provider_map: HashMap<String, (i64, i64, i64)> = HashMap::new();
        for (trace, providers) in stats {
            let provider_name = providers
                .first()
                .map_or_else(|| "Unknown".to_string(), |pt| pt.display_name.clone());
            let entry = provider_map.entry(provider_name).or_insert((0, 0, 0));
            entry.0 += 1;
            if trace.is_success {
                entry.1 += 1;
            }
            if let Some(duration) = trace.duration_ms {
                entry.2 += duration;
            }
        }

        Ok(provider_map
            .into_iter()
            .map(|(name, (total, success, total_duration))| {
                let success_rate = calculate_percentage(success, total);
                let avg_response_time = if total > 0 { total_duration / total } else { 0 };
                ProviderDistribution {
                    provider_name: name,
                    request_count: total,
                    success_rate: (success_rate * 100.0).round() / 100.0,
                    avg_response_time,
                }
            })
            .collect())
    }

    async fn status_distribution(
        &self,
        start_time: DateTime<Utc>,
        total_requests: i64,
    ) -> Result<Vec<StatusDistribution>> {
        let stats = ProxyTracing::find()
            .filter(proxy_tracing::Column::CreatedAt.gte(start_time))
            .filter(proxy_tracing::Column::StatusCode.is_not_null())
            .select_only()
            .column(proxy_tracing::Column::StatusCode)
            .column_as(proxy_tracing::Column::Id.count(), "count")
            .group_by(proxy_tracing::Column::StatusCode)
            .into_tuple::<(Option<i32>, i64)>()
            .all(self.db())
            .await
            .map_err(|err| db_error("统计状态码分布失败", &err))?;

        Ok(stats
            .into_iter()
            .map(|(status_code, count)| StatusDistribution {
                status_code: status_code.unwrap_or(0),
                count,
                percentage: (calculate_percentage(count, total_requests) * 100.0).round() / 100.0,
            })
            .collect())
    }

    async fn time_series_metrics(
        &self,
        start_time: DateTime<Utc>,
        total_requests: i64,
        timezone: &TimezoneContext,
        now: DateTime<Utc>,
    ) -> Result<Vec<TimeSeriesData>> {
        let successful_requests = ProxyTracing::find()
            .filter(proxy_tracing::Column::CreatedAt.gte(start_time))
            .filter(proxy_tracing::Column::IsSuccess.eq(true))
            .count(self.db())
            .await
            .map(|count| i64::try_from(count).unwrap_or(0))
            .map_err(|err| db_error("统计成功请求数失败", &err))?;

        let failed_requests = ProxyTracing::find()
            .filter(proxy_tracing::Column::CreatedAt.gte(start_time))
            .filter(proxy_tracing::Column::IsSuccess.eq(false))
            .count(self.db())
            .await
            .map(|count| i64::try_from(count).unwrap_or(0))
            .map_err(|err| db_error("统计失败请求数失败", &err))?;

        let total_tokens = ProxyTracing::find()
            .filter(proxy_tracing::Column::CreatedAt.gte(start_time))
            .select_only()
            .column_as(proxy_tracing::Column::TokensTotal.sum(), "total_tokens")
            .into_tuple::<Option<i64>>()
            .one(self.db())
            .await
            .map_err(|err| db_error("统计 Token 使用量失败", &err))?
            .flatten()
            .unwrap_or(0);

        let total_cost = ProxyTracing::find()
            .filter(proxy_tracing::Column::CreatedAt.gte(start_time))
            .select_only()
            .column_as(proxy_tracing::Column::Cost.sum(), "total_cost")
            .into_tuple::<Option<f64>>()
            .one(self.db())
            .await
            .map_err(|err| db_error("统计费用失败", &err))?
            .flatten()
            .unwrap_or(0.0);

        let avg_response_time = {
            let duration_result = ProxyTracing::find()
                .filter(proxy_tracing::Column::CreatedAt.gte(start_time))
                .filter(proxy_tracing::Column::DurationMs.is_not_null())
                .select_only()
                .column_as(proxy_tracing::Column::DurationMs.sum(), "total_duration")
                .column_as(proxy_tracing::Column::Id.count(), "request_count")
                .into_tuple::<(Option<i64>, i64)>()
                .one(self.db())
                .await
                .map_err(|err| db_error("统计平均响应时间失败", &err))?;

            match duration_result {
                Some((Some(total_duration), count)) if count > 0 => total_duration / count,
                _ => 0,
            }
        };

        Ok(vec![TimeSeriesData {
            timestamp: timezone_utils::format_utc_for_response(&now, &timezone.timezone),
            total_requests,
            successful_requests,
            failed_requests,
            total_tokens,
            total_cost,
            avg_response_time,
        }])
    }

    async fn fetch_trace_detail(
        &self,
        id: i32,
        timezone: &TimezoneContext,
    ) -> Result<Option<ProxyTraceEntry>> {
        let trace_with_relations = ProxyTracing::find_by_id(id)
            .find_with_related(ProviderTypes)
            .all(self.db())
            .await
            .map_err(|err| db_error("查询日志详情失败", &err))?;

        if let Some((trace_model, provider_types)) = trace_with_relations.into_iter().next() {
            let provider_name = provider_types.first().map(|pt| pt.display_name.clone());

            let provider_key_name = if let Some(provider_key_id) = trace_model.user_provider_key_id
            {
                UserProviderKeys::find_by_id(provider_key_id)
                    .one(self.db())
                    .await
                    .map_err(|err| db_error("查询用户密钥详情失败", &err))?
                    .map(|pk| pk.name)
            } else {
                None
            };

            let user_service_api_name =
                UserServiceApis::find_by_id(trace_model.user_service_api_id)
                    .one(self.db())
                    .await
                    .map_err(|err| db_error("查询用户服务 API 详情失败", &err))?
                    .and_then(|api| api.name);

            Ok(Some(ProxyTraceEntry {
                id: trace_model.id,
                request_id: trace_model.request_id,
                user_service_api_id: trace_model.user_service_api_id,
                user_provider_key_id: trace_model.user_provider_key_id,
                user_id: trace_model.user_id,
                method: trace_model.method,
                path: trace_model.path,
                status_code: trace_model.status_code,
                tokens_prompt: trace_model.tokens_prompt.unwrap_or(0),
                tokens_completion: trace_model.tokens_completion.unwrap_or(0),
                tokens_total: trace_model.tokens_total.unwrap_or(0),
                token_efficiency_ratio: trace_model.token_efficiency_ratio,
                cache_create_tokens: trace_model.cache_create_tokens.unwrap_or(0),
                cache_read_tokens: trace_model.cache_read_tokens.unwrap_or(0),
                cost: trace_model.cost,
                cost_currency: trace_model
                    .cost_currency
                    .unwrap_or_else(|| "USD".to_string()),
                model_used: trace_model.model_used,
                client_ip: trace_model.client_ip,
                user_agent: trace_model.user_agent,
                error_type: trace_model.error_type,
                error_message: trace_model.error_message,
                retry_count: trace_model.retry_count.unwrap_or(0),
                provider_type_id: trace_model.provider_type_id,
                start_time: timezone_utils::format_option_naive_utc_for_response(
                    trace_model.start_time.as_ref(),
                    &timezone.timezone,
                ),
                end_time: timezone_utils::format_option_naive_utc_for_response(
                    trace_model.end_time.as_ref(),
                    &timezone.timezone,
                ),
                duration_ms: trace_model.duration_ms,
                is_success: trace_model.is_success,
                created_at: timezone_utils::format_naive_utc_for_response(
                    &trace_model.created_at,
                    &timezone.timezone,
                ),
                provider_name,
                user_service_api_name,
                user_provider_key_name: provider_key_name,
            }))
        } else {
            Ok(None)
        }
    }
    async fn fetch_logs_analytics(
        &self,
        time_range: &str,
        _group_by: &str,
        timezone: &TimezoneContext,
    ) -> Result<LogsAnalyticsResponse> {
        let now = Utc::now();
        let start_time = resolve_start_time(time_range, now);
        let total_requests = self.count_requests_since(start_time).await?;
        let model_distribution = self.model_distribution(start_time, total_requests).await?;
        let provider_distribution = self.provider_distribution(start_time).await?;
        let status_distribution = self.status_distribution(start_time, total_requests).await?;
        let time_series = self
            .time_series_metrics(start_time, total_requests, timezone, now)
            .await?;

        Ok(LogsAnalyticsResponse {
            time_series,
            model_distribution,
            provider_distribution,
            status_distribution,
        })
    }
}

fn db_error(message: &str, err: &DbErr) -> ProxyError {
    lerror!(
        "system",
        LogStage::Db,
        LogComponent::Database,
        "logs_service_db_error",
        &format!("{message}: {err}")
    );
    crate::error!(Database, format!("{message}: {err}"))
}

fn resolve_start_time(time_range: &str, now: DateTime<Utc>) -> DateTime<Utc> {
    match time_range {
        "1h" => now - chrono::Duration::hours(1),
        "6h" => now - chrono::Duration::hours(6),
        "7d" => now - chrono::Duration::days(7),
        "30d" => now - chrono::Duration::days(30),
        _ => now - chrono::Duration::hours(24),
    }
}

fn calculate_percentage(part: i64, total: i64) -> f64 {
    if total <= 0 {
        0.0
    } else {
        clamp_ratio_component(part) / clamp_ratio_component(total)
    }
}

fn clamp_ratio_component(value: i64) -> f64 {
    let clamped = value.clamp(0, i64::from(i32::MAX));
    let limited = i32::try_from(clamped).unwrap_or(i32::MAX);
    f64::from(limited)
}

struct TraceRecord {
    trace: proxy_tracing::Model,
    provider_name: Option<String>,
}

struct TraceLookups {
    service_api_names: HashMap<i32, String>,
    provider_key_names: HashMap<i32, String>,
}

fn build_trace_entries(
    records: Vec<TraceRecord>,
    lookups: TraceLookups,
    timezone: &TimezoneContext,
) -> Vec<ProxyTraceListEntry> {
    let TraceLookups {
        service_api_names,
        provider_key_names,
    } = lookups;
    let mut traces = Vec::with_capacity(records.len());
    for record in records {
        let service_name = service_api_names
            .get(&record.trace.user_service_api_id)
            .cloned();
        let provider_key_name = record
            .trace
            .user_provider_key_id
            .and_then(|id| provider_key_names.get(&id).cloned());

        traces.push(ProxyTraceListEntry {
            id: record.trace.id,
            user_service_api_id: record.trace.user_service_api_id,
            user_provider_key_id: record.trace.user_provider_key_id,
            user_id: record.trace.user_id,
            method: record.trace.method,
            path: record.trace.path,
            status_code: record.trace.status_code,
            tokens_prompt: record.trace.tokens_prompt.unwrap_or(0),
            tokens_completion: record.trace.tokens_completion.unwrap_or(0),
            tokens_total: record.trace.tokens_total.unwrap_or(0),
            token_efficiency_ratio: record.trace.token_efficiency_ratio,
            cache_create_tokens: record.trace.cache_create_tokens.unwrap_or(0),
            cache_read_tokens: record.trace.cache_read_tokens.unwrap_or(0),
            cost: record.trace.cost,
            cost_currency: record
                .trace
                .cost_currency
                .clone()
                .unwrap_or_else(|| "USD".to_string()),
            model_used: record.trace.model_used.clone(),
            client_ip: record.trace.client_ip.clone(),
            user_agent: record.trace.user_agent.clone(),
            error_type: record.trace.error_type.clone(),
            error_message: record.trace.error_message.clone(),
            retry_count: record.trace.retry_count.unwrap_or(0),
            provider_type_id: record.trace.provider_type_id,
            start_time: timezone_utils::format_option_naive_utc_for_response(
                record.trace.start_time.as_ref(),
                &timezone.timezone,
            ),
            end_time: timezone_utils::format_option_naive_utc_for_response(
                record.trace.end_time.as_ref(),
                &timezone.timezone,
            ),
            duration_ms: record.trace.duration_ms,
            is_success: record.trace.is_success,
            created_at: timezone_utils::format_naive_utc_for_response(
                &record.trace.created_at,
                &timezone.timezone,
            ),
            provider_name: record.provider_name,
            user_service_api_name: service_name,
            user_provider_key_name: provider_key_name,
        });
    }
    traces
}
