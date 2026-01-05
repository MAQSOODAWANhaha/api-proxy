//! Gemini 提供商策略（占位实现）
//!
//! 说明：当前仅做最小无害改写示例（如补充少量兼容性 Header），
//! 实际的路径/JSON 注入逻辑仍留在 RequestHandler，后续再迁移。

use super::ProviderStrategy;
use crate::error::{Context, Result};
use crate::proxy::ProxyContext;
use crate::{
    ldebug, linfo,
    logging::{LogComponent, LogStage},
};
use pingora_http::RequestHeader;
use pingora_proxy::Session;

use crate::key_pool::ApiKeyHealthService;
use std::sync::Arc;

#[derive(Default)]
pub struct GeminiStrategy {
    health_checker: Option<Arc<ApiKeyHealthService>>,
}

impl GeminiStrategy {
    #[must_use]
    pub const fn new(health_checker: Option<Arc<ApiKeyHealthService>>) -> Self {
        Self { health_checker }
    }
}

#[async_trait::async_trait]
impl ProviderStrategy for GeminiStrategy {
    fn name(&self) -> &'static str {
        "gemini"
    }

    async fn select_upstream_host(
        &self,
        ctx: &crate::proxy::ProxyContext,
    ) -> Result<Option<String>> {
        ctx.provider_type
            .as_ref()
            .map_or_else(|| Ok(None), |provider| Ok(Some(provider.base_url.clone())))
    }

    async fn modify_request(
        &self,
        session: &Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut ProxyContext,
    ) -> Result<()> {
        // 设置正确的 Host 头（使用数据库配置的 base_url）
        if let Ok(Some(host)) = self.select_upstream_host(ctx).await {
            upstream_request
                .insert_header("host", &host)
                .context("Failed to set host header for Gemini")?;

            linfo!(
                &ctx.request_id,
                LogStage::RequestModify,
                LogComponent::GeminiStrategy,
                "set_host_header",
                "已设置 Gemini 上游 Host 头",
                host = %host,
                uri = %upstream_request.uri
            );
        }

        // 判断是否需要后续 JSON 注入（在 body filter 里执行）
        if let Some(backend) = &ctx.selected_backend
            && backend.auth_type.as_str() == "oauth"
            && let Some(pid) = &backend.project_id
            && !pid.is_empty()
        {
            let path = session.req_header().uri.path();
            let need_generate =
                path.contains("streamGenerateContent") || path.contains("generateContent");
            let need_load = path.contains("loadCodeAssist");
            ctx.will_modify_body = need_generate || need_load;
        }
        Ok(())
    }

    async fn modify_request_body_json(
        &self,
        session: &Session,
        ctx: &ProxyContext,
        json_value: &mut serde_json::Value,
    ) -> Result<bool> {
        let Some(backend) = &ctx.selected_backend else {
            return Ok(false);
        };

        // 仅处理 OAuth 认证
        if backend.auth_type.as_str() != "oauth" {
            return Ok(false);
        }

        let request_path = session.req_header().uri.path();

        // loadCodeAssist 请求需要补充 Project 与默认的元数据字段
        if request_path.contains("loadCodeAssist") {
            let modified = backend.project_id.as_ref().is_some_and(|project_id| {
                !project_id.is_empty() && inject_load_code_assist_fields(json_value, project_id)
            });

            if modified {
                linfo!(
                    &ctx.request_id,
                    LogStage::RequestModify,
                    LogComponent::GeminiStrategy,
                    "inject_load_code_assist_body",
                    "Gemini 策略为 loadCodeAssist 请求补充项目及元数据字段",
                    backend_project_id = backend.project_id.as_deref().unwrap_or("<none>"),
                    route_path = request_path
                );
            }

            return Ok(modified);
        }

        // 使用 will_modify_body 判断是否需要注入，仅当存在真实的project_id时才执行注入
        let modified = if ctx.will_modify_body {
            backend.project_id.as_ref().is_some_and(|project_id| {
                !project_id.is_empty() && inject_generatecontent_fields(json_value, project_id)
            })
        } else {
            false
        };

        if modified {
            linfo!(
                &ctx.request_id,
                LogStage::RequestModify,
                LogComponent::GeminiStrategy,
                "smart_project_id_selected",
                "Gemini策略智能选择项目ID并注入到请求中",
                backend_project_id = backend.project_id.as_deref().unwrap_or("<none>"),
                route_path = request_path
            );
        }

        Ok(modified)
    }

    fn build_auth_headers(&self, api_key: &str) -> Vec<(String, String)> {
        // Gemini支持两种认证方式
        let auth_headers = vec![
            ("Authorization".to_string(), format!("Bearer {api_key}")),
            ("X-goog-api-key".to_string(), api_key.to_string()),
        ];

        ldebug!(
            "system",
            LogStage::Authentication,
            LogComponent::GeminiStrategy,
            "build_auth_headers",
            "Generated Gemini-specific authentication headers",
            provider_name = "gemini",
            generated_headers = format!(
                "{:?}",
                auth_headers
                    .iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>()
            )
        );

        auth_headers
    }
}

