//! # 请求转发处理模块
//! 
//! 处理AI代理请求的转发、负载均衡、统计收集等核心功能

use crate::error::{ProxyError, Result};
use crate::proxy::upstream::{UpstreamType, UpstreamServer, UpstreamManager};
use crate::health::HealthCheckService;
use crate::scheduler::SchedulingStrategy;
use crate::providers::{AdapterManager, AdapterRequest};
use pingora_http::{RequestHeader, ResponseHeader};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// 请求转发器
pub struct RequestForwarder {
    /// 上游管理器
    upstream_manager: Arc<UpstreamManager>,
    /// 健康检查服务
    health_service: Arc<HealthCheckService>,
    /// 适配器管理器
    adapter_manager: Arc<AdapterManager>,
    /// 转发统计
    stats: Arc<RwLock<ForwardingStats>>,
    /// 配置
    config: ForwardingConfig,
}

/// 转发配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardingConfig {
    /// 请求超时时间
    pub request_timeout: Duration,
    /// 重试次数
    pub max_retries: u32,
    /// 重试间隔
    pub retry_interval: Duration,
    /// 是否启用统计收集
    pub enable_stats: bool,
    /// 是否启用请求日志
    pub enable_request_logging: bool,
    /// 最大并发请求数
    pub max_concurrent_requests: u32,
    /// 电路熔断配置
    pub circuit_breaker: CircuitBreakerConfig,
}

/// 电路熔断配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// 失败阈值
    pub failure_threshold: u32,
    /// 恢复超时
    pub recovery_timeout: Duration,
    /// 半开状态测试请求数
    pub half_open_max_calls: u32,
    /// 是否启用
    pub enabled: bool,
}

/// 转发上下文
#[derive(Debug, Clone)]
pub struct ForwardingContext {
    /// 请求ID
    pub request_id: String,
    /// 开始时间
    pub start_time: Instant,
    /// 上游类型
    pub upstream_type: UpstreamType,
    /// 选中的服务器
    pub selected_server: Option<UpstreamServer>,
    /// 负载均衡决策
    pub lb_decision: Option<LoadBalancingDecision>,
    /// 重试次数
    pub retry_count: u32,
    /// 适配器请求
    pub adapter_request: Option<AdapterRequest>,
    /// 用户信息
    pub user_id: Option<String>,
    /// 客户端IP
    pub client_ip: Option<String>,
}

/// 负载均衡决策
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancingDecision {
    /// 选择的服务器索引
    pub server_index: usize,
    /// 决策原因
    pub reason: String,
    /// 使用的策略
    pub strategy: SchedulingStrategy,
    /// 健康分数
    pub health_score: Option<f32>,
    /// 决策时间
    #[serde(skip, default = "Instant::now")]
    pub decision_time: Instant,
}

/// 转发结果
#[derive(Debug, Clone)]
pub struct ForwardingResult {
    /// 是否成功
    pub success: bool,
    /// 响应时间
    pub response_time: Duration,
    /// 状态码
    pub status_code: Option<u16>,
    /// 错误信息
    pub error_message: Option<String>,
    /// 重试次数
    pub retry_count: u32,
    /// 传输字节数
    pub bytes_transferred: u64,
    /// 上游服务器地址
    pub upstream_server: Option<String>,
}

/// 转发统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardingStats {
    /// 总请求数
    pub total_requests: u64,
    /// 成功请求数
    pub successful_requests: u64,
    /// 失败请求数
    pub failed_requests: u64,
    /// 总响应时间
    pub total_response_time: Duration,
    /// 平均响应时间
    pub avg_response_time: Duration,
    /// 按上游类型分组的统计
    pub by_upstream_type: HashMap<String, UpstreamTypeStats>,
    /// 按状态码分组的统计
    pub by_status_code: HashMap<u16, u64>,
    /// 重试统计
    pub retry_stats: RetryStats,
    /// 最后更新时间
    #[serde(skip, default = "Instant::now")]
    pub last_updated: Instant,
}

