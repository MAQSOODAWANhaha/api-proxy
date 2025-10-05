//! # API密钥池选择算法实现
//!
//! 专注于从用户的多个API密钥中选择合适的密钥进行请求

use crate::{ldebug, logging::{LogComponent, LogStage}};
use super::types::SchedulingStrategy;
use crate::error::{ProxyError, Result};
use entity::user_provider_keys;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 选择上下文
#[derive(Debug, Clone)]
pub struct SelectionContext {
    /// 请求ID
    pub request_id: String,
    /// 用户ID
    pub user_id: i32,
    /// 用户服务API ID
    pub user_service_api_id: i32,
    /// 提供商类型ID
    pub provider_type_id: i32,
    /// 额外提示信息
    pub hints: std::collections::HashMap<String, String>,
}

impl SelectionContext {
    pub fn new(
        request_id: String,
        user_id: i32,
        user_service_api_id: i32,
        provider_type_id: i32,
    ) -> Self {
        Self {
            request_id,
            user_id,
            user_service_api_id,
            provider_type_id,
            hints: std::collections::HashMap::new(),
        }
    }
}

/// API密钥选择结果
#[derive(Debug, Clone)]
pub struct ApiKeySelectionResult {
    /// 选中API密钥的索引
    pub selected_index: usize,
    /// 选中的API密钥
    pub selected_key: user_provider_keys::Model,
    /// 选择原因
    pub reason: String,
    /// 选择策略
    pub strategy: SchedulingStrategy,
    /// 选择时间戳
    pub timestamp: std::time::Instant,
}

impl ApiKeySelectionResult {
    pub fn new(
        selected_index: usize,
        selected_key: user_provider_keys::Model,
        reason: String,
        strategy: SchedulingStrategy,
    ) -> Self {
        Self {
            selected_index,
            selected_key,
            reason,
            strategy,
            timestamp: std::time::Instant::now(),
        }
    }
}

/// API密钥选择器特质
#[async_trait::async_trait]
pub trait ApiKeySelector: Send + Sync {
    /// 从用户的API密钥池中选择一个密钥
    async fn select_key(
        &self,
        keys: &[user_provider_keys::Model],
        context: &SelectionContext,
    ) -> Result<ApiKeySelectionResult>;

    /// 获取选择器名称
    fn name(&self) -> &'static str;

    /// 重置内部状态
    async fn reset(&self);
}

/// 轮询API密钥选择器
pub struct RoundRobinApiKeySelector {
    counter: AtomicUsize,
}

impl RoundRobinApiKeySelector {
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }
}

impl Default for RoundRobinApiKeySelector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ApiKeySelector for RoundRobinApiKeySelector {
    async fn select_key(
        &self,
        keys: &[user_provider_keys::Model],
        context: &SelectionContext,
    ) -> Result<ApiKeySelectionResult> {
        if keys.is_empty() {
            return Err(ProxyError::upstream_not_available(
                "No API keys available for selection".to_string(),
            ));
        }

        // 过滤活跃的密钥
        let active_keys: Vec<&user_provider_keys::Model> =
            keys.iter().filter(|key| key.is_active).collect();

        if active_keys.is_empty() {
            return Err(ProxyError::upstream_not_available(
                "No active API keys available for selection".to_string(),
            ));
        }

        // 轮询选择
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        let selected_relative_index = counter % active_keys.len();
        let selected_key = active_keys[selected_relative_index];

        // 找到在原始数组中的索引
        let selected_index = keys
            .iter()
            .position(|key| key.id == selected_key.id)
            .unwrap();

        let reason = format!(
            "Round robin selection: counter={}, active_keys={}, selected_key_id={}",
            counter,
            active_keys.len(),
            selected_key.id
        );

        ldebug!(
            &context.request_id,
            LogStage::Scheduling,
            LogComponent::Scheduler,
            "select_key",
            "Selected API key using round robin strategy",
            selected_key_id = selected_key.id,
            reason = %reason
        );

        Ok(ApiKeySelectionResult::new(
            selected_index,
            selected_key.clone(),
            reason,
            SchedulingStrategy::RoundRobin,
        ))
    }

    fn name(&self) -> &'static str {
        "RoundRobinApiKeySelector"
    }

    async fn reset(&self) {
        self.counter.store(0, Ordering::SeqCst);
    }
}

/// 基于健康度的API密钥选择器
pub struct HealthBestApiKeySelector;

impl HealthBestApiKeySelector {
    pub fn new() -> Self {
        Self
    }

    /// 计算API密钥的健康评分
    /// 评分规则：
    /// - healthy: 100分
    /// - unknown: 80分
    /// - rate_limited: 根据剩余时间计算，最多60分
    /// - unhealthy/error: 0分
    fn calculate_health_score(
        &self,
        key: &user_provider_keys::Model,
        now: chrono::NaiveDateTime,
    ) -> i32 {
        match key.health_status.as_str() {
            "healthy" => 100,
            "unknown" => 80,
            "rate_limited" => {
                if let Some(resets_at) = key.rate_limit_resets_at {
                    if now > resets_at {
                        // 限流已解除，给予较高分数
                        90
                    } else {
                        // 根据剩余限流时间计算分数
                        let duration = resets_at.signed_duration_since(now);
                        let minutes_left = duration.num_minutes();
                        if minutes_left <= 1 {
                            70 // 1分钟内解除
                        } else if minutes_left <= 5 {
                            50 // 5分钟内解除
                        } else if minutes_left <= 15 {
                            30 // 15分钟内解除
                        } else {
                            10 // 更长时间
                        }
                    }
                } else {
                    20 // 没有重置时间，给予较低分数
                }
            }
            "unhealthy" | "error" => 0,
            _ => 50, // 未知状态，中等分数
        }
    }
}

