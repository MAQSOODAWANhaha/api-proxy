//! Claude 提供商策略
//!
//! 处理 Claude API 特有的逻辑，包括 client ID 替换以保护隐私

use super::ProviderStrategy;
use crate::error::{Context, Result, config::ConfigError};
use crate::key_pool::ApiKeyHealthService;
use crate::proxy::ProxyContext;
use crate::{
    ldebug, linfo,
    logging::{LogComponent, LogStage},
    lwarn,
};
use pingora_http::RequestHeader;
use pingora_proxy::Session;
use regex::Regex;
use std::sync::Arc;

/// Claude 策略实现
///
/// 主要功能：
/// 1. 从数据库配置动态获取上游地址
/// 2. 替换 `metadata.user_id` 中的 client ID 以保护隐私
/// 3. 设置 Claude 特定的请求头
pub struct ClaudeStrategy {
    health_checker: Option<Arc<ApiKeyHealthService>>,
    unified_client_id: String,
}

impl ClaudeStrategy {
    #[must_use]
    pub fn new(health_checker: Option<Arc<ApiKeyHealthService>>) -> Self {
        Self {
            health_checker,
            unified_client_id: "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456"
                .to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ProviderStrategy for ClaudeStrategy {
    fn name(&self) -> &'static str {
        "anthropic" // 统一使用 anthropic，与数据库中的名称一致
    }

    async fn select_upstream_host(&self, ctx: &ProxyContext) -> Result<Option<String>> {
        ctx.provider_type.as_ref().map_or_else(
            || {
                lwarn!(
                    &ctx.request_id,
                    LogStage::RequestModify,
                    LogComponent::ClaudeStrategy,
                    "no_provider_config",
                    "未找到提供商配置，无法确定上游地址"
                );
                Ok(None)
            },
            |provider| {
                linfo!(
                    &ctx.request_id,
                    LogStage::RequestModify,
                    LogComponent::ClaudeStrategy,
                    "using_provider_base_url",
                    "使用数据库配置的BaseUrl",
                    base_url = %provider.base_url
                );
                Ok(Some(provider.base_url.clone()))
            },
        )
    }

    async fn modify_request(
        &self,
        session: &Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut ProxyContext,
    ) -> Result<()> {
        // 从配置获取 Host 头
        let host = if let Some(provider) = &ctx.provider_type {
            provider.base_url.clone()
        } else {
            return Err(ConfigError::Load(
                "No provider configuration found for Claude".to_string(),
            )
            .into());
        };

        upstream_request
            .insert_header("host", &host)
            .context("Failed to set host header for Claude")?;

        // 检测是否为需要修改 body 的请求
        let path = session.req_header().uri.path();
        let need_body_modify = path.contains("/v1/messages") || path.contains("/v1/complete");

        if need_body_modify {
            ctx.will_modify_body = true;
        }

        linfo!(
            &ctx.request_id,
            LogStage::RequestModify,
            LogComponent::ClaudeStrategy,
            "request_modified",
            "Claude请求修改完成",
            host = %host,
            path = path,
            will_modify_body = ctx.will_modify_body
        );

        Ok(())
    }

    async fn modify_request_body_json(
        &self,
        _session: &Session,
        ctx: &ProxyContext,
        json_value: &mut serde_json::Value,
    ) -> Result<bool> {
        let modified = replace_client_id(json_value, &self.unified_client_id);

        if modified {
            linfo!(
                &ctx.request_id,
                LogStage::RequestModify,
                LogComponent::ClaudeStrategy,
                "client_id_replaced",
                "成功替换metadata中的client ID",
                unified_client_id = &self.unified_client_id
            );
        } else {
            ldebug!(
                &ctx.request_id,
                LogStage::RequestModify,
                LogComponent::ClaudeStrategy,
                "client_id_not_replaced",
                "未找到需要替换的client ID或格式不匹配"
            );
        }

        Ok(modified)
    }

    fn build_auth_headers(&self, api_key: &str) -> Vec<(String, String)> {
        vec![("Authorization".to_string(), format!("Bearer {api_key}"))]
    }
}

/// 替换 `metadata.user_id` 中的 client ID
///
/// 基于 claude-relay-service 的逻辑：
/// - 匹配模式：user_{`CLIENT_ID`}_`account__session`_{UUID}
/// - 替换为：user_{unifiedClientId}_`account__session`_{UUID}
/// - 保留 session UUID 部分不变
fn replace_client_id(json_value: &mut serde_json::Value, unified_client_id: &str) -> bool {
    if let Some(metadata) = json_value
        .get_mut("metadata")
        .and_then(|m| m.as_object_mut())
        && let Some(user_id) = metadata.get_mut("user_id").and_then(|u| u.as_str())
    {
        let re = Regex::new(r"^user_[a-f0-9]{64}(_account__session_[a-f0-9-]{36})$").unwrap();
        if let Some(captures) = re.captures(user_id)
            && let Some(session_suffix) = captures.get(1)
        {
            let new_user_id = format!("user_{}{}", unified_client_id, session_suffix.as_str());
            metadata.insert(
                "user_id".to_string(),
                serde_json::Value::String(new_user_id),
            );
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::ProxyContext;
    use sea_orm::prelude::DateTime;
    use serde_json::json;

    fn dummy_claude_provider() -> entity::provider_types::Model {
        let now: DateTime = chrono::Utc::now().naive_utc();
        entity::provider_types::Model {
            id: 3,
            name: "anthropic".to_string(),
            display_name: "Anthropic Claude".to_string(),
            base_url: "api.anthropic.com".to_string(),
            is_active: true,
            config_json: Some(r#"{"request_stage":{"required_headers":{"anthropic-version":"2023-06-01"}},"response_stage":{}}"#.to_string()),
            token_mappings_json: Some(r#"{"tokens_prompt":{"type":"direct","path":"usage.input_tokens","fallback":{"type":"direct","path":"usage.prompt_tokens"}},"tokens_completion":{"type":"direct","path":"usage.output_tokens","fallback":{"type":"direct","path":"usage.completion_tokens"}},"tokens_total":{"type":"expression","formula":"usage.total_tokens","fallback":{"type":"expression","formula":"usage.input_tokens + usage.output_tokens"}},"cache_create_tokens":{"type":"direct","path":"usage.cache_creation_input_tokens","fallback":{"type":"direct","path":"usage.prompt_tokens_details.cached_tokens"}},"cache_read_tokens":{"type":"direct","path":"usage.cache_read_input_tokens","fallback":{"type":"direct","path":"usage.cached_tokens"}}}"#.to_string()),
            model_extraction_json: Some(r#"{"extraction_rules":[{"type":"body_json","path":"model","priority":1,"description":"从请求body提取模型名"}],"fallback_model":"claude-4-sonnet"}"#.to_string()),
            auth_type: "api_key".to_string(),
            auth_configs_json: Some(r"{}".to_string()),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn test_replace_client_id_success() {
        let mut json = serde_json::json!({
            "metadata": {
                "user_id": "user_1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef_account__session_550e8400-e29b-41d4-a716-446655440000"
            }
        });

        let unified_id = "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456";
        let result = replace_client_id(&mut json, unified_id);
        assert!(result);
        assert_eq!(
            json["metadata"]["user_id"],
            "user_a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456_account__session_550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn test_replace_client_id_no_match() {
        let mut json = serde_json::json!({
            "metadata": {
                "user_id": "invalid_format"
            }
        });

        let unified_id = "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456";
        let result = replace_client_id(&mut json, unified_id);
        assert!(!result);
        assert_eq!(json["metadata"]["user_id"], "invalid_format");
    }

    #[test]
    fn test_replace_client_id_no_metadata() {
        let mut json = serde_json::json!({
            "model": "claude-3.5-sonnet",
            "messages": []
        });

        let unified_id = "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456";
        let result = replace_client_id(&mut json, unified_id);
        assert!(!result);
    }

    #[test]
    fn test_replace_client_id_no_user_id() {
        let mut json = serde_json::json!({
            "metadata": {
                "session_id": "test-session"
            }
        });

        let unified_id = "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456";
        let result = replace_client_id(&mut json, unified_id);
        assert!(!result);
    }

    #[test]
    fn test_replace_client_id_various_formats() {
        let test_cases = vec![
            // 标准格式
            (
                "user_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef_account__session_550e8400-e29b-41d4-a716-446655440000",
                true,
            ),
            // 另一个有效的格式
            (
                "user_fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210_account__session_123e4567-e89b-12d3-a456-426614174000",
                true,
            ),
            // 无效格式 - 长度不对
            (
                "user_123_account__session_550e8400-e29b-41d4-a716-446655440000",
                false,
            ),
            // 无效格式 - 缺少session部分
            (
                "user_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                false,
            ),
            // 无效格式 - 不是小写字母数字
            (
                "user_ABCDEF1234567890ABCDEF1234567890ABCDEF1234567890ABCDEF1234567890_account__session_550e8400-e29b-41d4-a716-446655440000",
                false,
            ),
        ];

        let unified_id = "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456";

        for (input, expected) in test_cases {
            let mut json = serde_json::json!({
                "metadata": {
                    "user_id": input
                }
            });

            let result = replace_client_id(&mut json, unified_id);
            assert_eq!(result, expected, "Failed for input: {input}");

            if expected {
                // 验证替换后的格式正确
                let new_user_id = json["metadata"]["user_id"].as_str().unwrap();
                assert!(new_user_id.starts_with(&format!("user_{unified_id}")));
                assert!(new_user_id.contains("_account__session_"));
            }
        }
    }

    #[tokio::test]
    async fn test_select_upstream_host_from_config() {
        let strategy = ClaudeStrategy::new(None);
        let mut ctx = ProxyContext {
            request_id: "test-request-123".to_string(),
            ..Default::default()
        };

        let provider = dummy_claude_provider();
        ctx.provider_type = Some(provider);

        let host = strategy.select_upstream_host(&ctx).await.unwrap();
        assert_eq!(host.as_deref(), Some("api.anthropic.com"));
    }

    #[tokio::test]
    async fn test_select_upstream_host_no_config() {
        let strategy = ClaudeStrategy::new(None);
        let ctx = ProxyContext {
            request_id: "test-request-456".to_string(),
            ..Default::default()
        };

        let host = strategy.select_upstream_host(&ctx).await.unwrap();
        assert!(host.is_none());
    }

    #[test]
    fn test_claude_strategy_default() {
        let strategy = ClaudeStrategy::new(None);
        assert_eq!(strategy.name(), "anthropic"); // 更新为 anthropic
        assert_eq!(
            strategy.unified_client_id,
            "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456"
        );
    }

    #[test]
    fn test_build_auth_headers() {
        let strategy = ClaudeStrategy::new(None);
        let headers = strategy.build_auth_headers("sk-test-key-123");

        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].0, "Authorization");
        assert_eq!(headers[0].1, "Bearer sk-test-key-123");
    }

    // ==================== 集成测试 ====================

    #[tokio::test]
    async fn test_claude_strategy_full_integration() {
        // 创建 Claude 策略
        let strategy = ClaudeStrategy::new(None);

        // 创建代理上下文
        let ctx = ProxyContext {
            request_id: "test-integration-123".to_string(),
            provider_type: Some(dummy_claude_provider()),
            ..Default::default()
        };

        // 1. 测试 select_upstream_host
        let host = strategy.select_upstream_host(&ctx).await.unwrap();
        assert_eq!(host.as_deref(), Some("api.anthropic.com"));

        // 2. 直接测试核心 client ID 替换逻辑
        let test_body = json!({
            "model": "claude-3.5-sonnet",
            "max_tokens": 1000,
            "metadata": {
                "user_id": "user_1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef_account__session_550e8400-e29b-41d4-a716-446655440000"
            },
            "messages": [
                {
                    "role": "user",
                    "content": "Hello, Claude!"
                }
            ]
        });

        let mut json_value = test_body;
        let modified = replace_client_id(&mut json_value, &strategy.unified_client_id);

        // 验证 client ID 被正确替换
        assert!(modified);
        let new_user_id = json_value["metadata"]["user_id"].as_str().unwrap();
        assert!(
            new_user_id.starts_with(
                "user_a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456"
            )
        );
        assert!(new_user_id.ends_with("_account__session_550e8400-e29b-41d4-a716-446655440000"));

        // 3. 测试认证头构建
        let auth_headers = strategy.build_auth_headers("sk-test-key-123");
        assert_eq!(auth_headers.len(), 1);
        assert_eq!(auth_headers[0].0, "Authorization");
        assert_eq!(auth_headers[0].1, "Bearer sk-test-key-123");
    }

    #[tokio::test]
    async fn test_claude_strategy_no_provider_config() {
        // 测试没有提供商配置的情况

        let strategy = ClaudeStrategy::new(None);
        let ctx = ProxyContext {
            request_id: "test-no-provider-789".to_string(),
            ..Default::default()
        };
        // 故意不设置 provider_type

        let host = strategy.select_upstream_host(&ctx).await.unwrap();
        assert!(host.is_none());
    }

    #[tokio::test]
    async fn test_claude_strategy_with_real_database_config() {
        // 测试使用真实数据库配置的情况

        let strategy = ClaudeStrategy::new(None);
        let mut ctx = ProxyContext {
            request_id: "test-real-config-456".to_string(),
            ..Default::default()
        };

        // 使用模拟的真实提供商配置
        let real_provider = dummy_claude_provider();
        ctx.provider_type = Some(real_provider);

        // 验证从配置中获取的 URL
        let host = strategy.select_upstream_host(&ctx).await.unwrap();
        assert_eq!(host.as_deref(), Some("api.anthropic.com"));

        // 验证策略名称
        assert_eq!(strategy.name(), "anthropic");

        // 验证 unified_client_id
        assert_eq!(
            strategy.unified_client_id,
            "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456"
        );
    }
}
