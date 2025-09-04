//! # OAuth轮询状态机制
//!
//! 实现客户端侧OAuth轮询机制，替代传统的服务器回调方式
//! 客户端定期轮询服务器检查OAuth授权状态，避免对特定域名的依赖
//!
//! ## 轮询流程
//! 1. 客户端获取授权URL并开始轮询
//! 2. 服务器检查会话状态（pending/completed/failed/expired）
//! 3. 授权完成后，自动进行Token交换
//! 4. 返回最终的OAuth令牌或错误信息

use super::session_manager::SessionManager;
use super::{OAuthError, OAuthResult, OAuthTokenResponse};
use entity::oauth_client_sessions;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// 轮询状态枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "data")]
pub enum PollingStatus {
    /// 等待用户授权
    #[serde(rename = "pending")]
    Pending {
        /// 剩余过期时间（秒）
        expires_in: i64,
        /// 建议轮询间隔（秒）
        interval: u32,
    },
    /// 授权完成，令牌已获取
    #[serde(rename = "completed")]
    Completed {
        /// OAuth令牌响应
        token_response: OAuthTokenResponse,
    },
    /// 授权失败
    #[serde(rename = "failed")]
    Failed {
        /// 错误代码
        error: String,
        /// 错误描述
        error_description: Option<String>,
    },
    /// 会话已过期
    #[serde(rename = "expired")]
    Expired,
    /// 等待Token交换中
    #[serde(rename = "exchanging")]
    Exchanging,
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

/// OAuth轮询客户端
#[derive(Debug)]
pub struct OAuthPollingClient {
    config: PollingConfig,
    http_client: reqwest::Client,
}

impl OAuthPollingClient {
    /// 创建新的轮询客户端
    pub fn new() -> Self {
        Self::with_config(PollingConfig::default())
    }

    /// 使用指定配置创建轮询客户端
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
    ) -> OAuthResult<PollingStatus> {
        // 获取会话信息
        let session = session_manager.get_session(session_id).await?;

        // 检查会话状态
        match self.analyze_session_status(&session).await? {
            PollingStatus::Pending {
                expires_in,
                interval,
            } => Ok(PollingStatus::Pending {
                expires_in,
                interval,
            }),
            PollingStatus::Completed { .. } => {
                // 会话已完成，返回令牌信息
                self.get_completed_session_response(&session).await
            }
            other_status => Ok(other_status),
        }
    }

    /// 持续轮询直到完成或超时
    pub async fn poll_until_completion(
        &self,
        session_manager: &SessionManager,
        session_id: &str,
    ) -> OAuthResult<OAuthTokenResponse> {
        let start_time = Instant::now();
        let timeout_duration = Duration::from_secs(self.config.timeout as u64);
        let mut current_interval = self.config.default_interval;
        let mut retry_count = 0;

        loop {
            // 检查超时
            if start_time.elapsed() > timeout_duration {
                return Err(OAuthError::PollingTimeout);
            }

            // 轮询状态
            match self.poll_session(session_manager, session_id).await {
                Ok(PollingStatus::Completed { token_response }) => {
                    return Ok(token_response);
                }
                Ok(PollingStatus::Failed {
                    error,
                    error_description,
                }) => {
                    return Err(OAuthError::TokenExchangeFailed(format!(
                        "{}: {}",
                        error,
                        error_description.unwrap_or_default()
                    )));
                }
                Ok(PollingStatus::Expired) => {
                    return Err(OAuthError::SessionExpired(session_id.to_string()));
                }
                Ok(PollingStatus::Pending { interval, .. }) => {
                    // 使用服务器建议的间隔或当前间隔
                    current_interval = interval
                        .max(self.config.min_interval)
                        .min(self.config.max_interval);
                }
                Ok(PollingStatus::Exchanging) => {
                    // Token交换中，继续轮询
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= self.config.max_retries {
                        return Err(e);
                    }
                    // 指数退避
                    current_interval =
                        ((current_interval as f64) * self.config.backoff_factor) as u32;
                    current_interval = current_interval.min(self.config.max_interval);
                }
            }

            // 等待下次轮询
            sleep(Duration::from_secs(current_interval as u64)).await;
        }
    }

    /// 批量轮询多个会话
    pub async fn poll_multiple_sessions(
        &self,
        session_manager: &SessionManager,
        session_ids: &[String],
    ) -> Vec<(String, OAuthResult<PollingStatus>)> {
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

    // 私有方法

    /// 分析会话状态
    async fn analyze_session_status(
        &self,
        session: &oauth_client_sessions::Model,
    ) -> OAuthResult<PollingStatus> {
        // 检查是否过期
        if session.is_expired() {
            return Ok(PollingStatus::Expired);
        }

        // 检查状态
        match session.status.as_str() {
            "completed" => {
                if session.access_token.is_some() {
                    Ok(PollingStatus::Completed {
                        token_response: self.build_token_response(session)?,
                    })
                } else {
                    Ok(PollingStatus::Exchanging)
                }
            }
            "failed" => Ok(PollingStatus::Failed {
                error: "authorization_failed".to_string(),
                error_description: session.error_message.clone(),
            }),
            "pending" => {
                let expires_in = (session.expires_at.and_utc().timestamp()
                    - chrono::Utc::now().timestamp())
                .max(0);
                Ok(PollingStatus::Pending {
                    expires_in,
                    interval: self.config.default_interval,
                })
            }
            _ => Ok(PollingStatus::Pending {
                expires_in: 0,
                interval: self.config.default_interval,
            }),
        }
    }

    /// 获取已完成会话的响应
    async fn get_completed_session_response(
        &self,
        session: &oauth_client_sessions::Model,
    ) -> OAuthResult<PollingStatus> {
        if let Some(_) = &session.access_token {
            Ok(PollingStatus::Completed {
                token_response: self.build_token_response(session)?,
            })
        } else {
            Ok(PollingStatus::Exchanging)
        }
    }

    /// 构建令牌响应
    fn build_token_response(
        &self,
        session: &oauth_client_sessions::Model,
    ) -> OAuthResult<OAuthTokenResponse> {
        let access_token = session
            .access_token
            .as_ref()
            .ok_or_else(|| OAuthError::InvalidSession("Missing access token".to_string()))?
            .clone();

        Ok(OAuthTokenResponse {
            access_token,
            refresh_token: session.refresh_token.clone(),
            id_token: session.id_token.clone(),
            token_type: session
                .token_type
                .clone()
                .unwrap_or_else(|| "Bearer".to_string()),
            expires_in: session.expires_in,
            scopes: Vec::new(), // TODO: 从session中解析scopes
        })
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
    pub fn record_completion(&self, status: &PollingStatus) {
        if let Ok(mut stats) = self.stats.lock() {
            match status {
                PollingStatus::Completed { .. } => stats.completed_sessions += 1,
                PollingStatus::Failed { .. } => stats.failed_sessions += 1,
                PollingStatus::Expired => stats.expired_sessions += 1,
                _ => {}
            }
            stats.last_updated = chrono::Utc::now();
        }
    }

    /// 获取统计信息
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
