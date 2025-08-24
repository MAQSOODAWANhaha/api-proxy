//! # 统计服务
//!
//! 收集和聚合系统统计信息

use crate::cache::abstract_cache::UnifiedCacheManager;
use crate::cache::keys::CacheKeyBuilder;
use crate::config::AppConfig;
use anyhow::Result;
use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

/// 统计服务
pub struct StatisticsService {
    /// 应用配置
    #[allow(dead_code)]
    config: Arc<AppConfig>,
    /// 缓存管理器
    cache_manager: Arc<UnifiedCacheManager>,
    /// 内存统计数据
    memory_stats: Arc<RwLock<MemoryStats>>,
}

/// 内存统计数据
#[derive(Debug, Default)]
struct MemoryStats {
    /// 请求计数器
    request_counters: HashMap<String, u64>,
    /// 响应时间记录
    response_times: Vec<ResponseTimeRecord>,
    /// 错误计数器
    error_counters: HashMap<String, u64>,
}

/// 响应时间记录
#[derive(Debug, Clone)]
struct ResponseTimeRecord {
    /// 时间戳
    timestamp: DateTime<Utc>,
    /// 响应时间（毫秒）
    duration_ms: u64,
    /// 端点
    #[allow(dead_code)]
    endpoint: String,
    /// 上游类型
    upstream_type: String,
}

/// 缓存的请求统计数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedRequestStats {
    /// 日期
    pub date: String,
    /// 小时
    pub hour: u8,
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 总响应时间（毫秒）
    pub total_response_time_ms: u64,
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: f64,
    /// 端点统计
    pub endpoints: HashMap<String, EndpointStats>,
    /// 上游类型统计
    pub upstream_types: HashMap<String, UpstreamStats>,
    /// 最后更新时间
    pub last_updated: DateTime<Utc>,
}

/// 端点统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointStats {
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 总响应时间（毫秒）
    pub total_response_time_ms: u64,
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: f64,
}

/// 上游类型统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamStats {
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 总响应时间（毫秒）
    pub total_response_time_ms: u64,
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: f64,
}

/// 请求统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestStats {
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 成功率
    pub success_rate: f64,
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: f64,
    /// P95响应时间（毫秒）
    pub p95_response_time_ms: f64,
    /// P99响应时间（毫秒）
    pub p99_response_time_ms: f64,
}

/// 时间范围统计查询
#[derive(Debug, Clone)]
pub struct TimeRangeQuery {
    /// 开始时间
    pub start_time: DateTime<Utc>,
    /// 结束时间
    pub end_time: DateTime<Utc>,
    /// 分组间隔（小时）
    pub group_by_hours: Option<u32>,
}

impl StatisticsService {
    /// 创建新的统计服务
    pub fn new(config: Arc<AppConfig>, cache_manager: Arc<UnifiedCacheManager>) -> Self {
        Self {
            config,
            cache_manager,
            memory_stats: Arc::new(RwLock::new(MemoryStats::default())),
        }
    }

    /// 记录请求
    pub async fn record_request(
        &self,
        endpoint: &str,
        upstream_type: &str,
        duration_ms: u64,
        success: bool,
    ) -> Result<()> {
        let mut stats = self.memory_stats.write().await;

        // 更新请求计数器
        let key = format!("{}:{}", upstream_type, endpoint);
        *stats.request_counters.entry(key.clone()).or_insert(0) += 1;

        // 记录响应时间
        stats.response_times.push(ResponseTimeRecord {
            timestamp: Utc::now(),
            duration_ms,
            endpoint: endpoint.to_string(),
            upstream_type: upstream_type.to_string(),
        });

        // 清理旧的响应时间记录（保留最近1小时）
        let cutoff_time = Utc::now() - chrono::Duration::hours(1);
        stats
            .response_times
            .retain(|record| record.timestamp > cutoff_time);

        // 更新错误计数器
        if !success {
            *stats.error_counters.entry(key).or_insert(0) += 1;
        }

        debug!(
            endpoint = endpoint,
            upstream_type = upstream_type,
            duration_ms = duration_ms,
            success = success,
            "Recorded request statistics"
        );

        // 异步写入缓存
        self.persist_to_cache(endpoint, upstream_type, duration_ms, success)
            .await?;

        Ok(())
    }