/// 上游类型统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamTypeStats {
    /// 请求数
    pub request_count: u64,
    /// 成功数
    pub success_count: u64,
    /// 失败数
    pub failure_count: u64,
    /// 平均响应时间
    pub avg_response_time: Duration,
    /// 总传输字节数
    pub total_bytes: u64,
}

/// 重试统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryStats {
    /// 重试请求总数
    pub total_retries: u64,
    /// 重试成功数
    pub successful_retries: u64,
    /// 重试失败数
    pub failed_retries: u64,
    /// 按重试次数分组
    pub by_retry_count: HashMap<u32, u64>,
}

impl RequestForwarder {
    /// 创建新的请求转发器
    pub fn new(
        upstream_manager: Arc<UpstreamManager>,
        health_service: Arc<HealthCheckService>,
        adapter_manager: Arc<AdapterManager>,
        config: ForwardingConfig,
    ) -> Self {
        Self {
            upstream_manager,
            health_service,
            adapter_manager,
            stats: Arc::new(RwLock::new(ForwardingStats::new())),
            config,
        }
    }

    /// 转发请求
    pub async fn forward_request(
        &self,
        request_header: &mut RequestHeader,
        context: &mut ForwardingContext,
    ) -> Result<ForwardingResult> {
        let start_time = Instant::now();
        context.start_time = start_time;

        // 检查并发限制
        self.check_concurrency_limit().await?;

        // 执行负载均衡选择上游服务器
        let lb_decision = self.select_upstream_server(&context.upstream_type).await?;
        context.lb_decision = Some(lb_decision.clone());

        // 获取选中的服务器
        let server = self.get_server_by_decision(&lb_decision).await?;
        context.selected_server = Some(server.clone());

        // 处理适配器请求转换
        if let Some(ref adapter_req) = context.adapter_request {
            self.apply_adapter_modifications(request_header, adapter_req)?;
        }

        // 添加代理头
        self.add_proxy_headers(request_header, context)?;

        // 执行转发（包含重试逻辑）
        let result = self.execute_forward_with_retry(context).await;

        // 更新统计
        if self.config.enable_stats {
            if let Ok(ref result_ok) = result {
                self.update_stats(result_ok, context).await;
            }
        }

        // 记录请求日志
        if self.config.enable_request_logging {
            if let Ok(ref result_ok) = result {
                self.log_request(context, result_ok).await;
            }
        }

        result
    }

    /// 选择上游服务器
    async fn select_upstream_server(&self, upstream_type: &UpstreamType) -> Result<LoadBalancingDecision> {
        // 获取健康的服务器列表
        let healthy_servers = self.health_service.get_healthy_servers(upstream_type).await;
        
        if healthy_servers.is_empty() {
            return Err(ProxyError::upstream_not_available(
                format!("No healthy servers available for upstream type: {:?}", upstream_type)
            ));
        }

        // 使用负载均衡器选择服务器
        let server = self.upstream_manager.select_upstream(upstream_type)?;
        let server_address = server.address();

        // 查找服务器索引
        let server_index = healthy_servers
            .iter()
            .position(|addr| addr == &server_address)
            .unwrap_or(0);

        // 获取健康状态
        let health_status = self.health_service.get_server_health(&server_address).await;
        let health_score = health_status.map(|h| h.health_score);

        Ok(LoadBalancingDecision {
            server_index,
            reason: format!("Load balancer selected {}", server_address),
            strategy: SchedulingStrategy::HealthBased, // 这里应该从配置获取
            health_score,
            decision_time: Instant::now(),
        })
    }

    /// 根据决策获取服务器
    async fn get_server_by_decision(&self, _decision: &LoadBalancingDecision) -> Result<UpstreamServer> {
        // 这里简化实现，实际应该从上游管理器获取服务器列表
        let dummy_server = UpstreamServer {
            host: "api.openai.com".to_string(),
            port: 443,
            use_tls: true,
            weight: 100,
            max_connections: Some(1000),
            timeout_ms: 30000,
            health_check_interval: 30000,
            is_healthy: true,
        };
        Ok(dummy_server)
    }