impl Default for HealthBestApiKeySelector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ApiKeySelector for HealthBestApiKeySelector {
    async fn select_key(
        &self,
        keys: &[user_provider_keys::Model],
        context: &SelectionContext,
    ) -> Result<ApiKeySelectionResult> {
        if keys.is_empty() {
            return Err(ProxyError::upstream_not_available(
                "No API keys available for selection".to_string(),
            ));
        }

        // 过滤活跃的密钥
        let mut active_keys: Vec<(usize, &user_provider_keys::Model)> = keys
            .iter()
            .enumerate()
            .filter(|(_, key)| key.is_active)
            .collect();

        if active_keys.is_empty() {
            return Err(ProxyError::upstream_not_available(
                "No active API keys available for selection".to_string(),
            ));
        }

        // 按健康状态排序，优先选择健康状态最好的密钥
        let now = chrono::Utc::now().naive_utc();
        active_keys.sort_by(|a, b| {
            let health_score_a = self.calculate_health_score(a.1, now);
            let health_score_b = self.calculate_health_score(b.1, now);
            health_score_b.cmp(&health_score_a) // 降序排列，分数高的优先
        });

        let (selected_index, selected_key) = active_keys[0];
        let health_score = self.calculate_health_score(selected_key, now);

        let reason = format!(
            "Health-based selection: health_score={}, health_status={}, key_id={}",
            health_score, selected_key.health_status, selected_key.id
        );

        ldebug!(
            &context.request_id,
            LogStage::Scheduling,
            LogComponent::Scheduler,
            "select_key",
            "Selected API key using health-based strategy",
            selected_key_id = selected_key.id,
            health_score = %health_score,
            health_status = %selected_key.health_status,
            reason = %reason
        );

        Ok(ApiKeySelectionResult::new(
            selected_index,
            selected_key.clone(),
            reason,
            SchedulingStrategy::HealthBest,
        ))
    }

    fn name(&self) -> &'static str {
        "HealthBestApiKeySelector"
    }

    async fn reset(&self) {
        // 无状态，无需重置
    }
}

/// 基于权重的API密钥选择器
pub struct WeightedApiKeySelector;

impl WeightedApiKeySelector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WeightedApiKeySelector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ApiKeySelector for WeightedApiKeySelector {
    async fn select_key(
        &self,
        keys: &[user_provider_keys::Model],
        context: &SelectionContext,
    ) -> Result<ApiKeySelectionResult> {
        if keys.is_empty() {
            return Err(ProxyError::upstream_not_available(
                "No API keys available for selection".to_string(),
            ));
        }

        // 过滤活跃的密钥并计算总权重
        let active_keys: Vec<(usize, &user_provider_keys::Model)> = keys
            .iter()
            .enumerate()
            .filter(|(_, key)| key.is_active)
            .collect();

        if active_keys.is_empty() {
            return Err(ProxyError::upstream_not_available(
                "No active API keys available for selection".to_string(),
            ));
        }

        // 计算总权重，如果没有设置权重则默认为1
        let total_weight: i32 = active_keys
            .iter()
            .map(|(_, key)| key.weight.unwrap_or(1))
            .sum();

        if total_weight <= 0 {
            // 如果所有权重都是0，则回退到轮询
            let selected_index = 0;
            let (_, selected_key) = active_keys[selected_index];

            let reason = format!(
                "Weighted selection fallback to round robin: total_weight=0, key_id={}",
                selected_key.id
            );

            ldebug!(
                &context.request_id,
                LogStage::Scheduling,
                LogComponent::Scheduler,
                "select_key",
                "Selected API key using weighted strategy (fallback)",
                selected_key_id = selected_key.id,
                reason = %reason
            );

            return Ok(ApiKeySelectionResult::new(
                selected_index,
                selected_key.clone(),
                reason,
                SchedulingStrategy::Weighted,
            ));
        }

        // 生成随机数进行权重选择
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let random_value: i32 = rng.gen_range(0..total_weight);

        // 根据权重选择密钥
        let mut accumulated_weight = 0;
        let mut selected_index = 0;
        let mut selected_key = active_keys[0].1.clone();

        for &(index, key) in &active_keys {
            let key_weight = key.weight.unwrap_or(1);
            accumulated_weight += key_weight;

            if random_value <= accumulated_weight {
                selected_index = index;
                selected_key = key.clone();
                break;
            }
        }

        let reason = format!(
            "Weighted selection: random_value={}, total_weight={}, key_weight={}, key_id={}",
            random_value,
            total_weight,
            selected_key.weight.unwrap_or(1),
            selected_key.id
        );

        ldebug!(
            &context.request_id,
            LogStage::Scheduling,
            LogComponent::Scheduler,
            "select_key",
            "Selected API key using weighted strategy",
            selected_key_id = selected_key.id,
            reason = %reason
        );

        Ok(ApiKeySelectionResult::new(
            selected_index,
            selected_key.clone(),
            reason,
            SchedulingStrategy::Weighted,
        ))
    }

    fn name(&self) -> &'static str {
        "WeightedApiKeySelector"
    }

    async fn reset(&self) {
        // 无状态，无需重置
    }
}

/// 创建API密钥选择器
pub fn create_api_key_selector(strategy: SchedulingStrategy) -> Arc<dyn ApiKeySelector> {
    match strategy {
        SchedulingStrategy::RoundRobin => Arc::new(RoundRobinApiKeySelector::new()),
        SchedulingStrategy::Weighted => Arc::new(WeightedApiKeySelector::new()),
        SchedulingStrategy::HealthBest => Arc::new(HealthBestApiKeySelector::new()),
    }
}
