//! # 双端口架构配置
//!
//! 支持管理端口和代理端口分离的配置结构

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// 双端口服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualPortServerConfig {
    /// 管理服务配置
    pub management: ManagementPortConfig,
    /// 代理服务配置  
    pub proxy: ProxyPortConfig,
    /// 全局工作线程数
    pub workers: usize,
    /// 启用的服务类型
    pub enabled_services: EnabledServices,
}

/// 管理端口配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagementPortConfig {
    /// HTTP 监听地址
    pub http: ListenerConfig,
    /// HTTPS 监听地址（可选）
    pub https: Option<ListenerConfig>,
    /// 是否启用管理接口
    pub enabled: bool,
    /// 访问控制
    pub access_control: AccessControlConfig,
    /// 路由前缀
    pub route_prefixes: Vec<String>,
}

/// 代理端口配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyPortConfig {
    /// HTTP 监听地址
    pub http: ListenerConfig,
    /// HTTPS 监听地址（可选）
    pub https: Option<ListenerConfig>,
    /// 是否启用代理接口
    pub enabled: bool,
    /// 负载均衡配置
    pub load_balancing: LoadBalancingConfig,
    /// 路由前缀
    pub route_prefixes: Vec<String>,
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
    /// TLS 配置（对于 HTTPS）
    pub tls: Option<TlsListenerConfig>,
}

/// TLS 监听器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsListenerConfig {
    /// 证书存储路径
    pub cert_path: String,
    /// 支持的域名
    pub domains: Vec<String>,
    /// ACME 配置
    pub acme: Option<AcmeConfig>,
    /// 手动证书配置
    pub manual_cert: Option<ManualCertConfig>,
}

/// ACME 证书配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcmeConfig {
    /// ACME 提供商（Let's Encrypt, ZeroSSL 等）
    pub provider: AcmeProvider,
    /// 联系邮箱
    pub email: String,
    /// 是否使用生产环境
    pub production: bool,
}

/// ACME 提供商
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AcmeProvider {
    LetsEncrypt,
    ZeroSSL,
    Custom { url: String },
}

/// 手动证书配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualCertConfig {
    /// 证书文件路径
    pub cert_file: String,
    /// 私钥文件路径
    pub key_file: String,
    /// CA 证书链文件路径（可选）
    pub ca_file: Option<String>,
}

/// 访问控制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControlConfig {
    /// 允许的 IP 地址范围
    pub allowed_ips: Vec<String>,
    /// 拒绝的 IP 地址范围
    pub denied_ips: Vec<String>,
    /// 是否需要认证
    pub require_auth: bool,
    /// 认证方式
    pub auth_methods: Vec<AuthMethod>,
}

/// 认证方式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    ApiKey,
    JWT,
    BasicAuth,
    ClientCert,
}

/// 负载均衡配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancingConfig {
    /// 负载均衡策略
    pub strategy: LoadBalancingStrategy,
    /// 健康检查间隔（秒）
    pub health_check_interval: u64,
    /// 失败阈值
    pub failure_threshold: u32,
    /// 恢复阈值
    pub recovery_threshold: u32,
}

/// 负载均衡策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    RoundRobin,
    WeightedRoundRobin,
    LeastConnections,
    IpHash,
    Random,
}

/// 启用的服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnabledServices {
    /// 是否启用管理服务
    pub management: bool,
    /// 是否启用代理服务
    pub proxy: bool,
    /// 是否启用健康检查
    pub health_check: bool,
    /// 是否启用监控统计
    pub monitoring: bool,
}

impl Default for DualPortServerConfig {
    fn default() -> Self {
        Self {
            management: ManagementPortConfig::default(),
            proxy: ProxyPortConfig::default(),
            workers: num_cpus::get(),
            enabled_services: EnabledServices::default(),
        }
    }
}

impl Default for ManagementPortConfig {
    fn default() -> Self {
        Self {
            http: ListenerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                bind_addr: None,
                tls: None,
            },
            https: None,
            enabled: true,
            access_control: AccessControlConfig::default(),
            route_prefixes: vec![
                "/api".to_string(),
                "/admin".to_string(),
                "/".to_string(),
            ],
        }
    }
}

impl Default for ProxyPortConfig {
    fn default() -> Self {
        Self {
            http: ListenerConfig {
                host: "0.0.0.0".to_string(),
                port: 8081,
                bind_addr: None,
                tls: None,
            },
            https: Some(ListenerConfig {
                host: "0.0.0.0".to_string(),
                port: 8443,
                bind_addr: None,
                tls: Some(TlsListenerConfig::default()),
            }),
            enabled: true,
            load_balancing: LoadBalancingConfig::default(),
            route_prefixes: vec![
                "/v1".to_string(),
                "/proxy".to_string(),
            ],
        }
    }
}

impl Default for TlsListenerConfig {
    fn default() -> Self {
        Self {
            cert_path: "./certs".to_string(),
            domains: vec!["localhost".to_string()],
            acme: Some(AcmeConfig {
                provider: AcmeProvider::LetsEncrypt,
                email: "admin@example.com".to_string(),
                production: false,
            }),
            manual_cert: None,
        }
    }
}