// ---------------- 注入帮助函数（取自原 RequestHandler 逻辑，简化后无副作用） ----------------

fn overwrite_string_field(json_value: &mut serde_json::Value, key_path: &str, value: &str) -> bool {
    use serde_json::{Map, Value};

    let mut cursor = match json_value.as_object_mut() {
        Some(obj) => Value::Object(std::mem::take(obj)),
        None => return false,
    };

    let mut keys = key_path.split('.').peekable();
    let mut current = &mut cursor;

    while let Some(key) = keys.next() {
        let is_last = keys.peek().is_none();

        match current {
            Value::Object(obj) if is_last => {
                obj.insert(key.to_string(), Value::String(value.to_string()));
            }
            Value::Object(obj) => {
                let entry = obj
                    .entry(key.to_string())
                    .or_insert_with(|| Value::Object(Map::new()));
                if !entry.is_object() {
                    *entry = Value::Object(Map::new());
                }
                current = entry;
            }
            _ => return false,
        }
    }

    if let Value::Object(updated) = cursor {
        *json_value = Value::Object(updated);
        return true;
    }

    false
}

fn inject_generatecontent_fields(json_value: &mut serde_json::Value, project_id: &str) -> bool {
    overwrite_string_field(json_value, "project", project_id)
}

fn inject_load_code_assist_fields(json_value: &mut serde_json::Value, project_id: &str) -> bool {
    let set_project = overwrite_string_field(json_value, "cloudaicompanionProject", project_id);
    let set_duet = overwrite_string_field(json_value, "metadata.duetProject", project_id);
    set_project || set_duet
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{auth::types::AuthStatus, proxy::ProxyContext};
    use entity::provider_types;
    use entity::user_provider_keys;
    use sea_orm::prelude::DateTime;

    fn dummy_key(auth_type: &str, project_id: Option<&str>) -> user_provider_keys::Model {
        let now: DateTime = chrono::Utc::now().naive_utc();
        user_provider_keys::Model {
            id: 1,
            user_id: 1,
            provider_type_id: 1,
            api_key: "sk-test".to_string(),
            auth_type: auth_type.to_string(),
            name: "key1".to_string(),
            weight: Some(1),
            max_requests_per_minute: Some(1000),
            max_tokens_prompt_per_minute: Some(100_000),
            max_requests_per_day: Some(100_000),
            is_active: true,
            health_status: "healthy".to_string(),
            health_status_detail: None,
            rate_limit_resets_at: None,
            last_error_time: None,
            auth_status: Some(AuthStatus::Authorized.to_string()),
            expires_at: None,
            last_auth_check: Some(now),
            project_id: project_id.map(std::string::ToString::to_string),
            created_at: now,
            updated_at: now,
        }
    }

    fn dummy_provider(base_url: &str, auth_type: &str) -> provider_types::Model {
        let now: DateTime = chrono::Utc::now().naive_utc();
        provider_types::Model {
            id: 1,
            name: "gemini".to_string(),
            display_name: "Google Gemini".to_string(),
            auth_type: auth_type.to_string(),
            base_url: base_url.to_string(),
            is_active: true,
            config_json: None,
            token_mappings_json: None,
            model_extraction_json: None,
            auth_configs_json: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn test_select_upstream_host_oauth_with_project() {
        let strat = GeminiStrategy::new(None);
        let ctx = ProxyContext {
            selected_backend: Some(dummy_key("oauth", Some("proj-123"))),
            provider_type: Some(dummy_provider("cloudcode-pa.googleapis.com", "oauth")),
            ..Default::default()
        };
        let host = strat.select_upstream_host(&ctx).await.unwrap();
        assert_eq!(host.as_deref(), Some("cloudcode-pa.googleapis.com"));
    }

    #[tokio::test]
    async fn test_select_upstream_host_api_key() {
        let strat = GeminiStrategy::new(None);
        let ctx = ProxyContext {
            selected_backend: Some(dummy_key("api_key", None)),
            provider_type: Some(dummy_provider(
                "generativelanguage.googleapis.com",
                "api_key",
            )),
            ..Default::default()
        };
        let host = strat.select_upstream_host(&ctx).await.unwrap();
        assert_eq!(host.as_deref(), Some("generativelanguage.googleapis.com"));
    }

    #[test]
    fn test_inject_generatecontent_fields() {
        let mut v = serde_json::json!({});
        let changed = inject_generatecontent_fields(&mut v, "p");
        assert!(changed);
        assert_eq!(v["project"], "p");
    }

    #[test]
    fn test_inject_generatecontent_fields_with_empty_project() {
        let mut v = serde_json::json!({});
        let changed = inject_generatecontent_fields(&mut v, "");
        assert!(changed);
        assert_eq!(v["project"], "");
    }

    #[tokio::test]
    async fn test_smart_project_id_selection_with_real_project() {
        let mut ctx = ProxyContext {
            request_id: "test-request-123".to_string(),
            ..Default::default()
        };

        // 创建一个有真实project_id的backend
        let backend = dummy_key("oauth", Some("my-real-project-123"));
        ctx.selected_backend = Some(backend);

        // 测试JSON数据
        let mut json_value =
            serde_json::json!({ "contents": [{"role": "user", "parts": [{"text": "Hello"}]}] });

        // 直接测试核心逻辑：智能项目ID选择
        let effective_project_id = ctx
            .selected_backend
            .as_ref()
            .and_then(|b| b.project_id.as_deref())
            .unwrap_or("");

        assert_eq!(effective_project_id, "my-real-project-123");

        // 测试注入函数
        let result = inject_generatecontent_fields(&mut json_value, effective_project_id);
        assert!(result);
        assert_eq!(json_value["project"], "my-real-project-123");
    }

    #[tokio::test]
    async fn test_smart_project_id_selection_with_empty_project() {
        let mut ctx = ProxyContext {
            request_id: "test-request-456".to_string(),
            ..Default::default()
        };

        // 创建一个project_id为空字符串的backend
        let mut backend = dummy_key("oauth", None);
        backend.project_id = Some(String::new()); // 显式设置为空字符串
        ctx.selected_backend = Some(backend);

        // 测试JSON数据
        let mut json_value =
            serde_json::json!({ "contents": [{"role": "user", "parts": [{"text": "Hello"}]}] });

        // 直接测试核心逻辑：智能项目ID选择
        let effective_project_id = ctx
            .selected_backend
            .as_ref()
            .and_then(|b| b.project_id.as_deref())
            .unwrap_or("");

        assert_eq!(effective_project_id, "");

        // 测试注入函数
        let result = inject_generatecontent_fields(&mut json_value, effective_project_id);
        assert!(result);
        assert_eq!(json_value["project"], "");
    }

    #[tokio::test]
    async fn test_smart_project_id_selection_with_no_project() {
        let mut ctx = ProxyContext {
            request_id: "test-request-789".to_string(),
            ..Default::default()
        };

        // 创建一个没有project_id的backend
        let backend = dummy_key("oauth", None);
        ctx.selected_backend = Some(backend);

        // 测试JSON数据
        let mut json_value =
            serde_json::json!({ "contents": [{"role": "user", "parts": [{"text": "Hello"}]}] });

        // 直接测试核心逻辑：智能项目ID选择
        let effective_project_id = ctx
            .selected_backend
            .as_ref()
            .and_then(|b| b.project_id.as_deref())
            .unwrap_or("");

        assert_eq!(effective_project_id, "");

        // 测试注入函数
        let result = inject_generatecontent_fields(&mut json_value, effective_project_id);
        assert!(result);
        assert_eq!(json_value["project"], "");
    }

    #[test]
    fn test_inject_generatecontent_fields_with_real_gemini_structure() {
        // 测试真实的 Gemini API 请求结构
        let mut json_value = serde_json::json!({
            "model": "gemini-2.5-flash",
            "contents": [
                {
                    "role": "user",
                    "parts": [{"text": "Hello, how are you?"}]
                }
            ],
            "generationConfig": {
                "temperature": 0.7,
                "maxOutputTokens": 1000
            }
        });

        let project_id = "my-gemini-project";
        let result = inject_generatecontent_fields(&mut json_value, project_id);

        assert!(result);
        assert_eq!(json_value["project"], project_id);
        assert_eq!(json_value["model"], "gemini-2.5-flash");
        assert!(json_value["contents"].is_array());
        assert!(json_value["generationConfig"].is_object());

        // 验证注入后的结构符合 Gemini API 要求
        assert!(json_value.as_object().unwrap().contains_key("project"));
        assert!(json_value.as_object().unwrap().contains_key("model"));
        assert!(json_value.as_object().unwrap().contains_key("contents"));
    }

    #[test]
    fn test_inject_generatecontent_fields_existing_project_replacement() {
        // 测试 project 字段已存在时的替换逻辑
        let mut json_value = serde_json::json!({
            "model": "gemini-2.5-flash",
            "project": "old-project-id",
            "contents": [
                {
                    "role": "user",
                    "parts": [{"text": "Hello, how are you?"}]
                }
            ]
        });

        let new_project_id = "new-gemini-project";
        let result = inject_generatecontent_fields(&mut json_value, new_project_id);

        assert!(result);
        assert_eq!(json_value["project"], new_project_id);
        assert_eq!(json_value["model"], "gemini-2.5-flash");
        assert!(json_value["contents"].is_array());

        // 验证 project 字段被正确替换
        assert_ne!(json_value["project"], "old-project-id");
    }

    #[test]
    fn test_inject_generatecontent_fields_no_existing_project() {
        // 测试 project 字段不存在时的添加逻辑
        let mut json_value = serde_json::json!({
            "model": "gemini-2.5-flash",
            "contents": [
                {
                    "role": "user",
                    "parts": [{"text": "Hello, how are you?"}]
                }
            ]
        });

        let project_id = "test-gemini-project";
        let result = inject_generatecontent_fields(&mut json_value, project_id);

        assert!(result);
        assert_eq!(json_value["project"], project_id);
        assert_eq!(json_value["model"], "gemini-2.5-flash");
        assert!(json_value["contents"].is_array());

        // 验证 project 字段被正确添加
        assert!(json_value.as_object().unwrap().contains_key("project"));
    }

    #[test]
    fn test_inject_load_code_assist_fields_with_minimal_body() {
        let mut json_value = serde_json::json!({});

        let changed = inject_load_code_assist_fields(&mut json_value, "project-123");

        assert!(changed);
        let obj = json_value.as_object().unwrap();
        assert_eq!(obj["cloudaicompanionProject"], "project-123");

        let metadata = obj["metadata"].as_object().unwrap();
        assert_eq!(metadata["duetProject"], "project-123");
        assert_eq!(metadata.len(), 1);
    }

    #[test]
    fn test_inject_load_code_assist_fields_overrides_different_values() {
        let mut json_value = serde_json::json!({
            "cloudaicompanionProject": "old-project",
            "metadata": {
                "duetProject": "legacy",
                "otherField": "keep-me"
            }
        });

        let changed = inject_load_code_assist_fields(&mut json_value, "project-456");

        assert!(changed);
        let obj = json_value.as_object().unwrap();
        assert_eq!(obj["cloudaicompanionProject"], "project-456");

        let metadata = obj["metadata"].as_object().unwrap();
        assert_eq!(metadata["duetProject"], "project-456");
        assert_eq!(metadata["otherField"], "keep-me");
    }

    #[test]
    fn test_smart_project_id_selection_logic() {
        // 测试核心的项目ID选择逻辑
        let backend_with_project = dummy_key("oauth", Some("test-project"));
        let backend_empty_project = {
            let mut b = dummy_key("oauth", None);
            b.project_id = Some(String::new());
            b
        };
        let backend_no_project = dummy_key("oauth", None);

        // 测试有真实project_id
        let effective_id = backend_with_project.project_id.as_deref().unwrap_or("");
        assert_eq!(effective_id, "test-project");

        // 测试空字符串project_id
        let effective_id = backend_empty_project.project_id.as_deref().unwrap_or("");
        assert_eq!(effective_id, "");

        // 测试没有project_id
        let effective_id = backend_no_project.project_id.as_deref().unwrap_or("");
        assert_eq!(effective_id, "");
    }
}
