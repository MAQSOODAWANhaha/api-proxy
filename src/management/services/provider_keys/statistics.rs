//! # 提供商密钥统计查询
//!
//! 密钥使用统计、趋势数据等查询功能。

use std::{collections::HashMap, convert::TryFrom};

use chrono::{DateTime, Utc};
use entity::{proxy_tracing::Column, proxy_tracing::Entity as ProxyTracing};
use sea_orm::{
    ColumnTrait, DbErr, EntityTrait, FromQueryResult, QueryFilter, QuerySelect, sea_query::Expr,
};
use serde_json::Value;

use crate::types::{TimezoneContext, ratio_as_percentage, timezone_utils};

use super::models::{DailyStats, TrendData, TrendDataPoint};

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

/// 获取提供商密钥使用统计
pub async fn fetch_provider_keys_usage_stats(
    provider_key_ids: &[i32],
    timezone_ctx: &TimezoneContext,
    db: &sea_orm::DatabaseConnection,
) -> HashMap<i32, super::models::ProviderKeyUsageStats> {
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
            use crate::{lerror, logging::LogComponent, logging::LogStage};

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
                super::models::ProviderKeyUsageStats {
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

/// 获取密钥趋势数据
pub async fn fetch_key_trends_data(
    db: &sea_orm::DatabaseConnection,
    key_id: i32,
    start_utc: &DateTime<Utc>,
    end_utc: &DateTime<Utc>,
    key_type: &str,
    timezone: &TimezoneContext,
) -> std::result::Result<TrendData, DbErr> {
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

/// 聚合每日统计数据
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

/// 四舍五入到两位小数
fn round_two_decimal(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// 构建提供商密钥 JSON 响应
pub fn build_provider_key_json(
    provider_key: &entity::user_provider_keys::Model,
    provider_type_opt: Option<entity::provider_types::Model>,
    usage_stats: &HashMap<i32, super::models::ProviderKeyUsageStats>,
    timezone_context: &TimezoneContext,
) -> Value {
    let provider_name =
        provider_type_opt.map_or_else(|| "Unknown".to_string(), |pt| pt.display_name);

    let stats = usage_stats
        .get(&provider_key.id)
        .cloned()
        .unwrap_or_default();

    serde_json::json!({
        "id": provider_key.id,
        "provider": provider_name,
        "provider_type_id": provider_key.provider_type_id,
        "name": provider_key.name,
        "api_key": provider_key.api_key,
        "auth_type": provider_key.auth_type,
        "auth_status": provider_key.auth_status,
        "health_status": provider_key.health_status,
        "health_status_detail": provider_key.health_status_detail,
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

/// 掩码 API 密钥
pub fn mask_api_key(key: &entity::user_provider_keys::Model) -> String {
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

/// 计算速率限制剩余秒数
pub fn rate_limit_remaining_seconds(
    key: &entity::user_provider_keys::Model,
    user_id: i32,
) -> Option<u64> {
    use crate::{
        linfo,
        logging::{LogComponent, LogStage},
    };

    let Some(resets_at) = key.rate_limit_resets_at else {
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
        return None;
    };
    let now = Utc::now().naive_utc();

    linfo!(
        "system",
        LogStage::Scheduling,
        LogComponent::KeyPool,
        "calc_rate_limit_remaining",
        "Calculating rate limit remaining time - reset time found in DB",
        key_id = key.id,
        user_id = user_id,
        rate_limit_resets_at = ?resets_at,
        current_time = ?now,
    );
    if resets_at <= now {
        linfo!(
            "system",
            LogStage::Scheduling,
            LogComponent::KeyPool,
            "rate_limit_expired",
            "Rate limit expired, returning None",
            key_id = key.id,
            user_id = user_id,
            rate_limit_resets_at = ?resets_at,
            current_time = ?now,
        );
        return None;
    }

    let seconds = resets_at.signed_duration_since(now).num_seconds().max(0);
    u64::try_from(seconds).map_or_else(
        |err| {
            linfo!(
                "system",
                LogStage::Scheduling,
                LogComponent::KeyPool,
                "rate_limit_conversion_fail",
                &format!("Failed to convert rate limit duration: {err}"),
                key_id = key.id,
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
                key_id = key.id,
                user_id = user_id,
                remaining_seconds = remaining_seconds,
                duration_seconds = seconds,
            );
            Some(remaining_seconds)
        },
    )
}

/// 构建更新响应
pub fn build_update_response(
    updated_key: &entity::user_provider_keys::Model,
    timezone_context: &TimezoneContext,
) -> Value {
    serde_json::json!({
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
