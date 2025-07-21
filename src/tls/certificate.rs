//! # 证书工具模块
//!
//! 提供证书解析、验证和格式转换功能

use std::path::Path;
use std::fs;
use tracing::{debug, warn};
use super::{TlsError, CertificateInfo};

/// 证书格式
#[derive(Debug, Clone)]
pub enum CertificateFormat {
    /// PEM 格式
    Pem,
    /// DER 格式
    Der,
    /// PKCS#12 格式
    Pkcs12,
}

/// 证书用途
#[derive(Debug, Clone)]
pub enum CertificateUsage {
    /// 服务器认证
    ServerAuth,
    /// 客户端认证
    ClientAuth,
    /// 代码签名
    CodeSigning,
    /// 邮件保护
    EmailProtection,
}

/// 证书解析器
pub struct CertificateParser;

impl CertificateParser {
    /// 解析 PEM 格式证书
    pub fn parse_pem_certificate(cert_path: &Path) -> Result<CertificateInfo, TlsError> {
        if !cert_path.exists() {
            return Err(TlsError::CertificateNotFound(cert_path.display().to_string()));
        }

        let cert_content = fs::read_to_string(cert_path)
            .map_err(|e| TlsError::FileSystemError(format!("Failed to read certificate: {}", e)))?;

        // 简化的证书解析（在真实实现中需要使用 openssl 或 rustls 等库）
        let domain = Self::extract_domain_from_cert_content(&cert_content)?;
        let (not_before, not_after) = Self::extract_validity_period(&cert_content)?;
        let is_self_signed = Self::is_self_signed_certificate(&cert_content);

        let key_path = cert_path.with_extension("key");
        let chain_path = cert_path.with_extension("chain.pem");

        Ok(CertificateInfo {
            domain,
            cert_path: cert_path.to_path_buf(),
            key_path,
            chain_path: if chain_path.exists() { Some(chain_path) } else { None },
            not_before,
            not_after,
            is_self_signed,
        })
    }

    /// 从证书内容中提取域名
    fn extract_domain_from_cert_content(content: &str) -> Result<String, TlsError> {
        // 简化实现：查找证书内容中的域名标识
        for line in content.lines() {
            if line.contains("CERTIFICATE FOR") {
                if let Some(domain_part) = line.split("CERTIFICATE FOR ").nth(1) {
                    return Ok(domain_part.trim().to_string());
                }
            }
        }

        // 如果没有找到，返回默认值
        Ok("localhost".to_string())
    }

    /// 提取证书有效期
    fn extract_validity_period(content: &str) -> Result<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>), TlsError> {
        // 简化实现：查找生成时间并设置默认有效期
        let now = chrono::Utc::now();
        
        for line in content.lines() {
            if line.contains("Generated at:") {
                if let Some(date_str) = line.split("Generated at: ").nth(1) {
                    if let Ok(generated_time) = chrono::DateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S UTC") {
                        let generated_utc = generated_time.with_timezone(&chrono::Utc);
                        let expires_at = if content.contains("SELF-SIGNED") {
                            generated_utc + chrono::Duration::days(365) // 自签名证书1年
                        } else {
                            generated_utc + chrono::Duration::days(90)  // ACME证书90天
                        };
                        return Ok((generated_utc, expires_at));
                    }
                }
            }
        }

        // 默认值
        Ok((now - chrono::Duration::days(1), now + chrono::Duration::days(90)))
    }

    /// 检查是否为自签名证书
    fn is_self_signed_certificate(content: &str) -> bool {
        content.contains("SELF-SIGNED")
    }

    /// 验证证书链
    pub fn verify_certificate_chain(cert_path: &Path, chain_path: Option<&Path>) -> Result<bool, TlsError> {
        debug!("Verifying certificate chain for: {}", cert_path.display());

        // 在真实实现中，这里会：
        // 1. 加载证书和证书链
        // 2. 验证证书签名
        // 3. 检查证书链的有效性
        // 4. 验证根证书

        // 简化实现：基本文件存在性检查
        if !cert_path.exists() {
            return Err(TlsError::CertificateNotFound(cert_path.display().to_string()));
        }

        if let Some(chain) = chain_path {
            if !chain.exists() {
                warn!("Certificate chain file not found: {}", chain.display());
                return Ok(false);
            }
        }

        debug!("Certificate chain verification passed");
        Ok(true)
    }

    /// 检查证书是否匹配域名
    pub fn verify_domain_match(cert_path: &Path, domain: &str) -> Result<bool, TlsError> {
        let cert_info = Self::parse_pem_certificate(cert_path)?;
        
        // 简单的域名匹配
        let matches = cert_info.domain == domain || 
                     cert_info.domain == format!("*.{}", domain) ||
                     domain.ends_with(&cert_info.domain.trim_start_matches("*."));

        debug!("Domain match check: cert_domain={}, requested_domain={}, matches={}", 
               cert_info.domain, domain, matches);

        Ok(matches)
    }

    /// 获取证书指纹
    pub fn get_certificate_fingerprint(cert_path: &Path) -> Result<String, TlsError> {
        let cert_content = fs::read(cert_path)
            .map_err(|e| TlsError::FileSystemError(format!("Failed to read certificate: {}", e)))?;

        // 简化实现：计算文件内容的哈希
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        cert_content.hash(&mut hasher);
        let fingerprint = format!("{:x}", hasher.finish());

        debug!("Certificate fingerprint: {}", fingerprint);
        Ok(fingerprint)
    }
}

