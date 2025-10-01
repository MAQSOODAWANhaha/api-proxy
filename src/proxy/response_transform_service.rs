//! # 响应转换服务
//!
//! 负责修改从上游返回的响应头，例如添加CORS头、移除敏感信息等。

use crate::error::Result;
use crate::proxy_err;
use pingora_http::ResponseHeader;
use pingora_proxy::Session;

use crate::logging::{LogComponent, LogStage};
use crate::proxy::context::ProxyContext;
use crate::proxy_info;

/// 响应转换服务
pub struct ResponseTransformService;

impl ResponseTransformService {
    /// 创建新的响应转换服务
    pub fn new() -> Self {
        Self
    }

    /// 过滤并转换上游响应
    pub async fn filter_response(
        &self,
        _session: &Session,
        upstream_response: &mut ResponseHeader,
        ctx: &mut ProxyContext,
    ) -> Result<()> {
        // 1. 将上游响应的关键信息记录到上下文中
        ctx.response_details.status_code = Some(upstream_response.status.as_u16());
        if let Some(ct) = upstream_response
            .headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
        {
            ctx.response_details.content_type = Some(ct.to_string());
        }
        if let Some(ce) = upstream_response
            .headers
            .get("content-encoding")
            .and_then(|v| v.to_str().ok())
        {
            ctx.response_details.content_encoding = Some(ce.to_string());
        }

        // 2. 添加CORS头部，实现跨域支持
        self.add_cors_headers(upstream_response)?;

        // 3. 清理可能暴露服务器信息的头部
        self.cleanup_headers(upstream_response);

        proxy_info!(
            &ctx.request_id,
            LogStage::Response,
            LogComponent::ResponseTransformService,
            "response_transformed",
            "上游响应转换完成",
            status = upstream_response.status.as_u16()
        );

        Ok(())
    }

    /// 添加CORS头部
    fn add_cors_headers(&self, upstream_response: &mut ResponseHeader) -> Result<()> {
        if upstream_response
            .headers
            .get("access-control-allow-origin")
            .is_none()
        {
            upstream_response
                .insert_header("access-control-allow-origin", "*")
                .map_err(|e| proxy_err!(internal, "Failed to set CORS header: {}", e))?;
        }
        if upstream_response
            .headers
            .get("access-control-allow-methods")
            .is_none()
        {
            upstream_response
                .insert_header(
                    "access-control-allow-methods",
                    "GET, POST, PUT, DELETE, OPTIONS",
                )
                .map_err(|e| proxy_err!(internal, "Failed to set CORS header: {}", e))?;
        }
        if upstream_response
            .headers
            .get("access-control-allow-headers")
            .is_none()
        {
            upstream_response
                .insert_header(
                    "access-control-allow-headers",
                    "Content-Type, Authorization",
                )
                .map_err(|e| proxy_err!(internal, "Failed to set CORS header: {}", e))?;
        }
        Ok(())
    }

    /// 清理敏感或不必要的响应头
    fn cleanup_headers(&self, upstream_response: &mut ResponseHeader) {
        upstream_response.remove_header("x-powered-by");
        upstream_response.remove_header("server"); // 也可以选择保留或替换
    }
}
