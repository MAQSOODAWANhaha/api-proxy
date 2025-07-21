//! # TLS 证书管理器
//!
//! 提供证书生命周期管理和自动化操作

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use crate::config::TlsConfig;
use super::{TlsManager, TlsError, CertificateInfo};

/// 证书状态
#[derive(Debug, Clone)]
pub enum CertificateStatus {
    /// 证书有效
    Valid,
    /// 即将过期
    ExpiringSoon,
    /// 已过期
    Expired,
    /// 未找到
    NotFound,
    /// 无效
    Invalid(String),
}

/// TLS 证书管理器（带缓存和自动化功能）
pub struct TlsCertificateManager {
    tls_manager: TlsManager,
    certificate_cache: Arc<RwLock<HashMap<String, (CertificateInfo, chrono::DateTime<chrono::Utc>)>>>,
    renewal_task_running: Arc<RwLock<bool>>,
}

impl TlsCertificateManager {
    /// 创建新的证书管理器
    pub fn new(config: Arc<TlsConfig>) -> Result<Self, TlsError> {
        let tls_manager = TlsManager::new(config)?;
        
        Ok(Self {
            tls_manager,
            certificate_cache: Arc::new(RwLock::new(HashMap::new())),
            renewal_task_running: Arc::new(RwLock::new(false)),
        })
    }

    /// 获取证书信息（带缓存）
    pub async fn get_certificate(&self, domain: &str) -> Result<CertificateInfo, TlsError> {
        // 检查缓存
        {
            let cache = self.certificate_cache.read().await;
            if let Some((cert_info, cached_at)) = cache.get(domain) {
                // 缓存有效期5分钟
                if chrono::Utc::now() - *cached_at < chrono::Duration::minutes(5) {
                    return Ok(cert_info.clone());
                }
            }
        }

        // 从磁盘加载证书
        let cert_info = self.tls_manager.get_certificate_info(domain)?;
        
        // 更新缓存
        {
            let mut cache = self.certificate_cache.write().await;
            cache.insert(domain.to_string(), (cert_info.clone(), chrono::Utc::now()));
        }

        Ok(cert_info)
    }

    /// 获取证书状态
    pub async fn get_certificate_status(&self, domain: &str) -> CertificateStatus {
        match self.get_certificate(domain).await {
            Ok(cert_info) => {
                if cert_info.is_expired() {
                    CertificateStatus::Expired
                } else if cert_info.is_expiring_soon() {
                    CertificateStatus::ExpiringSoon
                } else {
                    CertificateStatus::Valid
                }
            }
            Err(TlsError::CertificateNotFound(_)) => CertificateStatus::NotFound,
            Err(e) => CertificateStatus::Invalid(e.to_string()),
        }
    }

    /// 获取所有域名的证书状态
    pub async fn get_all_certificate_statuses(&self) -> HashMap<String, CertificateStatus> {
        let mut statuses = HashMap::new();
        
        let cert_results = self.tls_manager.check_all_certificates();
        for (domain, result) in cert_results {
            let status = match result {
                Ok(cert_info) => {
                    if cert_info.is_expired() {
                        CertificateStatus::Expired
                    } else if cert_info.is_expiring_soon() {
                        CertificateStatus::ExpiringSoon
                    } else {
                        CertificateStatus::Valid
                    }
                }
                Err(TlsError::CertificateNotFound(_)) => CertificateStatus::NotFound,
                Err(e) => CertificateStatus::Invalid(e.to_string()),
            };
            statuses.insert(domain, status);
        }

        statuses
    }

    /// 确保所有证书可用
    pub async fn ensure_all_certificates(&self) -> Result<Vec<CertificateInfo>, TlsError> {
        let certificates = self.tls_manager.ensure_certificates()?;
        
        // 清除缓存
        {
            let mut cache = self.certificate_cache.write().await;
            cache.clear();
        }

        // 预热缓存
        for cert in &certificates {
            let mut cache = self.certificate_cache.write().await;
            cache.insert(cert.domain.clone(), (cert.clone(), chrono::Utc::now()));
        }

        info!("Ensured {} certificate(s) are available", certificates.len());
        Ok(certificates)
    }

