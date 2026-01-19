//! # 上游服务模块
//!
//! 负责所有与上游节点（Peer）相关的逻辑，包括根据服务商策略选择地址和配置连接参数。

use crate::error::{Context, Result, config::ConfigError};
use crate::linfo;
use crate::logging::{LogComponent, LogStage};
use crate::proxy::context::ProxyContext;
use crate::proxy::upstream_url::parse_base_url;
use pingora_core::protocols::TcpKeepalive;
use pingora_core::upstreams::peer::{ALPN, HttpPeer, Peer};
use sea_orm::DatabaseConnection;
use std::convert::TryFrom;
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
            .routing
            .provider_type
            .as_ref()
            .ok_or_else(|| ConfigError::Load("Provider type not set in context".to_string()))?;

        // 优先由 ProviderStrategy 决定上游地址
        let upstream_addr = if let Some(strategy) = &ctx.routing.strategy {
            match strategy.select_upstream_host(ctx).await {
                Ok(Some(host)) => Some(host),
                _ => None,
            }
        } else {
            None
        };

        // 回退：使用 provider_types.base_url
        let final_raw = upstream_addr.unwrap_or_else(|| provider_type.base_url.clone());
        let parsed =
            parse_base_url(&final_raw).with_context(|| format!("解析上游地址失败: {final_raw}"))?;

        linfo!(
            &ctx.request_id,
            LogStage::UpstreamRequest,
            LogComponent::Upstream,
            "upstream_peer_selected",
            "上游节点选择完成",
            upstream = parsed.addr,
            upstream_raw = final_raw,
            host_header = parsed.host_header,
            provider = provider_type.name,
            provider_url = provider_type.base_url
        );

        let mut peer = HttpPeer::new(&parsed.addr, true, parsed.sni.clone());

        let timeout = u64::try_from(ctx.control.timeout_seconds.unwrap_or(30).max(0)).unwrap_or(30);
        let read_timeout_secs = timeout * 2;

        if let Some(options) = peer.get_mut_peer_options() {
            options.alpn = ALPN::H2H1;
            // [优化] 连接建立应该快速失败，不要等待业务超时
            options.connection_timeout = Some(Duration::from_secs(6)); // TCP握手超时
            options.total_connection_timeout = Some(Duration::from_secs(10)); // 含TLS握手超时
            options.read_timeout = Some(Duration::from_secs(read_timeout_secs));
            options.write_timeout = Some(Duration::from_secs(read_timeout_secs));
            options.idle_timeout = Some(Duration::from_secs(20));
            options.h2_ping_interval = Some(Duration::from_secs(20));
            options.max_h2_streams = 100;
            // 启用 TCP Keepalive，防止长连接在无数据传输时被中间设备断开
            options.tcp_keepalive = Some(TcpKeepalive {
                idle: Duration::from_secs(20),
                interval: Duration::from_secs(5),
                count: 5,
                user_timeout: Duration::from_secs(timeout),
            });
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
