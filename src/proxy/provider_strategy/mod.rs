//! 提供商特定策略（Proxy 层）的最小骨架
//!
//! 目的：将 Gemini / `OpenAI` 等特殊改写从 `RequestHandler` 中抽离为可插拔策略，
//! 避免核心处理器越来越臃肿。当前仅提供接口与 Gemini 示例占位，不改变现有行为。

use self::provider_strategy_claude::ClaudeStrategy;
use self::provider_strategy_gemini::GeminiStrategy;
use self::provider_strategy_openai::OpenAIStrategy;
use std::sync::Arc;

pub mod provider_strategy_claude;
pub mod provider_strategy_gemini;
pub mod provider_strategy_openai;

use pingora_http::RequestHeader;
use pingora_proxy::Session;
use sea_orm::DatabaseConnection;

use crate::error::Result;
use crate::proxy::ProxyContext;
use entity::user_provider_keys;

/// 提供商类型枚举
///
/// 提供类型安全的 provider 标识，避免硬编码字符串
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderType {
    OpenAI,
    Gemini,
    Anthropic, // 统一使用 Anthropic，对应数据库中的 "anthropic"
}

impl ProviderType {
    /// 从字符串解析 ProviderType（支持多种别名）
    ///
    /// 支持的别名：
    /// - `OpenAI`: "openai", "chatgpt"
    /// - Gemini: "gemini", "google"
    /// - Anthropic: "anthropic", "claude"
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            // OpenAI 及其别名
            s if s.contains("openai") || s.contains("chatgpt") => Some(Self::OpenAI),

            // Gemini 及其别名
            s if s.contains("gemini") || s.contains("google") => Some(Self::Gemini),

            // Anthropic/Claude 及其别名
            s if s.contains("anthropic") || s.contains("claude") => Some(Self::Anthropic),

            _ => None,
        }
    }

    /// 获取策略名称
    #[must_use]
    pub const fn strategy_name(&self) -> &'static str {
        match self {
            Self::OpenAI => "openai",
            Self::Gemini => "gemini",
            Self::Anthropic => "anthropic", // 统一使用 anthropic
        }
    }

    /// 获取数据库中的名称
    #[must_use]
    pub const fn db_name(&self) -> &'static str {
        match self {
            Self::OpenAI => "openai",
            Self::Gemini => "gemini",
            Self::Anthropic => "anthropic", // 与数据库一致
        }
    }
}

#[async_trait::async_trait]
pub trait ProviderStrategy: Send + Sync {
    /// 策略名称（provider 标识）
    fn name(&self) -> &'static str;

    /// 设置数据库连接
    fn set_db_connection(&mut self, _db: Option<Arc<DatabaseConnection>>) {}

    /// 可选：根据上下文选择上游主机（host:port）。返回 None 表示使用默认逻辑
    async fn select_upstream_host(&self, _ctx: &ProxyContext) -> Result<Option<String>> {
        Ok(None)
    }

    /// 可选：修改请求（path/query/header/body 的最小改写入口）。
    async fn modify_request(
        &self,
        _session: &Session,
        _upstream_request: &mut RequestHeader,
        _ctx: &mut ProxyContext,
    ) -> Result<()> {
        Ok(())
    }

    /// 可选：在需要修改 JSON 请求体时进行注入/改写
    async fn modify_request_body_json(
        &self,
        _session: &Session,
        _ctx: &ProxyContext,
        _json_value: &mut serde_json::Value,
    ) -> Result<bool> {
        Ok(false)
    }

    /// 可选：处理响应体，包括错误处理和状态更新
    async fn handle_response_body(
        &self,
        _session: &Session,
        _ctx: &ProxyContext,
        _status_code: u16,
        _body: &[u8],
    ) -> Result<()> {
        Ok(())
    }

    /// 可选：检查密钥是否应该重试使用
    async fn should_retry_key(&self, _key: &user_provider_keys::Model) -> Result<bool> {
        Ok(true)
    }

    /// 构建上游认证头（提供商特定的认证逻辑）
    fn build_auth_headers(&self, api_key: &str) -> Vec<(String, String)>;
}

/// 简单注册表（进程内静态）
pub struct ProviderRegistry;

impl ProviderRegistry {
    /// 根据提供商名称匹配对应的策略名称
    ///
    /// 使用 `ProviderType` 枚举进行类型安全的匹配
    #[must_use]
    pub fn match_name(provider_name: &str) -> Option<&'static str> {
        ProviderType::from_str(provider_name).map(|provider| provider.strategy_name())
    }
}