    /// 获取请求统计信息
    pub async fn get_request_stats(
        &self,
        _time_range: Option<TimeRangeQuery>,
    ) -> Result<RequestStats> {
        let stats = self.memory_stats.read().await;

        // 计算总请求数
        let total_requests: u64 = stats.request_counters.values().sum();

        // 计算失败请求数
        let failed_requests: u64 = stats.error_counters.values().sum();
        let successful_requests = total_requests.saturating_sub(failed_requests);

        // 计算成功率
        let success_rate = if total_requests > 0 {
            (successful_requests as f64) / (total_requests as f64) * 100.0
        } else {
            0.0
        };

        // 计算响应时间统计
        let mut response_times: Vec<u64> = stats
            .response_times
            .iter()
            .map(|record| record.duration_ms)
            .collect();

        response_times.sort_unstable();

        let avg_response_time_ms = if !response_times.is_empty() {
            response_times.iter().sum::<u64>() as f64 / response_times.len() as f64
        } else {
            0.0
        };

        let p95_response_time_ms = if !response_times.is_empty() {
            let index = ((response_times.len() as f64) * 0.95) as usize;
            response_times
                .get(index.saturating_sub(1))
                .copied()
                .unwrap_or(0) as f64
        } else {
            0.0
        };

        let p99_response_time_ms = if !response_times.is_empty() {
            let index = ((response_times.len() as f64) * 0.99) as usize;
            response_times
                .get(index.saturating_sub(1))
                .copied()
                .unwrap_or(0) as f64
        } else {
            0.0
        };

        Ok(RequestStats {
            total_requests,
            successful_requests,
            failed_requests,
            success_rate,
            avg_response_time_ms,
            p95_response_time_ms,
            p99_response_time_ms,
        })
    }

    /// 获取按上游类型分组的统计信息
    pub async fn get_stats_by_upstream(&self) -> Result<HashMap<String, RequestStats>> {
        let stats = self.memory_stats.read().await;
        let mut upstream_stats: HashMap<String, (u64, u64, Vec<u64>)> = HashMap::new();

        // 按上游类型分组计算
        for (key, count) in &stats.request_counters {
            if let Some((upstream_type, _)) = key.split_once(':') {
                let entry =
                    upstream_stats
                        .entry(upstream_type.to_string())
                        .or_insert((0, 0, Vec::new()));
                entry.0 += count; // 总请求数
            }
        }

        // 计算错误数
        for (key, error_count) in &stats.error_counters {
            if let Some((upstream_type, _)) = key.split_once(':') {
                if let Some(entry) = upstream_stats.get_mut(upstream_type) {
                    entry.1 += error_count; // 错误数
                }
            }
        }

        // 收集响应时间
        for record in &stats.response_times {
            if let Some(entry) = upstream_stats.get_mut(&record.upstream_type) {
                entry.2.push(record.duration_ms);
            }
        }

        // 计算统计信息
        let mut result = HashMap::new();
        for (upstream_type, (total, failed, mut response_times)) in upstream_stats {
            let successful = total.saturating_sub(failed);
            let success_rate = if total > 0 {
                (successful as f64) / (total as f64) * 100.0
            } else {
                0.0
            };

            response_times.sort_unstable();
            let avg_response_time_ms = if !response_times.is_empty() {
                response_times.iter().sum::<u64>() as f64 / response_times.len() as f64
            } else {
                0.0
            };

            let p95_response_time_ms = if !response_times.is_empty() {
                let index = ((response_times.len() as f64) * 0.95) as usize;
                response_times
                    .get(index.saturating_sub(1))
                    .copied()
                    .unwrap_or(0) as f64
            } else {
                0.0
            };

            let p99_response_time_ms = if !response_times.is_empty() {
                let index = ((response_times.len() as f64) * 0.99) as usize;
                response_times
                    .get(index.saturating_sub(1))
                    .copied()
                    .unwrap_or(0) as f64
            } else {
                0.0
            };

            result.insert(
                upstream_type,
                RequestStats {
                    total_requests: total,
                    successful_requests: successful,
                    failed_requests: failed,
                    success_rate,
                    avg_response_time_ms,
                    p95_response_time_ms,
                    p99_response_time_ms,
                },
            );
        }

        Ok(result)
    }

