//! # 代理统计收集模块
//!
//! 收集和分析代理请求的详细统计信息

use super::types::{ForwardingContext, ForwardingResult};
use crate::error::{ProxyError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// 统计收集器
pub struct StatisticsCollector {
    /// 实时统计
    real_time_stats: Arc<RwLock<RealTimeStats>>,
    /// 历史统计
    historical_stats: Arc<RwLock<HistoricalStats>>,
    /// 配置
    config: StatisticsConfig,
}

/// 统计配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsConfig {
    /// 是否启用统计收集
    pub enabled: bool,
    /// 实时统计窗口大小（秒）
    pub real_time_window: u64,
    /// 历史统计保留天数
    pub historical_retention_days: u32,
    /// 是否收集详细指标
    pub collect_detailed_metrics: bool,
    /// 是否收集用户统计
    pub collect_user_stats: bool,
    /// 统计更新间隔
    pub update_interval: Duration,
}

/// 实时统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTimeStats {
    /// 当前时间窗口开始时间
    pub window_start: SystemTime,
    /// 窗口大小
    pub window_size: Duration,
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 当前QPS
    pub current_qps: f64,
    /// 平均响应时间
    pub avg_response_time: Duration,
    /// P95响应时间
    pub p95_response_time: Duration,
    /// P99响应时间
    pub p99_response_time: Duration,
    /// 按上游类型分组的统计
    pub by_upstream: HashMap<String, UpstreamRealTimeStats>,
    /// 按状态码分组的统计
    pub by_status_code: HashMap<u16, u64>,
    /// 错误统计
    pub error_stats: ErrorStats,
    /// 最后更新时间
    pub last_updated: SystemTime,
}

/// 上游实时统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamRealTimeStats {
    /// 请求数
    pub request_count: u64,
    /// 成功数
    pub success_count: u64,
    /// 失败数
    pub failure_count: u64,
    /// 平均响应时间
    pub avg_response_time: Duration,
    /// QPS
    pub qps: f64,
    /// 成功率
    pub success_rate: f64,
}

/// 错误统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorStats {
    /// 超时错误数
    pub timeout_errors: u64,
    /// 连接错误数
    pub connection_errors: u64,
    /// 认证错误数
    pub auth_errors: u64,
    /// 限流错误数
    pub rate_limit_errors: u64,
    /// 其他错误数
    pub other_errors: u64,
    /// 按错误类型分组
    pub by_error_type: HashMap<String, u64>,
}

/// 历史统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalStats {
    /// 按天分组的统计
    pub daily_stats: HashMap<String, DailyStats>,
    /// 按小时分组的统计
    pub hourly_stats: HashMap<String, HourlyStats>,
    /// 趋势数据
    pub trends: TrendData,
}

/// 每日统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStats {
    /// 日期 (YYYY-MM-DD)
    pub date: String,
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 平均QPS
    pub avg_qps: f64,
    /// 峰值QPS
    pub peak_qps: f64,
    /// 平均响应时间
    pub avg_response_time: Duration,
    /// 总传输字节数
    pub total_bytes: u64,
    /// 按上游类型分组
    pub by_upstream: HashMap<String, DailyUpstreamStats>,
    /// 唯一用户数
    pub unique_users: u64,
}

/// 每日上游统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyUpstreamStats {
    /// 请求数
    pub request_count: u64,
    /// 成功率
    pub success_rate: f64,
    /// 平均响应时间
    pub avg_response_time: Duration,
    /// 传输字节数
    pub bytes_transferred: u64,
}

/// 每小时统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourlyStats {
    /// 时间戳 (YYYY-MM-DD HH:00)
    pub timestamp: String,
    /// 请求数
    pub request_count: u64,
    /// 平均QPS
    pub avg_qps: f64,
    /// 成功率
    pub success_rate: f64,
    /// 平均响应时间
    pub avg_response_time: Duration,
}

/// 趋势数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendData {
    /// QPS趋势（过去24小时）
    pub qps_trend_24h: Vec<TrendPoint>,
    /// 响应时间趋势（过去24小时）
    pub response_time_trend_24h: Vec<TrendPoint>,
    /// 成功率趋势（过去24小时）
    pub success_rate_trend_24h: Vec<TrendPoint>,
    /// 错误率趋势（过去24小时）
    pub error_rate_trend_24h: Vec<TrendPoint>,
}

