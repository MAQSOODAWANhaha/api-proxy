//! # 统一代理追踪器
//!
//! 整合所有追踪功能的统一入口，支持多级别追踪和智能采样

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, EntityTrait, Set};
use serde_json;
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

use entity::proxy_tracing::{self, TraceLevel, PhaseInfo, PerformanceMetrics, QualityMetrics};

/// 统一代理追踪器配置
#[derive(Debug, Clone)]
pub struct UnifiedTracerConfig {
    /// 全局开关
    pub enabled: bool,
    /// 基础统计采样率（所有请求的统计信息）
    pub basic_sampling_rate: f64,
    /// 详细追踪采样率
    pub detailed_sampling_rate: f64,
    /// 完整追踪采样率（调试模式）
    pub full_sampling_rate: f64,
    /// 批量写入大小
    pub batch_size: usize,
    /// 批量写入间隔（秒）
    pub batch_interval_secs: u64,
    /// 异步写入缓冲区大小
    pub buffer_size: usize,
    /// 健康评分计算开关
    pub health_scoring_enabled: bool,
}

impl Default for UnifiedTracerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            basic_sampling_rate: 1.0,      // 100% 基础统计
            detailed_sampling_rate: 0.1,   // 10% 详细追踪
            full_sampling_rate: 0.01,      // 1% 完整追踪
            batch_size: 50,                // 减小批次大小，更频繁写入
            batch_interval_secs: 10,       // 增加间隔时间，适应长请求
            buffer_size: 2000,             // 增大缓冲区，支持更多并发长请求
            health_scoring_enabled: true,
        }
    }
}

/// 统一追踪数据
#[derive(Debug, Clone)]
pub struct UnifiedTrace {
    // 基础信息
    pub request_id: String,
    pub user_service_api_id: i32,
    pub user_provider_key_id: Option<i32>,
    pub method: String,
    pub path: Option<String>,
    
    // 提供商信息
    pub provider_type_id: Option<i32>,
    pub provider_name: Option<String>,
    pub backend_key_id: Option<i32>,
    pub upstream_addr: Option<String>,
    
    // 时间信息
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub response_time_ms: Option<u64>,
    
    // 结果信息
    pub status_code: Option<u16>,
    pub is_success: bool,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub retry_count: u32,
    
    // Token统计
    pub tokens_prompt: u32,
    pub tokens_completion: u32,
    pub token_efficiency_ratio: Option<f64>,
    
    // 业务信息
    pub model_used: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub request_size: Option<u64>,
    pub response_size: Option<u64>,
    
    // 追踪信息
    pub trace_level: TraceLevel,
    pub sampling_rate: f64,
    
    // 详细追踪数据
    pub phases: Vec<PhaseInfo>,
    pub performance_metrics: Option<PerformanceMetrics>,
    pub labels: HashMap<String, String>,
    
    // 健康状态
    pub health_impact_score: Option<f64>,
    pub is_anomaly: bool,
    pub quality_metrics: Option<QualityMetrics>,
}

impl UnifiedTrace {
    /// 创建新的追踪记录
    pub fn new(
        request_id: String,
        user_service_api_id: i32,
        method: String,
        trace_level: TraceLevel,
        sampling_rate: f64,
    ) -> Self {
        Self {
            request_id,
            user_service_api_id,
            user_provider_key_id: None,
            method,
            path: None,
            provider_type_id: None,
            provider_name: None,
            backend_key_id: None,
            upstream_addr: None,
            start_time: Utc::now(),
            end_time: None,
            response_time_ms: None,
            status_code: None,
            is_success: false,
            error_type: None,
            error_message: None,
            retry_count: 0,
            tokens_prompt: 0,
            tokens_completion: 0,
            token_efficiency_ratio: None,
            model_used: None,
            client_ip: None,
            user_agent: None,
            request_size: None,
            response_size: None,
            trace_level,
            sampling_rate,
            phases: Vec::new(),
            performance_metrics: None,
            labels: HashMap::new(),
            health_impact_score: None,
            is_anomaly: false,
            quality_metrics: None,
        }
    }
    
