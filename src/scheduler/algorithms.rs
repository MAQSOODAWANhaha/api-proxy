//! # API密钥池选择算法实现
//!
//! 专注于从用户的多个API密钥中选择合适的密钥进行请求

use super::types::SchedulingStrategy;
use crate::error::{ProxyError, Result};
use entity::user_provider_keys;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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
        let active_keys: Vec<&user_provider_keys::Model> = keys
            .iter()
            .filter(|key| key.is_active)
            .collect();

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

        tracing::debug!(
            request_id = %context.request_id,
            selected_key_id = selected_key.id,
            reason = %reason,
            "Selected API key using round robin strategy"
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
pub struct HealthBasedApiKeySelector;

impl HealthBasedApiKeySelector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HealthBasedApiKeySelector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ApiKeySelector for HealthBasedApiKeySelector {
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

        // 过滤活跃的密钥，优先选择最近创建的（假设更健康）
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

        // 按创建时间排序，最新的排在前面（简单的健康度判断）
        active_keys.sort_by(|a, b| b.1.created_at.cmp(&a.1.created_at));

        let (selected_index, selected_key) = active_keys[0];
        
        let reason = format!(
            "Health-based selection: newest key created at {}, key_id={}",
            selected_key.created_at,
            selected_key.id
        );

        tracing::debug!(
            request_id = %context.request_id,
            selected_key_id = selected_key.id,
            reason = %reason,
            "Selected API key using health-based strategy"
        );

        Ok(ApiKeySelectionResult::new(
            selected_index,
            selected_key.clone(),
            reason,
            SchedulingStrategy::HealthBased,
        ))
    }

    fn name(&self) -> &'static str {
        "HealthBasedApiKeySelector"
    }

    async fn reset(&self) {
        // 无状态，无需重置
    }
}

/// 创建API密钥选择器
pub fn create_api_key_selector(strategy: SchedulingStrategy) -> Arc<dyn ApiKeySelector> {
    match strategy {
        SchedulingStrategy::RoundRobin => Arc::new(RoundRobinApiKeySelector::new()),
        SchedulingStrategy::HealthBased => Arc::new(HealthBasedApiKeySelector::new()),
        SchedulingStrategy::Weighted => {
            // 权重选择暂时回退到轮询，可以以后实现
            Arc::new(RoundRobinApiKeySelector::new())
        }
    }
}
