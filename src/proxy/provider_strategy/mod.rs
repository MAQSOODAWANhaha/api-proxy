//! 提供商特定策略（Proxy 层）的最小骨架
//!
//! 目的：将 Gemini / OpenAI 等特殊改写从 RequestHandler 中抽离为可插拔策略，
//! 避免核心处理器越来越臃肿。当前仅提供接口与 Gemini 示例占位，不改变现有行为。

use std::sync::Arc;
use self::provider_strategy_gemini::GeminiStrategy;

pub mod provider_strategy_gemini;

use pingora_http::RequestHeader;
use pingora_proxy::Session;

use crate::error::ProxyError;
use crate::proxy::ProxyContext;

#[async_trait::async_trait]
pub trait ProviderStrategy: Send + Sync {
    /// 策略名称（provider 标识）
    fn name(&self) -> &'static str;

    /// 可选：根据上下文选择上游主机（host:port）。返回 None 表示使用默认逻辑
    async fn select_upstream_host(&self, _ctx: &ProxyContext) -> Result<Option<String>, ProxyError> {
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
}

/// 简单注册表（进程内静态）
pub struct ProviderRegistry;

impl ProviderRegistry {
    pub fn match_name(provider_name: &str) -> Option<&'static str> {
        let p = provider_name.to_ascii_lowercase();
        if p.contains("gemini") { Some("gemini") }
        else if p.contains("openai") { Some("openai") }
        else { None }
    }
}

// 预留：将来可切换为动态注册（HashMap<&'static str, Arc<dyn ProviderStrategy>>）
// 这里先提供一个工厂方法，避免无谓的全局可变状态。
pub fn make_strategy(name: &str) -> Option<Arc<dyn ProviderStrategy>> {
    match name {
        "gemini" => Some(Arc::new(GeminiStrategy::default())),
        _ => None,
    }
}
