//! # 即时写入追踪器
//!
//! 解决长时间请求的内存占用和数据丢失问题，采用即时数据库写入模式

use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, lerror, linfo, lwarn};
use anyhow::Result;
use chrono::Utc;
use entity::proxy_tracing;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, NotSet, QueryFilter, QuerySelect, Set,
};
use std::sync::Arc;

/// `简化完成追踪参数`（`用于complete_trace函数`）
#[derive(Debug, Clone)]
pub struct SimpleCompleteTraceParams {
    pub request_id: String,
    pub status_code: u16,
    pub is_success: bool,
    pub tokens_prompt: Option<u32>,
    pub tokens_completion: Option<u32>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
}

/// 完整完成追踪参数
#[derive(Debug, Clone)]
pub struct CompleteTraceParams {
    pub status_code: u16,
    pub is_success: bool,
    pub tokens_prompt: Option<u32>,
    pub tokens_completion: Option<u32>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub cache_create_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    pub cost: Option<f64>,
    pub cost_currency: Option<String>,
}

/// 开始追踪参数
#[derive(Debug, Clone)]
pub struct StartTraceParams {
    pub request_id: String,
    pub user_service_api_id: i32,
    pub user_id: Option<i32>,
    pub provider_type_id: Option<i32>,
    pub user_provider_key_id: Option<i32>,
    pub method: String,
    pub path: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
}

/// 即时写入追踪器配置（已简化）
#[derive(Debug, Clone, Default)]
pub struct ImmediateTracerConfig {}

/// 即时写入追踪器
///
/// 与原有设计不同，此追踪器不在内存中保持状态，
/// 而是在请求开始时立即写入数据库，响应结束时更新记录
#[derive(Debug, Clone)]
pub struct ImmediateProxyTracer {
    /// 配置
    #[allow(dead_code)]
    config: ImmediateTracerConfig,
    /// 数据库连接
    db: Arc<DatabaseConnection>,
}

impl ImmediateProxyTracer {
    /// 创建新的即时写入追踪器
    pub fn new(db: Arc<DatabaseConnection>, _config: ImmediateTracerConfig) -> Self {
        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::TracingService,
            "init",
            "Initializing immediate proxy tracer with all requests traced"
        );

