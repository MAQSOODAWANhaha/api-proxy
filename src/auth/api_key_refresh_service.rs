//! OAuth Token 刷新执行服务
//!
//! 负责对单个 OAuth 会话执行刷新动作，并维护刷新统计信息。
//! 不直接访问数据库，也不承担调度与状态管理职责。

use crate::auth::oauth_client::OAuthClient;
use crate::error::Result;
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, lerror, linfo};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// OAuth Token 刷新执行器
#[derive(Debug)]
pub struct ApiKeyRefreshService {
    oauth_client: Arc<OAuthClient>,
    refresh_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
}

/// 刷新结果
#[derive(Debug, Clone)]
pub struct TokenRefreshResult {
    pub success: bool,
    pub new_access_token: Option<String>,
    pub new_expires_at: Option<DateTime<Utc>>,
    pub new_refresh_token: Option<String>,
    pub error_message: Option<String>,
}

impl ApiKeyRefreshService {
    #[must_use]
    pub fn new(oauth_client: Arc<OAuthClient>) -> Self {
        Self {
            oauth_client,
            refresh_locks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 执行单次 Token 刷新
    #[allow(clippy::cognitive_complexity)]
    pub async fn execute_token_refresh(
        &self,
        request_id: String,
        session_id: &str,
    ) -> Result<TokenRefreshResult> {
        let stage = LogStage::BackgroundTask;
        let component = LogComponent::OAuth;

        let refresh_lock = self.get_refresh_lock(session_id).await;
        let _guard = refresh_lock.lock().await;

        let outcome = match self.oauth_client.refresh_token(session_id).await {
            Ok(response) => {
                let access_token = response.access_token.clone();
                let expires_at = response.expires_in.map(|secs| {
                    let secs = i64::from(secs);
                    Utc::now() + Duration::seconds(secs)
                });

                linfo!(
                    &request_id,
                    stage,
                    component,
                    "refresh_success",
                    "OAuth token refreshed successfully",
                    session_id = %session_id,
                    has_refresh_token = response.refresh_token.is_some()
                );

                TokenRefreshResult {
                    success: true,
                    new_access_token: Some(access_token),
                    new_expires_at: expires_at,
                    new_refresh_token: response.refresh_token,
                    error_message: None,
                }
            }
            Err(err) => {
                err.log();
                lerror!(
                    &request_id,
                    stage,
                    component,
                    "refresh_failed",
                    "Failed to refresh OAuth token",
                    session_id = %session_id,
                    error = %err
                );

                TokenRefreshResult {
                    success: false,
                    new_access_token: None,
                    new_expires_at: None,
                    new_refresh_token: None,
                    error_message: Some(err.to_string()),
                }
            }
        };

        if !outcome.success {
            ldebug!(
                &request_id,
                stage,
                component,
                "refresh_result",
                "OAuth token refresh did not succeed",
                session_id = %session_id,
                error_message = ?outcome.error_message
            );
        }

        Ok(outcome)
    }

    async fn get_refresh_lock(&self, session_id: &str) -> Arc<Mutex<()>> {
        let mut locks = self.refresh_locks.write().await;
        locks
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}
