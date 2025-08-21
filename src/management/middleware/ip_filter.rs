//! # IP访问控制中间件
//!
//! 提供基于IP地址的访问控制功能

use axum::{
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use ipnetwork::IpNetwork;
use std::net::{IpAddr, SocketAddr};
use tracing::{debug, warn};

/// IP访问控制配置
#[derive(Debug, Clone)]
pub struct IpFilterConfig {
    /// 允许的IP地址/网段列表
    pub allowed_ips: Vec<IpNetwork>,
    /// 拒绝的IP地址/网段列表
    pub denied_ips: Vec<IpNetwork>,
}

impl IpFilterConfig {
    /// 从字符串列表创建配置
    pub fn from_strings(
        allowed: &[String],
        denied: &[String],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut allowed_ips = Vec::new();
        let mut denied_ips = Vec::new();

        // 解析允许的IP列表
        for ip_str in allowed {
            match ip_str.parse::<IpNetwork>() {
                Ok(network) => allowed_ips.push(network),
                Err(e) => {
                    warn!("Failed to parse allowed IP '{}': {}", ip_str, e);
                    // 尝试解析为单个IP地址
                    if let Ok(ip) = ip_str.parse::<IpAddr>() {
                        let network = match ip {
                            IpAddr::V4(ipv4) => {
                                IpNetwork::V4(ipnetwork::Ipv4Network::new(ipv4, 32)?)
                            }
                            IpAddr::V6(ipv6) => {
                                IpNetwork::V6(ipnetwork::Ipv6Network::new(ipv6, 128)?)
                            }
                        };
                        allowed_ips.push(network);
                    }
                }
            }
        }

        // 解析拒绝的IP列表
        for ip_str in denied {
            match ip_str.parse::<IpNetwork>() {
                Ok(network) => denied_ips.push(network),
                Err(e) => {
                    warn!("Failed to parse denied IP '{}': {}", ip_str, e);
                    if let Ok(ip) = ip_str.parse::<IpAddr>() {
                        let network = match ip {
                            IpAddr::V4(ipv4) => {
                                IpNetwork::V4(ipnetwork::Ipv4Network::new(ipv4, 32)?)
                            }
                            IpAddr::V6(ipv6) => {
                                IpNetwork::V6(ipnetwork::Ipv6Network::new(ipv6, 128)?)
                            }
                        };
                        denied_ips.push(network);
                    }
                }
            }
        }

        Ok(Self {
            allowed_ips,
            denied_ips,
        })
    }

    /// 检查IP是否被允许访问
    pub fn is_allowed(&self, ip: IpAddr) -> bool {
        // 首先检查是否在拒绝列表中
        for denied_network in &self.denied_ips {
            if denied_network.contains(ip) {
                debug!("IP {} is in denied list", ip);
                return false;
            }
        }

        // 如果允许列表为空，默认允许所有
        if self.allowed_ips.is_empty() {
            return true;
        }

        // 检查是否在允许列表中
        for allowed_network in &self.allowed_ips {
            if allowed_network.contains(ip) {
                debug!("IP {} is in allowed list", ip);
                return true;
            }
        }

        debug!("IP {} is not in allowed list", ip);
        false
    }
}

/// IP访问控制中间件
pub async fn ip_filter_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let client_ip = addr.ip();

    // 从请求扩展中获取IP过滤配置
    let config = request.extensions().get::<IpFilterConfig>().cloned();

    if let Some(config) = config {
        if !config.is_allowed(client_ip) {
            warn!("Access denied for IP: {}", client_ip);
            return Err(StatusCode::FORBIDDEN);
        }
    }

    debug!("Access allowed for IP: {}", client_ip);
    Ok(next.run(request).await)
}

/// 获取真实客户端IP地址（考虑代理情况）
pub fn get_real_client_ip(request: &Request) -> Option<IpAddr> {
    // 尝试从 X-Forwarded-For 头获取
    if let Some(forwarded_for) = request.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded_for.to_str() {
            // X-Forwarded-For 可能包含多个IP，取第一个
            if let Some(first_ip) = forwarded_str.split(',').next() {
                if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                    return Some(ip);
                }
            }
        }
    }

    // 尝试从 X-Real-IP 头获取
    if let Some(real_ip) = request.headers().get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                return Some(ip);
            }
        }
    }

    // 从连接信息获取
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|connect_info| connect_info.0.ip())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_ip_filter_config() {
        let allowed = vec!["192.168.1.0/24".to_string(), "10.0.0.1".to_string()];
        let denied = vec!["192.168.1.100".to_string()];

        let config = IpFilterConfig::from_strings(&allowed, &denied).unwrap();

        // 测试允许的IP
        assert!(config.is_allowed(Ipv4Addr::new(192, 168, 1, 1).into()));
        assert!(config.is_allowed(Ipv4Addr::new(10, 0, 0, 1).into()));

        // 测试拒绝的IP
        assert!(!config.is_allowed(Ipv4Addr::new(192, 168, 1, 100).into()));

        // 测试不在列表中的IP
        assert!(!config.is_allowed(Ipv4Addr::new(192, 168, 2, 1).into()));
    }
}
