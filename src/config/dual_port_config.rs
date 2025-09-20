//! # 双端口架构配置 - 简化版
//!
//! 仅保留核心必需的配置字段

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// 双端口服务器配置 - 简化版
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualPortServerConfig {
    /// 管理服务配置
    pub management: ManagementPortConfig,
    /// 代理服务配置  
    pub proxy: ProxyPortConfig,
    /// 全局工作线程数 (可选，默认CPU核心数)
    #[serde(default = "default_workers")]
    pub workers: usize,
}

fn default_workers() -> usize {
    num_cpus::get()
}

/// 管理端口配置 - 极简版
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagementPortConfig {
    /// HTTP 监听配置
    pub http: ListenerConfig,
    /// 访问控制 (可选，使用默认值)
    #[serde(default)]
    pub access_control: AccessControlConfig,
}

/// 代理端口配置 - 极简版
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyPortConfig {
    /// HTTP 监听配置
    pub http: ListenerConfig,
}

/// 监听器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerConfig {
    /// 监听主机
    pub host: String,
    /// 监听端口
    pub port: u16,
    /// 绑定地址（自动计算）
    #[serde(skip)]
    pub bind_addr: Option<SocketAddr>,
}

/// 访问控制配置 - 简化版（管理端肯定都是JWT认证）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControlConfig {
    /// 允许的 IP 地址范围
    pub allowed_ips: Vec<String>,
    /// 拒绝的 IP 地址范围
    pub denied_ips: Vec<String>,
}

// 已删除：LoadBalancingConfig 和 LoadBalancingStrategy
// 原因：在实际代码中未使用，完全冗余

impl Default for DualPortServerConfig {
    fn default() -> Self {
        Self {
            management: ManagementPortConfig::default(),
            proxy: ProxyPortConfig::default(),
            workers: default_workers(),
        }
    }
}

impl Default for ManagementPortConfig {
    fn default() -> Self {
        Self {
            http: ListenerConfig {
                host: "127.0.0.1".to_string(),
                port: 9090,
                bind_addr: None,
            },
            access_control: AccessControlConfig::default(),
        }
    }
}

impl Default for ProxyPortConfig {
    fn default() -> Self {
        Self {
            http: ListenerConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                bind_addr: None,
            },
        }
    }
}

impl Default for AccessControlConfig {
    fn default() -> Self {
        Self {
            allowed_ips: vec!["127.0.0.1/32".to_string(), "::1/128".to_string()],
            denied_ips: vec![],
        }
    }
}

// 已删除：LoadBalancingConfig::default() 实现
// 原因：LoadBalancingConfig 已被删除

impl ListenerConfig {
    /// 获取绑定地址
    pub fn bind_address(&self) -> std::io::Result<SocketAddr> {
        let addr = format!("{}:{}", self.host, self.port);
        addr.parse().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid address '{}': {}", addr, e),
            )
        })
    }
}

impl DualPortServerConfig {
    /// 验证配置的有效性
    pub fn validate(&self) -> Result<(), String> {
        // 检查端口冲突 - 简化版（仅检查HTTP端口）
        // 管理和代理服务现在始终启用
        let mgmt_port = self.management.http.port;
        let proxy_port = self.proxy.http.port;

        if mgmt_port == proxy_port {
            return Err(format!(
                "Management port ({}) conflicts with proxy port ({})",
                mgmt_port, proxy_port
            ));
        }

        // 检查工作线程数
        if self.workers == 0 {
            return Err("Worker count must be greater than 0".to_string());
        }

        // 验证监听配置 - 简化版（仅验证HTTP）
        self.management
            .http
            .bind_address()
            .map_err(|e| format!("Invalid management HTTP address: {}", e))?;

        self.proxy
            .http
            .bind_address()
            .map_err(|e| format!("Invalid proxy HTTP address: {}", e))?;

        Ok(())
    }

    /// 获取所有监听地址 - 简化版（仅HTTP）
    pub fn get_all_listeners(&self) -> Vec<(String, SocketAddr, String)> {
        let mut listeners = Vec::new();

        // 管理和代理服务现在始终启用
        if let Ok(addr) = self.management.http.bind_address() {
            listeners.push(("management-http".to_string(), addr, "HTTP".to_string()));
        }

        if let Ok(addr) = self.proxy.http.bind_address() {
            listeners.push(("proxy-http".to_string(), addr, "HTTP".to_string()));
        }

        listeners
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dual_port_config_default() {
        let config = DualPortServerConfig::default();

        assert_eq!(config.management.http.port, 9090);
        assert_eq!(config.proxy.http.port, 8080);
    }

    #[test]
    fn test_config_validation() {
        let mut config = DualPortServerConfig::default();
        assert!(config.validate().is_ok());

        // 测试端口冲突
        config.proxy.http.port = 9090; // 与管理端口冲突
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_listener_bind_address() {
        let listener = ListenerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            bind_addr: None,
        };

        let addr = listener.bind_address().unwrap();
        assert_eq!(addr.to_string(), "127.0.0.1:8080");
    }

    #[test]
    fn test_get_all_listeners() {
        let config = DualPortServerConfig::default();
        let listeners = config.get_all_listeners();

        // 简化配置应该有 2 个监听器：管理HTTP、代理HTTP
        assert_eq!(listeners.len(), 2);

        let names: Vec<&str> = listeners.iter().map(|(name, _, _)| name.as_str()).collect();
        assert!(names.contains(&"management-http"));
        assert!(names.contains(&"proxy-http"));
    }
}
