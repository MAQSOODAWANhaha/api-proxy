//! # API密钥健康检查系统
//!
//! 负责检测和管理API密钥的可用性状态，通过真实API调用验证密钥健康度

use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::proxy::types::ProviderId;
use entity::{user_provider_keys, provider_types};

/// API密钥健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyHealthStatus {
    /// 密钥ID
    pub key_id: i32,
    /// 提供商类型ID
    pub provider_type_id: i32,
    /// 提供商ID
    pub provider_id: ProviderId,
    /// 当前健康状态
    pub is_healthy: bool,
    /// 最后检查时间
    pub last_check: Option<DateTime<Utc>>,
    /// 最后健康时间
    pub last_healthy: Option<DateTime<Utc>>,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 连续成功次数
    pub consecutive_successes: u32,
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: u64,
    /// 健康分数 (0-100)
    pub health_score: f32,
    /// 最后错误信息
    pub last_error: Option<String>,
    /// 最近检查结果历史
    pub recent_results: Vec<ApiKeyCheckResult>,
}

/// API密钥检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyCheckResult {
    /// 检查时间
    pub timestamp: DateTime<Utc>,
    /// 是否成功
    pub is_success: bool,
    /// 响应时间（毫秒）
    pub response_time_ms: u64,
    /// HTTP状态码
    pub status_code: Option<u16>,
    /// 错误信息
    pub error_message: Option<String>,
    /// 检查类型
    pub check_type: ApiKeyCheckType,
    /// 详细错误分类
    pub error_category: Option<ApiKeyErrorCategory>,
}

/// API密钥检查类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiKeyCheckType {
    /// 模型列表检查
    ModelList,
    /// 简单completion检查
    SimpleCompletion,
    /// 自定义检查
    Custom,
}

/// API密钥错误分类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiKeyErrorCategory {
    /// 密钥无效或过期
    InvalidKey,
    /// 配额耗尽
    QuotaExceeded,
    /// 权限不足
    InsufficientPermissions,
    /// 网络错误
    NetworkError,
    /// 服务器错误
    ServerError,
    /// 未知错误
    Unknown,
}

/// API密钥健康检查配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyHealthConfig {
    /// 健康密钥的检查间隔
    pub healthy_check_interval: Duration,
    /// 不健康密钥的重试间隔
    pub unhealthy_retry_interval: Duration,
    /// 请求超时时间
    pub request_timeout: Duration,
    /// 失败阈值（连续失败多少次标记为不健康）
    pub failure_threshold: u32,
    /// 成功阈值（连续成功多少次标记为健康）
    pub success_threshold: u32,
    /// 保留历史结果数量
    pub max_history_results: usize,
    /// 是否启用健康检查
    pub enabled: bool,
}

impl Default for ApiKeyHealthConfig {
    fn default() -> Self {
        Self {
            healthy_check_interval: Duration::from_secs(600), // 10分钟
            unhealthy_retry_interval: Duration::from_secs(120), // 2分钟
            request_timeout: Duration::from_secs(30),
            failure_threshold: 3,
            success_threshold: 2,
            max_history_results: 20,
            enabled: true,
        }
    }
}

/// API密钥健康检查器
pub struct ApiKeyHealthChecker {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// HTTP客户端
    client: Client,
    /// 健康状态存储
    health_status: Arc<RwLock<HashMap<i32, ApiKeyHealthStatus>>>,
    /// 检查配置
    config: ApiKeyHealthConfig,
    /// 是否正在运行
    is_running: Arc<RwLock<bool>>,
}

