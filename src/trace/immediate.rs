//! # 即时写入追踪器
//!
//! 解决长时间请求的内存占用和数据丢失问题，采用即时数据库写入模式

use std::sync::Arc;
use anyhow::Result;
use chrono::Utc;
use sea_orm::{DatabaseConnection, EntityTrait, Set, NotSet, ColumnTrait, QueryFilter, QuerySelect};
use tracing::{debug, error, info};
use entity::proxy_tracing::{self, TraceLevel};

/// 即时写入追踪器配置
#[derive(Debug, Clone)]
pub struct ImmediateTracerConfig {
    /// 全局开关
    pub enabled: bool,
    /// 基础统计采样率
    pub basic_sampling_rate: f64,
    /// 详细追踪采样率
    pub detailed_sampling_rate: f64,
    /// 完整追踪采样率
    pub full_sampling_rate: f64,
    /// 健康评分计算开关
    pub health_scoring_enabled: bool,
    /// 数据库连接池大小
    pub db_pool_size: u32,
}

impl Default for ImmediateTracerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            basic_sampling_rate: 1.0,      // 100% 基础统计
            detailed_sampling_rate: 0.1,   // 10% 详细追踪
            full_sampling_rate: 0.01,      // 1% 完整追踪
            health_scoring_enabled: true,
            db_pool_size: 10,              // 数据库连接池大小
        }
    }
}

/// 即时写入追踪器
/// 
/// 与原有设计不同，此追踪器不在内存中保持状态，
/// 而是在请求开始时立即写入数据库，响应结束时更新记录
#[derive(Debug)]
pub struct ImmediateProxyTracer {
    /// 配置
    config: ImmediateTracerConfig,
    /// 数据库连接
    db: Arc<DatabaseConnection>,
}

impl ImmediateProxyTracer {
    /// 创建新的即时写入追踪器
    pub fn new(db: Arc<DatabaseConnection>, config: ImmediateTracerConfig) -> Self {
        info!(
            "Initializing immediate proxy tracer with sampling rates - basic: {}, detailed: {}, full: {}",
            config.basic_sampling_rate, config.detailed_sampling_rate, config.full_sampling_rate
        );
        
        Self {
            config,
            db,
        }
    }
    
    /// 开始追踪请求 - 立即写入数据库
    pub async fn start_trace(
        &self,
        request_id: String,
        user_service_api_id: i32,
        method: String,
        path: Option<String>,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        
        // 确定追踪级别
        let trace_level = self.determine_trace_level();
        let sampling_rate = self.get_sampling_rate(trace_level);
        
        // 采样检查
        if !self.should_sample(sampling_rate) {
            debug!(request_id = %request_id, "Request excluded by sampling");
            return Ok(());
        }
        
        let now = Utc::now().naive_utc();
        
        // 创建初始追踪记录
        let trace_record = proxy_tracing::ActiveModel {
            id: NotSet,
            user_service_api_id: Set(user_service_api_id),
            user_provider_key_id: Set(None), // 稍后可更新
            request_id: Set(request_id.clone()),
            method: Set(method),
            path: Set(path),
            client_ip: Set(client_ip),
            user_agent: Set(user_agent),
            start_time: Set(Some(now)),
            trace_level: Set(trace_level.into()),
            sampling_rate: Set(Some(sampling_rate)),
            is_success: Set(false), // 默认失败，响应时更新
            created_at: Set(now),
            // 其他字段保持NotSet，待后续更新
            status_code: NotSet,
            response_time_ms: NotSet,
            request_size: NotSet,
            response_size: NotSet,
            tokens_prompt: NotSet,
            tokens_completion: NotSet,
            tokens_total: NotSet,
            token_efficiency_ratio: NotSet,
            model_used: NotSet,
            error_type: NotSet,
            error_message: NotSet,
            retry_count: Set(Some(0)),
            provider_type_id: NotSet,
            provider_name: NotSet,
            backend_key_id: NotSet,
            upstream_addr: NotSet,
            end_time: NotSet,
            duration_ms: NotSet,
            phases_data: NotSet,
            performance_metrics: NotSet,
            labels: NotSet,
            health_impact_score: NotSet,
            is_anomaly: NotSet,
            quality_metrics: NotSet,
        };
        
        // 立即写入数据库
        let insert_result = proxy_tracing::Entity::insert(trace_record)
            .exec(&*self.db)
            .await?;
        
        debug!(
            request_id = %request_id,
            trace_id = insert_result.last_insert_id,
            trace_level = ?trace_level,
            sampling_rate = sampling_rate,
            "Started immediate proxy trace"
        );
        
        Ok(())
    }
    