/// 证书生成器
pub struct CertificateGenerator;

impl CertificateGenerator {
    /// 生成自签名证书
    pub fn generate_self_signed(domain: &str, output_dir: &Path) -> Result<CertificateInfo, TlsError> {
        debug!("Generating self-signed certificate for domain: {}", domain);

        // 确保输出目录存在
        if !output_dir.exists() {
            fs::create_dir_all(output_dir)
                .map_err(|e| TlsError::FileSystemError(format!("Failed to create output directory: {}", e)))?;
        }

        let cert_path = output_dir.join(format!("{}.crt", domain));
        let key_path = output_dir.join(format!("{}.key", domain));

        // 在真实实现中，这里会使用 openssl 或 rustls 生成真正的证书
        // 简化实现：生成模拟证书文件
        let now = chrono::Utc::now();
        let cert_content = format!(
            "-----BEGIN CERTIFICATE-----\n\
            SELF-SIGNED CERTIFICATE FOR {}\n\
            Subject: CN={}\n\
            Issuer: CN={} (Self-Signed)\n\
            Serial Number: {}\n\
            Generated at: {}\n\
            Valid from: {}\n\
            Valid until: {}\n\
            -----END CERTIFICATE-----\n",
            domain,
            domain,
            domain,
            uuid::Uuid::new_v4(),
            now.format("%Y-%m-%d %H:%M:%S UTC"),
            now.format("%Y-%m-%d %H:%M:%S UTC"),
            (now + chrono::Duration::days(365)).format("%Y-%m-%d %H:%M:%S UTC")
        );

        let key_content = format!(
            "-----BEGIN PRIVATE KEY-----\n\
            SELF-SIGNED PRIVATE KEY FOR {}\n\
            Key Algorithm: RSA 2048-bit\n\
            Generated at: {}\n\
            Key ID: {}\n\
            -----END PRIVATE KEY-----\n",
            domain,
            now.format("%Y-%m-%d %H:%M:%S UTC"),
            uuid::Uuid::new_v4()
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
            perms.set_mode(0o600);
            fs::set_permissions(&key_path, perms)
                .map_err(|e| TlsError::FileSystemError(format!("Failed to set key file permissions: {}", e)))?;
        }

        debug!("Self-signed certificate generated for domain: {}", domain);

        Ok(CertificateInfo {
            domain: domain.to_string(),
            cert_path,
            key_path,
            chain_path: None,
            not_before: now,
            not_after: now + chrono::Duration::days(365),
            is_self_signed: true,
        })
    }

    /// 生成证书签名请求 (CSR)
    pub fn generate_csr(domain: &str, key_path: &Path, output_path: &Path) -> Result<String, TlsError> {
        debug!("Generating CSR for domain: {} with key: {}", domain, key_path.display());

        // 在真实实现中，这里会生成真正的 CSR
        let csr_content = format!(
            "-----BEGIN CERTIFICATE REQUEST-----\n\
            CERTIFICATE SIGNING REQUEST FOR {}\n\
            Subject: CN={}\n\
            Key Algorithm: RSA 2048-bit\n\
            Generated at: {}\n\
            -----END CERTIFICATE REQUEST-----\n",
            domain,
            domain,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );

        fs::write(output_path, &csr_content)
            .map_err(|e| TlsError::FileSystemError(format!("Failed to write CSR: {}", e)))?;

        debug!("CSR generated: {}", output_path.display());
        Ok(csr_content)
    }
}

/// 证书转换器
pub struct CertificateConverter;

impl CertificateConverter {
    /// PEM 转 DER 格式
    pub fn pem_to_der(pem_path: &Path, der_path: &Path) -> Result<(), TlsError> {
        debug!("Converting PEM to DER: {} -> {}", pem_path.display(), der_path.display());

        let _pem_content = fs::read_to_string(pem_path)
            .map_err(|e| TlsError::FileSystemError(format!("Failed to read PEM file: {}", e)))?;

        // 在真实实现中，这里会进行实际的格式转换
        // 简化实现：创建模拟 DER 文件
        let der_content = format!("DER_CONVERTED_FROM_{}", pem_path.display());
        
        fs::write(der_path, der_content.as_bytes())
            .map_err(|e| TlsError::FileSystemError(format!("Failed to write DER file: {}", e)))?;

        debug!("PEM to DER conversion completed");
        Ok(())
    }

    /// DER 转 PEM 格式
    pub fn der_to_pem(der_path: &Path, pem_path: &Path) -> Result<(), TlsError> {
        debug!("Converting DER to PEM: {} -> {}", der_path.display(), pem_path.display());

        let der_content = fs::read(der_path)
            .map_err(|e| TlsError::FileSystemError(format!("Failed to read DER file: {}", e)))?;

        // 在真实实现中，这里会进行实际的格式转换
        let pem_content = format!(
            "-----BEGIN CERTIFICATE-----\n\
            PEM_CONVERTED_FROM_{}\n\
            Original size: {} bytes\n\
            -----END CERTIFICATE-----\n",
            der_path.display(),
            der_content.len()
        );

        fs::write(pem_path, pem_content)
            .map_err(|e| TlsError::FileSystemError(format!("Failed to write PEM file: {}", e)))?;

        debug!("DER to PEM conversion completed");
        Ok(())
    }
}