    /// 清理旧统计数据
    pub async fn cleanup_old_data(&self) -> Result<()> {
        let mut stats = self.memory_stats.write().await;

        // 清理超过24小时的响应时间记录
        let cutoff_time = Utc::now() - chrono::Duration::hours(24);
        stats
            .response_times
            .retain(|record| record.timestamp > cutoff_time);

        debug!("Cleaned up old statistics data");
        Ok(())
    }

    /// 持久化到缓存
    async fn persist_to_cache(
        &self,
        endpoint: &str,
        upstream_type: &str,
        duration_ms: u64,
        success: bool,
    ) -> Result<()> {
        let now = Utc::now();
        let date_key = now.format("%Y-%m-%d").to_string();
        let hour = now.hour() as u8;

        // 构建缓存键
        let stats_key = CacheKeyBuilder::request_stats(&date_key, hour);

        // 获取现有统计数据或创建新的
        let mut cached_stats: CachedRequestStats =
            match self.cache_manager.get(&stats_key.build()).await {
                Ok(Some(stats)) => stats,
                Ok(None) => CachedRequestStats {
                    date: date_key.clone(),
                    hour,
                    total_requests: 0,
                    successful_requests: 0,
                    failed_requests: 0,
                    total_response_time_ms: 0,
                    avg_response_time_ms: 0.0,
                    endpoints: HashMap::new(),
                    upstream_types: HashMap::new(),
                    last_updated: now,
                },
                Err(e) => {
                    warn!("Failed to get cached stats, creating new: {}", e);
                    CachedRequestStats {
                        date: date_key.clone(),
                        hour,
                        total_requests: 0,
                        successful_requests: 0,
                        failed_requests: 0,
                        total_response_time_ms: 0,
                        avg_response_time_ms: 0.0,
                        endpoints: HashMap::new(),
                        upstream_types: HashMap::new(),
                        last_updated: now,
                    }
                }
            };

        // 更新统计数据
        cached_stats.total_requests += 1;
        cached_stats.total_response_time_ms += duration_ms;
        cached_stats.avg_response_time_ms =
            cached_stats.total_response_time_ms as f64 / cached_stats.total_requests as f64;
        cached_stats.last_updated = now;

        if success {
            cached_stats.successful_requests += 1;
        } else {
            cached_stats.failed_requests += 1;
        }

        // 更新端点统计
        let endpoint_stats = cached_stats
            .endpoints
            .entry(endpoint.to_string())
            .or_insert_with(|| EndpointStats {
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                total_response_time_ms: 0,
                avg_response_time_ms: 0.0,
            });

        endpoint_stats.total_requests += 1;
        endpoint_stats.total_response_time_ms += duration_ms;
        endpoint_stats.avg_response_time_ms =
            endpoint_stats.total_response_time_ms as f64 / endpoint_stats.total_requests as f64;

        if success {
            endpoint_stats.successful_requests += 1;
        } else {
            endpoint_stats.failed_requests += 1;
        }

        // 更新上游类型统计
        let upstream_stats = cached_stats
            .upstream_types
            .entry(upstream_type.to_string())
            .or_insert_with(|| UpstreamStats {
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                total_response_time_ms: 0,
                avg_response_time_ms: 0.0,
            });

        upstream_stats.total_requests += 1;
        upstream_stats.total_response_time_ms += duration_ms;
        upstream_stats.avg_response_time_ms =
            upstream_stats.total_response_time_ms as f64 / upstream_stats.total_requests as f64;

        if success {
            upstream_stats.successful_requests += 1;
        } else {
            upstream_stats.failed_requests += 1;
        }

        // 缓存更新后的统计数据
        if let Err(e) = self
            .cache_manager
            .set_with_strategy(&stats_key, &cached_stats)
            .await
        {
            error!("Failed to cache request statistics: {}", e);
            return Err(e.into());
        }

        debug!(
            "Successfully persisted request statistics to cache: endpoint={}, upstream={}, success={}",
            endpoint, upstream_type, success
        );

        Ok(())
    }