    /// 应用适配器修改
    fn apply_adapter_modifications(
        &self,
        request_header: &mut RequestHeader,
        adapter_request: &AdapterRequest,
    ) -> Result<()> {
        // 更新请求头
        for (name, value) in &adapter_request.headers {
            request_header
                .insert_header(name.clone(), value.clone())
                .map_err(|e| ProxyError::internal(format!("Failed to set adapter header {}: {}", name, e)))?;
        }

        // 更新路径（如果需要）
        if adapter_request.path != request_header.uri.path() {
            tracing::debug!(
                "Path rewrite: {} -> {}",
                request_header.uri.path(),
                adapter_request.path
            );
            // 这里需要实际的URI重写逻辑
        }

        Ok(())
    }

    /// 添加代理头
    fn add_proxy_headers(
        &self,
        request_header: &mut RequestHeader,
        context: &ForwardingContext,
    ) -> Result<()> {
        // 添加请求ID
        request_header
            .insert_header("X-Request-ID", &context.request_id)
            .map_err(|e| ProxyError::internal(format!("Failed to set Request-ID header: {}", e)))?;

        // 添加上游类型
        request_header
            .insert_header("X-Upstream-Type", &format!("{:?}", context.upstream_type))
            .map_err(|e| ProxyError::internal(format!("Failed to set Upstream-Type header: {}", e)))?;

        // 添加用户ID（如果有）
        if let Some(ref user_id) = context.user_id {
            request_header
                .insert_header("X-User-ID", user_id)
                .map_err(|e| ProxyError::internal(format!("Failed to set User-ID header: {}", e)))?;
        }

        // 添加客户端IP（如果有）
        if let Some(ref client_ip) = context.client_ip {
            request_header
                .insert_header("X-Forwarded-For", client_ip)
                .map_err(|e| ProxyError::internal(format!("Failed to set Forwarded-For header: {}", e)))?;
        }

        Ok(())
    }

    /// 执行转发（包含重试逻辑）
    async fn execute_forward_with_retry(&self, context: &mut ForwardingContext) -> Result<ForwardingResult> {
        let mut last_error = None;
        let mut retry_count = 0;

        while retry_count <= self.config.max_retries {
            match self.execute_single_forward(context).await {
                Ok(result) => {
                    return Ok(ForwardingResult {
                        retry_count,
                        ..result
                    });
                }
                Err(e) => {
                    last_error = Some(e);
                    retry_count += 1;
                    context.retry_count = retry_count;

                    if retry_count <= self.config.max_retries {
                        tracing::warn!(
                            "Request {} failed, retrying {}/{}: {}",
                            context.request_id,
                            retry_count,
                            self.config.max_retries,
                            last_error.as_ref().unwrap()
                        );

                        // 等待重试间隔
                        tokio::time::sleep(self.config.retry_interval).await;

                        // 重新选择服务器（为了故障转移）
                        if let Ok(new_decision) = self.select_upstream_server(&context.upstream_type).await {
                            context.lb_decision = Some(new_decision);
                        }
                    }
                }
            }
        }

        // 所有重试都失败了
        Err(last_error.unwrap_or_else(|| ProxyError::internal("All retries failed")))
    }

    /// 执行单次转发
    async fn execute_single_forward(&self, context: &ForwardingContext) -> Result<ForwardingResult> {
        let start_time = Instant::now();
        
        // 模拟转发逻辑
        // 在实际实现中，这里会使用 Pingora 的代理功能
        tokio::time::sleep(Duration::from_millis(100)).await;

        let response_time = start_time.elapsed();
        
        // 模拟成功响应
        Ok(ForwardingResult {
            success: true,
            response_time,
            status_code: Some(200),
            error_message: None,
            retry_count: context.retry_count,
            bytes_transferred: 1024, // 模拟数据
            upstream_server: context.selected_server.as_ref().map(|s| s.address()),
        })
    }

