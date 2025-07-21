//! # TLS 模块
//!
//! 提供 TLS 证书管理和自动化证书获取功能

use std::path::{Path, PathBuf};
use std::fs;
use std::sync::Arc;
use thiserror::Error;
use tracing::{info, warn, error};
use crate::config::TlsConfig;

pub mod manager;
pub mod acme;
pub mod certificate;

/// TLS 错误类型
#[derive(Debug, Error)]
pub enum TlsError {
    #[error("Certificate not found: {0}")]
    CertificateNotFound(String),
    #[error("Certificate expired: {0}")]
    CertificateExpired(String),
    #[error("Invalid certificate: {0}")]
    InvalidCertificate(String),
    #[error("ACME challenge failed: {0}")]
    AcmeChallengeFailed(String),
    #[error("File system error: {0}")]
    FileSystemError(String),
    #[error("TLS configuration error: {0}")]
    ConfigurationError(String),
}

/// TLS 证书信息
#[derive(Debug, Clone)]
pub struct CertificateInfo {
    /// 域名
    pub domain: String,
    /// 证书文件路径
    pub cert_path: PathBuf,
    /// 私钥文件路径
    pub key_path: PathBuf,
    /// 证书链文件路径（可选）
    pub chain_path: Option<PathBuf>,
    /// 证书有效期开始时间
    pub not_before: chrono::DateTime<chrono::Utc>,
    /// 证书有效期结束时间
    pub not_after: chrono::DateTime<chrono::Utc>,
    /// 是否为自签名证书
    pub is_self_signed: bool,
}

impl CertificateInfo {
    /// 检查证书是否即将过期（30天内）
    pub fn is_expiring_soon(&self) -> bool {
        let now = chrono::Utc::now();
        let thirty_days = chrono::Duration::days(30);
        self.not_after - now < thirty_days
    }

    /// 检查证书是否已过期
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now() > self.not_after
    }

    /// 获取证书剩余有效时间
    pub fn remaining_validity(&self) -> chrono::Duration {
        self.not_after - chrono::Utc::now()
    }
}

/// TLS 证书管理器
pub struct TlsManager {
    config: Arc<TlsConfig>,
    cert_storage: PathBuf,
}

impl TlsManager {
    /// 创建新的 TLS 管理器
    pub fn new(config: Arc<TlsConfig>) -> Result<Self, TlsError> {
        let cert_storage = PathBuf::from(&config.cert_path);
        
        // 确保证书存储目录存在
        if !cert_storage.exists() {
            fs::create_dir_all(&cert_storage)
                .map_err(|e| TlsError::FileSystemError(format!("Failed to create cert directory: {}", e)))?;
            info!("Created certificate storage directory: {}", cert_storage.display());
        }

        Ok(Self {
            config,
            cert_storage,
        })
    }

    /// 获取域名的证书信息
    pub fn get_certificate_info(&self, domain: &str) -> Result<CertificateInfo, TlsError> {
        let cert_path = self.cert_storage.join(format!("{}.crt", domain));
        let key_path = self.cert_storage.join(format!("{}.key", domain));
        let chain_path = self.cert_storage.join(format!("{}.chain.pem", domain));

        if !cert_path.exists() || !key_path.exists() {
            return Err(TlsError::CertificateNotFound(format!("Certificate files not found for domain: {}", domain)));
        }

        // 读取证书文件并解析时间信息（简化实现）
        let cert_content = fs::read_to_string(&cert_path)
            .map_err(|e| TlsError::FileSystemError(format!("Failed to read certificate: {}", e)))?;

        // 在真实实现中，这里应该解析 X.509 证书
        // 目前使用示例值
        let now = chrono::Utc::now();
        let cert_info = CertificateInfo {
            domain: domain.to_string(),
            cert_path,
            key_path,
            chain_path: if chain_path.exists() { Some(chain_path) } else { None },
            not_before: now - chrono::Duration::days(1),
            not_after: now + chrono::Duration::days(90), // 默认90天有效期
            is_self_signed: cert_content.contains("SELF-SIGNED"), // 简化判断
        };

        Ok(cert_info)
    }

    /// 检查所有域名的证书状态
    pub fn check_all_certificates(&self) -> Vec<(String, Result<CertificateInfo, TlsError>)> {
        let mut results = Vec::new();

        for domain in &self.config.domains {
            let result = self.get_certificate_info(domain);
            results.push((domain.clone(), result));
        }

        results
    }

