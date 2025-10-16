//! # `OAuth轮询状态机制`
//!
//! 实现客户端侧OAuth轮询机制，替代传统的服务器回调方式
//! 客户端定期轮询服务器检查OAuth授权状态，避免对特定域名的依赖
//!
//! ## 轮询流程
//! 1. 客户端获取授权URL并开始轮询
//! 2. 服务器检查会话状态（pending/authorized/error/expired）
//! 3. 授权完成后，自动进行Token交换
//! 4. `返回最终的OAuth令牌或错误信息`

use super::session_manager::SessionManager;
use super::{OAuthError, OAuthTokenResponse};
use crate::auth::types::AuthStatus;
use crate::error::AuthResult;
use entity::oauth_client_sessions;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// `统一的OAuth轮询响应`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthPollingResponse {
    /// 会话状态（直接使用 `AuthStatus`）
    pub status: AuthStatus,
    /// 访问令牌（已授权时存在）
    pub access_token: Option<String>,
    /// 刷新令牌（已授权时存在）
    pub refresh_token: Option<String>,
    /// ID令牌（已授权时存在）
    pub id_token: Option<String>,
    /// 错误信息（状态为Error时存在）
    pub error: Option<String>,
    /// 错误描述（可选）
    pub error_description: Option<String>,
    /// 剩余过期时间（秒，Pending/Authorized状态有效）
    pub expires_in: Option<i64>,
    /// 建议轮询间隔（秒，Pending状态需要）
    pub polling_interval: u32,
}

impl OAuthPollingResponse {
    #[must_use]
    pub fn pending(session: &oauth_client_sessions::Model, interval: u32) -> Self {
        let expires_in =
            (session.expires_at.and_utc().timestamp() - chrono::Utc::now().timestamp()).max(0);
        Self {
            status: AuthStatus::Pending,
            access_token: None,
            refresh_token: None,
            id_token: None,
            error: None,
            error_description: None,
            expires_in: Some(expires_in),
            polling_interval: interval,
        }
    }

    #[must_use]
    pub fn authorized(session: &oauth_client_sessions::Model) -> Self {
        let expires_in =
            (session.expires_at.and_utc().timestamp() - chrono::Utc::now().timestamp()).max(0);
        Self {
            status: AuthStatus::Authorized,
            access_token: session.access_token.clone(),
            refresh_token: session.refresh_token.clone(),
            id_token: session.id_token.clone(),
            error: None,
            error_description: None,
            expires_in: Some(expires_in),
            polling_interval: 0, // 已完成，无需轮询
        }
    }

    #[must_use]
    pub fn error(session: &oauth_client_sessions::Model) -> Self {
        Self {
            status: AuthStatus::Error,
            access_token: None,
            refresh_token: None,
            id_token: None,
            error: session.error_message.clone(),
            error_description: None,
            expires_in: None,
            polling_interval: 0,
        }
    }

    #[must_use]
    pub fn expired() -> Self {
        Self {
            status: AuthStatus::Expired,
            access_token: None,
            refresh_token: None,
            id_token: None,
            error: Some("Session expired".to_string()),
            error_description: None,
            expires_in: None,
            polling_interval: 0,
        }
    }

    #[must_use]
    pub fn revoked() -> Self {
        Self {
            status: AuthStatus::Revoked,
            access_token: None,
            refresh_token: None,
            id_token: None,
            error: Some("Session revoked".to_string()),
            error_description: None,
            expires_in: None,
            polling_interval: 0,
        }
    }
}

/// 轮询配置
#[derive(Debug, Clone)]
pub struct PollingConfig {
    /// 默认轮询间隔（秒）
    pub default_interval: u32,
    /// 最小轮询间隔（秒）
    pub min_interval: u32,
    /// 最大轮询间隔（秒）
    pub max_interval: u32,
    /// 轮询超时时间（秒）
    pub timeout: u32,
    /// 指数退避因子
    pub backoff_factor: f64,
    /// 最大重试次数
    pub max_retries: u32,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            default_interval: 2,
            min_interval: 1,
            max_interval: 30,
            timeout: 300, // 5分钟
            backoff_factor: 1.5,
            max_retries: 3,
        }
    }
}

