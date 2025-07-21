//! # 统计信息处理器

use crate::management::server::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use sea_orm::{entity::*, query::*, DatabaseConnection, DbErr};
use entity::{
    request_statistics, 
    request_statistics::Entity as RequestStatistics,
    provider_types,
    provider_types::Entity as ProviderTypes,
    user_service_apis,
    user_service_apis::Entity as UserServiceApis,
};
use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;

/// 统计查询参数
#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    /// 时间范围（小时）
    pub hours: Option<u32>,
    /// 分组方式
    pub group_by: Option<String>,
    /// 上游类型过滤
    pub upstream_type: Option<String>,
}

/// 提供商统计信息
#[derive(Debug)]
struct ProviderStats {
    requests: i64,
    successful_requests: i64,
    avg_response_time: f64,
    success_rate: f64,
}

/// 端点统计信息
#[derive(Debug)]
struct EndpointStats {
    path: String,
    requests: i64,
    percentage: f64,
}

/// 获取统计概览
pub async fn get_overview(
    State(state): State<AppState>,
    Query(query): Query<StatsQuery>,
) -> Result<Json<Value>, StatusCode> {
    let hours = query.hours.unwrap_or(24);
    let start_time = Utc::now() - Duration::hours(hours as i64);
    let end_time = Utc::now();
    
    // 查询指定时间范围内的请求统计
    let mut select = RequestStatistics::find()
        .filter(request_statistics::Column::CreatedAt.gte(start_time.naive_utc()));
    
    // 如果指定了上游类型过滤，需要通过子查询实现
    if let Some(upstream_type) = &query.upstream_type {
        // 首先获取匹配的provider_type_id
        let provider_ids: Vec<i32> = match ProviderTypes::find()
            .filter(provider_types::Column::Name.eq(upstream_type))
            .all(state.database.as_ref())
            .await
        {
            Ok(providers) => providers.into_iter().map(|p| p.id).collect(),
            Err(err) => {
                tracing::error!("Failed to fetch provider types: {}", err);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };
        
        if !provider_ids.is_empty() {
            // 获取匹配的user_service_api_id
            let api_ids: Vec<i32> = match UserServiceApis::find()
                .filter(user_service_apis::Column::ProviderTypeId.is_in(provider_ids))
                .all(state.database.as_ref())
                .await
            {
                Ok(apis) => apis.into_iter().map(|a| a.id).collect(),
                Err(err) => {
                    tracing::error!("Failed to fetch user service APIs: {}", err);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            };
            
            if !api_ids.is_empty() {
                select = select.filter(request_statistics::Column::UserServiceApiId.is_in(api_ids));
            } else {
                // 没有匹配的API，返回空结果
                select = select.filter(request_statistics::Column::Id.eq(-1));
            }
        } else {
            // 没有匹配的提供商类型，返回空结果
            select = select.filter(request_statistics::Column::Id.eq(-1));
        }
    }
    
    let stats = match select.all(state.database.as_ref()).await {
        Ok(stats) => stats,
        Err(err) => {
            tracing::error!("Failed to fetch request statistics: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // 计算基础统计
    let total_requests = stats.len() as i64;
    let successful_requests = stats.iter()
        .filter(|s| s.status_code.map_or(true, |code| code < 400))
        .count() as i64;
    let failed_requests = total_requests - successful_requests;
    let success_rate = if total_requests > 0 {
        (successful_requests as f64 / total_requests as f64) * 100.0
    } else {
        0.0
    };
    
    // 计算响应时间统计
    let mut response_times: Vec<i32> = stats.iter()
        .filter_map(|s| s.response_time_ms)
        .collect();
    response_times.sort_unstable();
    
    let avg_ms = if !response_times.is_empty() {
        response_times.iter().sum::<i32>() as f64 / response_times.len() as f64
    } else {
        0.0
    };
    
    let p50_ms = calculate_percentile(&response_times, 0.5);
    let p95_ms = calculate_percentile(&response_times, 0.95);
    let p99_ms = calculate_percentile(&response_times, 0.99);
    
    // 计算流量统计
    let total_request_size: i64 = stats.iter()
        .filter_map(|s| s.request_size)
        .map(|size| size as i64)
        .sum();
    let total_response_size: i64 = stats.iter()
        .filter_map(|s| s.response_size)
        .map(|size| size as i64)
        .sum();
    let requests_per_second = if hours > 0 {
        total_requests as f64 / (hours as f64 * 3600.0)
    } else {
        0.0
    };
    
    // 按提供商分组统计
    let provider_stats = match get_provider_stats(&state.database, &start_time, &end_time).await {
        Ok(stats) => stats,
        Err(err) => {
            tracing::error!("Failed to get provider stats: {}", err);
            HashMap::new()
        }
    };
    
    // 获取热门端点
    let top_endpoints = match get_top_endpoints(&stats, total_requests).await {
        Ok(endpoints) => endpoints,
        Err(err) => {
            tracing::error!("Failed to get top endpoints: {}", err);
            Vec::new()
        }
    };
    
    let overview = json!({
        "time_range": {
            "hours": hours,
            "start_time": start_time,
            "end_time": end_time
        },
        "requests": {
            "total": total_requests,
            "successful": successful_requests,
            "failed": failed_requests,
            "success_rate": success_rate
        },
        "response_times": {
            "avg_ms": avg_ms as i32,
            "p50_ms": p50_ms,
            "p95_ms": p95_ms,
            "p99_ms": p99_ms
        },
        "traffic": {
            "requests_per_second": requests_per_second,
            "bytes_sent": total_response_size,
            "bytes_received": total_request_size
        },
        "by_provider": provider_stats.into_iter().map(|(name, stats)| {
            (name, json!({
                "requests": stats.requests,
                "success_rate": stats.success_rate,
                "avg_response_ms": stats.avg_response_time as i32
            }))
        }).collect::<serde_json::Map<_, _>>(),
        "top_endpoints": top_endpoints.into_iter().map(|endpoint| {
            json!({
                "path": endpoint.path,
                "requests": endpoint.requests,
                "percentage": endpoint.percentage
            })
        }).collect::<Vec<_>>()
    });

    Ok(Json(overview))
}

/// 获取请求统计
pub async fn get_request_stats(
    State(state): State<AppState>,
    Query(query): Query<StatsQuery>,
) -> Result<Json<Value>, StatusCode> {
    let hours = query.hours.unwrap_or(24);
    let group_by = query.group_by.as_deref().unwrap_or("hour");
    
    let start_time = Utc::now() - Duration::hours(hours as i64);
    let end_time = Utc::now();
    
    // 查询指定时间范围内的请求统计
    let mut select = RequestStatistics::find()
        .filter(request_statistics::Column::CreatedAt.between(start_time.naive_utc(), end_time.naive_utc()));
    
    // 如果指定了上游类型过滤
    if let Some(upstream_type) = &query.upstream_type {
        // 首先获取匹配的provider_type_id
        let provider_ids: Vec<i32> = match ProviderTypes::find()
            .filter(provider_types::Column::Name.eq(upstream_type))
            .all(state.database.as_ref())
            .await
        {
            Ok(providers) => providers.into_iter().map(|p| p.id).collect(),
            Err(err) => {
                tracing::error!("Failed to fetch provider types: {}", err);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };
        
        if !provider_ids.is_empty() {
            // 获取匹配的user_service_api_id
            let api_ids: Vec<i32> = match UserServiceApis::find()
                .filter(user_service_apis::Column::ProviderTypeId.is_in(provider_ids))
                .all(state.database.as_ref())
                .await
            {
                Ok(apis) => apis.into_iter().map(|a| a.id).collect(),
                Err(err) => {
                    tracing::error!("Failed to fetch user service APIs: {}", err);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            };
            
            if !api_ids.is_empty() {
                select = select.filter(request_statistics::Column::UserServiceApiId.is_in(api_ids));
            } else {
                select = select.filter(request_statistics::Column::Id.eq(-1));
            }
        } else {
            select = select.filter(request_statistics::Column::Id.eq(-1));
        }
    }
    
    let stats = match select.all(state.database.as_ref()).await {
        Ok(stats) => stats,
        Err(err) => {
            tracing::error!("Failed to fetch request statistics: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // 生成时间序列数据
    let time_series = match generate_time_series(&stats, hours, group_by, &start_time).await {
        Ok(series) => series,
        Err(err) => {
            tracing::error!("Failed to generate time series: {}", err);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // 计算汇总数据
    let total_requests: i64 = time_series.iter()
        .map(|v| v.get("requests").and_then(|v| v.as_i64()).unwrap_or(0))
        .sum();
    let total_successful: i64 = time_series.iter()
        .map(|v| v.get("successful").and_then(|v| v.as_i64()).unwrap_or(0))
        .sum();
    let total_failed: i64 = time_series.iter()
        .map(|v| v.get("failed").and_then(|v| v.as_i64()).unwrap_or(0))
        .sum();
    let avg_response_ms = if total_requests > 0 {
        time_series.iter()
            .map(|v| v.get("avg_response_ms").and_then(|v| v.as_i64()).unwrap_or(0))
            .sum::<i64>() / time_series.len() as i64
    } else {
        0
    };
    
    let stats = json!({
        "time_range": {
            "hours": hours,
            "group_by": group_by,
            "start_time": start_time,
            "end_time": end_time,
            "points": time_series.len()
        },
        "data": time_series,
        "aggregated": {
            "total_requests": total_requests,
            "total_successful": total_successful,
            "total_failed": total_failed,
            "avg_response_ms": avg_response_ms
        }
    });

    Ok(Json(stats))
}

/// 计算百分位数
fn calculate_percentile(values: &[i32], percentile: f64) -> i32 {
    if values.is_empty() {
        return 0;
    }
    
    let index = (values.len() as f64 * percentile) as usize;
    let index = if index >= values.len() { values.len() - 1 } else { index };
    values[index]
}

/// 获取按提供商分组的统计信息
async fn get_provider_stats(
    db: &DatabaseConnection,
    start_time: &DateTime<Utc>,
    end_time: &DateTime<Utc>,
) -> Result<HashMap<String, ProviderStats>, DbErr> {
    // 获取统计数据，关联用户服务API
    let stats = RequestStatistics::find()
        .filter(request_statistics::Column::CreatedAt.between(start_time.naive_utc(), end_time.naive_utc()))
        .find_also_related(UserServiceApis)
        .all(db)
        .await?;
    
    // 获取提供商信息
    let providers: HashMap<i32, String> = ProviderTypes::find()
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    
    let mut provider_stats: HashMap<String, (i64, i64, Vec<i32>)> = HashMap::new();
    
    for (request_stat, user_service_api) in stats {
        if let Some(api) = user_service_api {
            if let Some(provider_name) = providers.get(&api.provider_type_id) {
                let entry = provider_stats.entry(provider_name.clone()).or_insert((0, 0, Vec::new()));
                entry.0 += 1; // 总请求数
                
                if request_stat.status_code.map_or(true, |code| code < 400) {
                    entry.1 += 1; // 成功请求数
                }
                
                if let Some(response_time) = request_stat.response_time_ms {
                    entry.2.push(response_time);
                }
            }
        }
    }
    
    let mut result = HashMap::new();
    for (provider_name, (total, successful, response_times)) in provider_stats {
        let success_rate = if total > 0 {
            (successful as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        
        let avg_response_time = if !response_times.is_empty() {
            response_times.iter().sum::<i32>() as f64 / response_times.len() as f64
        } else {
            0.0
        };
        
        result.insert(provider_name, ProviderStats {
            requests: total,
            successful_requests: successful,
            success_rate,
            avg_response_time,
        });
    }
    
    Ok(result)
}

/// 获取热门端点统计
async fn get_top_endpoints(
    stats: &[request_statistics::Model],
    total_requests: i64,
) -> Result<Vec<EndpointStats>, Box<dyn std::error::Error>> {
    let mut endpoint_counts: HashMap<String, i64> = HashMap::new();
    
    for stat in stats {
        if let Some(ref path) = stat.path {
            *endpoint_counts.entry(path.clone()).or_insert(0) += 1;
        }
    }
    
    let mut endpoints: Vec<EndpointStats> = endpoint_counts
        .into_iter()
        .map(|(path, requests)| {
            let percentage = if total_requests > 0 {
                (requests as f64 / total_requests as f64) * 100.0
            } else {
                0.0
            };
            EndpointStats {
                path,
                requests,
                percentage,
            }
        })
        .collect();
    
    // 按请求数排序，取前10个
    endpoints.sort_by(|a, b| b.requests.cmp(&a.requests));
    endpoints.truncate(10);
    
    Ok(endpoints)
}

/// 生成时间序列数据
async fn generate_time_series(
    stats: &[request_statistics::Model],
    hours: u32,
    group_by: &str,
    start_time: &DateTime<Utc>,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let mut time_series = Vec::new();
    
    // 根据分组方式决定时间间隔
    let (interval_duration, points) = match group_by {
        "day" => (Duration::days(1), std::cmp::max(1, hours / 24)),
        _ => (Duration::hours(1), hours), // 默认按小时
    };
    
    // 为每个时间点生成统计数据
    for i in 0..points {
        let interval_start = *start_time + (interval_duration * i as i32);
        let interval_end = interval_start + interval_duration;
        
        // 筛选当前时间段内的数据
        let interval_stats: Vec<&request_statistics::Model> = stats
            .iter()
            .filter(|stat| {
                let stat_time = stat.created_at.and_utc();
                stat_time >= interval_start && stat_time < interval_end
            })
            .collect();
        
        // 计算当前时间段的统计信息
        let requests = interval_stats.len() as i64;
        let successful = interval_stats
            .iter()
            .filter(|s| s.status_code.map_or(true, |code| code < 400))
            .count() as i64;
        let failed = requests - successful;
        
        // 计算平均响应时间
        let response_times: Vec<i32> = interval_stats
            .iter()
            .filter_map(|s| s.response_time_ms)
            .collect();
        let avg_response_ms = if !response_times.is_empty() {
            response_times.iter().sum::<i32>() as f64 / response_times.len() as f64
        } else {
            0.0
        } as i32;
        
        time_series.push(json!({
            "timestamp": interval_start,
            "requests": requests,
            "successful": successful,
            "failed": failed,
            "avg_response_ms": avg_response_ms,
            "success_rate": if requests > 0 {
                (successful as f64 / requests as f64) * 100.0
            } else {
                0.0
            }
        }));
    }
    
    Ok(time_series)
}