    /// 检查并发限制
    async fn check_concurrency_limit(&self) -> Result<()> {
        // 这里应该实现实际的并发检查逻辑
        // 简化实现，直接返回成功
        Ok(())
    }

    /// 更新统计
    async fn update_stats(&self, result: &ForwardingResult, context: &ForwardingContext) {
        let mut stats = self.stats.write().await;
        stats.update(result, context);
    }

    /// 记录请求日志
    async fn log_request(&self, context: &ForwardingContext, result: &ForwardingResult) {
        if result.success {
            tracing::info!(
                "Request {} forwarded successfully to {:?} in {:?} (retries: {})",
                context.request_id,
                context.upstream_type,
                result.response_time,
                result.retry_count
            );
        } else {
            tracing::error!(
                "Request {} failed after {} retries: {}",
                context.request_id,
                result.retry_count,
                result.error_message.as_deref().unwrap_or("Unknown error")
            );
        }
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> ForwardingStats {
        self.stats.read().await.clone()
    }

    /// 重置统计信息
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = ForwardingStats::new();
    }

    /// 处理响应
    pub async fn process_response(
        &self,
        response_header: &mut ResponseHeader,
        context: &ForwardingContext,
        result: &ForwardingResult,
    ) -> Result<()> {
        // 添加代理相关响应头
        response_header
            .insert_header("X-Proxy-By", "AI-Proxy-Pingora")
            .map_err(|e| ProxyError::internal(format!("Failed to set Proxy-By header: {}", e)))?;

        response_header
            .insert_header("X-Request-ID", &context.request_id)
            .map_err(|e| ProxyError::internal(format!("Failed to set Request-ID header: {}", e)))?;

        response_header
            .insert_header("X-Response-Time", &format!("{}ms", result.response_time.as_millis()))
            .map_err(|e| ProxyError::internal(format!("Failed to set Response-Time header: {}", e)))?;

        if result.retry_count > 0 {
            response_header
                .insert_header("X-Retry-Count", &result.retry_count.to_string())
                .map_err(|e| ProxyError::internal(format!("Failed to set Retry-Count header: {}", e)))?;
        }

        if let Some(ref server) = result.upstream_server {
            response_header
                .insert_header("X-Upstream-Server", server)
                .map_err(|e| ProxyError::internal(format!("Failed to set Upstream-Server header: {}", e)))?;
        }

        // 移除敏感头
        response_header.remove_header("Server");
        response_header.remove_header("X-Powered-By");

        Ok(())
    }
}

impl Default for ForwardingConfig {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            max_retries: 2,
            retry_interval: Duration::from_millis(500),
            enable_stats: true,
            enable_request_logging: true,
            max_concurrent_requests: 1000,
            circuit_breaker: CircuitBreakerConfig::default(),
        }
    }
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(60),
            half_open_max_calls: 3,
            enabled: true,
        }
    }
}

impl ForwardingContext {
    /// 创建新的转发上下文
    pub fn new(request_id: String, upstream_type: UpstreamType) -> Self {
        Self {
            request_id,
            start_time: Instant::now(),
            upstream_type,
            selected_server: None,
            lb_decision: None,
            retry_count: 0,
            adapter_request: None,
            user_id: None,
            client_ip: None,
        }
    }