/// 趋势点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendPoint {
    /// 时间戳
    pub timestamp: SystemTime,
    /// 值
    pub value: f64,
}

/// 用户统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStats {
    /// 用户ID
    pub user_id: String,
    /// 请求数
    pub request_count: u64,
    /// 成功数
    pub success_count: u64,
    /// 失败数
    pub failure_count: u64,
    /// 总响应时间
    pub total_response_time: Duration,
    /// 传输字节数
    pub bytes_transferred: u64,
    /// 最后活动时间
    pub last_activity: SystemTime,
    /// 使用的上游类型
    pub upstream_types: HashMap<String, u64>,
}

impl StatisticsCollector {
    /// 创建新的统计收集器
    pub fn new(config: StatisticsConfig) -> Self {
        Self {
            real_time_stats: Arc::new(RwLock::new(RealTimeStats::new(config.real_time_window))),
            historical_stats: Arc::new(RwLock::new(HistoricalStats::new())),
            config,
        }
    }

    /// 记录请求完成
    pub async fn record_request_completion(
        &self,
        context: &ForwardingContext,
        result: &ForwardingResult,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // 更新实时统计
        self.update_real_time_stats(context, result).await?;

        // 更新历史统计
        if self.config.collect_detailed_metrics {
            self.update_historical_stats(context, result).await?;
        }

        Ok(())
    }

    /// 更新实时统计
    async fn update_real_time_stats(
        &self,
        context: &ForwardingContext,
        result: &ForwardingResult,
    ) -> Result<()> {
        let mut stats = self.real_time_stats.write().await;

        // 检查是否需要重置窗口
        let now = SystemTime::now();
        if now
            .duration_since(stats.window_start)
            .unwrap_or(Duration::from_secs(0))
            > Duration::from_secs(self.config.real_time_window)
        {
            stats.reset_window(now);
        }

        // 更新基础统计
        stats.total_requests += 1;
        if result.success {
            stats.successful_requests += 1;
        } else {
            stats.failed_requests += 1;
        }

        // 更新QPS
        let window_duration = now
            .duration_since(stats.window_start)
            .unwrap_or(Duration::from_secs(1))
            .as_secs_f64();
        stats.current_qps = stats.total_requests as f64 / window_duration.max(1.0);

        // 更新响应时间
        stats.update_response_time(result.response_time);

        // 更新上游统计
        let upstream_key = format!("{:?}", context.provider_id);
        let upstream_stats = stats
            .by_upstream
            .entry(upstream_key)
            .or_insert_with(UpstreamRealTimeStats::new);
        upstream_stats.update(result, window_duration);

        // 更新状态码统计
        *stats.by_status_code.entry(result.status_code).or_insert(0) += 1;

        // 更新错误统计
        if !result.success {
            stats.error_stats.update(result);
        }

        stats.last_updated = now;
        Ok(())
    }

    /// 更新历史统计
    async fn update_historical_stats(
        &self,
        context: &ForwardingContext,
        result: &ForwardingResult,
    ) -> Result<()> {
        let mut stats = self.historical_stats.write().await;
        let now = SystemTime::now();

        // 更新每日统计
        let date_key = format_date(now);
        let daily_stats = stats
            .daily_stats
            .entry(date_key)
            .or_insert_with(|| DailyStats::new(&format_date(now)));
        daily_stats.update(context, result);

        // 更新每小时统计
        let hour_key = format_hour(now);
        let hourly_stats = stats
            .hourly_stats
            .entry(hour_key)
            .or_insert_with(|| HourlyStats::new(&format_hour(now)));
        hourly_stats.update(result);

        // 更新趋势数据
        stats.trends.update(result, now);

        Ok(())
    }

    /// 获取实时统计
    pub async fn get_real_time_stats(&self) -> RealTimeStats {
        self.real_time_stats.read().await.clone()
    }

    /// 获取历史统计
    pub async fn get_historical_stats(&self) -> HistoricalStats {
        self.historical_stats.read().await.clone()
    }