    /// 更新请求中间信息（可选）
    pub async fn update_trace_info(
        &self,
        request_id: &str,
        provider_name: Option<String>,
        model_used: Option<String>,
        upstream_addr: Option<String>,
        request_size: Option<u64>,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        
        // 构建更新模型
        let mut update_model = proxy_tracing::ActiveModel {
            ..Default::default()
        };
        
        if let Some(provider) = provider_name {
            update_model.provider_name = Set(Some(provider));
        }
        if let Some(model) = model_used {
            update_model.model_used = Set(Some(model));
        }
        if let Some(addr) = upstream_addr {
            update_model.upstream_addr = Set(Some(addr));
        }
        if let Some(size) = request_size {
            update_model.request_size = Set(Some(size as i32));
        }
        
        // 更新数据库记录
        let update_result = proxy_tracing::Entity::update_many()
            .filter(proxy_tracing::Column::RequestId.eq(request_id))
            .set(update_model)
            .exec(&*self.db)
            .await?;
        
        if update_result.rows_affected > 0 {
            debug!(
                request_id = %request_id,
                rows_affected = update_result.rows_affected,
                "Updated trace intermediate info"
            );
        }
        
        Ok(())
    }
    
    /// 更新扩展的请求信息（包含更多字段）
    pub async fn update_extended_trace_info(
        &self,
        request_id: &str,
        provider_name: Option<String>,
        provider_type_id: Option<i32>,
        backend_key_id: Option<i32>,
        model_used: Option<String>,
        upstream_addr: Option<String>,
        request_size: Option<u64>,
        user_provider_key_id: Option<i32>,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        
        // 构建更新模型
        let mut update_model = proxy_tracing::ActiveModel {
            ..Default::default()
        };
        
        let mut updated_fields = Vec::new();
        
        if let Some(provider) = provider_name {
            update_model.provider_name = Set(Some(provider.clone()));
            updated_fields.push(format!("provider_name={}", provider));
        }
        if let Some(provider_id) = provider_type_id {
            update_model.provider_type_id = Set(Some(provider_id));
            updated_fields.push(format!("provider_type_id={}", provider_id));
        }
        if let Some(backend_id) = backend_key_id {
            update_model.backend_key_id = Set(Some(backend_id));
            updated_fields.push(format!("backend_key_id={}", backend_id));
        }
        if let Some(model) = model_used {
            update_model.model_used = Set(Some(model.clone()));
            updated_fields.push(format!("model_used={}", model));
        }
        if let Some(addr) = upstream_addr {
            update_model.upstream_addr = Set(Some(addr.clone()));
            updated_fields.push(format!("upstream_addr={}", addr));
        }
        if let Some(size) = request_size {
            update_model.request_size = Set(Some(size as i32));
            updated_fields.push(format!("request_size={}", size));
        }
        if let Some(user_key_id) = user_provider_key_id {
            update_model.user_provider_key_id = Set(Some(user_key_id));
            updated_fields.push(format!("user_provider_key_id={}", user_key_id));
        }
        
        // 更新数据库记录
        let update_result = proxy_tracing::Entity::update_many()
            .filter(proxy_tracing::Column::RequestId.eq(request_id))
            .set(update_model)
            .exec(&*self.db)
            .await?;
        
        if update_result.rows_affected > 0 {
            info!(
                request_id = %request_id,
                rows_affected = update_result.rows_affected,
                updated_fields = ?updated_fields,
                "Updated extended trace information"
            );
        } else {
            tracing::warn!(
                request_id = %request_id,
                "No trace record found to update extended info"
            );
        }
        
        Ok(())
    }
    
    /// 完成追踪 - 更新最终结果
    pub async fn complete_trace(
        &self,
        request_id: &str,
        status_code: u16,
        is_success: bool,
        response_size: Option<u64>,
        tokens_prompt: Option<u32>,
        tokens_completion: Option<u32>,
        error_type: Option<String>,
        error_message: Option<String>,
    ) -> Result<()> {
        self.complete_trace_with_details(
            request_id,
            status_code,
            is_success,
            response_size,
            tokens_prompt,
            tokens_completion,
            error_type,
            error_message,
            None, // request_details
            None, // response_details
        ).await
    }

