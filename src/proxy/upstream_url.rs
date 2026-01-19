//! 上游地址解析工具
//!
//! 统一处理 `base_url` 可能包含的 scheme / path / port，并输出可用于 Pingora 的 `host:port`。

use crate::ensure;
use crate::error::{Result, config::ConfigError};
use url::{Host, Url};

#[derive(Debug, Clone)]
pub(crate) struct UpstreamAddress {
    pub addr: String,
    pub host_header: String,
    pub sni: String,
}

/// 解析上游 `base_url`，输出 Peer 地址与 Host/SNI
pub(crate) fn parse_base_url(raw: &str) -> Result<UpstreamAddress> {
    let trimmed = raw.trim();
    ensure!(
        !trimmed.is_empty(),
        ConfigError::Load("base_url 不能为空".to_string())
    );

    let url = if trimmed.contains("://") {
        Url::parse(trimmed)?
    } else {
        Url::parse(&format!("https://{trimmed}"))?
    };

    let host = url
        .host()
        .ok_or_else(|| ConfigError::Load(format!("base_url 缺少 host: {trimmed}")))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| ConfigError::Load(format!("base_url 缺少端口: {trimmed}")))?;

    let (host_display, sni) = match host {
        Host::Domain(domain) => (domain.to_string(), domain.to_string()),
        Host::Ipv4(ip) => (ip.to_string(), ip.to_string()),
        Host::Ipv6(ip) => (format!("[{ip}]"), ip.to_string()),
    };

    let addr = format!("{host_display}:{port}");
    let host_header = if url.port().is_some() {
        format!("{host_display}:{port}")
    } else {
        host_display
    };

    Ok(UpstreamAddress {
        addr,
        host_header,
        sni,
    })
}