        Self {
            config: ImmediateTracerConfig::default(),
            db,
        }
    }

    /// 开始追踪请求 - 立即写入数据库
    pub async fn start_trace(&self, params: StartTraceParams) -> Result<()> {
        // 强制追踪所有请求，移除配置开关

        let now = Utc::now().naive_utc();

        // 创建初始追踪记录
        let trace_record = proxy_tracing::ActiveModel {
            id: NotSet,
            user_service_api_id: Set(params.user_service_api_id),
            user_provider_key_id: Set(params.user_provider_key_id),
            user_id: Set(params.user_id),
            request_id: Set(params.request_id.clone()),
            method: Set(params.method),
            path: Set(params.path),
            client_ip: Set(params.client_ip),
            user_agent: Set(params.user_agent),
            start_time: Set(Some(now)),
            is_success: Set(false), // 默认失败，响应时更新
            created_at: Set(now),
            // 其他字段保持NotSet，待后续更新
            status_code: NotSet,
            tokens_prompt: NotSet,
            tokens_completion: NotSet,
            tokens_total: NotSet,
            token_efficiency_ratio: NotSet,
            cache_create_tokens: NotSet,
            cache_read_tokens: NotSet,
            cost: NotSet,
            cost_currency: NotSet,
            model_used: NotSet,
            error_type: NotSet,
            error_message: NotSet,
            retry_count: Set(Some(0)),
            provider_type_id: Set(params.provider_type_id),
            end_time: NotSet,
            duration_ms: NotSet,
        };

        // 立即写入数据库
        let insert_result = proxy_tracing::Entity::insert(trace_record)
            .exec(&*self.db)
            .await?;

        ldebug!(
            &params.request_id,
            LogStage::Internal,
            LogComponent::Tracing,
            "trace_started",
            "Started simplified proxy trace",
            trace_id = insert_result.last_insert_id,
            user_id = ?params.user_id,
            provider_type_id = ?params.provider_type_id,
            user_provider_key_id = ?params.user_provider_key_id
        );

        Ok(())
    }

    /// 更新请求中间信息（可选）
    pub async fn update_trace_info(
        &self,
        request_id: &str,
        model_used: Option<String>,
    ) -> Result<()> {
        // 强制追踪所有请求，移除配置开关

        // 构建更新模型
        let mut update_model = proxy_tracing::ActiveModel::default();

        if let Some(model) = model_used {
            update_model.model_used = Set(Some(model));
        }

        // 更新数据库记录
        let update_result = proxy_tracing::Entity::update_many()
            .filter(proxy_tracing::Column::RequestId.eq(request_id))
            .set(update_model)
            .exec(&*self.db)
            .await?;

        if update_result.rows_affected > 0 {
            ldebug!(
                request_id,
                LogStage::Internal,
                LogComponent::Tracing,
                "trace_info_updated",
                "Updated trace intermediate info",
                rows_affected = update_result.rows_affected
            );
        }

        Ok(())
    }

    /// 更新模型信息（第一层：立即更新核心模型信息）
    ///
    /// 在获取到模型和后端信息时立即更新，确保核心追踪数据实时性
    pub async fn update_trace_model_info(
        &self,
        request_id: &str,
        provider_type_id: Option<i32>,
        model_used: Option<String>,
        user_provider_key_id: Option<i32>,
    ) -> Result<()> {
        // 构建更新模型
        let mut update_model = proxy_tracing::ActiveModel::default();

        let mut updated_fields = Vec::new();

        if let Some(provider_id) = provider_type_id {
            update_model.provider_type_id = Set(Some(provider_id));
            updated_fields.push(format!("provider_type_id={provider_id}"));
        }
        if let Some(model) = model_used {
            update_model.model_used = Set(Some(model.clone()));
            updated_fields.push(format!("model_used={model}"));
        }
        if let Some(user_key_id) = user_provider_key_id {
            update_model.user_provider_key_id = Set(Some(user_key_id));
            updated_fields.push(format!("user_provider_key_id={user_key_id}"));
        }

        // 更新数据库记录
        let update_result = proxy_tracing::Entity::update_many()
            .filter(proxy_tracing::Column::RequestId.eq(request_id))
            .set(update_model)
            .exec(&*self.db)
            .await?;

        if update_result.rows_affected > 0 {
            linfo!(
                request_id,
                LogStage::RequestModify,
                LogComponent::Tracing,
                "model_info_updated",
                "模型信息更新成功（第一层：立即更新）",
                rows_affected = update_result.rows_affected,
                updated_fields = ?updated_fields
            );
        } else {
            lwarn!(
                request_id,
                LogStage::RequestModify,
                LogComponent::Tracing,
                "model_info_update_not_found",
                "No trace record found to update model info"
            );
        }

        Ok(())
    }

    /// 完成追踪 - 更新最终结果
    pub async fn complete_trace(&self, params: SimpleCompleteTraceParams) -> Result<()> {
        let complete_params = CompleteTraceParams {
            status_code: params.status_code,
            is_success: params.is_success,
            tokens_prompt: params.tokens_prompt,
            tokens_completion: params.tokens_completion,
            error_type: params.error_type,
            error_message: params.error_message,
            cache_create_tokens: None,
            cache_read_tokens: None,
            cost: None,
            cost_currency: None,
        };
        self.complete_trace_with_stats(&params.request_id, complete_params)
            .await
    }

    /// 验证状态码一致性 - 检测并报告状态码不匹配问题
    fn validate_status_code_consistency(request_id: &str, reported_status: u16, is_success: bool) {
        // 检查状态码与成功标志的一致性
        let actual_success = reported_status < 400;
        if actual_success != is_success {
            lwarn!(
                request_id,
                LogStage::Response,
                LogComponent::Tracing,
                "status_code_success_mismatch",
                "状态码与成功标志不一致",
                reported_status = reported_status,
                is_success_flag = is_success,
                actual_success = actual_success
            );
        }

        // 检查连接失败相关的状态码
        if reported_status == 502 || reported_status == 504 {
            linfo!(
                request_id,
                LogStage::Response,
                LogComponent::Tracing,
                "connection_failure_detected",
                "检测到连接失败状态码",
                status_code = reported_status,
                is_success = is_success
            );
        }
    }

    /// 完成追踪 - `使用TraceStats`
    pub async fn complete_trace_with_stats(
        &self,
        request_id: &str,
        params: CompleteTraceParams,
    ) -> Result<()> {
        // 验证状态码一致性
        Self::validate_status_code_consistency(request_id, params.status_code, params.is_success);

        let end_time = Utc::now().naive_utc();

        // 计算token总数和效率
        let tokens_total = match (params.tokens_prompt, params.tokens_completion) {
            (Some(p), Some(c)) => Some(p + c),
            _ => None,
        };

        let token_efficiency_ratio = match (params.tokens_prompt, params.tokens_completion) {
            (Some(p), Some(c)) if p > 0 => Some(f64::from(c) / f64::from(p)),
            _ => None,
        };

        // 获取起始时间并计算持续时间
        let start_time_result = proxy_tracing::Entity::find()
            .filter(proxy_tracing::Column::RequestId.eq(request_id))
            .select_only()
            .column(proxy_tracing::Column::StartTime)
            .into_tuple::<Option<chrono::NaiveDateTime>>()
            .one(&*self.db)
            .await?;

        let duration_ms = if let Some(Some(start_time)) = start_time_result {
            let duration = end_time.signed_duration_since(start_time);
            Some(duration.num_milliseconds())
        } else {
            None
        };

        // 构建完成更新模型
        let complete_model = proxy_tracing::ActiveModel {
            status_code: Set(Some(i32::from(params.status_code))),
            is_success: Set(params.is_success),
            end_time: Set(Some(end_time)),
            duration_ms: Set(duration_ms),
            tokens_prompt: Set(params.tokens_prompt.and_then(|t| i32::try_from(t).ok())),
            tokens_completion: Set(params.tokens_completion.and_then(|t| i32::try_from(t).ok())),
            tokens_total: Set(tokens_total.and_then(|t| i32::try_from(t).ok())),
            token_efficiency_ratio: Set(token_efficiency_ratio),
            cache_create_tokens: Set(params
                .cache_create_tokens
                .and_then(|t| i32::try_from(t).ok())),
            cache_read_tokens: Set(params.cache_read_tokens.and_then(|t| i32::try_from(t).ok())),
            cost: Set(params.cost),
            cost_currency: Set(params.cost_currency),
            error_type: Set(params.error_type),
            error_message: Set(params.error_message),
            ..Default::default()
        };

        // 更新数据库记录
        let update_result = proxy_tracing::Entity::update_many()
            .filter(proxy_tracing::Column::RequestId.eq(request_id))
            .set(complete_model)
            .exec(&*self.db)
            .await?;

        if update_result.rows_affected > 0 {
            linfo!(
                request_id,
                LogStage::Response,
                LogComponent::Tracing,
                "trace_completed",
                "Completed trace with comprehensive stats",
                status_code = params.status_code,
                is_success = params.is_success,
                tokens_total = ?tokens_total,
                cost = ?params.cost,
                cache_create_tokens = ?params.cache_create_tokens,
                cache_read_tokens = ?params.cache_read_tokens,
                duration_ms = ?duration_ms,
                rows_affected = update_result.rows_affected
            );
        } else {
            lerror!(
                request_id,
                LogStage::Response,
                LogComponent::Tracing,
                "trace_complete_failed",
                "Failed to complete trace - no matching record found"
            );
        }

        Ok(())
    }

    /// 使用TraceStats完成追踪（第二层：批量更新统计信息）
    ///
    /// 在请求完成时一次性更新所有统计字段，减少数据库压力
    pub async fn complete_trace_with_trace_stats(
        &self,
        request_id: &str,
        status_code: u16,
        is_success: bool,
        trace_stats: crate::trace::TraceStats,
    ) -> Result<()> {
        let params = CompleteTraceParams {
            status_code,
            is_success,
            tokens_prompt: trace_stats.input_tokens,
            tokens_completion: trace_stats.output_tokens,
            error_type: trace_stats.error_type,
            error_message: trace_stats.error_message,
            cache_create_tokens: trace_stats.cache_create_tokens,
            cache_read_tokens: trace_stats.cache_read_tokens,
            cost: trace_stats.cost,
            cost_currency: trace_stats.cost_currency,
        };
        let result = self.complete_trace_with_stats(request_id, params).await;

        match &result {
            Ok(()) => {
                linfo!(
                    request_id,
                    LogStage::Response,
                    LogComponent::Tracing,
                    "trace_completed_batch",
                    "Completed trace with batch statistics update (Layer 2: Batch)",
                    status_code = status_code,
                    is_success = is_success,
                    input_tokens = ?trace_stats.input_tokens,
                    output_tokens = ?trace_stats.output_tokens,
                    cost = ?trace_stats.cost
                );
            }
            Err(e) => {
                lerror!(
                    request_id,
                    LogStage::Response,
                    LogComponent::Tracing,
                    "trace_complete_batch_failed",
                    &format!("Failed to complete trace with batch statistics: {e}")
                );
            }
        }

        result
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
            linfo!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::Tracing,
                "cleanup_orphaned",
                &format!(
                    "Cleaned up orphaned trace records: {} rows deleted",
                    delete_result.rows_affected
                ),
                hours_threshold = hours_threshold
            );
        }

        Ok(delete_result.rows_affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{Database, EntityTrait, PaginatorTrait, Set};
    use serial_test::serial;
    use std::sync::Arc;

    async fn setup_test_db() -> Arc<DatabaseConnection> {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to test database");
        Migrator::up(&db, None)
            .await
            .expect("Failed to run migrations");
        Arc::new(db)
    }

    #[tokio::test]
    #[serial]
    async fn test_immediate_trace_lifecycle() {
        let db = setup_test_db().await;
        let config = ImmediateTracerConfig::default();
        let tracer = ImmediateProxyTracer::new(db.clone(), config);

        // Insert a user record
        let user = entity::users::ActiveModel {
            id: Set(999), // 使用不同的ID避免冲突
            username: Set("testuser_immediate".to_string()),
            password_hash: Set("...".to_string()),
            email: Set("test_immediate@test.com".to_string()),
            salt: Set("salt".to_string()),
            is_admin: Set(false),
            is_active: Set(true),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };
        entity::users::Entity::insert(user)
            .exec(&*db)
            .await
            .unwrap();

        // Insert a user_service_api record
        let user_service_api = entity::user_service_apis::ActiveModel {
            id: Set(999),
            user_id: Set(999),
            provider_type_id: Set(1), // 添加必需的provider_type_id
            api_key: Set("test-api-key-999".to_string()), // 添加必需的api_key
            name: Set(Some("Test API".to_string())),
            is_active: Set(true),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };
        entity::user_service_apis::Entity::insert(user_service_api)
            .exec(&*db)
            .await
            .unwrap();

        let request_id = "test_immediate_12345".to_string();

        // 开始追踪
        let start_params = StartTraceParams {
            request_id: request_id.clone(),
            user_service_api_id: 999,
            user_id: Some(999),
            provider_type_id: Some(1),
            user_provider_key_id: None,
            method: "POST".to_string(),
            path: Some("/v1/chat/completions".to_string()),
            client_ip: Some("127.0.0.1".to_string()),
            user_agent: Some("test-client/1.0".to_string()),
        };
        tracer
            .start_trace(start_params)
            .await
            .expect("Failed to start trace");

        // 更新中间信息
        tracer
            .update_trace_info(&request_id, Some("gpt-4".to_string()))
            .await
            .expect("Failed to update trace info");

        // 完成追踪
        let complete_params = SimpleCompleteTraceParams {
            request_id: request_id.clone(),
            status_code: 200,
            is_success: true,
            tokens_prompt: Some(100),
            tokens_completion: Some(50),
            error_type: None,
            error_message: None,
        };
        tracer
            .complete_trace(complete_params)
            .await
            .expect("Failed to complete trace");

        // 验证记录存在
        let count = proxy_tracing::Entity::find()
            .filter(proxy_tracing::Column::RequestId.eq(&request_id))
            .count(&*db)
            .await
            .expect("Failed to count records");

        assert_eq!(count, 1, "Should have exactly one trace record");
    }
}