    /// 完成追踪 - 包含详细的请求响应信息
    pub async fn complete_trace_with_details(
        &self,
        request_id: &str,
        status_code: u16,
        is_success: bool,
        response_size: Option<u64>,
        tokens_prompt: Option<u32>,
        tokens_completion: Option<u32>,
        error_type: Option<String>,
        error_message: Option<String>,
        request_details: Option<serde_json::Value>,
        response_details: Option<serde_json::Value>,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        
        let end_time = Utc::now().naive_utc();
        
        // 计算token总数和效率
        let tokens_total = match (tokens_prompt, tokens_completion) {
            (Some(p), Some(c)) => Some(p + c),
            _ => None,
        };
        
        let token_efficiency_ratio = match (tokens_prompt, tokens_completion) {
            (Some(p), Some(c)) if p > 0 => Some(c as f64 / p as f64),
            _ => None,
        };
        
        // 计算健康影响评分
        let health_impact_score = if self.config.health_scoring_enabled {
            self.calculate_health_impact(status_code, is_success, token_efficiency_ratio)
        } else {
            None
        };
        
        // 异常检测
        let is_anomaly = self.detect_anomaly(status_code, is_success, token_efficiency_ratio);
        
        // 构建详细性能指标JSON（包含请求响应详情）
        let performance_metrics = if request_details.is_some() || response_details.is_some() {
            let mut metrics = serde_json::Map::new();
            let mut components = Vec::new();
            
            if let Some(req_details) = &request_details {
                metrics.insert("request".to_string(), req_details.clone());
                components.push("request");
            }
            if let Some(resp_details) = &response_details {
                metrics.insert("response".to_string(), resp_details.clone());
                components.push("response");
            }
            
            match serde_json::Value::Object(metrics).to_string() {
                json_str => {
                    tracing::info!(
                        request_id = %request_id,
                        components = ?components,
                        json_length = json_str.len(),
                        "Constructed performance_metrics JSON for database storage"
                    );
                    Some(json_str)
                }
            }
        } else {
            tracing::debug!(
                request_id = %request_id,
                "No detailed request/response data available for performance_metrics"
            );
            None
        };
        
        // 获取起始时间并计算持续时间
        let start_time_result = proxy_tracing::Entity::find()
            .filter(proxy_tracing::Column::RequestId.eq(request_id))
            .select_only()
            .column(proxy_tracing::Column::StartTime)
            .into_tuple::<Option<chrono::NaiveDateTime>>()
            .one(&*self.db)
            .await?;
            
        let (duration_ms, response_time_ms) = if let Some(Some(start_time)) = start_time_result {
            let duration = end_time.signed_duration_since(start_time);
            let duration_ms = duration.num_milliseconds();
            (Some(duration_ms), Some(duration_ms as i32))
        } else {
            (None, None)
        };

        // 构建完成更新模型
        let complete_model = proxy_tracing::ActiveModel {
            status_code: Set(Some(status_code as i32)),
            is_success: Set(is_success),
            end_time: Set(Some(end_time)),
            duration_ms: Set(duration_ms),
            response_time_ms: Set(response_time_ms),
            response_size: Set(response_size.map(|s| s as i32)),
            tokens_prompt: Set(tokens_prompt.map(|t| t as i32)),
            tokens_completion: Set(tokens_completion.map(|t| t as i32)),
            tokens_total: Set(tokens_total.map(|t| t as i32)),
            token_efficiency_ratio: Set(token_efficiency_ratio),
            error_type: Set(error_type),
            error_message: Set(error_message),
            health_impact_score: Set(health_impact_score),
            is_anomaly: Set(Some(is_anomaly)),
            performance_metrics: Set(performance_metrics.clone()),
            ..Default::default()
        };
        
        // 更新数据库记录
        let update_result = proxy_tracing::Entity::update_many()
            .filter(proxy_tracing::Column::RequestId.eq(request_id))
            .set(complete_model)
            .exec(&*self.db)
            .await?;
        
        if update_result.rows_affected > 0 {
            info!(
                request_id = %request_id,
                status_code = status_code,
                is_success = is_success,
                tokens_total = ?tokens_total,
                health_score = ?health_impact_score,
                duration_ms = ?duration_ms,
                response_time_ms = ?response_time_ms,
                performance_metrics_stored = performance_metrics.is_some(),
                performance_metrics_size = performance_metrics.as_ref().map(|m| m.len()),
                rows_affected = update_result.rows_affected,
                "Completed immediate proxy trace with detailed performance metrics"
            );
        } else {
            error!(
                request_id = %request_id,
                "Failed to complete trace - no matching record found"
            );
        }
        
        Ok(())
    }
    