impl ApiKeyHealthChecker {
    /// 创建新的API密钥健康检查器
    pub fn new(db: Arc<DatabaseConnection>, config: Option<ApiKeyHealthConfig>) -> Self {
        let client = Client::builder()
            .timeout(config.as_ref().map_or(Duration::from_secs(30), |c| c.request_timeout))
            .build()
            .expect("Failed to create HTTP client for API key health checking");

        Self {
            db,
            client,
            health_status: Arc::new(RwLock::new(HashMap::new())),
            config: config.unwrap_or_default(),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// 启动健康检查服务
    pub async fn start(&self) -> Result<()> {
        let mut running = self.is_running.write().await;
        if *running {
            return Ok(());
        }

        *running = true;
        info!("API key health checker started");
        Ok(())
    }

    /// 停止健康检查服务
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.is_running.write().await;
        *running = false;
        info!("API key health checker stopped");
        Ok(())
    }

    /// 检查单个API密钥的健康状态
    pub async fn check_api_key(
        &self,
        key_model: &user_provider_keys::Model,
    ) -> Result<ApiKeyCheckResult> {
        if !self.config.enabled {
            return Ok(ApiKeyCheckResult {
                timestamp: Utc::now(),
                is_success: true,
                response_time_ms: 0,
                status_code: Some(200),
                error_message: Some("Health check disabled".to_string()),
                check_type: ApiKeyCheckType::Custom,
                error_category: None,
            });
        }

        let start_time = Instant::now();
        let provider_id = ProviderId::from_database_id(key_model.provider_type_id);

        // 获取provider信息来确定检查方式
        let provider_info = provider_types::Entity::find_by_id(key_model.provider_type_id)
            .one(&*self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Provider type not found: {}", key_model.provider_type_id))?;

        debug!(
            key_id = key_model.id,
            provider = %provider_id,
            provider_name = %provider_info.name,
            "Starting API key health check"
        );

        // 使用数据库配置的健康检查逻辑
        let result = self.check_provider_key(
            &key_model.api_key,
            &provider_info,
        ).await;

        let response_time_ms = start_time.elapsed().as_millis() as u64;

        let check_result = match result {
            Ok((status_code, success)) => {
                debug!(
                    key_id = key_model.id,
                    provider_name = %provider_info.name,
                    status_code = status_code,
                    response_time = response_time_ms,
                    success = success,
                    "API key health check completed"
                );

                ApiKeyCheckResult {
                    timestamp: Utc::now(),
                    is_success: success,
                    response_time_ms,
                    status_code: Some(status_code),
                    error_message: None,
                    check_type: ApiKeyCheckType::ModelList,
                    error_category: None,
                }
            }
            Err(e) => {
                let error_category = self.categorize_error(&e);
                warn!(
                    key_id = key_model.id,
                    provider_name = %provider_info.name,
                    error = %e,
                    category = ?error_category,
                    "API key health check failed"
                );

                ApiKeyCheckResult {
                    timestamp: Utc::now(),
                    is_success: false,
                    response_time_ms,
                    status_code: None,
                    error_message: Some(e.to_string()),
                    check_type: ApiKeyCheckType::ModelList,
                    error_category: Some(error_category),
                }
            }
        };

        // 更新健康状态
        self.update_health_status(key_model.id, key_model.provider_type_id, check_result.clone())
            .await?;

        Ok(check_result)
    }

    /// 基于数据库配置检查API密钥
    async fn check_provider_key(
        &self,
        api_key: &str,
        provider_info: &provider_types::Model,
    ) -> Result<(u16, bool)> {
        // 构建健康检查URL
        let base_url = if provider_info.base_url.starts_with("http") {
            provider_info.base_url.clone()
        } else {
            format!("https://{}", provider_info.base_url)
        };
        
        let health_check_path = provider_info
            .health_check_path
            .as_deref()
            .unwrap_or("/models");
            
        let url = if health_check_path.starts_with("http") {
            // 如果health_check_path是完整URL，直接使用
            health_check_path.to_string()
        } else if provider_info.name == "gemini" || provider_info.name == "custom_gemini" {
            // Gemini特殊处理：在URL中包含API key
            format!("{}{}?key={}", base_url, health_check_path, api_key)
        } else {
            // 标准拼接
            format!("{}{}", base_url, health_check_path)
        };

        debug!(
            provider_name = %provider_info.name,
            url = %url,
            "Performing API key health check"
        );

        // 解析认证头格式
        let auth_header_format = provider_info
            .auth_header_format
            .as_deref()
            .unwrap_or("Bearer {key}");

        // 构建请求
        let mut request = if provider_info.name == "anthropic" && health_check_path.contains("/messages") {
            // Claude需要POST请求和payload
            let test_payload = serde_json::json!({
                "model": provider_info.default_model.as_deref().unwrap_or("claude-3-haiku-20240307"),
                "max_tokens": 1,
                "messages": [{"role": "user", "content": "test"}]
            });

            let mut req = self.client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&test_payload);

            // 添加anthropic特有的头部
            if provider_info.name == "anthropic" {
                req = req.header("anthropic-version", "2023-06-01");
            }
            req
        } else {
            // 标准GET请求
            self.client.get(&url)
        };

        // 添加认证头（除了Gemini，因为它在URL中）
        if provider_info.name != "gemini" && provider_info.name != "custom_gemini" {
            if auth_header_format.contains(':') {
                // 格式如 "Authorization: Bearer {key}" 或 "x-api-key: {key}"
                let parts: Vec<&str> = auth_header_format.split(':').collect();
                if parts.len() == 2 {
                    let header_name = parts[0].trim();
                    let header_value = parts[1].trim().replace("{key}", api_key);
                    request = request.header(header_name, header_value);
                }
            } else {
                // 简单格式如 "Bearer {key}"
                let header_value = auth_header_format.replace("{key}", api_key);
                request = request.header("Authorization", header_value);
            }
        }

        // 添加User-Agent
        request = request.header("User-Agent", "api-proxy-health-checker/1.0");

        // 发送请求
        let response = request.send().await?;
        let status_code = response.status().as_u16();
        
        // 判断成功状态
        let success = match status_code {
            200..=299 => true,
            401 => false, // 密钥无效
            403 => false, // 权限不足
            429 => false, // 配额耗尽
            _ => status_code < 500, // 4xx可能是配置问题，5xx是服务器问题
        };

        debug!(
            provider_name = %provider_info.name,
            status_code = status_code,
            success = success,
            "API key health check completed"
        );

        Ok((status_code, success))
    }

    /// 分类错误类型
    fn categorize_error(&self, error: &anyhow::Error) -> ApiKeyErrorCategory {
        let error_string = error.to_string().to_lowercase();
        
        if error_string.contains("unauthorized") || error_string.contains("invalid") {
            ApiKeyErrorCategory::InvalidKey
        } else if error_string.contains("quota") || error_string.contains("rate limit") {
            ApiKeyErrorCategory::QuotaExceeded
        } else if error_string.contains("forbidden") || error_string.contains("permission") {
            ApiKeyErrorCategory::InsufficientPermissions
        } else if error_string.contains("network") || error_string.contains("timeout") {
            ApiKeyErrorCategory::NetworkError
        } else if error_string.contains("server") || error_string.contains("internal") {
            ApiKeyErrorCategory::ServerError
        } else {
            ApiKeyErrorCategory::Unknown
        }
    }

    /// 更新API密钥健康状态
    async fn update_health_status(
        &self,
        key_id: i32,
        provider_type_id: i32,
        check_result: ApiKeyCheckResult,
    ) -> Result<()> {
        let mut health_map = self.health_status.write().await;
        
        let status = health_map.entry(key_id).or_insert_with(|| ApiKeyHealthStatus {
            key_id,
            provider_type_id,
            provider_id: ProviderId::from_database_id(provider_type_id),
            is_healthy: true,
            last_check: None,
            last_healthy: None,
            consecutive_failures: 0,
            consecutive_successes: 0,
            avg_response_time_ms: 0,
            health_score: 100.0,
            last_error: None,
            recent_results: Vec::new(),
        });

        // 更新检查时间
        status.last_check = Some(check_result.timestamp);

        // 更新连续成功/失败计数
        if check_result.is_success {
            status.consecutive_successes += 1;
            status.consecutive_failures = 0;
            status.last_healthy = Some(check_result.timestamp);
            status.last_error = None;
        } else {
            status.consecutive_failures += 1;
            status.consecutive_successes = 0;
            status.last_error = check_result.error_message.clone();
        }

        // 添加检查结果到历史记录
        status.recent_results.push(check_result.clone());
        if status.recent_results.len() > self.config.max_history_results {
            status.recent_results.remove(0);
        }

        // 重新计算健康状态
        let was_healthy = status.is_healthy;
        status.is_healthy = status.consecutive_failures < self.config.failure_threshold;

        // 计算平均响应时间
        if !status.recent_results.is_empty() {
            let total_response_time: u64 = status
                .recent_results
                .iter()
                .filter(|r| r.is_success)
                .map(|r| r.response_time_ms)
                .sum();
            let successful_count = status.recent_results.iter().filter(|r| r.is_success).count();
            
            if successful_count > 0 {
                status.avg_response_time_ms = total_response_time / successful_count as u64;
            }
        }

        // 计算健康分数
        status.health_score = self.calculate_health_score(status);

        // 记录状态变化
        if was_healthy != status.is_healthy {
            if status.is_healthy {
                info!(key_id = key_id, "API key recovered (healthy)");
            } else {
                warn!(
                    key_id = key_id,
                    consecutive_failures = status.consecutive_failures,
                    last_error = ?status.last_error,
                    "API key marked as unhealthy"
                );
            }
        }

        Ok(())
    }

    /// 计算健康分数
    fn calculate_health_score(&self, status: &ApiKeyHealthStatus) -> f32 {
        if status.recent_results.is_empty() {
            return 100.0;
        }

        let recent_count = std::cmp::min(status.recent_results.len(), 10);
        let recent_results = &status.recent_results[status.recent_results.len() - recent_count..];

        // 基础成功率
        let success_count = recent_results.iter().filter(|r| r.is_success).count();
        let success_rate = success_count as f32 / recent_results.len() as f32;
        let mut score = success_rate * 100.0;

        // 响应时间惩罚
        if status.avg_response_time_ms > 3000 {
            let penalty = ((status.avg_response_time_ms - 3000) as f32 / 1000.0) * 5.0;
            score -= penalty.min(20.0);
        }

        // 连续失败惩罚
        if status.consecutive_failures > 0 {
            let penalty = (status.consecutive_failures as f32 * 10.0).min(50.0);
            score -= penalty;
        }

        score.max(0.0).min(100.0)
    }

    /// 获取所有健康的API密钥
    pub async fn get_healthy_keys(&self) -> Vec<i32> {
        let health_map = self.health_status.read().await;
        health_map
            .values()
            .filter(|status| status.is_healthy)
            .map(|status| status.key_id)
            .collect()
    }

    /// 获取特定API密钥的健康状态
    pub async fn get_key_health_status(&self, key_id: i32) -> Option<ApiKeyHealthStatus> {
        let health_map = self.health_status.read().await;
        health_map.get(&key_id).cloned()
    }

    /// 获取所有API密钥的健康状态
    pub async fn get_all_health_status(&self) -> HashMap<i32, ApiKeyHealthStatus> {
        let health_map = self.health_status.read().await;
        health_map.clone()
    }

    /// 检查服务是否正在运行
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// 强制标记密钥为不健康
    pub async fn mark_key_unhealthy(&self, key_id: i32, reason: String) -> Result<()> {
        let mut health_map = self.health_status.write().await;
        
        if let Some(status) = health_map.get_mut(&key_id) {
            status.is_healthy = false;
            status.consecutive_failures += 1;
            status.consecutive_successes = 0;
            status.last_error = Some(format!("Manually marked unhealthy: {}", reason));

            warn!(key_id = key_id, reason = %reason, "Manually marked API key as unhealthy");
        }
        
        Ok(())
    }

    /// 批量检查多个API密钥
    pub async fn batch_check_keys(
        &self,
        keys: Vec<user_provider_keys::Model>,
    ) -> Result<HashMap<i32, ApiKeyCheckResult>> {
        let mut results = HashMap::new();
        
        // 并发执行所有检查
        let check_futures: Vec<_> = keys
            .iter()
            .map(|key| {
                let checker = self;
                async move {
                    let result = checker.check_api_key(key).await;
                    (key.id, result)
                }
            })
            .collect();

        let check_results = futures::future::join_all(check_futures).await;

        for (key_id, result) in check_results {
            match result {
                Ok(check_result) => {
                    results.insert(key_id, check_result);
                }
                Err(e) => {
                    error!(key_id = key_id, error = %e, "Failed to check API key");
                    // 创建失败结果
                    results.insert(
                        key_id,
                        ApiKeyCheckResult {
                            timestamp: Utc::now(),
                            is_success: false,
                            response_time_ms: 0,
                            status_code: None,
                            error_message: Some(e.to_string()),
                            check_type: ApiKeyCheckType::Custom,
                            error_category: Some(ApiKeyErrorCategory::Unknown),
                        },
                    );
                }
            }
        }

        debug!(checked_keys = results.len(), "Batch API key health check completed");
        Ok(results)
    }
}

impl ApiKeyHealthStatus {
    /// 检查是否应该进行下次检查
    pub fn should_check(&self, config: &ApiKeyHealthConfig) -> bool {
        if let Some(last_check) = self.last_check {
            let interval = if self.is_healthy {
                config.healthy_check_interval
            } else {
                config.unhealthy_retry_interval
            };
            
            let next_check_time = last_check + chrono::Duration::from_std(interval).unwrap_or_default();
            Utc::now() > next_check_time
        } else {
            // 从未检查过，立即检查
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_health_config_default() {
        let config = ApiKeyHealthConfig::default();
        assert_eq!(config.healthy_check_interval, Duration::from_secs(600));
        assert_eq!(config.unhealthy_retry_interval, Duration::from_secs(120));
        assert!(config.enabled);
    }

    #[tokio::test]
    async fn test_error_categorization() {
        let db = Arc::new(sea_orm::DatabaseConnection::Disconnected);
        let checker = ApiKeyHealthChecker::new(db, None);
        
        let error = anyhow::anyhow!("unauthorized access");
        assert_eq!(
            checker.categorize_error(&error),
            ApiKeyErrorCategory::InvalidKey
        );

        let error = anyhow::anyhow!("rate limit exceeded");
        assert_eq!(
            checker.categorize_error(&error),
            ApiKeyErrorCategory::QuotaExceeded
        );
    }

    #[tokio::test]
    async fn test_health_score_calculation() {
        let db = Arc::new(sea_orm::DatabaseConnection::Disconnected);
        let checker = ApiKeyHealthChecker::new(db, None);
        
        let mut status = ApiKeyHealthStatus {
            key_id: 1,
            provider_type_id: 1,
            provider_id: ProviderId::from_database_id(1),
            is_healthy: true,
            last_check: None,
            last_healthy: None,
            consecutive_failures: 0,
            consecutive_successes: 5,
            avg_response_time_ms: 100,
            health_score: 0.0,
            last_error: None,
            recent_results: vec![
                ApiKeyCheckResult {
                    timestamp: Utc::now(),
                    is_success: true,
                    response_time_ms: 100,
                    status_code: Some(200),
                    error_message: None,
                    check_type: ApiKeyCheckType::ModelList,
                    error_category: None,
                };
                5
            ],
        };

        let score = checker.calculate_health_score(&status);
        assert!(score > 90.0);
        assert!(score <= 100.0);

        // 测试连续失败的情况
        status.consecutive_failures = 3;
        status.consecutive_successes = 0;
        let score = checker.calculate_health_score(&status);
        assert!(score < 80.0);
    }
}