    /// 获取统计摘要
    pub async fn get_stats_summary(&self) -> StatsSummary {
        let real_time = self.real_time_stats.read().await;
        let historical = self.historical_stats.read().await;

        StatsSummary {
            current_qps: real_time.current_qps,
            avg_response_time: real_time.avg_response_time,
            success_rate: if real_time.total_requests > 0 {
                real_time.successful_requests as f64 / real_time.total_requests as f64 * 100.0
            } else {
                0.0
            },
            total_requests_today: historical
                .daily_stats
                .get(&format_date(SystemTime::now()))
                .map(|d| d.total_requests)
                .unwrap_or(0),
            active_upstreams: real_time.by_upstream.len(),
            error_rate: if real_time.total_requests > 0 {
                real_time.failed_requests as f64 / real_time.total_requests as f64 * 100.0
            } else {
                0.0
            },
        }
    }

    /// 清理过期数据
    pub async fn cleanup_expired_data(&self) -> Result<usize> {
        let mut historical = self.historical_stats.write().await;
        let cutoff_date = SystemTime::now()
            - Duration::from_secs(86400 * self.config.historical_retention_days as u64);

        let mut removed_count = 0;

        // 清理过期的每日统计
        historical.daily_stats.retain(|date_str, _| {
            if let Ok(date) = parse_date(date_str) {
                date > cutoff_date
            } else {
                removed_count += 1;
                false
            }
        });

        // 清理过期的每小时统计
        historical.hourly_stats.retain(|hour_str, _| {
            if let Ok(hour) = parse_hour(hour_str) {
                hour > cutoff_date
            } else {
                removed_count += 1;
                false
            }
        });

        Ok(removed_count)
    }

    /// 重置所有统计
    pub async fn reset_all_stats(&self) -> Result<()> {
        let mut real_time = self.real_time_stats.write().await;
        let mut historical = self.historical_stats.write().await;

        *real_time = RealTimeStats::new(self.config.real_time_window);
        *historical = HistoricalStats::new();

        Ok(())
    }
}

/// 统计摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSummary {
    /// 当前QPS
    pub current_qps: f64,
    /// 平均响应时间
    pub avg_response_time: Duration,
    /// 成功率
    pub success_rate: f64,
    /// 今日总请求数
    pub total_requests_today: u64,
    /// 活跃上游数量
    pub active_upstreams: usize,
    /// 错误率
    pub error_rate: f64,
}

impl Default for StatisticsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            real_time_window: 300, // 5分钟窗口
            historical_retention_days: 30,
            collect_detailed_metrics: true,
            collect_user_stats: true,
            update_interval: Duration::from_secs(10),
        }
    }
}

impl RealTimeStats {
    /// 创建新的实时统计
    pub fn new(window_size_seconds: u64) -> Self {
        let now = SystemTime::now();
        Self {
            window_start: now,
            window_size: Duration::from_secs(window_size_seconds),
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            current_qps: 0.0,
            avg_response_time: Duration::from_millis(0),
            p95_response_time: Duration::from_millis(0),
            p99_response_time: Duration::from_millis(0),
            by_upstream: HashMap::new(),
            by_status_code: HashMap::new(),
            error_stats: ErrorStats::new(),
            last_updated: now,
        }
    }

    /// 重置时间窗口
    pub fn reset_window(&mut self, new_start: SystemTime) {
        self.window_start = new_start;
        self.total_requests = 0;
        self.successful_requests = 0;
        self.failed_requests = 0;
        self.current_qps = 0.0;
        self.by_upstream.clear();
        self.by_status_code.clear();
        self.error_stats = ErrorStats::new();
    }

    /// 更新响应时间统计
    pub fn update_response_time(&mut self, response_time: Duration) {
        // 简化实现，实际应该维护响应时间的分布
        let total_time = self.avg_response_time * (self.total_requests - 1) as u32 + response_time;
        self.avg_response_time = total_time / self.total_requests as u32;

        // 简化的百分位数计算
        self.p95_response_time = response_time; // 应该基于实际分布计算
        self.p99_response_time = response_time;
    }
}

