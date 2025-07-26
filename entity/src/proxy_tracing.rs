//! # 统一代理追踪实体定义
//!
//! 整合了请求统计、详细追踪和健康监控的统一表模型

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 统一代理追踪实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "proxy_tracing")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    // === 基础请求信息（兼容request_statistics） ===
    pub user_service_api_id: i32,
    pub user_provider_key_id: Option<i32>,
    pub request_id: String,
    pub method: String,
    pub path: Option<String>,
    pub status_code: Option<i32>,
    pub response_time_ms: Option<i32>,
    pub request_size: Option<i32>,
    pub response_size: Option<i32>,
    
    // === Token使用统计 ===
    pub tokens_prompt: Option<i32>,
    pub tokens_completion: Option<i32>,
    pub tokens_total: Option<i32>,
    pub token_efficiency_ratio: Option<f64>,
    
    // === 业务信息 ===
    pub model_used: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub retry_count: Option<i32>,
    
    // === 追踪控制 ===
    pub trace_level: i32, // 0=基础, 1=详细, 2=完整
    pub sampling_rate: Option<f64>,
    
    // === 提供商信息 ===
    pub provider_type_id: Option<i32>,
    pub provider_name: Option<String>,
    pub backend_key_id: Option<i32>,
    pub upstream_addr: Option<String>,
    
    // === 详细时间追踪 ===
    pub start_time: Option<DateTime>,
    pub end_time: Option<DateTime>,
    pub duration_ms: Option<i64>,
    pub is_success: bool,
    
    // === 阶段追踪数据（JSON） ===
    pub phases_data: Option<String>, // JSON: 各阶段详细信息
    pub performance_metrics: Option<String>, // JSON: 性能指标
    pub labels: Option<String>, // JSON: 自定义标签
    
    // === 健康状态评估 ===
    pub health_impact_score: Option<f64>,
    pub is_anomaly: Option<bool>,
    pub quality_metrics: Option<String>, // JSON: 质量指标
    
    // === 创建时间 ===
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user_service_apis::Entity",
        from = "Column::UserServiceApiId",
        to = "super::user_service_apis::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    UserServiceApi,
    #[sea_orm(
        belongs_to = "super::user_provider_keys::Entity",
        from = "Column::UserProviderKeyId",
        to = "super::user_provider_keys::Column::Id",
        on_update = "Cascade",
        on_delete = "SetNull"
    )]
    UserProviderKey,
    #[sea_orm(
        belongs_to = "super::provider_types::Entity",
        from = "Column::ProviderTypeId",
        to = "super::provider_types::Column::Id",
        on_update = "Cascade",
        on_delete = "SetNull"
    )]
    ProviderType,
}

impl Related<super::user_service_apis::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserServiceApi.def()
    }
}

impl Related<super::user_provider_keys::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserProviderKey.def()
    }
}

impl Related<super::provider_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProviderType.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// === 辅助模型和枚举 ===

/// 追踪级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceLevel {
    /// 基础统计（向下兼容）
    Basic = 0,
    /// 详细追踪
    Detailed = 1,
    /// 完整追踪（调试模式）
    Full = 2,
}

impl From<i32> for TraceLevel {
    fn from(value: i32) -> Self {
        match value {
            0 => TraceLevel::Basic,
            1 => TraceLevel::Detailed,
            2 => TraceLevel::Full,
            _ => TraceLevel::Basic,
        }
    }
}

impl From<TraceLevel> for i32 {
    fn from(level: TraceLevel) -> Self {
        level as i32
    }
}

/// 请求阶段信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseInfo {
    pub phase: String,
    pub start_time: DateTime,
    pub end_time: Option<DateTime>,
    pub duration_ms: Option<u64>,
    pub status: String,
    pub details: Option<String>,
}

/// 性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub dns_lookup_ms: Option<u64>,
    pub tcp_connect_ms: Option<u64>,
    pub tls_handshake_ms: Option<u64>,
    pub first_byte_ms: Option<u64>,
    pub transfer_ms: Option<u64>,
    pub cpu_usage: Option<f64>,
    pub memory_usage: Option<u64>,
}

/// 质量指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub response_completeness: Option<f64>, // 响应完整性 0-1
    pub content_quality: Option<f64>, // 内容质量 0-1
    pub semantic_accuracy: Option<f64>, // 语义准确性 0-1
    pub latency_percentile: Option<f64>, // 延迟百分位
    pub availability_score: Option<f64>, // 可用性评分 0-1
}

impl Model {
    /// 获取追踪级别
    pub fn get_trace_level(&self) -> TraceLevel {
        TraceLevel::from(self.trace_level)
    }
    
    /// 解析阶段数据
    pub fn get_phases(&self) -> Result<Vec<PhaseInfo>, serde_json::Error> {
        match &self.phases_data {
            Some(data) => serde_json::from_str(data),
            None => Ok(Vec::new()),
        }
    }
    
    /// 解析性能指标
    pub fn get_performance_metrics(&self) -> Result<Option<PerformanceMetrics>, serde_json::Error> {
        match &self.performance_metrics {
            Some(data) => serde_json::from_str(data).map(Some),
            None => Ok(None),
        }
    }
    
    /// 解析标签
    pub fn get_labels(&self) -> Result<HashMap<String, String>, serde_json::Error> {
        match &self.labels {
            Some(data) => serde_json::from_str(data),
            None => Ok(HashMap::new()),
        }
    }
    
    /// 解析质量指标
    pub fn get_quality_metrics(&self) -> Result<Option<QualityMetrics>, serde_json::Error> {
        match &self.quality_metrics {
            Some(data) => serde_json::from_str(data).map(Some),
            None => Ok(None),
        }
    }
    
    /// 判断是否为成功请求
    pub fn is_successful(&self) -> bool {
        self.is_success && self.status_code.map_or(true, |code| code < 400)
    }
    
    /// 计算实际响应时间（优先使用duration_ms）
    pub fn get_response_time(&self) -> Option<u64> {
        if let Some(duration) = self.duration_ms {
            Some(duration as u64)
        } else if let Some(response_time) = self.response_time_ms {
            Some(response_time as u64)
        } else {
            None
        }
    }
    
    /// 获取总token数
    pub fn get_total_tokens(&self) -> u32 {
        self.tokens_total.unwrap_or(0) as u32
    }
    
    /// 计算token效率比率
    pub fn calculate_token_efficiency(&self) -> Option<f64> {
        if let Some(ratio) = self.token_efficiency_ratio {
            return Some(ratio);
        }
        
        let prompt = self.tokens_prompt.unwrap_or(0);
        let completion = self.tokens_completion.unwrap_or(0);
        
        if prompt > 0 {
            Some(completion as f64 / prompt as f64)
        } else {
            None
        }
    }
    
    /// 判断是否为异常请求
    pub fn is_anomalous(&self) -> bool {
        self.is_anomaly.unwrap_or(false) || 
        self.health_impact_score.map_or(false, |score| score < -10.0)
    }
}