/// `OAuth轮询客户端`
#[derive(Debug)]
pub struct OAuthPollingClient {
    config: PollingConfig,
    http_client: reqwest::Client,
}

impl OAuthPollingClient {
    /// 创建新的轮询客户端
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(PollingConfig::default())
    }

    /// 使用指定配置创建轮询客户端
    #[must_use]
    pub fn with_config(config: PollingConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("OAuth-Client/1.0")
            .build()
            .unwrap_or_default();

        Self {
            config,
            http_client,
        }
    }

    /// 轮询单个会话状态
    pub async fn poll_session(
        &self,
        session_manager: &SessionManager,
        session_id: &str,
    ) -> AuthResult<OAuthPollingResponse> {
        // 获取会话信息
        let session = session_manager.get_session(session_id).await?;

        // 检查会话状态并返回统一的OAuthPollingResponse
        let response = match session.status.as_str() {
            "pending" => {
                let _expires_in = (session.expires_at.and_utc().timestamp()
                    - chrono::Utc::now().timestamp())
                .max(0);
                OAuthPollingResponse::pending(&session, self.config.default_interval)
            }
            "authorized" => {
                if session.access_token.is_some() {
                    OAuthPollingResponse::authorized(&session)
                } else {
                    // Token交换中
                    let expires_in = (session.expires_at.and_utc().timestamp()
                        - chrono::Utc::now().timestamp())
                    .max(0);
                    OAuthPollingResponse {
                        status: AuthStatus::Pending,
                        access_token: None,
                        refresh_token: None,
                        id_token: None,
                        error: None,
                        error_description: None,
                        expires_in: Some(expires_in),
                        polling_interval: self.config.default_interval,
                    }
                }
            }
            "failed" | "error" => OAuthPollingResponse::error(&session),
            "expired" => OAuthPollingResponse::expired(),
            "revoked" => OAuthPollingResponse::revoked(),
            _ => {
                // 未知状态，返回pending
                let expires_in = (session.expires_at.and_utc().timestamp()
                    - chrono::Utc::now().timestamp())
                .max(0);
                OAuthPollingResponse {
                    status: AuthStatus::Pending,
                    access_token: None,
                    refresh_token: None,
                    id_token: None,
                    error: None,
                    error_description: None,
                    expires_in: Some(expires_in),
                    polling_interval: self.config.default_interval,
                }
            }
        };

        Ok(response)
    }

    /// 持续轮询直到完成或超时
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    pub async fn poll_until_completion(
        &self,
        session_manager: &SessionManager,
        session_id: &str,
    ) -> AuthResult<OAuthTokenResponse> {
        let start_time = Instant::now();
        let timeout_duration = Duration::from_secs(u64::from(self.config.timeout));
        let mut current_interval = self.config.default_interval;
        let mut retry_count = 0;

        loop {
            // 检查超时
            if start_time.elapsed() > timeout_duration {
                return Err(OAuthError::PollingTimeout.into());
            }

            // 轮询状态
            match self.poll_session(session_manager, session_id).await {
                Ok(response) => {
                    match response.status {
                        AuthStatus::Authorized => {
                            if let Some(access_token) = response.access_token {
                                // 构建OAuthTokenResponse
                                return Ok(OAuthTokenResponse {
                                    session_id: session_id.to_string(),
                                    access_token,
                                    refresh_token: response.refresh_token,
                                    id_token: response.id_token,
                                    token_type: "Bearer".to_string(),
                                    expires_in: response
                                        .expires_in
                                        .and_then(|x| i32::try_from(x).ok()),
                                    scopes: Vec::new(),
                                });
                            }
                            // Token交换中，继续轮询
                        }
                        AuthStatus::Error => {
                            return Err(OAuthError::TokenExchangeFailed(format!(
                                "{}: {}",
                                response.error.unwrap_or_default(),
                                response.error_description.unwrap_or_default()
                            ))
                            .into());
                        }
                        AuthStatus::Expired => {
                            return Err(OAuthError::SessionExpired(session_id.to_string()).into());
                        }
                        AuthStatus::Revoked => {
                            return Err(
                                OAuthError::InvalidSession("Session revoked".to_string()).into()
                            );
                        }
                        AuthStatus::Pending => {
                            // 使用建议的轮询间隔
                            current_interval = response
                                .polling_interval
                                .max(self.config.min_interval)
                                .min(self.config.max_interval);
                        }
                    }
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= self.config.max_retries {
                        return Err(e);
                    }
                    // 指数退避
                    current_interval =
                        (f64::from(current_interval) * self.config.backoff_factor) as u32;
                    current_interval = current_interval.min(self.config.max_interval);
                }
            }

            // 等待下次轮询
            sleep(Duration::from_secs(u64::from(current_interval))).await;
        }
    }

    /// 批量轮询多个会话
    pub async fn poll_multiple_sessions(
        &self,
        session_manager: &SessionManager,
        session_ids: &[String],
    ) -> Vec<(String, AuthResult<OAuthPollingResponse>)> {
        let mut results = Vec::new();

        // 并发轮询所有会话
        let futures = session_ids.iter().map(|session_id| {
            let session_id = session_id.clone();
            async move {
                let result = self.poll_session(session_manager, &session_id).await;
                (session_id, result)
            }
        });

        results.extend(futures::future::join_all(futures).await);
        results
    }
}