impl UpstreamRealTimeStats {
    /// 创建新的上游实时统计
    pub fn new() -> Self {
        Self {
            request_count: 0,
            success_count: 0,
            failure_count: 0,
            avg_response_time: Duration::from_millis(0),
            qps: 0.0,
            success_rate: 0.0,
        }
    }

    /// 更新统计
    pub fn update(&mut self, result: &ForwardingResult, window_duration: f64) {
        self.request_count += 1;

        if result.success {
            self.success_count += 1;
        } else {
            self.failure_count += 1;
        }

        // 更新平均响应时间
        let total_time =
            self.avg_response_time * (self.request_count - 1) as u32 + result.response_time;
        self.avg_response_time = total_time / self.request_count as u32;

        // 更新QPS
        self.qps = self.request_count as f64 / window_duration.max(1.0);

        // 更新成功率
        self.success_rate = self.success_count as f64 / self.request_count as f64 * 100.0;
    }
}

impl ErrorStats {
    /// 创建新的错误统计
    pub fn new() -> Self {
        Self {
            timeout_errors: 0,
            connection_errors: 0,
            auth_errors: 0,
            rate_limit_errors: 0,
            other_errors: 0,
            by_error_type: HashMap::new(),
        }
    }

    /// 更新错误统计
    pub fn update(&mut self, result: &ForwardingResult) {
        if let Some(ref error_msg) = result.error_message {
            if error_msg.contains("timeout") {
                self.timeout_errors += 1;
            } else if error_msg.contains("connection") {
                self.connection_errors += 1;
            } else if error_msg.contains("auth") {
                self.auth_errors += 1;
            } else if error_msg.contains("rate limit") {
                self.rate_limit_errors += 1;
            } else {
                self.other_errors += 1;
            }

            // 按错误类型分组
            *self.by_error_type.entry(error_msg.clone()).or_insert(0) += 1;
        }
    }
}

impl HistoricalStats {
    /// 创建新的历史统计
    pub fn new() -> Self {
        Self {
            daily_stats: HashMap::new(),
            hourly_stats: HashMap::new(),
            trends: TrendData::new(),
        }
    }
}

impl DailyStats {
    /// 创建新的每日统计
    pub fn new(date: &str) -> Self {
        Self {
            date: date.to_string(),
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            avg_qps: 0.0,
            peak_qps: 0.0,
            avg_response_time: Duration::from_millis(0),
            total_bytes: 0,
            by_upstream: HashMap::new(),
            unique_users: 0,
        }
    }

    /// 更新每日统计
    pub fn update(&mut self, context: &ForwardingContext, result: &ForwardingResult) {
        self.total_requests += 1;

        if result.success {
            self.successful_requests += 1;
        } else {
            self.failed_requests += 1;
        }

        self.total_bytes += result.bytes_transferred;

        // 更新平均响应时间
        let total_time =
            self.avg_response_time * (self.total_requests - 1) as u32 + result.response_time;
        self.avg_response_time = total_time / self.total_requests as u32;

        // 更新上游统计
        let upstream_key = format!("{:?}", context.provider_id);
        let upstream_stats = self
            .by_upstream
            .entry(upstream_key)
            .or_insert_with(DailyUpstreamStats::new);
        upstream_stats.update(result);
    }
}

impl DailyUpstreamStats {
    /// 创建新的每日上游统计
    pub fn new() -> Self {
        Self {
            request_count: 0,
            success_rate: 0.0,
            avg_response_time: Duration::from_millis(0),
            bytes_transferred: 0,
        }
    }

    /// 更新统计
    pub fn update(&mut self, result: &ForwardingResult) {
        self.request_count += 1;
        self.bytes_transferred += result.bytes_transferred;

        // 更新成功率
        let success_count = if result.success {
            self.request_count as f64 * self.success_rate / 100.0 + 1.0
        } else {
            self.request_count as f64 * self.success_rate / 100.0
        };
        self.success_rate = success_count / self.request_count as f64 * 100.0;

        // 更新平均响应时间
        let total_time =
            self.avg_response_time * (self.request_count - 1) as u32 + result.response_time;
        self.avg_response_time = total_time / self.request_count as u32;
    }
}