    /// 从缓存获取统计数据
    pub async fn get_cached_stats(
        &self,
        date: &str,
        hour: Option<u8>,
    ) -> Result<Option<CachedRequestStats>> {
        if let Some(h) = hour {
            let stats_key = CacheKeyBuilder::request_stats(date, h);
            self.cache_manager
                .get(&stats_key.build())
                .await
                .map_err(|e| e.into())
        } else {
            // 如果没有指定小时，返回当天的汇总数据
            let mut daily_stats = None;

            // 聚合24小时的数据
            for h in 0..24u8 {
                let stats_key = CacheKeyBuilder::request_stats(date, h);
                if let Ok(Some(hourly_stats)) = self
                    .cache_manager
                    .get::<CachedRequestStats>(&stats_key.build())
                    .await
                {
                    match &mut daily_stats {
                        Some(total) => {
                            self.merge_stats(total, &hourly_stats);
                        }
                        None => {
                            daily_stats = Some(hourly_stats);
                        }
                    }
                }
            }

            Ok(daily_stats)
        }
    }

    /// 合并统计数据
    fn merge_stats(&self, target: &mut CachedRequestStats, source: &CachedRequestStats) {
        target.total_requests += source.total_requests;
        target.successful_requests += source.successful_requests;
        target.failed_requests += source.failed_requests;
        target.total_response_time_ms += source.total_response_time_ms;

        if target.total_requests > 0 {
            target.avg_response_time_ms =
                target.total_response_time_ms as f64 / target.total_requests as f64;
        }

        // 合并端点统计
        for (endpoint, stats) in &source.endpoints {
            let target_stats =
                target
                    .endpoints
                    .entry(endpoint.clone())
                    .or_insert_with(|| EndpointStats {
                        total_requests: 0,
                        successful_requests: 0,
                        failed_requests: 0,
                        total_response_time_ms: 0,
                        avg_response_time_ms: 0.0,
                    });

            target_stats.total_requests += stats.total_requests;
            target_stats.successful_requests += stats.successful_requests;
            target_stats.failed_requests += stats.failed_requests;
            target_stats.total_response_time_ms += stats.total_response_time_ms;

            if target_stats.total_requests > 0 {
                target_stats.avg_response_time_ms =
                    target_stats.total_response_time_ms as f64 / target_stats.total_requests as f64;
            }
        }

        // 合并上游类型统计
        for (upstream, stats) in &source.upstream_types {
            let target_stats = target
                .upstream_types
                .entry(upstream.clone())
                .or_insert_with(|| UpstreamStats {
                    total_requests: 0,
                    successful_requests: 0,
                    failed_requests: 0,
                    total_response_time_ms: 0,
                    avg_response_time_ms: 0.0,
                });

            target_stats.total_requests += stats.total_requests;
            target_stats.successful_requests += stats.successful_requests;
            target_stats.failed_requests += stats.failed_requests;
            target_stats.total_response_time_ms += stats.total_response_time_ms;

            if target_stats.total_requests > 0 {
                target_stats.avg_response_time_ms =
                    target_stats.total_response_time_ms as f64 / target_stats.total_requests as f64;
            }
        }

        // 更新最后更新时间
        if source.last_updated > target.last_updated {
            target.last_updated = source.last_updated;
        }
    }
}
