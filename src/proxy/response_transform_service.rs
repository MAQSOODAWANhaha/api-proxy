//! # 响应转换服务
//!
//! 负责修改从上游返回的响应头，例如添加CORS头、移除敏感信息等。

use crate::error::{Context, Result};
use crate::linfo;
use crate::logging::{LogComponent, LogStage};
use crate::proxy::context::ProxyContext;
use pingora_http::ResponseHeader;
use pingora_proxy::Session;

/// 响应转换服务
pub struct ResponseTransformService;

impl Default for ResponseTransformService {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponseTransformService {
    /// 创建新的响应转换服务
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// 过滤并转换上游响应
    pub fn filter_response(
        &self,
        _session: &Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut ProxyContext,
    ) -> Result<()> {
        // 1. 将上游响应的关键信息记录到上下文中
        ctx.response.details.status_code = Some(upstream_response.status.as_u16());
        if let Some(ct) = upstream_response
            .headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
        {
            ctx.response.details.content_type = Some(ct.to_string());
        }
        if let Some(ce) = upstream_response
            .headers
            .get("content-encoding")
            .and_then(|v| v.to_str().ok())
        {
            ctx.response.details.content_encoding = Some(ce.to_string());
        }
        if ctx
            .response
            .details
            .content_type
            .as_deref()
            .is_some_and(Self::is_sse_content_type)
        {
            Self::apply_sse_headers(upstream_response)?;
        }

        // 2. 添加CORS头部，实现跨域支持
        Self::add_cors_headers(upstream_response)?;

        // 3. 清理可能暴露服务器信息的头部
        Self::cleanup_headers(upstream_response);

        linfo!(
            &ctx.request_id,
            LogStage::Response,
            LogComponent::ResponseTransform,
            "response_transformed",
            "上游响应转换完成",
            status = upstream_response.status.as_u16()
        );

        Ok(())
    }

    /// 添加CORS头部
    fn add_cors_headers(upstream_response: &mut ResponseHeader) -> Result<()> {
        if upstream_response
            .headers
            .get("access-control-allow-origin")
            .is_none()
        {
            Self::set_header(upstream_response, "access-control-allow-origin", "*")?;
        }
        if upstream_response
            .headers
            .get("access-control-allow-methods")
            .is_none()
        {
            Self::set_header(
                upstream_response,
                "access-control-allow-methods",
                "GET, POST, PUT, DELETE, OPTIONS",
            )?;
        }
        if upstream_response
            .headers
            .get("access-control-allow-headers")
            .is_none()
        {
            Self::set_header(
                upstream_response,
                "access-control-allow-headers",
                "Content-Type, Authorization",
            )?;
        }
        Ok(())
    }

    fn set_header(
        upstream_response: &mut ResponseHeader,
        key: &'static str,
        value: &'static str,
    ) -> Result<()> {
        upstream_response
            .insert_header(key, value)
            .context("Failed to set response header")
    }

    fn apply_sse_headers(upstream_response: &mut ResponseHeader) -> Result<()> {
        // SSE 需要流式发送，确保不使用 Content-Length 以避免长度不一致。
        upstream_response.remove_header("content-length");
        Self::ensure_cache_control_directive(upstream_response, "no-cache")?;
        Self::ensure_cache_control_directive(upstream_response, "no-transform")?;
        if upstream_response.headers.get("x-accel-buffering").is_none() {
            Self::set_header(upstream_response, "x-accel-buffering", "no")?;
        }
        if upstream_response.headers.get("connection").is_none() {
            Self::set_header(upstream_response, "connection", "keep-alive")?;
        }
        Ok(())
    }

    fn is_sse_content_type(content_type: &str) -> bool {
        content_type
            .to_ascii_lowercase()
            .contains("text/event-stream")
    }

    fn ensure_cache_control_directive(
        upstream_response: &mut ResponseHeader,
        directive: &'static str,
    ) -> Result<()> {
        if let Some(value) = upstream_response
            .headers
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
        {
            if Self::has_cache_control_directive(value, directive) {
                return Ok(());
            }
            let mut new_value = value.to_string();
            if !new_value.ends_with(',') {
                new_value.push_str(", ");
            }
            new_value.push_str(directive);
            upstream_response
                .insert_header("cache-control", new_value)
                .context("Failed to update cache-control header")?;
        } else {
            Self::set_header(upstream_response, "cache-control", directive)?;
        }
        Ok(())
    }

    fn has_cache_control_directive(value: &str, directive: &str) -> bool {
        value
            .split(',')
            .map(|item| item.trim().to_ascii_lowercase())
            .any(|item| item == directive)
    }

    /// 清理敏感或不必要的响应头
    fn cleanup_headers(upstream_response: &mut ResponseHeader) {
        upstream_response.remove_header("x-powered-by");
        upstream_response.remove_header("server"); // 也可以选择保留或替换
    }
}