    /// 添加阶段追踪信息（用于详细追踪）
    pub async fn add_phase_info(
        &self,
        request_id: &str,
        phase_name: &str,
        duration_ms: u64,
        success: bool,
        details: Option<String>,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        
        // 这里可以实现阶段信息的JSON更新逻辑
        // 为简化实现，暂时只记录最后一个阶段信息
        let phase_json = serde_json::json!({
            "phase": phase_name,
            "duration_ms": duration_ms,
            "success": success,
            "details": details,
            "timestamp": Utc::now().to_rfc3339()
        }).to_string();
        
        let phase_model = proxy_tracing::ActiveModel {
            phases_data: Set(Some(phase_json)),
            ..Default::default()
        };
        
        proxy_tracing::Entity::update_many()
            .filter(proxy_tracing::Column::RequestId.eq(request_id))
            .set(phase_model)
            .exec(&*self.db)
            .await?;
        
        debug!(
            request_id = %request_id,
            phase = phase_name,
            duration_ms = duration_ms,
            success = success,
            "Added phase info to trace"
        );
        
        Ok(())
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
    
    /// 计算健康影响评分
    fn calculate_health_impact(
        &self,
        status_code: u16,
        is_success: bool,
        token_efficiency_ratio: Option<f64>,
    ) -> Option<f64> {
        let mut score = 0.0;
        
        // 成功率影响（最重要）
        if is_success {
            score += 10.0;
        } else {
            score -= 20.0;
        }
        
        // 状态码影响
        match status_code {
            200..=299 => score += 3.0,   // 成功
            300..=399 => score += 0.0,   // 重定向
            400..=499 => score -= 10.0,  // 客户端错误
            500..=599 => score -= 20.0,  // 服务器错误
            _ => score -= 5.0,
        }
        
        // Token效率影响
        if let Some(efficiency) = token_efficiency_ratio {
            if efficiency > 0.1 && efficiency < 5.0 {
                score += 2.0; // 正常效率范围
            } else {
                score -= 3.0; // 异常效率
            }
        }
        
        Some(score)
    }
    
    /// 异常检测
    fn detect_anomaly(
        &self,
        status_code: u16,
        is_success: bool,
        token_efficiency_ratio: Option<f64>,
    ) -> bool {
        // Token使用异常
        if let Some(efficiency) = token_efficiency_ratio {
            if efficiency > 10.0 || efficiency < 0.01 {
                return true;
            }
        }
        
        // 错误状态
        if !is_success || status_code >= 400 {
            return true;
        }
        
        false
    }
    
    /// 查询进行中的请求（未完成的追踪记录）
    pub async fn get_active_requests(&self, limit: u64) -> Result<Vec<proxy_tracing::Model>> {
        let records = proxy_tracing::Entity::find()
            .filter(proxy_tracing::Column::EndTime.is_null())
            .limit(limit)
            .all(&*self.db)
            .await?;
        
        Ok(records)
    }
    
    /// 清理过期的未完成记录（真正的孤儿记录）
    pub async fn cleanup_orphaned_records(&self, hours_threshold: i64) -> Result<u64> {
        let cutoff_time = Utc::now() - chrono::Duration::hours(hours_threshold);
        
        let delete_result = proxy_tracing::Entity::delete_many()
            .filter(proxy_tracing::Column::EndTime.is_null())
            .filter(proxy_tracing::Column::StartTime.lt(cutoff_time.naive_utc()))
            .exec(&*self.db)
            .await?;
        
        if delete_result.rows_affected > 0 {
            info!(
                rows_deleted = delete_result.rows_affected,
                hours_threshold = hours_threshold,
                "Cleaned up orphaned trace records"
            );
        }
        
        Ok(delete_result.rows_affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use sea_orm::{Database, EntityTrait, PaginatorTrait};
    
    async fn setup_test_db() -> Arc<DatabaseConnection> {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to test database");
        Arc::new(db)
    }
    
    #[tokio::test]
    async fn test_immediate_trace_lifecycle() {
        let db = setup_test_db().await;
        let config = ImmediateTracerConfig::default();
        let tracer = ImmediateProxyTracer::new(db.clone(), config);
        
        let request_id = "test_immediate_12345".to_string();
        
        // 开始追踪
        tracer.start_trace(
            request_id.clone(),
            1,
            "POST".to_string(),
            Some("/v1/chat/completions".to_string()),
            Some("127.0.0.1".to_string()),
            Some("test-client/1.0".to_string()),
        ).await.expect("Failed to start trace");
        
        // 更新中间信息
        tracer.update_trace_info(
            &request_id,
            Some("openai".to_string()),
            Some("gpt-4".to_string()),
            Some("api.openai.com:443".to_string()),
            Some(1024),
        ).await.expect("Failed to update trace info");
        
        // 完成追踪
        tracer.complete_trace(
            &request_id,
            200,
            true,
            Some(2048),
            Some(100),
            Some(50),
            None,
            None,
        ).await.expect("Failed to complete trace");
        
        // 验证记录存在
        let count = proxy_tracing::Entity::find()
            .filter(proxy_tracing::Column::RequestId.eq(&request_id))
            .count(&*db)
            .await
            .expect("Failed to count records");
        
        assert_eq!(count, 1, "Should have exactly one trace record");
    }
}