// 预留：将来可切换为动态注册（HashMap<&'static str, Arc<dyn ProviderStrategy>>）
// 这里先提供一个工厂方法，避免无谓的全局可变状态。
#[must_use]
pub fn make_strategy(
    name: &str,
    db: Option<Arc<DatabaseConnection>>,
) -> Option<Arc<dyn ProviderStrategy>> {
    ProviderType::from_str(name).map_or_else(
        || None,
        |provider_type| match provider_type {
            ProviderType::Gemini => {
                let mut strategy = GeminiStrategy::default();
                strategy.set_db_connection(db);
                Some(Arc::new(strategy) as Arc<dyn ProviderStrategy>)
            }
            ProviderType::OpenAI => {
                let mut strategy = OpenAIStrategy::new();
                strategy.set_db_connection(db);
                Some(Arc::new(strategy) as Arc<dyn ProviderStrategy>)
            }
            ProviderType::Anthropic => {
                let mut strategy = ClaudeStrategy::default();
                strategy.set_db_connection(db);
                Some(Arc::new(strategy) as Arc<dyn ProviderStrategy>)
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_registry_match_name() {
        // 测试 Anthropic/Claude 匹配（现在统一返回 anthropic）
        assert_eq!(ProviderRegistry::match_name("claude"), Some("anthropic"));
        assert_eq!(ProviderRegistry::match_name("anthropic"), Some("anthropic"));
        assert_eq!(ProviderRegistry::match_name("Claude"), Some("anthropic"));
        assert_eq!(ProviderRegistry::match_name("ANTHROPIC"), Some("anthropic"));
        assert_eq!(
            ProviderRegistry::match_name("claude-api"),
            Some("anthropic")
        );

        // 测试其他提供商匹配
        assert_eq!(ProviderRegistry::match_name("gemini"), Some("gemini"));
        assert_eq!(ProviderRegistry::match_name("openai"), Some("openai"));

        // 测试不匹配的情况
        assert_eq!(ProviderRegistry::match_name("unknown"), None);
        assert_eq!(
            ProviderRegistry::match_name("claude_invalid"),
            Some("anthropic")
        ); // 包含 claude
    }

    #[test]
    fn test_make_strategy() {
        // 测试创建 Anthropic 策略（使用别名 "claude"）
        let claude_strategy = make_strategy("claude", None);
        assert!(claude_strategy.is_some());
        assert_eq!(claude_strategy.unwrap().name(), "anthropic");

        // 测试创建 Anthropic 策略（使用全名 "anthropic"）
        let anthropic_strategy = make_strategy("anthropic", None);
        assert!(anthropic_strategy.is_some());
        assert_eq!(anthropic_strategy.unwrap().name(), "anthropic");

        // 测试创建其他策略
        let gemini_strategy = make_strategy("gemini", None);
        assert!(gemini_strategy.is_some());
        assert_eq!(gemini_strategy.unwrap().name(), "gemini");

        let openai_strategy = make_strategy("openai", None);
        assert!(openai_strategy.is_some());
        assert_eq!(openai_strategy.unwrap().name(), "openai");

        // 测试不存在的策略
        let unknown_strategy = make_strategy("unknown", None);
        assert!(unknown_strategy.is_none());
    }

    #[test]
    fn test_provider_type_from_str() {
        // 测试 OpenAI 及其别名
        assert_eq!(ProviderType::from_str("openai"), Some(ProviderType::OpenAI));
        assert_eq!(ProviderType::from_str("OpenAI"), Some(ProviderType::OpenAI));
        assert_eq!(
            ProviderType::from_str("chatgpt"),
            Some(ProviderType::OpenAI)
        );
        assert_eq!(
            ProviderType::from_str("ChatGPT"),
            Some(ProviderType::OpenAI)
        );
        assert_eq!(
            ProviderType::from_str("openai-api"),
            Some(ProviderType::OpenAI)
        );

        // 测试 Gemini 及其别名
        assert_eq!(ProviderType::from_str("gemini"), Some(ProviderType::Gemini));
        assert_eq!(ProviderType::from_str("Gemini"), Some(ProviderType::Gemini));
        assert_eq!(ProviderType::from_str("google"), Some(ProviderType::Gemini));
        assert_eq!(ProviderType::from_str("Google"), Some(ProviderType::Gemini));

        // 测试 Anthropic/Claude 及其别名
        assert_eq!(
            ProviderType::from_str("anthropic"),
            Some(ProviderType::Anthropic)
        );
        assert_eq!(
            ProviderType::from_str("Anthropic"),
            Some(ProviderType::Anthropic)
        );
        assert_eq!(
            ProviderType::from_str("claude"),
            Some(ProviderType::Anthropic)
        );
        assert_eq!(
            ProviderType::from_str("Claude"),
            Some(ProviderType::Anthropic)
        );
        assert_eq!(
            ProviderType::from_str("claude-api"),
            Some(ProviderType::Anthropic)
        );

        // 测试不匹配的情况
        assert_eq!(ProviderType::from_str("unknown"), None);
        assert_eq!(ProviderType::from_str("test"), None);
        assert_eq!(ProviderType::from_str(""), None);
    }

    #[test]
    fn test_provider_type_strategy_name() {
        assert_eq!(ProviderType::OpenAI.strategy_name(), "openai");
        assert_eq!(ProviderType::Gemini.strategy_name(), "gemini");
        assert_eq!(ProviderType::Anthropic.strategy_name(), "anthropic");
    }

    #[test]
    fn test_provider_type_db_name() {
        assert_eq!(ProviderType::OpenAI.db_name(), "openai");
        assert_eq!(ProviderType::Gemini.db_name(), "gemini");
        assert_eq!(ProviderType::Anthropic.db_name(), "anthropic");
    }
}
