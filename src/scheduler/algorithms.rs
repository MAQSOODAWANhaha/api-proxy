//! # API密钥池选择算法实现
//!
//! 专注于从用户的多个API密钥中选择合适的密钥进行请求

use super::types::SchedulingStrategy;
use crate::error::{ProxyError, Result};
use crate::{
    ldebug, linfo,
    logging::{LogComponent, LogStage},
    lwarn,
};
use dashmap::DashMap;
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
    /// 路由分组（通常为请求路径）
    pub route_group: String,
}

impl SelectionContext {
    pub fn new(
        request_id: String,
        user_id: i32,
        user_service_api_id: i32,
        provider_type_id: i32,
        route_group: String,
    ) -> Self {
        Self {
            request_id,
            user_id,
            user_service_api_id,
            provider_type_id,
            route_group,
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
    counters: DashMap<(i32, String), Arc<AtomicUsize>>,
}

impl RoundRobinApiKeySelector {
    pub fn new() -> Self {
        Self {
            counters: DashMap::new(),
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

        let incoming_key_ids: Vec<i32> = keys.iter().map(|k| k.id).collect();

        // 过滤活跃的密钥
        let active_keys: Vec<&user_provider_keys::Model> =
            keys.iter().filter(|key| key.is_active).collect();

        let active_key_ids: Vec<i32> = active_keys.iter().map(|k| k.id).collect();

        if active_keys.is_empty() {
            return Err(ProxyError::upstream_not_available(
                "No active API keys available for selection".to_string(),
            ));
        }

        // 轮询选择（按路由分组维护计数器）
        let group_key = (context.user_service_api_id, context.route_group.clone());
        let counter_arc = self
            .counters
            .entry(group_key.clone())
            .or_insert_with(|| Arc::new(AtomicUsize::new(0)))
            .clone();

        let previous_counter = counter_arc.load(Ordering::SeqCst);
        let counter = counter_arc.fetch_add(1, Ordering::SeqCst);
        let selected_relative_index = counter % active_keys.len();
        let selected_key = active_keys[selected_relative_index];

        linfo!(
            &context.request_id,
            LogStage::Scheduling,
            LogComponent::Scheduler,
            "round_robin_state",
            "Round-robin internal state for selection",
            group_key = ?group_key,
            incoming_keys = ?incoming_key_ids,
            active_keys = ?active_key_ids,
            active_keys_len = active_keys.len(),
            previous_counter = previous_counter,
            next_counter = counter + 1,
            selected_index_in_active = selected_relative_index,
            selected_key_id = selected_key.id
        );

        // 找到在原始数组中的索引
        let selected_index = keys
            .iter()
            .position(|key| key.id == selected_key.id)
            .unwrap();

        let reason = format!(
            "Round robin selection: group='{}', counter={}, active_keys={}, selected_key_id={}",
            context.route_group,
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
            route_group = context.route_group.as_str(),
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
        self.counters.clear();
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

/// 基于权重的API密钥选择器 (有状态, 按分组轮询)
pub struct WeightedApiKeySelector {
    counters: DashMap<(i32, String), Arc<AtomicUsize>>,
}

impl WeightedApiKeySelector {
    pub fn new() -> Self {
        Self {
            counters: DashMap::new(),
        }
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

        // 过滤活跃的密钥
        let active_keys: Vec<&user_provider_keys::Model> =
            keys.iter().filter(|key| key.is_active).collect();

        if active_keys.is_empty() {
            return Err(ProxyError::upstream_not_available(
                "No active API keys available for selection".to_string(),
            ));
        }

        // 根据权重创建扩展列表
        let mut weighted_list: Vec<&user_provider_keys::Model> = Vec::new();
        for key in &active_keys {
            // 如果权重为None或无效，则默认为1
            let weight = key.weight.unwrap_or(1).max(0) as usize;
            for _ in 0..weight {
                weighted_list.push(key);
            }
        }

        // 如果所有权重都为0，则回退到无权重的轮询
        if weighted_list.is_empty() {
            lwarn!(
                &context.request_id,
                LogStage::Scheduling,
                LogComponent::Scheduler,
                "weighted_fallback",
                "All key weights are zero, falling back to simple round-robin for this selection.",
                route_group = context.route_group.as_str()
            );
            let round_robin_selector = RoundRobinApiKeySelector::new();
            return round_robin_selector.select_key(keys, context).await;
        }

        // 使用轮询逻辑在加权列表上选择
        let key = (context.user_service_api_id, context.route_group.clone());
        let counter_arc = self
            .counters
            .entry(key)
            .or_insert_with(|| Arc::new(AtomicUsize::new(0)))
            .clone();
        let counter = counter_arc.fetch_add(1, Ordering::SeqCst);
        let selected_relative_index = counter % weighted_list.len();
        let selected_key = weighted_list[selected_relative_index];

        // 找到在原始数组中的索引
        let selected_index = keys
            .iter()
            .position(|key| key.id == selected_key.id)
            .unwrap_or(0); // Fallback to 0 if not found, though it should always be found

        let reason = format!(
            "Stateful Weighted selection: group='{}', counter={}, total_weight={}, key_weight={}, key_id={}",
            context.route_group,
            counter,
            weighted_list.len(),
            selected_key.weight.unwrap_or(1),
            selected_key.id
        );

        ldebug!(
            &context.request_id,
            LogStage::Scheduling,
            LogComponent::Scheduler,
            "select_key",
            "Selected API key using stateful weighted strategy",
            selected_key_id = selected_key.id,
            route_group = context.route_group.as_str(),
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
        self.counters.clear();
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
