//! Gemini 提供商策略（占位实现）
//!
//! 说明：当前仅做最小无害改写示例（如补充少量兼容性 Header），
//! 实际的路径/JSON 注入逻辑仍留在 RequestHandler，后续再迁移。

use pingora_http::RequestHeader;
use pingora_proxy::Session;

use crate::error::ProxyError;
use crate::logging::LogComponent;
use crate::proxy::ProxyContext;
use crate::{proxy_debug, proxy_info};

use super::ProviderStrategy;

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

#[derive(Default)]
pub struct GeminiStrategy;

#[async_trait::async_trait]
impl ProviderStrategy for GeminiStrategy {
    fn name(&self) -> &'static str {
        "gemini"
    }

    async fn select_upstream_host(
        &self,
        ctx: &crate::proxy::ProxyContext,
    ) -> Result<Option<String>, ProxyError> {
        let Some(backend) = ctx.selected_backend.as_ref() else {
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
        upstream_request: &mut RequestHeader,
        ctx: &mut ProxyContext,
    ) -> Result<(), ProxyError> {
        // 对 generateContent/streamGenerateContent 额外设置 x-goog-request-params，
        // 便于上游在未读取 body 时即可进行资源路由与校验
        if let Some(backend) = ctx.selected_backend.as_ref() {
            if backend.auth_type.as_str() == "oauth" {
                if let Some(pid) = &backend.project_id {
                    if !pid.is_empty() {
                        let path = session.req_header().uri.path();
                        let path_for_log = path.to_string();
                        if path.contains("generateContent") {
                            // 使用原始项目ID格式，不添加 projects/ 前缀
                            let header_val = format!("project={}", pid);
                            let _ =
                                upstream_request.insert_header("x-goog-request-params", &header_val);
                            proxy_info!(
                                &ctx.request_id,
                                LogStage::RequestModify,
                                LogComponent::GeminiStrategy,
                                "x_goog_request_params_added",
                                "添加 x-goog-request-params 头",
                                path = path_for_log,
                                header_val = header_val
                            );
                        } else {
                            proxy_debug!(
                                &ctx.request_id,
                                LogStage::RequestModify,
                                LogComponent::GeminiStrategy,
                                "x_goog_request_params_skipped",
                                "路径不包含generateContent，跳过添加x-goog-request-params头",
                                path = path_for_log
                            );
                        }
                    }
                }
            }
        }

        // 判断是否需要后续 JSON 注入（在 body filter 里执行）
        if let Some(backend) = ctx.selected_backend.as_ref() {
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
        // 仅当 OAuth 且有 project_id 时注入
        let Some(backend) = ctx.selected_backend.as_ref() else {
            return Ok(false);
        };
        let Some(project_id) = backend.project_id.as_ref() else {
            return Ok(false);
        };
        if backend.auth_type.as_str() != "oauth" || project_id.is_empty() {
            return Ok(false);
        }

        let request_path = session.req_header().uri.path();

        let modified = if request_path.contains("loadCodeAssist") {
            inject_loadcodeassist_fields(json_value, project_id, &ctx.request_id)
        } else if request_path.contains("onboardUser") {
            inject_onboarduser_fields(json_value, project_id, &ctx.request_id)
        } else if request_path.contains("countTokens") {
            inject_counttokens_fields(json_value, &ctx.request_id)
        } else if request_path.contains("generateContent")
            && !request_path.contains("streamGenerateContent")
        {
            inject_generatecontent_fields(json_value, project_id, &ctx.request_id)
        } else if request_path.contains("streamGenerateContent") {
            inject_generatecontent_fields(json_value, project_id, &ctx.request_id)
        } else {
            false
        };

        if modified {
            if let Ok(json_str) = serde_json::to_string_pretty(json_value) {
                proxy_info!(
                    &ctx.request_id,
                    LogStage::RequestModify,
                    LogComponent::GeminiStrategy,
                    "project_fields_injected",
                    "Gemini策略将项目字段注入到请求JSON中",
                    project_id = project_id,
                    route_path = request_path,
                    modified_json = json_str
                );
            } else {
                proxy_info!(
                    &ctx.request_id,
                    LogStage::RequestModify,
                    LogComponent::GeminiStrategy,
                    "project_fields_injected",
                    "Gemini策略将项目字段注入到请求JSON中",
                    project_id = project_id,
                    route_path = request_path
                );
            }
        }

        Ok(modified)
    }
}

// ---------------- 注入帮助函数（取自原 RequestHandler 逻辑，简化后无副作用） ----------------

fn inject_loadcodeassist_fields(
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

fn inject_generatecontent_fields(
    json_value: &mut serde_json::Value,
    project_id: &str,
    request_id: &str,
) -> bool {
    if let Some(obj) = json_value.as_object_mut() {
        // 将 project 字段添加到 request 对象内部
        if let Some(request_obj) = obj.get_mut("request").and_then(|v| v.as_object_mut()) {
            request_obj.insert(
                "project".to_string(),
                serde_json::Value::String(project_id.to_string()),
            );
            proxy_debug!(
                request_id,
                LogStage::RequestModify,
                LogComponent::GeminiStrategy,
                "project_injected_into_request",
                "将project字段注入到request对象内部",
                project_id = project_id,
                location = "request.object"
            );
            return true;
        } else {
            // 如果 request 对象不存在，在顶层添加 project 字段
            obj.insert(
                "project".to_string(),
                serde_json::Value::String(project_id.to_string()),
            );
            proxy_debug!(
                request_id,
                LogStage::RequestModify,
                LogComponent::GeminiStrategy,
                "project_injected_at_top",
                "在顶层注入project字段 (request对象不存在)",
                project_id = project_id,
                location = "top_level"
            );
            return true;
        }
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
    use crate::proxy::ProxyContext;
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
            auth_status: Some("authorized".to_string()),
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
        let changed = inject_generatecontent_fields(&mut v, "p", "req");
        assert!(changed);
        assert_eq!(v["project"], "p");
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
}
