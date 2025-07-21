//! # 统计服务
//!
//! 收集和聚合系统统计信息

use crate::cache::integration::CacheManager;
use crate::config::AppConfig;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// 统计服务
pub struct StatisticsService {
    /// 应用配置
    config: Arc<AppConfig>,
    /// 缓存管理器
    cache_manager: Arc<CacheManager>,
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
    endpoint: String,
    /// 上游类型
    upstream_type: String,
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
    pub fn new(config: Arc<AppConfig>, cache_manager: Arc<CacheManager>) -> Self {
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
        stats.response_times.retain(|record| record.timestamp > cutoff_time);
        
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
        self.persist_to_cache(endpoint, upstream_type, duration_ms, success).await?;
        
        Ok(())
    }

    /// 获取请求统计信息
    pub async fn get_request_stats(&self, _time_range: Option<TimeRangeQuery>) -> Result<RequestStats> {
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
        let mut response_times: Vec<u64> = stats.response_times
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
            response_times.get(index.saturating_sub(1)).copied().unwrap_or(0) as f64
        } else {
            0.0
        };
        
        let p99_response_time_ms = if !response_times.is_empty() {
            let index = ((response_times.len() as f64) * 0.99) as usize;
            response_times.get(index.saturating_sub(1)).copied().unwrap_or(0) as f64
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
                let entry = upstream_stats.entry(upstream_type.to_string()).or_insert((0, 0, Vec::new()));
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
                response_times.get(index.saturating_sub(1)).copied().unwrap_or(0) as f64
            } else {
                0.0
            };
            
            let p99_response_time_ms = if !response_times.is_empty() {
                let index = ((response_times.len() as f64) * 0.99) as usize;
                response_times.get(index.saturating_sub(1)).copied().unwrap_or(0) as f64
            } else {
                0.0
            };
            
            result.insert(upstream_type, RequestStats {
                total_requests: total,
                successful_requests: successful,
                failed_requests: failed,
                success_rate,
                avg_response_time_ms,
                p95_response_time_ms,
                p99_response_time_ms,
            });
        }
        
        Ok(result)
    }

    /// 清理旧统计数据
    pub async fn cleanup_old_data(&self) -> Result<()> {
        let mut stats = self.memory_stats.write().await;
        
        // 清理超过24小时的响应时间记录
        let cutoff_time = Utc::now() - chrono::Duration::hours(24);
        stats.response_times.retain(|record| record.timestamp > cutoff_time);
        
        debug!("Cleaned up old statistics data");
        Ok(())
    }

    /// 持久化到缓存
    async fn persist_to_cache(
        &self,
        _endpoint: &str,
        _upstream_type: &str,
        _duration_ms: u64,
        _success: bool,
    ) -> Result<()> {
        // TODO: 实现缓存持久化逻辑
        // 这里可以将统计数据写入Redis或其他持久化存储
        Ok(())
    }
}