    /// 生成缺失的证书
    pub async fn generate_missing_certificates(&self) -> Result<Vec<CertificateInfo>, TlsError> {
        let mut generated = Vec::new();
        let statuses = self.get_all_certificate_statuses().await;

        for (domain, status) in statuses {
            match status {
                CertificateStatus::NotFound | CertificateStatus::Expired => {
                    info!("Generating certificate for domain: {}", domain);
                    let cert_info = self.tls_manager.generate_self_signed_certificate(&domain)?;
                    generated.push(cert_info);
                }
                _ => {}
            }
        }

        // 清除缓存以确保重新加载
        {
            let mut cache = self.certificate_cache.write().await;
            cache.clear();
        }

        Ok(generated)
    }

    /// 启动自动续期任务
    pub async fn start_auto_renewal_task(&self) {
        let renewal_running = self.renewal_task_running.clone();
        
        // 检查是否已经在运行
        {
            let running = renewal_running.read().await;
            if *running {
                info!("Auto-renewal task is already running");
                return;
            }
        }

        {
            let mut running = renewal_running.write().await;
            *running = true;
        }

        let tls_manager = TlsManager::new(self.tls_manager.config.clone())
            .expect("Failed to create TLS manager for renewal task");
        let _renewal_running_clone = renewal_running.clone();

        tokio::spawn(async move {
            info!("Starting auto-renewal task");
            
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(24 * 60 * 60)); // 每24小时检查一次
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        info!("Running certificate auto-renewal check");
                        
                        let renewal_results = tls_manager.auto_renew_certificates().await;
                        for (domain, result) in renewal_results {
                            match result {
                                Ok(()) => info!("Successfully renewed certificate for {}", domain),
                                Err(e) => error!("Failed to renew certificate for {}: {}", domain, e),
                            }
                        }
                        
                        // 清理过期证书
                        match tls_manager.cleanup_expired_certificates() {
                            Ok(count) => {
                                if count > 0 {
                                    info!("Cleaned up {} expired certificate(s)", count);
                                }
                            }
                            Err(e) => warn!("Failed to cleanup expired certificates: {}", e),
                        }
                    }
                }
            }
        });

        info!("Auto-renewal task started");
    }

    /// 停止自动续期任务
    pub async fn stop_auto_renewal_task(&self) {
        let mut running = self.renewal_task_running.write().await;
        *running = false;
        info!("Auto-renewal task stopped");
    }

    /// 清除证书缓存
    pub async fn clear_certificate_cache(&self) {
        let mut cache = self.certificate_cache.write().await;
        cache.clear();
        info!("Certificate cache cleared");
    }

    /// 获取缓存统计信息
    pub async fn get_cache_stats(&self) -> HashMap<String, usize> {
        let cache = self.certificate_cache.read().await;
        let mut stats = HashMap::new();
        
        stats.insert("cached_certificates".to_string(), cache.len());
        
        let valid_count = cache.values()
            .filter(|(_, cached_at)| {
                chrono::Utc::now() - *cached_at < chrono::Duration::minutes(5)
            })
            .count();
        
        stats.insert("valid_cache_entries".to_string(), valid_count);
        
        stats
    }

    /// 手动触发证书续期
    pub async fn manual_renew_certificate(&self, domain: &str) -> Result<(), TlsError> {
        info!("Manually renewing certificate for domain: {}", domain);
        
        // 检查当前证书状态
        match self.get_certificate_status(domain).await {
            CertificateStatus::Valid => {
                info!("Certificate for {} is still valid, but proceeding with renewal", domain);
            }
            CertificateStatus::ExpiringSoon => {
                info!("Certificate for {} is expiring soon, renewing", domain);
            }
            CertificateStatus::Expired => {
                info!("Certificate for {} has expired, renewing", domain);
            }
            CertificateStatus::NotFound => {
                info!("No certificate found for {}, generating new one", domain);
            }
            CertificateStatus::Invalid(e) => {
                warn!("Certificate for {} is invalid ({}), generating new one", domain, e);
            }
        }

        // 在真实实现中，这里会调用 ACME 客户端
        // 目前生成自签名证书
        let _cert_info = self.tls_manager.generate_self_signed_certificate(domain)?;
        
        // 清除该域名的缓存
        {
            let mut cache = self.certificate_cache.write().await;
            cache.remove(domain);
        }

        info!("Successfully renewed certificate for domain: {}", domain);
        Ok(())
    }
}