    /// 开始新阶段
    pub fn start_phase(&mut self, phase_name: &str) {
        if matches!(self.trace_level, TraceLevel::Detailed | TraceLevel::Full) {
            self.phases.push(PhaseInfo {
                phase: phase_name.to_string(),
                start_time: Utc::now().naive_utc(),
                end_time: None,
                duration_ms: None,
                status: "in_progress".to_string(),
                details: None,
            });
        }
    }
    
    /// 完成当前阶段
    pub fn complete_phase(&mut self, phase_name: &str, status: &str, details: Option<String>) {
        if matches!(self.trace_level, TraceLevel::Detailed | TraceLevel::Full) {
            if let Some(phase) = self.phases.iter_mut().rev().find(|p| p.phase == phase_name && p.status == "in_progress") {
                let end_time = Utc::now();
                phase.end_time = Some(end_time.naive_utc());
                phase.status = status.to_string();
                phase.details = details;
                phase.duration_ms = Some((end_time.naive_utc() - phase.start_time).num_milliseconds() as u64);
            }
        }
    }
    
    /// 设置完成状态
    pub fn complete(&mut self, status_code: u16, is_success: bool) {
        self.end_time = Some(Utc::now());
        self.status_code = Some(status_code);
        self.is_success = is_success;
        
        if let Some(end_time) = self.end_time {
            self.response_time_ms = Some((end_time - self.start_time).num_milliseconds() as u64);
        }
        
        // 计算健康影响评分
        if let Some(score) = self.calculate_health_impact() {
            self.health_impact_score = Some(score);
        }
        
        // 异常检测
        self.is_anomaly = self.detect_anomaly();
    }
    
    /// 设置Token使用
    pub fn set_token_usage(&mut self, prompt_tokens: u32, completion_tokens: u32) {
        self.tokens_prompt = prompt_tokens;
        self.tokens_completion = completion_tokens;
        
        if prompt_tokens > 0 {
            self.token_efficiency_ratio = Some(completion_tokens as f64 / prompt_tokens as f64);
        }
    }
    
    /// 计算健康影响评分
    fn calculate_health_impact(&self) -> Option<f64> {
        let mut score = 0.0;
        
        // 成功率影响（最重要）
        if self.is_success {
            score += 10.0;
        } else {
            score -= 20.0;
        }
        
        // 响应时间影响
        if let Some(response_time) = self.response_time_ms {
            match response_time {
                0..=1000 => score += 5.0,    // 优秀
                1001..=3000 => score += 0.0, // 正常
                3001..=10000 => score -= 5.0, // 较慢
                _ => score -= 15.0,           // 很慢
            }
        }
        
        // 状态码影响
        if let Some(status) = self.status_code {
            match status {
                200..=299 => score += 3.0,   // 成功
                300..=399 => score += 0.0,   // 重定向
                400..=499 => score -= 10.0,  // 客户端错误
                500..=599 => score -= 20.0,  // 服务器错误
                _ => score -= 5.0,
            }
        }
        
        // Token效率影响
        if let Some(efficiency) = self.token_efficiency_ratio {
            if efficiency > 0.1 && efficiency < 5.0 {
                score += 2.0; // 正常效率范围
            } else {
                score -= 3.0; // 异常效率
            }
        }
        
        Some(score)
    }
    
    /// 异常检测
    fn detect_anomaly(&self) -> bool {
        // 响应时间异常
        if let Some(response_time) = self.response_time_ms {
            if response_time > 30000 { // 超过30秒
                return true;
            }
        }
        
        // Token使用异常
        if let Some(efficiency) = self.token_efficiency_ratio {
            if efficiency > 10.0 || efficiency < 0.01 {
                return true;
            }
        }
        
        // 错误状态
        if !self.is_success {
            return true;
        }
        
        // 健康评分异常
        if let Some(score) = self.health_impact_score {
            if score < -30.0 {
                return true;
            }
        }
        
        false
    }
    
