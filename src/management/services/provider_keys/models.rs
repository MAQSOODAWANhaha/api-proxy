//! # 提供商密钥数据模型
//!
//! 定义提供商密钥相关的请求和响应数据结构。

use serde::{Deserialize, Serialize};

use crate::{key_pool::types::ApiKeyHealthStatus, types::ProviderTypeId};

/// 提供商密钥列表查询参数
#[derive(Debug, Deserialize)]
pub struct ProviderKeysListQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
    pub search: Option<String>,
    pub provider: Option<String>,
    pub status: Option<ApiKeyHealthStatus>,
}

/// 创建提供商密钥请求
#[derive(Debug, Deserialize)]
pub struct CreateProviderKeyRequest {
    pub provider_type_id: ProviderTypeId,
    pub name: String,
    pub api_key: Option<String>,
    pub auth_type: String,
    pub weight: Option<i32>,
    pub max_requests_per_minute: Option<i32>,
    pub max_tokens_prompt_per_minute: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub is_active: Option<bool>,
    pub project_id: Option<String>,
}

/// 更新提供商密钥请求
#[derive(Debug, Deserialize)]
pub struct UpdateProviderKeyRequest {
    pub provider_type_id: ProviderTypeId,
    pub name: String,
    pub api_key: Option<String>,
    pub auth_type: String,
    pub weight: Option<i32>,
    pub max_requests_per_minute: Option<i32>,
    pub max_tokens_prompt_per_minute: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub is_active: Option<bool>,
    pub project_id: Option<String>,
}

/// 密钥使用统计
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProviderKeyUsageStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub success_rate: f64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub avg_response_time: i64,
    pub last_used_at: Option<String>,
}

/// 用户提供商密钥查询参数
#[derive(Debug, Deserialize)]
pub struct UserProviderKeyQuery {
    pub provider_type_id: Option<ProviderTypeId>,
    pub is_active: Option<bool>,
}

/// 趋势查询参数
#[derive(Debug, Deserialize)]
pub struct TrendQuery {
    #[serde(default = "default_days")]
    pub days: u32,
}

const fn default_days() -> u32 {
    7
}

#[derive(Debug, Default, Serialize, Clone)]
pub struct TrendData {
    #[serde(rename = "trend_data")]
    pub points: Vec<TrendDataPoint>,
    pub total_requests: i64,
    pub total_cost: f64,
    pub total_tokens: i64,
    pub avg_response_time: i64,
    pub success_rate: f64,
    #[serde(skip_serializing)]
    pub total_successful_requests: i64,
}

#[derive(Debug, Default, Serialize, Clone)]
pub struct TrendDataPoint {
    pub date: String,
    pub requests: i64,
    pub successful_requests: i64,
    pub failed_requests: i64,
    pub success_rate: f64,
    pub avg_response_time: i64,
    pub tokens: i64,
    pub cost: f64,
}

#[derive(Debug, Default)]
pub struct DailyStats {
    pub total_requests: i64,
    pub successful_requests: i64,
    pub total_cost: f64,
    pub total_response_time: i64,
    pub total_tokens: i64,
}

/// Gemini 上下文准备结果
pub struct PrepareGeminiContext {
    /// 最终的 `project_id`
    pub final_project_id: Option<String>,
    /// 健康状态
    pub health_status: String,
    /// 是否需要异步获取 `project_id`
    pub needs_auto_get_project_id_async: bool,
}