    /// 生成自签名证书（用于开发环境）
    pub fn generate_self_signed_certificate(&self, domain: &str) -> Result<CertificateInfo, TlsError> {
        let cert_path = self.cert_storage.join(format!("{}.crt", domain));
        let key_path = self.cert_storage.join(format!("{}.key", domain));

        info!("Generating self-signed certificate for domain: {}", domain);

        // 简化的自签名证书生成（实际实现需要使用 openssl 或其他库）
        let cert_content = format!(
            "-----BEGIN CERTIFICATE-----\n\
            SELF-SIGNED CERTIFICATE FOR {}\n\
            Generated at: {}\n\
            -----END CERTIFICATE-----\n",
            domain,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );

        let key_content = format!(
            "-----BEGIN PRIVATE KEY-----\n\
            SELF-SIGNED PRIVATE KEY FOR {}\n\
            Generated at: {}\n\
            -----END PRIVATE KEY-----\n",
            domain,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );

        fs::write(&cert_path, cert_content)
            .map_err(|e| TlsError::FileSystemError(format!("Failed to write certificate: {}", e)))?;

        fs::write(&key_path, key_content)
            .map_err(|e| TlsError::FileSystemError(format!("Failed to write private key: {}", e)))?;

        // 设置私钥文件权限
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&key_path)
                .map_err(|e| TlsError::FileSystemError(format!("Failed to get key file metadata: {}", e)))?
                .permissions();
            perms.set_mode(0o600); // 只有所有者可读写
            fs::set_permissions(&key_path, perms)
                .map_err(|e| TlsError::FileSystemError(format!("Failed to set key file permissions: {}", e)))?;
        }

        info!("Generated self-signed certificate for domain: {}", domain);

        let now = chrono::Utc::now();
        Ok(CertificateInfo {
            domain: domain.to_string(),
            cert_path,
            key_path,
            chain_path: None,
            not_before: now,
            not_after: now + chrono::Duration::days(365), // 自签名证书1年有效期
            is_self_signed: true,
        })
    }

    /// 自动续期即将过期的证书
    pub async fn auto_renew_certificates(&self) -> Vec<(String, Result<(), TlsError>)> {
        let mut results = Vec::new();

        let cert_statuses = self.check_all_certificates();
        for (domain, cert_result) in cert_statuses {
            match cert_result {
                Ok(cert_info) => {
                    if cert_info.is_expiring_soon() && !cert_info.is_self_signed {
                        info!("Certificate for {} is expiring soon, attempting renewal", domain);
                        // 在真实实现中，这里会调用 ACME 客户端进行续期
                        results.push((domain.clone(), Ok(())));
                    } else if cert_info.is_expired() {
                        warn!("Certificate for {} has expired", domain);
                        results.push((domain.clone(), Err(TlsError::CertificateExpired(domain.clone()))));
                    }
                }
                Err(e) => {
                    warn!("Failed to check certificate for {}: {}", domain, e);
                    results.push((domain.clone(), Err(e)));
                }
            }
        }

        results
    }

    /// 确保所有配置的域名都有证书
    pub fn ensure_certificates(&self) -> Result<Vec<CertificateInfo>, TlsError> {
        let mut certificates = Vec::new();

        for domain in &self.config.domains {
            match self.get_certificate_info(domain) {
                Ok(cert_info) => {
                    if cert_info.is_expired() {
                        warn!("Certificate for {} is expired, generating new self-signed certificate", domain);
                        let new_cert = self.generate_self_signed_certificate(domain)?;
                        certificates.push(new_cert);
                    } else {
                        info!("Valid certificate found for domain: {}", domain);
                        certificates.push(cert_info);
                    }
                }
                Err(TlsError::CertificateNotFound(_)) => {
                    info!("No certificate found for {}, generating self-signed certificate", domain);
                    let cert_info = self.generate_self_signed_certificate(domain)?;
                    certificates.push(cert_info);
                }
                Err(e) => {
                    error!("Failed to process certificate for {}: {}", domain, e);
                    return Err(e);
                }
            }
        }

        Ok(certificates)
    }

    /// 获取证书存储路径
    pub fn get_cert_storage_path(&self) -> &Path {
        &self.cert_storage
    }

    /// 清理过期的证书文件
    pub fn cleanup_expired_certificates(&self) -> Result<usize, TlsError> {
        let mut cleaned_count = 0;

        // 遍历证书存储目录
        let entries = fs::read_dir(&self.cert_storage)
            .map_err(|e| TlsError::FileSystemError(format!("Failed to read cert directory: {}", e)))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| TlsError::FileSystemError(format!("Failed to read directory entry: {}", e)))?;
            
            let path = entry.path();
            if let Some(extension) = path.extension() {
                if extension == "crt" {
                    if let Some(domain) = path.file_stem().and_then(|s| s.to_str()) {
                        match self.get_certificate_info(domain) {
                            Ok(cert_info) => {
                                // 删除过期超过30天的证书
                                if cert_info.is_expired() && 
                                   (chrono::Utc::now() - cert_info.not_after).num_days() > 30 {
                                    info!("Cleaning up expired certificate for domain: {}", domain);
                                    
                                    // 删除相关文件
                                    let _ = fs::remove_file(&cert_info.cert_path);
                                    let _ = fs::remove_file(&cert_info.key_path);
                                    if let Some(chain_path) = &cert_info.chain_path {
                                        let _ = fs::remove_file(chain_path);
                                    }
                                    
                                    cleaned_count += 1;
                                }
                            }
                            Err(_) => {
                                // 如果证书文件损坏或无法读取，也可以考虑清理
                            }
                        }
                    }
                }
            }
        }

        info!("Cleaned up {} expired certificate(s)", cleaned_count);
        Ok(cleaned_count)
    }
}