    /// 转换为数据库模型
    pub fn to_active_model(&self) -> Result<proxy_tracing::ActiveModel> {
        Ok(proxy_tracing::ActiveModel {
            id: Set(0), // auto increment
            user_service_api_id: Set(self.user_service_api_id),
            user_provider_key_id: Set(self.user_provider_key_id),
            request_id: Set(self.request_id.clone()),
            method: Set(self.method.clone()),
            path: Set(self.path.clone()),
            status_code: Set(self.status_code.map(|s| s as i32)),
            response_time_ms: Set(self.response_time_ms.map(|r| r as i32)),
            request_size: Set(self.request_size.map(|s| s as i32)),
            response_size: Set(self.response_size.map(|s| s as i32)),
            tokens_prompt: Set(Some(self.tokens_prompt as i32)),
            tokens_completion: Set(Some(self.tokens_completion as i32)),
            tokens_total: Set(Some((self.tokens_prompt + self.tokens_completion) as i32)),
            token_efficiency_ratio: Set(self.token_efficiency_ratio),
            model_used: Set(self.model_used.clone()),
            client_ip: Set(self.client_ip.clone()),
            user_agent: Set(self.user_agent.clone()),
            error_type: Set(self.error_type.clone()),
            error_message: Set(self.error_message.clone()),
            retry_count: Set(Some(self.retry_count as i32)),
            trace_level: Set(self.trace_level.into()),
            sampling_rate: Set(Some(self.sampling_rate)),
            provider_type_id: Set(self.provider_type_id),
            provider_name: Set(self.provider_name.clone()),
            backend_key_id: Set(self.backend_key_id),
            upstream_addr: Set(self.upstream_addr.clone()),
            start_time: Set(Some(self.start_time.naive_utc())),
            end_time: Set(self.end_time.map(|t| t.naive_utc())),
            duration_ms: Set(self.response_time_ms.map(|d| d as i64)),
            is_success: Set(self.is_success),
            phases_data: Set(if self.phases.is_empty() { None } else { Some(serde_json::to_string(&self.phases)?) }),
            performance_metrics: Set(self.performance_metrics.as_ref().map(|m| serde_json::to_string(m)).transpose()?),
            labels: Set(if self.labels.is_empty() { None } else { Some(serde_json::to_string(&self.labels)?) }),
            health_impact_score: Set(self.health_impact_score),
            is_anomaly: Set(Some(self.is_anomaly)),
            quality_metrics: Set(self.quality_metrics.as_ref().map(|m| serde_json::to_string(m)).transpose()?),
            created_at: Set(self.start_time.naive_utc()),
        })
    }
}

/// 统一代理追踪器
pub struct UnifiedProxyTracer {
    /// 配置
    config: UnifiedTracerConfig,
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 活跃追踪记录
    active_traces: Arc<RwLock<HashMap<String, UnifiedTrace>>>,
    /// 写入缓冲区
    write_buffer: Arc<RwLock<Vec<UnifiedTrace>>>,
}

impl UnifiedProxyTracer {
    /// 创建新的统一追踪器
    pub fn new(db: Arc<DatabaseConnection>, config: UnifiedTracerConfig) -> Self {
        let tracer = Self {
            config: config.clone(),
            db,
            active_traces: Arc::new(RwLock::new(HashMap::new())),
            write_buffer: Arc::new(RwLock::new(Vec::new())),
        };
        
        // 启动后台写入任务
        tracer.start_background_writer();
        
        tracer
    }
    