impl Default for AccessControlConfig {
    fn default() -> Self {
        Self {
            allowed_ips: vec!["127.0.0.1/32".to_string(), "::1/128".to_string()],
            denied_ips: vec![],
            require_auth: false,
            auth_methods: vec![AuthMethod::ApiKey],
        }
    }
}

impl Default for LoadBalancingConfig {
    fn default() -> Self {
        Self {
            strategy: LoadBalancingStrategy::RoundRobin,
            health_check_interval: 30,
            failure_threshold: 3,
            recovery_threshold: 2,
        }
    }
}

impl Default for EnabledServices {
    fn default() -> Self {
        Self {
            management: true,
            proxy: true,
            health_check: true,
            monitoring: true,
        }
    }
}

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

    /// 是否启用 TLS
    pub fn is_tls_enabled(&self) -> bool {
        self.tls.is_some()
    }
}

impl DualPortServerConfig {
    /// 验证配置的有效性
    pub fn validate(&self) -> Result<(), String> {
        // 检查端口冲突
        if self.management.enabled && self.proxy.enabled {
            let mgmt_port = self.management.http.port;
            let proxy_port = self.proxy.http.port;
            
            if mgmt_port == proxy_port {
                return Err(format!(
                    "Management port ({}) conflicts with proxy port ({})", 
                    mgmt_port, proxy_port
                ));
            }

            // 检查 HTTPS 端口冲突
            if let Some(mgmt_https) = &self.management.https {
                if mgmt_https.port == proxy_port {
                    return Err(format!(
                        "Management HTTPS port ({}) conflicts with proxy HTTP port ({})", 
                        mgmt_https.port, proxy_port
                    ));
                }
                
                if let Some(proxy_https) = &self.proxy.https {
                    if mgmt_https.port == proxy_https.port {
                        return Err(format!(
                            "Management HTTPS port ({}) conflicts with proxy HTTPS port ({})", 
                            mgmt_https.port, proxy_https.port
                        ));
                    }
                }
            }
        }

        // 检查工作线程数
        if self.workers == 0 {
            return Err("Worker count must be greater than 0".to_string());
        }

        // 验证监听配置
        self.management.http.bind_address()
            .map_err(|e| format!("Invalid management HTTP address: {}", e))?;

        if let Some(https) = &self.management.https {
            https.bind_address()
                .map_err(|e| format!("Invalid management HTTPS address: {}", e))?;
        }

        self.proxy.http.bind_address()
            .map_err(|e| format!("Invalid proxy HTTP address: {}", e))?;

        if let Some(https) = &self.proxy.https {
            https.bind_address()
                .map_err(|e| format!("Invalid proxy HTTPS address: {}", e))?;
        }

        Ok(())
    }

    /// 获取所有监听地址
    pub fn get_all_listeners(&self) -> Vec<(String, SocketAddr, String)> {
        let mut listeners = Vec::new();

        if self.management.enabled {
            if let Ok(addr) = self.management.http.bind_address() {
                listeners.push(("management-http".to_string(), addr, "HTTP".to_string()));
            }
            
            if let Some(https) = &self.management.https {
                if let Ok(addr) = https.bind_address() {
                    listeners.push(("management-https".to_string(), addr, "HTTPS".to_string()));
                }
            }
        }

        if self.proxy.enabled {
            if let Ok(addr) = self.proxy.http.bind_address() {
                listeners.push(("proxy-http".to_string(), addr, "HTTP".to_string()));
            }
            
            if let Some(https) = &self.proxy.https {
                if let Ok(addr) = https.bind_address() {
                    listeners.push(("proxy-https".to_string(), addr, "HTTPS".to_string()));
                }
            }
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
        
        assert!(config.management.enabled);
        assert!(config.proxy.enabled);
        assert_eq!(config.management.http.port, 8080);
        assert_eq!(config.proxy.http.port, 8081);
    }

    #[test]
    fn test_config_validation() {
        let mut config = DualPortServerConfig::default();
        assert!(config.validate().is_ok());

        // 测试端口冲突
        config.proxy.http.port = 8080; // 与管理端口冲突
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_listener_bind_address() {
        let listener = ListenerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            bind_addr: None,
            tls: None,
        };
        
        let addr = listener.bind_address().unwrap();
        assert_eq!(addr.to_string(), "127.0.0.1:8080");
    }

    #[test]
    fn test_get_all_listeners() {
        let config = DualPortServerConfig::default();
        let listeners = config.get_all_listeners();
        
        // 默认配置应该有 3 个监听器：管理HTTP、代理HTTP、代理HTTPS
        assert_eq!(listeners.len(), 3);
        
        let names: Vec<&str> = listeners.iter().map(|(name, _, _)| name.as_str()).collect();
        assert!(names.contains(&"management-http"));
        assert!(names.contains(&"proxy-http"));
        assert!(names.contains(&"proxy-https"));
    }
}