impl HourlyStats {
    /// 创建新的每小时统计
    pub fn new(timestamp: &str) -> Self {
        Self {
            timestamp: timestamp.to_string(),
            request_count: 0,
            avg_qps: 0.0,
            success_rate: 0.0,
            avg_response_time: Duration::from_millis(0),
        }
    }

    /// 更新每小时统计
    pub fn update(&mut self, result: &ForwardingResult) {
        self.request_count += 1;

        // 更新成功率
        let success_count = if result.success {
            self.request_count as f64 * self.success_rate / 100.0 + 1.0
        } else {
            self.request_count as f64 * self.success_rate / 100.0
        };
        self.success_rate = success_count / self.request_count as f64 * 100.0;

        // 更新平均响应时间
        let total_time =
            self.avg_response_time * (self.request_count - 1) as u32 + result.response_time;
        self.avg_response_time = total_time / self.request_count as u32;

        // 更新QPS（基于小时）
        self.avg_qps = self.request_count as f64 / 3600.0;
    }
}

impl TrendData {
    /// 创建新的趋势数据
    pub fn new() -> Self {
        Self {
            qps_trend_24h: Vec::new(),
            response_time_trend_24h: Vec::new(),
            success_rate_trend_24h: Vec::new(),
            error_rate_trend_24h: Vec::new(),
        }
    }

    /// 更新趋势数据
    pub fn update(&mut self, result: &ForwardingResult, timestamp: SystemTime) {
        // 简化实现，实际应该按时间窗口聚合数据
        self.response_time_trend_24h.push(TrendPoint {
            timestamp,
            value: result.response_time.as_millis() as f64,
        });

        // 保持最近24小时的数据
        let cutoff = timestamp - Duration::from_secs(86400);
        self.response_time_trend_24h
            .retain(|point| point.timestamp > cutoff);
        self.qps_trend_24h.retain(|point| point.timestamp > cutoff);
        self.success_rate_trend_24h
            .retain(|point| point.timestamp > cutoff);
        self.error_rate_trend_24h
            .retain(|point| point.timestamp > cutoff);
    }
}

// 辅助函数
fn format_date(time: SystemTime) -> String {
    // 简化实现，实际应该使用正确的日期格式化
    format!(
        "{}",
        time.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            / 86400
    )
}

fn format_hour(time: SystemTime) -> String {
    // 简化实现，实际应该使用正确的小时格式化
    format!(
        "{}",
        time.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            / 3600
    )
}

fn parse_date(date_str: &str) -> Result<SystemTime> {
    // 简化实现，实际应该解析日期字符串
    let days: u64 = date_str
        .parse()
        .map_err(|_| ProxyError::internal("Failed to parse date string"))?;
    Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(days * 86400))
}

fn parse_hour(hour_str: &str) -> Result<SystemTime> {
    // 简化实现，实际应该解析小时字符串
    let hours: u64 = hour_str
        .parse()
        .map_err(|_| ProxyError::internal("Failed to parse hour string"))?;
    Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(hours * 3600))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_statistics_collector_creation() {
        let config = StatisticsConfig::default();
        let collector = StatisticsCollector::new(config);

        let stats = collector.get_real_time_stats().await;
        assert_eq!(stats.total_requests, 0);
    }

    #[tokio::test]
    async fn test_record_request_completion() {
        let config = StatisticsConfig::default();
        let collector = StatisticsCollector::new(config);

        let context =
            ForwardingContext::new("req_123".to_string(), ProviderId::from_database_id(1));
        let result = ForwardingResult {
            success: true,
            response_time: Duration::from_millis(100),
            status_code: Some(200),
            error_message: None,
            retry_count: 0,
            bytes_transferred: 1024,
            upstream_server: Some("test-upstream-server:443".to_string()),
        };

        assert!(
            collector
                .record_request_completion(&context, &result)
                .await
                .is_ok()
        );

        let stats = collector.get_real_time_stats().await;
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 1);
    }

    #[tokio::test]
    async fn test_stats_summary() {
        let config = StatisticsConfig::default();
        let collector = StatisticsCollector::new(config);

        let summary = collector.get_stats_summary().await;
        assert_eq!(summary.current_qps, 0.0);
        assert_eq!(summary.success_rate, 0.0);
    }
}