    /// 开始追踪请求
    pub async fn start_trace(
        &self,
        request_id: String,
        user_service_api_id: i32,
        method: String,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        
        // 确定追踪级别
        let trace_level = self.determine_trace_level();
        let sampling_rate = self.get_sampling_rate(trace_level);
        
        // 采样检查
        if !self.should_sample(sampling_rate) {
            return Ok(());
        }
        
        let trace = UnifiedTrace::new(
            request_id.clone(),
            user_service_api_id,
            method,
            trace_level,
            sampling_rate,
        );
        
        // 存储活跃追踪
        let mut active_traces = self.active_traces.write().await;
        active_traces.insert(request_id.clone(), trace);
        
        debug!(
            request_id = %request_id,
            trace_level = ?trace_level,
            sampling_rate = sampling_rate,
            "Started unified proxy trace"
        );
        
        Ok(())
    }
    
    /// 完成追踪
    pub async fn complete_trace(
        &self,
        request_id: &str,
        status_code: u16,
        is_success: bool,
    ) -> Result<()> {
        if let Some(mut trace) = self.remove_active_trace(request_id).await {
            trace.complete(status_code, is_success);
            
            // 添加到写入缓冲区
            self.buffer_trace(trace).await?;
        }
        
        Ok(())
    }
    
    /// 更新追踪信息
    pub async fn update_trace<F>(&self, request_id: &str, updater: F) -> Result<()>
    where
        F: FnOnce(&mut UnifiedTrace),
    {
        let mut active_traces = self.active_traces.write().await;
        if let Some(trace) = active_traces.get_mut(request_id) {
            updater(trace);
        }
        Ok(())
    }
    
    /// 开始阶段追踪
    pub async fn start_phase(&self, request_id: &str, phase_name: &str) -> Result<()> {
        self.update_trace(request_id, |trace| {
            trace.start_phase(phase_name);
        }).await
    }
    
    /// 完成阶段追踪
    pub async fn complete_phase(
        &self, 
        request_id: &str, 
        phase_name: &str, 
        success: bool, 
        details: Option<&str>
    ) -> Result<()> {
        let status = if success { "completed" } else { "failed" };
        self.update_trace(request_id, |trace| {
            trace.complete_phase(phase_name, status, details.map(|s| s.to_string()));
        }).await
    }
    
    /// 确定追踪级别
    fn determine_trace_level(&self) -> TraceLevel {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let sample: f64 = rng.gen();
        
        if sample < self.config.full_sampling_rate {
            TraceLevel::Full
        } else if sample < self.config.detailed_sampling_rate {
            TraceLevel::Detailed
        } else {
            TraceLevel::Basic
        }
    }
    
    /// 获取采样率
    fn get_sampling_rate(&self, level: TraceLevel) -> f64 {
        match level {
            TraceLevel::Basic => self.config.basic_sampling_rate,
            TraceLevel::Detailed => self.config.detailed_sampling_rate,
            TraceLevel::Full => self.config.full_sampling_rate,
        }
    }
    
    /// 检查是否应该采样
    fn should_sample(&self, rate: f64) -> bool {
        if rate >= 1.0 {
            return true;
        }
        if rate <= 0.0 {
            return false;
        }
        
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen::<f64>() < rate
    }
    
    /// 移除活跃追踪
    async fn remove_active_trace(&self, request_id: &str) -> Option<UnifiedTrace> {
        let mut active_traces = self.active_traces.write().await;
        active_traces.remove(request_id)
    }
    
    /// 缓冲追踪数据
    async fn buffer_trace(&self, trace: UnifiedTrace) -> Result<()> {
        let mut buffer = self.write_buffer.write().await;
        buffer.push(trace);
        
        // 如果缓冲区满了，触发写入
        if buffer.len() >= self.config.batch_size {
            let traces_to_write: Vec<UnifiedTrace> = buffer.drain(..).collect();
            drop(buffer); // 释放锁
            
            let db = self.db.clone();
            tokio::spawn(async move {
                if let Err(e) = Self::write_traces_batch(&db, traces_to_write).await {
                    error!("Failed to write traces batch: {}", e);
                }
            });
        }
        
        Ok(())
    }
    
