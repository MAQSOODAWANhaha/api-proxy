//! # 请求转换服务
//!
//! 负责在请求发往上游前对其进行修改，包括注入认证头、改写路径/请求体、清理代理痕迹等。

use crate::error::{Context, Result, auth::AuthError};
use crate::linfo;
use crate::logging::{LogComponent, LogStage};
use crate::proxy::context::{ProxyContext, ResolvedCredential};
use crate::proxy::upstream_url::parse_base_url;
use pingora_http::RequestHeader;
use pingora_proxy::Session;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// 请求转换服务
pub struct RequestTransformService {
    db: Arc<DatabaseConnection>,
}

impl RequestTransformService {
    /// 创建新的请求转换服务
    #[must_use]
    pub const fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// 过滤并转换上游请求
    pub async fn filter_request(
        &self,
        session: &Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut ProxyContext,
    ) -> Result<()> {
        // 1. 应用 ProviderStrategy 进行早期修改
        if let Some(strategy) = ctx.routing.strategy.clone() {
            strategy
                .modify_request(session, upstream_request, ctx)
                .await?;
        }

        // 2. 覆盖 Host 头为上游地址（避免下游 Host 影响上游路由）
        Self::ensure_host_header(upstream_request, ctx)?;

        // 3. 构建并注入认证头
        Self::build_and_inject_auth_headers(upstream_request, ctx)?;

        // 4. 清理代理相关和不必要的头部
        Self::cleanup_headers(upstream_request);

        // 5. 确保必要的头部存在（如 User-Agent, Accept）
        Self::ensure_essential_headers(session, upstream_request);

        // 6. 处理 Content-Length
        Self::handle_content_length(session, upstream_request, ctx);

        linfo!(
            &ctx.request_id,
            LogStage::UpstreamRequest,
            LogComponent::RequestTransform,
            "request_transformed",
            "上游请求转换完成",
            method = upstream_request.method.to_string(),
            uri = upstream_request.uri.to_string()
        );

        Ok(())
    }

    /// 确保 Host 头为上游地址，避免下游虚拟主机路由错误
    fn ensure_host_header(upstream_request: &mut RequestHeader, ctx: &ProxyContext) -> Result<()> {
        let Some(provider) = &ctx.routing.provider_type else {
            return Ok(());
        };

        let parsed = parse_base_url(&provider.base_url)
            .with_context(|| format!("解析上游地址失败: {}", provider.base_url))?;

        let previous_host = upstream_request
            .headers
            .get("host")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();

        upstream_request.remove_header("host");
        upstream_request
            .insert_header("host", &parsed.host_header)
            .context("Failed to set upstream host header")?;

        linfo!(
            &ctx.request_id,
            LogStage::RequestModify,
            LogComponent::RequestTransform,
            "set_host_header",
            "补齐上游 Host 头",
            provider = provider.name.as_str(),
            previous_host = previous_host,
            host = %parsed.host_header,
            upstream_addr = %parsed.addr
        );

        Ok(())
    }

    /// 构建并注入上游认证头
    fn build_and_inject_auth_headers(
        upstream_request: &mut RequestHeader,
        ctx: &ProxyContext,
    ) -> Result<()> {
        let credential = ctx
            .routing
            .resolved_credential
            .as_ref()
            .ok_or(AuthError::NotAuthenticated)?;

        Self::clear_auth_headers(upstream_request);

        Self::clear_auth_headers(upstream_request);

        match credential {
            ResolvedCredential::ApiKey(api_key) => {
                let auth_headers = ctx.routing.strategy.as_ref().map_or_else(
                    || vec![("Authorization".to_string(), format!("Bearer {api_key}"))],
                    |strategy| strategy.build_auth_headers(api_key.as_str()),
                );

                for (name, value) in auth_headers {
                    upstream_request
                        .insert_header(name, &value)
                        .context("Failed to set auth header")?;
                }
            }
            ResolvedCredential::OAuthAccessToken(token) => {
                upstream_request
                    .insert_header("Authorization", format!("Bearer {token}"))
                    .context("Failed to set OAuth header")?;
            }
        }

        Ok(())
    }

    /// 清理所有可能的认证头
    fn clear_auth_headers(upstream_request: &mut RequestHeader) {
        upstream_request.remove_header("authorization");
        upstream_request.remove_header("x-goog-api-key");
        upstream_request.remove_header("x-api-key");
        upstream_request.remove_header("api-key");
    }

    /// 清理代理相关的头部
    fn cleanup_headers(upstream_request: &mut RequestHeader) {
        let headers_to_remove = [
            "x-forwarded-for",
            "x-forwarded-host",
            "x-forwarded-proto",
            "x-real-ip",
            "forwarded",
            "proxy-authorization",
            "via",
        ];
        for header in &headers_to_remove {
            upstream_request.remove_header(*header);
        }
    }

    /// 确保通用头部存在
    fn ensure_essential_headers(session: &Session, upstream_request: &mut RequestHeader) {
        if upstream_request.headers.get("user-agent").is_none() {
            let ua = session
                .req_header()
                .headers
                .get("user-agent")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("Mozilla/5.0");
            let _ = upstream_request.insert_header("user-agent", ua);
        }

        if upstream_request.headers.get("accept").is_none() {
            let _ = upstream_request.insert_header("accept", "*/*");
        }
    }

    /// 处理 Content-Length
    fn handle_content_length(
        session: &Session,
        upstream_request: &mut RequestHeader,
        ctx: &ProxyContext,
    ) {
        let is_sse = session.req_header().uri.path().contains("stream"); // Simplified check

        if ctx.request.will_modify_body || is_sse {
            upstream_request.remove_header("content-length");
        } else {
            let method = upstream_request.method.as_str();
            if (method == "POST" || method == "PUT" || method == "PATCH")
                && upstream_request.headers.get("content-length").is_none()
                && upstream_request.headers.get("transfer-encoding").is_none()
            {
                let _ = upstream_request.insert_header("content-length", "0");
            }
        }
    }
}
