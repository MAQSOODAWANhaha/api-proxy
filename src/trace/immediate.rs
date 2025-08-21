//! # 即时写入追踪器
//!
//! 解决长时间请求的内存占用和数据丢失问题，采用即时数据库写入模式

use anyhow::Result;
use chrono::Utc;
use entity::proxy_tracing;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, NotSet, QueryFilter, QuerySelect, Set,
};
use std::sync::Arc;
use tracing::{debug, error, info};

/// 即时写入追踪器配置（已简化）
#[derive(Debug, Clone)]
pub struct ImmediateTracerConfig {}

impl Default for ImmediateTracerConfig {
    fn default() -> Self {
        Self {}
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
    pub fn new(db: Arc<DatabaseConnection>, _config: ImmediateTracerConfig) -> Self {
        info!("Initializing immediate proxy tracer with all requests traced");

        Self {
            config: ImmediateTracerConfig::default(),
            db,
        }
    }

    /// 开始追踪请求 - 立即写入数据库
    pub async fn start_trace(
        &self,
        request_id: String,
        user_service_api_id: i32,
        user_id: Option<i32>,
        method: String,
        path: Option<String>,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<()> {
        // 强制追踪所有请求，移除配置开关

        let now = Utc::now().naive_utc();

        // 创建初始追踪记录
        let trace_record = proxy_tracing::ActiveModel {
            id: NotSet,
            user_service_api_id: Set(user_service_api_id),
            user_provider_key_id: Set(None), // 稍后可更新
            user_id: Set(user_id),
            request_id: Set(request_id.clone()),
            method: Set(method),
            path: Set(path),
            client_ip: Set(client_ip),
            user_agent: Set(user_agent),
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
            provider_type_id: NotSet,
            end_time: NotSet,
            duration_ms: NotSet,
        };

        // 立即写入数据库
        let insert_result = proxy_tracing::Entity::insert(trace_record)
            .exec(&*self.db)
            .await?;

        debug!(
            request_id = %request_id,
            trace_id = insert_result.last_insert_id,
            user_id = ?user_id,
            "Started simplified proxy trace"
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
        let mut update_model = proxy_tracing::ActiveModel {
            ..Default::default()
        };

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
        provider_type_id: Option<i32>,
        model_used: Option<String>,
        user_provider_key_id: Option<i32>,
    ) -> Result<()> {
        // 强制追踪所有请求，移除配置开关

        // 构建更新模型
        let mut update_model = proxy_tracing::ActiveModel {
            ..Default::default()
        };

        let mut updated_fields = Vec::new();

        if let Some(provider_id) = provider_type_id {
            update_model.provider_type_id = Set(Some(provider_id));
            updated_fields.push(format!("provider_type_id={}", provider_id));
        }
        if let Some(model) = model_used {
            update_model.model_used = Set(Some(model.clone()));
            updated_fields.push(format!("model_used={}", model));
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
        tokens_prompt: Option<u32>,
        tokens_completion: Option<u32>,
        error_type: Option<String>,
        error_message: Option<String>,
    ) -> Result<()> {
        self.complete_trace_with_stats(
            request_id,
            status_code,
            is_success,
            tokens_prompt,
            tokens_completion,
            error_type,
            error_message,
            None, // cache_create_tokens
            None, // cache_read_tokens
            None, // cost
            None, // cost_currency
        )
        .await
    }

    /// 完成追踪 - 使用TraceStats
    pub async fn complete_trace_with_stats(
        &self,
        request_id: &str,
        status_code: u16,
        is_success: bool,
        tokens_prompt: Option<u32>,
        tokens_completion: Option<u32>,
        error_type: Option<String>,
        error_message: Option<String>,
        cache_create_tokens: Option<u32>,
        cache_read_tokens: Option<u32>,
        cost: Option<f64>,
        cost_currency: Option<String>,
    ) -> Result<()> {
        // 强制追踪所有请求，移除配置开关

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
            status_code: Set(Some(status_code as i32)),
            is_success: Set(is_success),
            end_time: Set(Some(end_time)),
            duration_ms: Set(duration_ms),
            tokens_prompt: Set(tokens_prompt.map(|t| t as i32)),
            tokens_completion: Set(tokens_completion.map(|t| t as i32)),
            tokens_total: Set(tokens_total.map(|t| t as i32)),
            token_efficiency_ratio: Set(token_efficiency_ratio),
            cache_create_tokens: Set(cache_create_tokens.map(|t| t as i32)),
            cache_read_tokens: Set(cache_read_tokens.map(|t| t as i32)),
            cost: Set(cost),
            cost_currency: Set(cost_currency),
            error_type: Set(error_type),
            error_message: Set(error_message),
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
                cost = ?cost,
                cache_create_tokens = ?cache_create_tokens,
                cache_read_tokens = ?cache_read_tokens,
                duration_ms = ?duration_ms,
                rows_affected = update_result.rows_affected,
                "Completed trace with comprehensive stats"
            );
        } else {
            error!(
                request_id = %request_id,
                "Failed to complete trace - no matching record found"
            );
        }

        Ok(())
    }

    /// 使用TraceStats完成追踪
    pub async fn complete_trace_with_trace_stats(
        &self,
        request_id: &str,
        status_code: u16,
        is_success: bool,
        trace_stats: crate::providers::TraceStats,
    ) -> Result<()> {
        self.complete_trace_with_stats(
            request_id,
            status_code,
            is_success,
            trace_stats.input_tokens,
            trace_stats.output_tokens,
            trace_stats.error_type,
            trace_stats.error_message,
            trace_stats.cache_create_tokens,
            trace_stats.cache_read_tokens,
            trace_stats.cost,
            trace_stats.cost_currency,
        )
        .await
    }

    /// 完成追踪 - 简化版本（已废弃，推荐使用complete_trace_with_stats）
    #[deprecated(note = "Use complete_trace_with_stats instead")]
    pub async fn complete_trace_with_details(
        &self,
        request_id: &str,
        status_code: u16,
        is_success: bool,
        tokens_prompt: Option<u32>,
        tokens_completion: Option<u32>,
        error_type: Option<String>,
        error_message: Option<String>,
        _request_details: Option<serde_json::Value>,
        _response_details: Option<serde_json::Value>,
    ) -> Result<()> {
        // 重定向到简化版本
        self.complete_trace_with_stats(
            request_id,
            status_code,
            is_success,
            tokens_prompt,
            tokens_completion,
            error_type,
            error_message,
            None, // cache_create_tokens
            None, // cache_read_tokens
            None, // cost
            None, // cost_currency
        )
        .await
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
    use sea_orm::{Database, EntityTrait, PaginatorTrait, Set};
    use std::sync::Arc;
    use migration::{Migrator, MigratorTrait};
    use chrono::Utc;

    async fn setup_test_db() -> Arc<DatabaseConnection> {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to test database");
        Migrator::up(&db, None).await.expect("Failed to run migrations");
        Arc::new(db)
    }

    #[tokio::test]
    async fn test_immediate_trace_lifecycle() {
        let db = setup_test_db().await;
        let config = ImmediateTracerConfig::default();
        let tracer = ImmediateProxyTracer::new(db.clone(), config);

        // Insert a user record
        let user = entity::users::ActiveModel {
            id: Set(1),
            username: Set("testuser".to_string()),
            password_hash: Set("...".to_string()),
            email: Set("test@test.com".to_string()),
            salt: Set("salt".to_string()),
            is_admin: Set(false),
            is_active: Set(true),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };
        entity::users::Entity::insert(user).exec(&*db).await.unwrap();

        // Insert a user_service_api record
        let user_service_api = entity::user_service_apis::ActiveModel {
            id: Set(1),
            user_id: Set(1),
            name: Set("Test API".to_string()),
            is_active: Set(true),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };
        entity::user_service_apis::Entity::insert(user_service_api).exec(&*db).await.unwrap();

        let request_id = "test_immediate_12345".to_string();

        // 开始追踪
        tracer
            .start_trace(
                request_id.clone(),
                1,
                Some(1), // user_id
                "POST".to_string(),
                Some("/v1/chat/completions".to_string()),
                Some("127.0.0.1".to_string()),
                Some("test-client/1.0".to_string()),
            )
            .await
            .expect("Failed to start trace");

        // 更新中间信息
        tracer
            .update_trace_info(&request_id, Some("gpt-4".to_string()))
            .await
            .expect("Failed to update trace info");

        // 完成追踪
        tracer
            .complete_trace(&request_id, 200, true, Some(100), Some(50), None, None)
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