//! # 健康检查器实现

use crate::error::Result;
use super::types::{HealthCheckResult, HealthCheckConfig, HealthCheckType};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use reqwest::Client;
use std::net::{TcpStream, ToSocketAddrs};

/// HTTP健康检查器
pub struct HealthChecker {
    client: Client,
}

impl HealthChecker {
    /// 创建新的健康检查器
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .danger_accept_invalid_certs(false) // 生产环境应该验证证书
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// 执行健康检查
    pub async fn check_health(
        &self,
        server_address: &str,
        config: &HealthCheckConfig,
    ) -> Result<HealthCheckResult> {
        if !config.enabled {
            return Ok(HealthCheckResult::failure(
                "Health check disabled".to_string(),
                config.check_type,
            ));
        }

        match config.check_type {
            HealthCheckType::Http => self.check_http(server_address, config).await,
            HealthCheckType::Https => self.check_https(server_address, config).await,
            HealthCheckType::Tcp => self.check_tcp(server_address, config).await,
            HealthCheckType::Custom => self.check_custom(server_address, config).await,
        }
    }

    /// HTTP健康检查
    async fn check_http(
        &self,
        server_address: &str,
        config: &HealthCheckConfig,
    ) -> Result<HealthCheckResult> {
        let start_time = Instant::now();
        
        // 构建URL
        let path = config.path.as_deref().unwrap_or("/health");
        let url = if server_address.starts_with("http") {
            format!("{}{}", server_address, path)
        } else {
            format!("http://{}{}", server_address, path)
        };

        // 执行HTTP请求
        let result = timeout(config.timeout, async {
            let mut request = match config.body.as_ref() {
                Some(body) => self.client.post(&url).body(body.clone()),
                None => self.client.get(&url),
            };

            // 添加请求头
            for (key, value) in &config.headers {
                request = request.header(key, value);
            }

            request.send().await
        }).await;

        let response_time = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(response)) => {
                let status_code = response.status().as_u16();
                
                if config.expected_status.contains(&status_code) {
                    Ok(HealthCheckResult::success(
                        response_time,
                        status_code,
                        HealthCheckType::Http,
                    ))
                } else {
                    Ok(HealthCheckResult::failure(
                        format!("Unexpected status code: {}", status_code),
                        HealthCheckType::Http,
                    ))
                }
            }
            Ok(Err(e)) => Ok(HealthCheckResult::failure(
                format!("HTTP request failed: {}", e),
                HealthCheckType::Http,
            )),
            Err(_) => Ok(HealthCheckResult::timeout(
                config.timeout.as_millis() as u64,
                HealthCheckType::Http,
            )),
        }
    }

    /// HTTPS健康检查
    async fn check_https(
        &self,
        server_address: &str,
        config: &HealthCheckConfig,
    ) -> Result<HealthCheckResult> {
        let start_time = Instant::now();
        
        // 构建HTTPS URL
        let path = config.path.as_deref().unwrap_or("/health");
        let url = if server_address.starts_with("https") {
            format!("{}{}", server_address, path)
        } else {
            format!("https://{}{}", server_address, path)
        };

        // 执行HTTPS请求
        let result = timeout(config.timeout, async {
            let mut request = match config.body.as_ref() {
                Some(body) => self.client.post(&url).body(body.clone()),
                None => self.client.get(&url),
            };

            // 添加请求头
            for (key, value) in &config.headers {
                request = request.header(key, value);
            }

            request.send().await
        }).await;

        let response_time = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(response)) => {
                let status_code = response.status().as_u16();
                
                if config.expected_status.contains(&status_code) {
                    Ok(HealthCheckResult::success(
                        response_time,
                        status_code,
                        HealthCheckType::Https,
                    ))
                } else {
                    Ok(HealthCheckResult::failure(
                        format!("Unexpected status code: {}", status_code),
                        HealthCheckType::Https,
                    ))
                }
            }
            Ok(Err(e)) => Ok(HealthCheckResult::failure(
                format!("HTTPS request failed: {}", e),
                HealthCheckType::Https,
            )),
            Err(_) => Ok(HealthCheckResult::timeout(
                config.timeout.as_millis() as u64,
                HealthCheckType::Https,
            )),
        }
    }

    /// TCP连接检查
    async fn check_tcp(
        &self,
        server_address: &str,
        config: &HealthCheckConfig,
    ) -> Result<HealthCheckResult> {
        let start_time = Instant::now();
        
        let result = timeout(config.timeout, async {
            // 解析服务器地址
            let addrs: Vec<_> = server_address.to_socket_addrs()
                .map_err(|e| format!("Invalid address: {}", e))?
                .collect();
            
            if addrs.is_empty() {
                return Err("No valid addresses found".to_string());
            }

            // 尝试TCP连接
            TcpStream::connect(&addrs[0])
                .map_err(|e| format!("TCP connection failed: {}", e))
        }).await;

        let response_time = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(_)) => Ok(HealthCheckResult::success(
                response_time,
                0, // TCP没有状态码
                HealthCheckType::Tcp,
            )),
            Ok(Err(e)) => Ok(HealthCheckResult::failure(e, HealthCheckType::Tcp)),
            Err(_) => Ok(HealthCheckResult::timeout(
                config.timeout.as_millis() as u64,
                HealthCheckType::Tcp,
            )),
        }
    }

    /// 自定义健康检查
    async fn check_custom(
        &self,
        server_address: &str,
        config: &HealthCheckConfig,
    ) -> Result<HealthCheckResult> {
        // 默认退化为HTTP检查
        // 在实际实现中，可以根据配置执行不同的自定义检查逻辑
        tracing::warn!("Custom health check not implemented, falling back to HTTP check");
        self.check_http(server_address, config).await
    }

    /// 批量健康检查
    pub async fn check_multiple(
        &self,
        servers: Vec<(String, HealthCheckConfig)>,
    ) -> Result<Vec<(String, HealthCheckResult)>> {
        let mut results = Vec::new();
        
        // 并发执行所有检查
        let futures: Vec<_> = servers
            .into_iter()
            .map(|(addr, config)| {
                let checker = &self;
                async move {
                    let result = checker.check_health(&addr, &config).await;
                    (addr, result)
                }
            })
            .collect();

        // 等待所有检查完成
        let check_results = futures::future::join_all(futures).await;
        
        for (addr, result) in check_results {
            match result {
                Ok(health_result) => results.push((addr, health_result)),
                Err(e) => {
                    tracing::error!("Health check failed for {}: {}", addr, e);
                    results.push((
                        addr,
                        HealthCheckResult::failure(
                            format!("Check error: {}", e),
                            HealthCheckType::Http,
                        ),
                    ));
                }
            }
        }

        Ok(results)
    }

    /// 快速TCP端口检查
    pub async fn quick_tcp_check(
        &self,
        server_address: &str,
        timeout_ms: u64,
    ) -> bool {
        let timeout_duration = Duration::from_millis(timeout_ms);
        
        let result = timeout(timeout_duration, async {
            let addrs: std::io::Result<Vec<_>> = server_address.to_socket_addrs()
                .map(|iter| iter.collect());
            
            match addrs {
                Ok(addr_list) => {
                    if let Some(addr) = addr_list.into_iter().next() {
                        TcpStream::connect(addr).is_ok()
                    } else {
                        false
                    }
                }
                Err(_) => false,
            }
        }).await;

        result.unwrap_or(false)
    }

    /// 获取服务器响应头信息
    pub async fn get_server_info(
        &self,
        server_address: &str,
        config: &HealthCheckConfig,
    ) -> Result<Option<std::collections::HashMap<String, String>>> {
        let path = config.path.as_deref().unwrap_or("/health");
        let url = if server_address.starts_with("http") {
            format!("{}{}", server_address, path)
        } else {
            format!("http://{}{}", server_address, path)
        };

        let result = timeout(config.timeout, async {
            self.client.head(&url).send().await
        }).await;

        match result {
            Ok(Ok(response)) => {
                let mut headers = std::collections::HashMap::new();
                
                for (key, value) in response.headers() {
                    if let Ok(value_str) = value.to_str() {
                        headers.insert(key.to_string(), value_str.to_string());
                    }
                }
                
                Ok(Some(headers))
            }
            _ => Ok(None),
        }
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[tokio::test]
    async fn test_health_checker_creation() {
        let checker = HealthChecker::new();
        assert!(true); // 如果能创建就说明成功
    }

    #[tokio::test]
    async fn test_tcp_check_invalid_address() {
        let checker = HealthChecker::new();
        let config = HealthCheckConfig {
            check_type: HealthCheckType::Tcp,
            timeout: Duration::from_millis(100),
            ..Default::default()
        };

        let result = checker.check_tcp("invalid:99999", &config).await;
        assert!(result.is_ok());
        
        let health_result = result.unwrap();
        assert!(!health_result.is_healthy);
    }

    #[tokio::test]
    async fn test_quick_tcp_check() {
        let checker = HealthChecker::new();
        
        // 测试无效地址
        let result = checker.quick_tcp_check("invalid:99999", 100).await;
        assert!(!result);
    }

    #[test]
    fn test_health_check_config_default() {
        let config = HealthCheckConfig::default();
        assert_eq!(config.check_type, HealthCheckType::Http);
        assert!(config.enabled);
        assert_eq!(config.path, Some("/health".to_string()));
        assert!(config.expected_status.contains(&200));
    }

    #[tokio::test]
    async fn test_disabled_health_check() {
        let checker = HealthChecker::new();
        let config = HealthCheckConfig {
            enabled: false,
            ..Default::default()
        };

        let result = checker.check_health("127.0.0.1:8080", &config).await;
        assert!(result.is_ok());
        
        let health_result = result.unwrap();
        assert!(!health_result.is_healthy);
        assert!(health_result.error_message.as_ref().unwrap().contains("disabled"));
    }

    #[tokio::test]
    async fn test_batch_health_check() {
        let checker = HealthChecker::new();
        let servers = vec![
            ("127.0.0.1:99999".to_string(), HealthCheckConfig::default()),
            ("127.0.0.1:99998".to_string(), HealthCheckConfig::default()),
        ];

        let results = checker.check_multiple(servers).await;
        assert!(results.is_ok());
        
        let health_results = results.unwrap();
        assert_eq!(health_results.len(), 2);
    }
}