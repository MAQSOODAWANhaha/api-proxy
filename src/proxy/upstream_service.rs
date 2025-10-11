//! # 上游服务模块
//!
//! 负责所有与上游节点（Peer）相关的逻辑，包括根据服务商策略选择地址和配置连接参数。

use crate::error::Result;
use crate::linfo;
use crate::logging::{LogComponent, LogStage};
use crate::proxy::context::ProxyContext;
use crate::proxy_err;
use pingora_core::upstreams::peer::{ALPN, HttpPeer, Peer};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use std::time::Duration;

/// 上游服务
pub struct UpstreamService {
    db: Arc<DatabaseConnection>,
}

impl UpstreamService {
    /// 创建新的上游服务
    #[must_use]
    pub const fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// 选择上游对等体
    pub async fn select_peer(&self, ctx: &ProxyContext) -> Result<Box<HttpPeer>> {
        let provider_type = ctx
            .provider_type
            .as_ref()
            .ok_or_else(|| proxy_err!(internal, "Provider type not set in context"))?;

        // 优先由 ProviderStrategy 决定上游地址
        let mut upstream_addr: Option<String> = None;
        if let Some(strategy) = &ctx.strategy
            && let Ok(Some(host)) = strategy.select_upstream_host(ctx).await
        {
            upstream_addr = Some(if host.contains(':') {
                host
            } else {
                format!("{host}:443")
            });
        }

        // 回退：使用 provider_types.base_url
        let final_addr = upstream_addr.unwrap_or_else(|| {
            if provider_type.base_url.contains(':') {
                provider_type.base_url.clone()
            } else {
                format!("{}:443", provider_type.base_url)
            }
        });

        linfo!(
            &ctx.request_id,
            LogStage::UpstreamRequest,
            LogComponent::Upstream,
            "upstream_peer_selected",
            "上游节点选择完成",
            upstream = final_addr,
            provider = provider_type.name,
            provider_url = provider_type.base_url
        );

        let sni = final_addr
            .split(':')
            .next()
            .unwrap_or(&final_addr)
            .to_string();
        let mut peer = HttpPeer::new(&final_addr, true, sni);

        let timeout = ctx.timeout_seconds.unwrap_or(30) as u64;
        let total_timeout_secs = timeout + 5;
        let read_timeout_secs = timeout * 2;

        if let Some(options) = peer.get_mut_peer_options() {
            options.alpn = ALPN::H2H1;
            options.connection_timeout = Some(Duration::from_secs(timeout));
            options.total_connection_timeout = Some(Duration::from_secs(total_timeout_secs));
            options.read_timeout = Some(Duration::from_secs(read_timeout_secs));
            options.write_timeout = Some(Duration::from_secs(read_timeout_secs));
            options.h2_ping_interval = Some(Duration::from_secs(30));
            options.max_h2_streams = 100;
        }

        linfo!(
            &ctx.request_id,
            LogStage::UpstreamRequest,
            LogComponent::Upstream,
            "peer_options_configured",
            "配置通用peer选项（动态超时）",
            provider = provider_type.name,
            timeout = timeout,
        );

        Ok(Box::new(peer))
    }
}