    /// 批量写入追踪数据
    async fn write_traces_batch(db: &DatabaseConnection, traces: Vec<UnifiedTrace>) -> Result<()> {
        if traces.is_empty() {
            return Ok(());
        }
        
        let mut active_models = Vec::new();
        for trace in traces {
            active_models.push(trace.to_active_model()?);
        }
        
        let batch_size = active_models.len();
        // 批量插入
        proxy_tracing::Entity::insert_many(active_models)
            .exec(db)
            .await?;
        
        debug!("Successfully wrote {} traces to database", batch_size);
        Ok(())
    }
    
    /// 启动后台写入任务
    fn start_background_writer(&self) {
        let db = self.db.clone();
        let buffer = self.write_buffer.clone();
        let active_traces = self.active_traces.clone();
        let interval = self.config.batch_interval_secs;
        
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(interval));
            
            loop {
                timer.tick().await;
                
                // 刷新缓冲区
                let traces_to_write = {
                    let mut buffer_guard = buffer.write().await;
                    buffer_guard.drain(..).collect::<Vec<_>>()
                };
                
                if !traces_to_write.is_empty() {
                    if let Err(e) = Self::write_traces_batch(&db, traces_to_write).await {
                        error!("Background writer failed: {}", e);
                    }
                }
                
                // 清理过期的活跃追踪
                Self::cleanup_stale_traces(&db, &active_traces).await;
            }
        });
    }
    
    /// 清理过期的活跃追踪
    async fn cleanup_stale_traces(
        db: &Arc<DatabaseConnection>,
        active_traces: &Arc<RwLock<HashMap<String, UnifiedTrace>>>,
    ) {
        // 调整超时时间为30分钟，适应长时间请求
        let cutoff_time = Utc::now() - chrono::Duration::minutes(30);
        let mut stale_traces = Vec::new();
        
        {
            let traces = active_traces.read().await;
            for (request_id, trace) in traces.iter() {
                if trace.start_time < cutoff_time {
                    stale_traces.push((request_id.clone(), trace.clone()));
                }
            }
        }
        
        if !stale_traces.is_empty() {
            // 先尝试将过期的追踪记录写入数据库，避免数据丢失
            let mut traces_to_save = Vec::new();
            for (_request_id, mut trace) in stale_traces {
                // 标记为超时完成
                trace.complete(408, false); // 408 Request Timeout
                trace.error_type = Some("timeout".to_string());
                trace.error_message = Some("Request exceeded maximum tracking time".to_string());
                traces_to_save.push(trace);
            }
            
            // 写入数据库保存过期记录
            if let Err(e) = Self::write_traces_batch(db, traces_to_save).await {
                error!("Failed to save stale traces before cleanup: {}", e);
            }
            
            // 然后清理内存中的记录
            let mut traces = active_traces.write().await;
            let mut removed_count = 0;
            let cutoff_time_check = Utc::now() - chrono::Duration::minutes(30);
            
            traces.retain(|_, trace| {
                if trace.start_time < cutoff_time_check {
                    removed_count += 1;
                    false
                } else {
                    true
                }
            });
            
            if removed_count > 0 {
                warn!("Cleaned up {} stale traces (saved to database as timeout)", removed_count);
            }
        }
    }
    
    /// 获取统计信息
    pub async fn get_stats(&self) -> HashMap<String, u64> {
        let active_count = self.active_traces.read().await.len() as u64;
        let buffer_count = self.write_buffer.read().await.len() as u64;
        
        let mut stats = HashMap::new();
        stats.insert("active_traces".to_string(), active_count);
        stats.insert("buffered_traces".to_string(), buffer_count);
        stats.insert("batch_size".to_string(), self.config.batch_size as u64);
        
        stats
    }
}