impl Default for OAuthPollingClient {
    fn default() -> Self {
        Self::new()
    }
}

/// 轮询统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollingStats {
    /// 总轮询次数
    pub total_polls: u64,
    /// 成功完成的会话数
    pub completed_sessions: u64,
    /// 失败的会话数
    pub failed_sessions: u64,
    /// 过期的会话数
    pub expired_sessions: u64,
    /// 平均轮询时间（毫秒）
    pub average_poll_time_ms: u64,
    /// 最后更新时间
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl Default for PollingStats {
    fn default() -> Self {
        Self {
            total_polls: 0,
            completed_sessions: 0,
            failed_sessions: 0,
            expired_sessions: 0,
            average_poll_time_ms: 0,
            last_updated: chrono::Utc::now(),
        }
    }
}

/// 轮询监控器
#[derive(Debug)]
pub struct PollingMonitor {
    stats: std::sync::Arc<std::sync::Mutex<PollingStats>>,
}

impl PollingMonitor {
    /// 创建新的监控器
    #[must_use]
    pub fn new() -> Self {
        Self {
            stats: std::sync::Arc::new(std::sync::Mutex::new(PollingStats::default())),
        }
    }

    /// 记录轮询事件
    pub fn record_poll(&self, duration_ms: u64) {
        if let Ok(mut stats) = self.stats.lock() {
            stats.total_polls += 1;
            stats.average_poll_time_ms = (stats.average_poll_time_ms * (stats.total_polls - 1)
                + duration_ms)
                / stats.total_polls;
            stats.last_updated = chrono::Utc::now();
        }
    }

    /// 记录完成事件
    pub fn record_completion(&self, status: &AuthStatus) {
        if let Ok(mut stats) = self.stats.lock() {
            match status {
                AuthStatus::Authorized => stats.completed_sessions += 1,
                AuthStatus::Error => stats.failed_sessions += 1,
                AuthStatus::Expired => stats.expired_sessions += 1,
                _ => {}
            }
            stats.last_updated = chrono::Utc::now();
        }
    }

    /// 获取统计信息
    #[must_use]
    pub fn get_stats(&self) -> PollingStats {
        self.stats
            .lock()
            .map(|stats| stats.clone())
            .unwrap_or_default()
    }
}

impl Default for PollingMonitor {
    fn default() -> Self {
        Self::new()
    }
}