    /// 设置用户ID
    pub fn with_user_id(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// 设置客户端IP
    pub fn with_client_ip(mut self, client_ip: String) -> Self {
        self.client_ip = Some(client_ip);
        self
    }

    /// 设置适配器请求
    pub fn with_adapter_request(mut self, adapter_request: AdapterRequest) -> Self {
        self.adapter_request = Some(adapter_request);
        self
    }
}

impl ForwardingStats {
    /// 创建新的统计
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            total_response_time: Duration::from_millis(0),
            avg_response_time: Duration::from_millis(0),
            by_upstream_type: HashMap::new(),
            by_status_code: HashMap::new(),
            retry_stats: RetryStats::new(),
            last_updated: Instant::now(),
        }
    }

    /// 更新统计
    pub fn update(&mut self, result: &ForwardingResult, context: &ForwardingContext) {
        self.total_requests += 1;
        
        if result.success {
            self.successful_requests += 1;
        } else {
            self.failed_requests += 1;
        }

        self.total_response_time += result.response_time;
        self.avg_response_time = self.total_response_time / self.total_requests as u32;

        // 更新上游类型统计
        let upstream_key = format!("{:?}", context.upstream_type);
        let upstream_stats = self.by_upstream_type.entry(upstream_key).or_insert_with(UpstreamTypeStats::new);
        upstream_stats.update(result);

        // 更新状态码统计
        if let Some(status_code) = result.status_code {
            *self.by_status_code.entry(status_code).or_insert(0) += 1;
        }

        // 更新重试统计
        if result.retry_count > 0 {
            self.retry_stats.update(result);
        }

        self.last_updated = Instant::now();
    }

    /// 获取成功率
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.successful_requests as f64 / self.total_requests as f64 * 100.0
        }
    }
}

impl UpstreamTypeStats {
    /// 创建新的上游类型统计
    pub fn new() -> Self {
        Self {
            request_count: 0,
            success_count: 0,
            failure_count: 0,
            avg_response_time: Duration::from_millis(0),
            total_bytes: 0,
        }
    }

    /// 更新统计
    pub fn update(&mut self, result: &ForwardingResult) {
        self.request_count += 1;
        
        if result.success {
            self.success_count += 1;
        } else {
            self.failure_count += 1;
        }

        // 更新平均响应时间
        let total_time = self.avg_response_time * (self.request_count - 1) as u32 + result.response_time;
        self.avg_response_time = total_time / self.request_count as u32;

        self.total_bytes += result.bytes_transferred;
    }
}

impl RetryStats {
    /// 创建新的重试统计
    pub fn new() -> Self {
        Self {
            total_retries: 0,
            successful_retries: 0,
            failed_retries: 0,
            by_retry_count: HashMap::new(),
        }
    }

    /// 更新重试统计
    pub fn update(&mut self, result: &ForwardingResult) {
        self.total_retries += result.retry_count as u64;
        
        if result.success {
            self.successful_retries += 1;
        } else {
            self.failed_retries += 1;
        }

        *self.by_retry_count.entry(result.retry_count).or_insert(0) += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fixtures::TestConfig;

    #[test]
    fn test_forwarding_context_creation() {
        let ctx = ForwardingContext::new("req_123".to_string(), UpstreamType::OpenAI);
        assert_eq!(ctx.request_id, "req_123");
        assert_eq!(ctx.upstream_type, UpstreamType::OpenAI);
        assert_eq!(ctx.retry_count, 0);
    }

    #[test]
    fn test_forwarding_stats_update() {
        let mut stats = ForwardingStats::new();
        let ctx = ForwardingContext::new("req_123".to_string(), UpstreamType::OpenAI);
        let result = ForwardingResult {
            success: true,
            response_time: Duration::from_millis(100),
            status_code: Some(200),
            error_message: None,
            retry_count: 0,
            bytes_transferred: 1024,
            upstream_server: Some("api.openai.com:443".to_string()),
        };

        stats.update(&result, &ctx);

        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.success_rate(), 100.0);
    }

    #[test]
    fn test_forwarding_config_default() {
        let config = ForwardingConfig::default();
        assert_eq!(config.max_retries, 2);
        assert!(config.enable_stats);
        assert!(config.circuit_breaker.enabled);
    }

    #[tokio::test]
    async fn test_request_forwarder_creation() {
        let config = Arc::new(TestConfig::app_config());
        let upstream_manager = Arc::new(UpstreamManager::new(config));
        let health_service = Arc::new(HealthCheckService::new(None));
        let adapter_manager = Arc::new(AdapterManager::new());
        let forwarding_config = ForwardingConfig::default();

        let forwarder = RequestForwarder::new(
            upstream_manager,
            health_service,
            adapter_manager,
            forwarding_config,
        );

        let stats = forwarder.get_stats().await;
        assert_eq!(stats.total_requests, 0);
    }
}