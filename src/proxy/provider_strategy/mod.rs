//! 提供商特定策略（Proxy 层）的最小骨架
//!
//! 目的：将 Gemini / OpenAI 等特殊改写从 RequestHandler 中抽离为可插拔策略，
//! 避免核心处理器越来越臃肿。当前仅提供接口与 Gemini 示例占位，不改变现有行为。

use self::provider_strategy_gemini::GeminiStrategy;
use self::provider_strategy_openai::OpenAIStrategy;
use std::sync::Arc;

pub mod provider_strategy_gemini;
pub mod provider_strategy_openai;

use pingora_http::RequestHeader;
use pingora_proxy::Session;
use sea_orm::DatabaseConnection;

use crate::error::ProxyError;
use crate::proxy::ProxyContext;
use crate::proxy::service::ResponseBodyService;
use entity::user_provider_keys;

#[async_trait::async_trait]
pub trait ProviderStrategy: Send + Sync {
    /// 策略名称（provider 标识）
    fn name(&self) -> &'static str;

    /// 设置数据库连接
    fn set_db_connection(&mut self, _db: Option<Arc<DatabaseConnection>>) {}

    /// 可选：根据上下文选择上游主机（host:port）。返回 None 表示使用默认逻辑
    async fn select_upstream_host(
        &self,
        _ctx: &ProxyContext,
    ) -> Result<Option<String>, ProxyError> {
        Ok(None)
    }

    /// 可选：修改请求（path/query/header/body 的最小改写入口）。
    async fn modify_request(
        &self,
        _session: &Session,
        _upstream_request: &mut RequestHeader,
        _ctx: &mut ProxyContext,
    ) -> Result<(), ProxyError> {
        Ok(())
    }

    /// 可选：在需要修改 JSON 请求体时进行注入/改写
    async fn modify_request_body_json(
        &self,
        _session: &Session,
        _ctx: &ProxyContext,
        _json_value: &mut serde_json::Value,
    ) -> Result<bool, ProxyError> {
        Ok(false)
    }

    /// 可选：处理响应体，包括错误处理和状态更新
    async fn handle_response_body(
        &self,
        _session: &Session,
        _ctx: &ProxyContext,
        _status_code: u16,
        _body: &[u8],
    ) -> Result<(), ProxyError> {
        Ok(())
    }

    /// 可选：检查密钥是否应该重试使用
    async fn should_retry_key(&self, _key: &user_provider_keys::Model) -> Result<bool, ProxyError> {
        Ok(true)
    }

    /// 构建上游认证头（提供商特定的认证逻辑）
    fn build_auth_headers(&self, api_key: &str) -> Vec<(String, String)>;
}

/// 简单注册表（进程内静态）
pub struct ProviderRegistry;

impl ProviderRegistry {
    pub fn match_name(provider_name: &str) -> Option<&'static str> {
        let p = provider_name.to_ascii_lowercase();
        if p.contains("gemini") {
            Some("gemini")
        } else if p.contains("openai") {
            Some("openai")
        } else {
            None
        }
    }
}

// 预留：将来可切换为动态注册（HashMap<&'static str, Arc<dyn ProviderStrategy>>）
// 这里先提供一个工厂方法，避免无谓的全局可变状态。
pub fn make_strategy(
    name: &str,
    db: Option<Arc<DatabaseConnection>>,
) -> Option<Arc<dyn ProviderStrategy>> {
    match name {
        "gemini" => {
            let mut strategy = GeminiStrategy::default();
            strategy.set_db_connection(db.clone());
            Some(Arc::new(strategy))
        }
        "openai" => {
            let mut strategy = OpenAIStrategy::new();
            strategy.set_db_connection(db);
            Some(Arc::new(strategy))
        }
        _ => None,
    }
}

/// 为响应体阶段创建可选的服务（若该策略实现了 ResponseBodyService 则返回）
pub fn make_provider_response_body_service(
    name: &str,
    db: Option<Arc<DatabaseConnection>>,
) -> Option<Arc<dyn ResponseBodyService>> {
    match name {
        // 目前仅 OpenAI 提供响应体阶段处理（429 立即处理）；其他返回 None
        "openai" => {
            let mut strategy = OpenAIStrategy::new();
            strategy.set_db_connection(db);
            Some(Arc::new(strategy))
        }
        _ => None,
    }
}
