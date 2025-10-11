//! Gemini 提供商策略（占位实现）
//!
//! 说明：当前仅做最小无害改写示例（如补充少量兼容性 Header），
//! 实际的路径/JSON 注入逻辑仍留在 RequestHandler，后续再迁移。

use super::ProviderStrategy;
use crate::error::Result;
use crate::proxy::ProxyContext;
use crate::proxy_err;
use crate::{
    ldebug, linfo,
    logging::{self, LogComponent, LogStage},
};
use pingora_http::RequestHeader;
use pingora_proxy::Session;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
enum GeminiProxyMode {
    OAuthWithoutProject,
    OAuthWithProject(String),
    ApiKey,
}

impl GeminiProxyMode {
    const fn upstream_host(&self) -> &'static str {
        match self {
            Self::OAuthWithoutProject => "cloudcode-pa.googleapis.com",
            Self::OAuthWithProject(_) => "cloudcode-pa.googleapis.com",
            Self::ApiKey => "generativelanguage.googleapis.com",
        }
    }
}

#[derive(Default)]
pub struct GeminiStrategy {
    db: Option<Arc<DatabaseConnection>>,
}


#[async_trait::async_trait]
impl ProviderStrategy for GeminiStrategy {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn set_db_connection(&mut self, db: Option<Arc<DatabaseConnection>>) {
        self.db = db;
    }

    async fn select_upstream_host(
        &self,
        ctx: &crate::proxy::ProxyContext,
    ) -> Result<Option<String>> {
        let Some(backend) = &ctx.selected_backend else {
            return Ok(None);
        };
        let mode = match backend.auth_type.as_str() {
            "oauth" => {
                if let Some(pid) = &backend.project_id {
                    if pid.is_empty() {
                        GeminiProxyMode::OAuthWithoutProject
                    } else {
                        GeminiProxyMode::OAuthWithProject(pid.clone())
                    }
                } else {
                    GeminiProxyMode::OAuthWithoutProject
                }
            }
            "api_key" => GeminiProxyMode::ApiKey,
            _ => GeminiProxyMode::ApiKey,
        };
        Ok(Some(mode.upstream_host().to_string()))
    }

    async fn modify_request(
        &self,
        session: &Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut ProxyContext,
    ) -> Result<()> {
        // 设置正确的Host头 - 复用 select_upstream_host 的逻辑
        if let Ok(Some(host)) = self.select_upstream_host(ctx).await {
            if let Err(e) = upstream_request.insert_header("host", &host) {
                return Err(proxy_err!(
                    internal,
                    "Failed to set host header for Gemini: {}",
                    e
                ));
            }

            linfo!(
                &ctx.request_id,
                LogStage::RequestModify,
                LogComponent::GeminiStrategy,
                "set_host_header",
                "Set correct Host header for Gemini provider",
                request_headers = logging::headers_json_string_request(upstream_request)
            );
        }

        // 判断是否需要后续 JSON 注入（在 body filter 里执行）
        if let Some(backend) = &ctx.selected_backend
            && backend.auth_type.as_str() == "oauth"
                && let Some(pid) = &backend.project_id
                    && !pid.is_empty() {
                        let path = session.req_header().uri.path();
                        let need = path.contains("streamGenerateContent")
                            || path.contains("generateContent");
                        ctx.will_modify_body = need;
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

        // 使用 will_modify_body 判断是否需要注入，仅当存在真实的project_id时才执行注入
        let modified = if ctx.will_modify_body {
            if let Some(project_id) = &backend.project_id {
                if project_id.is_empty() {
                    false
                } else {
                    inject_generatecontent_fields(json_value, project_id)
                }
            } else {
                false
            }
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

fn inject_generatecontent_fields(json_value: &mut serde_json::Value, project_id: &str) -> bool {
    if let Some(obj) = json_value.as_object_mut() {
        // 根据Google Gemini CLI实现，正确的格式是：
        // {
        //   "model": "gemini-2.5-flash",
        //   "project": "project-id", // 可以为空字符串
        //   "user_prompt_id": "uuid",
        //   "request": {
        //     "contents": [...],
        //     "generationConfig": {...}
        //   }
        // }

        // 设置 project 字段：insert 方法会自动处理插入新值或替换已存在值
        obj.insert(
            "project".to_string(),
            serde_json::Value::String(project_id.to_string()),
        );

        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{auth::AuthStatus, proxy::ProxyContext};
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

    #[tokio::test]
    async fn test_select_upstream_host_oauth_with_project() {
        let strat = GeminiStrategy::default();
        let ctx = ProxyContext {
            selected_backend: Some(dummy_key("oauth", Some("proj-123"))),
            ..Default::default()
        };
        let host = strat.select_upstream_host(&ctx).await.unwrap();
        assert_eq!(host.as_deref(), Some("cloudcode-pa.googleapis.com"));
    }

    #[tokio::test]
    async fn test_select_upstream_host_api_key() {
        let strat = GeminiStrategy::default();
        let ctx = ProxyContext {
            selected_backend: Some(dummy_key("api_key", None)),
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
