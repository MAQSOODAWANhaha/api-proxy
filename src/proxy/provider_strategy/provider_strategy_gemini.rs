//! Gemini 提供商策略（占位实现）
//!
//! 说明：当前仅做最小无害改写示例（如补充少量兼容性 Header），
//! 实际的路径/JSON 注入逻辑仍留在 RequestHandler，后续再迁移。

use super::ProviderStrategy;
use crate::error::ProxyError;
use crate::logging::LogComponent;
use crate::proxy::ProxyContext;
use crate::{proxy_debug, proxy_info};
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
    fn upstream_host(&self) -> &'static str {
        match self {
            Self::OAuthWithoutProject => "cloudcode-pa.googleapis.com",
            Self::OAuthWithProject(_) => "cloudcode-pa.googleapis.com",
            Self::ApiKey => "generativelanguage.googleapis.com",
        }
    }
}

pub struct GeminiStrategy {
    db: Option<Arc<DatabaseConnection>>,
}

impl Default for GeminiStrategy {
    fn default() -> Self {
        Self { db: None }
    }
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
    ) -> Result<Option<String>, ProxyError> {
        let Some(backend) = &ctx.selected_backend else {
            return Ok(None);
        };
        let mode = match backend.auth_type.as_str() {
            "oauth" => {
                if let Some(pid) = &backend.project_id {
                    if !pid.is_empty() {
                        GeminiProxyMode::OAuthWithProject(pid.clone())
                    } else {
                        GeminiProxyMode::OAuthWithoutProject
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
        _upstream_request: &mut RequestHeader,
        ctx: &mut ProxyContext,
    ) -> Result<(), ProxyError> {
        // 判断是否需要后续 JSON 注入（在 body filter 里执行）
        if let Some(backend) = &ctx.selected_backend {
            if backend.auth_type.as_str() == "oauth" {
                if let Some(pid) = &backend.project_id {
                    if !pid.is_empty() {
                        let path = session.req_header().uri.path();
                        let need = path.contains("loadCodeAssist")
                            || path.contains("onboardUser")
                            || path.contains("countTokens")
                            || path.contains("streamGenerateContent")
                            || (path.contains("generateContent")
                                && !path.contains("streamGenerateContent"));
                        ctx.will_modify_body = need;
                    }
                }
            }
        }
        Ok(())
    }

    async fn modify_request_body_json(
        &self,
        session: &Session,
        ctx: &ProxyContext,
        json_value: &mut serde_json::Value,
    ) -> Result<bool, ProxyError> {
        let Some(backend) = &ctx.selected_backend else {
            return Ok(false);
        };

        // 仅处理 OAuth 认证
        if backend.auth_type.as_str() != "oauth" {
            return Ok(false);
        }

        let request_path = session.req_header().uri.path();

        // 智能项目ID选择逻辑：优先使用数据库中的project_id，没有则使用空字符串
        let effective_project_id = backend.project_id.as_deref().unwrap_or("");

        let modified = if request_path.contains("loadCodeAssist") {
            inject_loadcodeassist_fields(json_value, effective_project_id, &ctx.request_id)
        } else if request_path.contains("onboardUser") {
            inject_onboarduser_fields(json_value, effective_project_id, &ctx.request_id)
        } else if request_path.contains("countTokens") {
            inject_counttokens_fields(json_value, &ctx.request_id)
        } else if request_path.contains("generateContent") {
            inject_generatecontent_fields(json_value, effective_project_id)
        } else {
            false
        };

        if modified {
            proxy_info!(
                &ctx.request_id,
                LogStage::RequestModify,
                LogComponent::GeminiStrategy,
                "smart_project_id_selected",
                "Gemini策略智能选择项目ID并注入到请求中",
                backend_project_id = backend.project_id.as_deref().unwrap_or("<none>"),
                effective_project_id = if effective_project_id.is_empty() {
                    "<empty>"
                } else {
                    effective_project_id
                },
                route_path = request_path
            );
        }

        Ok(modified)
    }

    fn build_auth_headers(&self, api_key: &str) -> Vec<(String, String)> {
        // Gemini支持两种认证方式
        let auth_headers = vec![
            ("Authorization".to_string(), format!("Bearer {}", api_key)),
            ("X-goog-api-key".to_string(), api_key.to_string()),
        ];

        tracing::debug!(
            provider_name = "gemini",
            generated_headers = format!(
                "{:?}",
                auth_headers
                    .iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>()
            ),
            "Generated Gemini-specific authentication headers"
        );

        auth_headers
    }
}

// ---------------- 项目字段处理函数 ----------------

/// 移除项目相关字段
/// 当backend没有project_id时调用此函数
pub fn remove_project_fields(
    json_value: &mut serde_json::Value,
    request_id: &str,
    route_path: &str,
) -> bool {
    let mut modified = false;

    if let Some(obj) = json_value.as_object_mut() {
        // 移除顶层的project字段
        if obj.remove("project").is_some() {
            modified = true;
            proxy_debug!(
                request_id,
                LogStage::RequestModify,
                LogComponent::GeminiStrategy,
                "project_field_removed",
                "移除顶层project字段",
                route_path = route_path
            );
        }

        // 移除cloudaicompanionProject字段
        if obj.remove("cloudaicompanionProject").is_some() {
            modified = true;
            proxy_debug!(
                request_id,
                LogStage::RequestModify,
                LogComponent::GeminiStrategy,
                "cloudaicompanion_project_field_removed",
                "移除cloudaicompanionProject字段",
                route_path = route_path
            );
        }

        // 移除metadata中的duetProject字段
        if let Some(metadata) = obj.get_mut("metadata").and_then(|v| v.as_object_mut()) {
            if metadata.remove("duetProject").is_some() {
                modified = true;
                proxy_debug!(
                    request_id,
                    LogStage::RequestModify,
                    LogComponent::GeminiStrategy,
                    "duet_project_field_removed",
                    "移除metadata.duetProject字段",
                    route_path = route_path
                );
            }
        }

        // 对于countTokens，保持标准格式但移除project相关字段
        if route_path.contains("countTokens") {
            if let Some(request) = obj.get_mut("request").and_then(|v| v.as_object_mut()) {
                // 确保request对象存在但移除project字段（如果有）
                if request.remove("project").is_some() {
                    modified = true;
                    proxy_debug!(
                        request_id,
                        LogStage::RequestModify,
                        LogComponent::GeminiStrategy,
                        "request_project_field_removed",
                        "移除request.project字段",
                        route_path = route_path
                    );
                }
            }
        }
    }

    if modified {
        proxy_info!(
            request_id,
            LogStage::RequestModify,
            LogComponent::GeminiStrategy,
            "project_fields_removed",
            "Gemini策略移除了所有项目相关字段",
            route_path = route_path
        );
    }

    modified
}

// ---------------- 注入帮助函数（取自原 RequestHandler 逻辑，简化后无副作用） ----------------

pub fn inject_loadcodeassist_fields(
    json_value: &mut serde_json::Value,
    project_id: &str,
    request_id: &str,
) -> bool {
    if let Some(obj) = json_value.as_object_mut() {
        let metadata = obj
            .entry("metadata")
            .or_insert_with(|| serde_json::json!({}));
        if let Some(metadata_obj) = metadata.as_object_mut() {
            metadata_obj.insert(
                "duetProject".to_string(),
                serde_json::Value::String(project_id.to_owned()),
            );
            proxy_debug!(
                request_id,
                LogStage::RequestModify,
                LogComponent::GeminiStrategy,
                "duet_project_injected",
                "注入metadata.duetProject",
                project_id = project_id,
                location = "metadata.duetProject"
            );
        }
        obj.insert(
            "cloudaicompanionProject".to_string(),
            serde_json::Value::String(project_id.to_owned()),
        );
        return true;
    }
    false
}

fn inject_onboarduser_fields(
    json_value: &mut serde_json::Value,
    project_id: &str,
    request_id: &str,
) -> bool {
    if let Some(obj) = json_value.as_object_mut() {
        obj.insert(
            "cloudaicompanionProject".to_string(),
            serde_json::Value::String(project_id.to_owned()),
        );
        proxy_debug!(
            request_id,
            LogStage::RequestModify,
            LogComponent::GeminiStrategy,
            "cloudaicompanion_project_injected_onboard",
            "注入cloudaicompanionProject (onboardUser)",
            project_id = project_id,
            location = "top_level"
        );
        return true;
    }
    false
}

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

        // 设置 project 字段：可以为真实 project_id 或空字符串
        obj.insert(
            "project".to_string(),
            serde_json::Value::String(project_id.to_string()),
        );

        return true;
    }
    false
}

fn inject_counttokens_fields(json_value: &mut serde_json::Value, request_id: &str) -> bool {
    let mut modified = false;
    if let Some(root) = json_value.as_object_mut() {
        let mut request_obj = if let Some(request_val) = root.get_mut("request") {
            if let Some(obj) = request_val.as_object_mut() {
                obj.clone()
            } else {
                serde_json::Map::new()
            }
        } else {
            serde_json::Map::new()
        };

        if let Some(model_val) = request_obj
            .get("model")
            .and_then(|v| v.as_str())
            .or_else(|| root.get("model").and_then(|v| v.as_str()))
        {
            let model_str = if model_val.starts_with("models/") {
                model_val.to_string()
            } else {
                format!("models/{}", model_val)
            };
            request_obj.insert("model".to_string(), serde_json::Value::String(model_str));
            modified = true;
        }

        if let Some(contents_val) = request_obj
            .get("contents")
            .cloned()
            .or_else(|| root.get("contents").cloned())
        {
            request_obj.insert("contents".to_string(), contents_val);
            modified = true;
        }

        root.insert(
            "request".to_string(),
            serde_json::Value::Object(request_obj),
        );
    }

    if modified {
        proxy_info!(
            request_id,
            LogStage::RequestModify,
            LogComponent::GeminiStrategy,
            "count_tokens_standardized",
            "标准化countTokens请求体结构",
            action = "standardize_count_tokens",
            result = "wrapped_in_request_object"
        );
    }
    modified
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
            max_tokens_prompt_per_minute: Some(100000),
            max_requests_per_day: Some(100000),
            is_active: true,
            health_status: "healthy".to_string(),
            health_status_detail: None,
            rate_limit_resets_at: None,
            last_error_time: None,
            auth_status: Some(AuthStatus::Authorized.to_string()),
            expires_at: None,
            last_auth_check: Some(now),
            project_id: project_id.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn test_select_upstream_host_oauth_with_project() {
        let strat = GeminiStrategy::default();
        let mut ctx = ProxyContext::default();
        ctx.selected_backend = Some(dummy_key("oauth", Some("proj-123")));
        let host = strat.select_upstream_host(&ctx).await.unwrap();
        assert_eq!(host.as_deref(), Some("cloudcode-pa.googleapis.com"));
    }

    #[tokio::test]
    async fn test_select_upstream_host_api_key() {
        let strat = GeminiStrategy::default();
        let mut ctx = ProxyContext::default();
        ctx.selected_backend = Some(dummy_key("api_key", None));
        let host = strat.select_upstream_host(&ctx).await.unwrap();
        assert_eq!(host.as_deref(), Some("generativelanguage.googleapis.com"));
    }

    #[test]
    fn test_inject_loadcodeassist_fields() {
        let mut v = serde_json::json!({});
        let changed = inject_loadcodeassist_fields(&mut v, "p", "req");
        assert!(changed);
        assert_eq!(v["metadata"]["duetProject"], "p");
        assert_eq!(v["cloudaicompanionProject"], "p");
    }

    #[test]
    fn test_inject_onboarduser_fields() {
        let mut v = serde_json::json!({});
        let changed = inject_onboarduser_fields(&mut v, "p", "req");
        assert!(changed);
        assert_eq!(v["cloudaicompanionProject"], "p");
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

    #[test]
    fn test_inject_counttokens_fields() {
        let mut v = serde_json::json!({
            "model": "g1",
            "contents": [ {"role":"user","parts":[{"text":"hi"}]} ]
        });
        let changed = inject_counttokens_fields(&mut v, "req");
        assert!(changed);
        assert!(v["request"].is_object());
        assert_eq!(v["request"]["model"], "models/g1");
        assert!(v["request"]["contents"].is_array());
    }

    #[tokio::test]
    async fn test_smart_project_id_selection_with_real_project() {
        let mut ctx = ProxyContext::default();
        ctx.request_id = "test-request-123".to_string();

        // 创建一个有真实project_id的backend
        let backend = dummy_key("oauth", Some("my-real-project-123"));
        ctx.selected_backend = Some(backend);

        // 测试JSON数据
        let mut json_value = serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": "Hello"}]}]
        });

        // 直接测试核心逻辑：智能项目ID选择
        let effective_project_id = ctx
            .selected_backend.as_ref()
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
        let mut ctx = ProxyContext::default();
        ctx.request_id = "test-request-456".to_string();

        // 创建一个project_id为空字符串的backend
        let mut backend = dummy_key("oauth", None);
        backend.project_id = Some("".to_string()); // 显式设置为空字符串
        ctx.selected_backend = Some(backend);

        // 测试JSON数据
        let mut json_value = serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": "Hello"}]}]
        });

        // 直接测试核心逻辑：智能项目ID选择
        let effective_project_id = ctx
            .selected_backend.as_ref()
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
        let mut ctx = ProxyContext::default();
        ctx.request_id = "test-request-789".to_string();

        // 创建一个没有project_id的backend
        let backend = dummy_key("oauth", None);
        ctx.selected_backend = Some(backend);

        // 测试JSON数据
        let mut json_value = serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": "Hello"}]}]
        });

        // 直接测试核心逻辑：智能项目ID选择
        let effective_project_id = ctx
            .selected_backend.as_ref()
            .and_then(|b| b.project_id.as_deref())
            .unwrap_or("");

        assert_eq!(effective_project_id, "");

        // 测试注入函数
        let result = inject_generatecontent_fields(&mut json_value, effective_project_id);
        assert!(result);
        assert_eq!(json_value["project"], "");
    }

    #[test]
    fn test_smart_project_id_selection_logic() {
        // 测试核心的项目ID选择逻辑
        let backend_with_project = dummy_key("oauth", Some("test-project"));
        let backend_empty_project = {
            let mut b = dummy_key("oauth", None);
            b.project_id = Some("".to_string());
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
