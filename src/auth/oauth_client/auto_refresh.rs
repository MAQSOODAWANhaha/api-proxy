//! # OAuth令牌自动刷新模块
//!
//! 实现智能token生命周期管理，当token即将过期时自动刷新
//! 提供对调用者透明的token获取接口

use super::{OAuthError, OAuthResult, OAuthTokenResponse};
use super::providers::OAuthProviderManager;
use super::session_manager::SessionManager;
use super::token_exchange::TokenExchangeClient;
use entity::oauth_client_sessions;
use chrono::{Duration, Utc};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, warn, error};

/// Token自动刷新管理器
#[derive(Debug)]
pub struct AutoRefreshManager {
    session_manager: SessionManager,
    provider_manager: OAuthProviderManager,
    token_exchange_client: TokenExchangeClient,
    /// 防止同一session并发刷新的锁
    refresh_locks: Arc<Mutex<std::collections::HashMap<String, Arc<Mutex<()>>>>>,
}

/// Token刷新策略配置
#[derive(Debug, Clone)]
pub struct RefreshPolicy {
    /// 提前刷新时间（token过期前多少秒开始刷新）
    pub refresh_threshold_seconds: i64,
    /// 最大重试次数
    pub max_retry_attempts: u32,
    /// 重试间隔（秒）
    pub retry_interval_seconds: u64,
}

impl Default for RefreshPolicy {
    fn default() -> Self {
        Self {
            refresh_threshold_seconds: 300, // 5分钟
            max_retry_attempts: 3,
            retry_interval_seconds: 5,
        }
    }
}

impl AutoRefreshManager {
    /// 创建自动刷新管理器
    pub fn new(
        session_manager: SessionManager,
        provider_manager: OAuthProviderManager,
        token_exchange_client: TokenExchangeClient,
    ) -> Self {
        Self {
            session_manager,
            provider_manager,
            token_exchange_client,
            refresh_locks: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// 智能获取有效的访问令牌
    /// 如果token即将过期，会自动刷新后返回新token
    pub async fn get_valid_access_token(
        &self,
        session_id: &str,
        policy: Option<RefreshPolicy>,
    ) -> OAuthResult<Option<String>> {
        let policy = policy.unwrap_or_default();
        
        // 获取会话信息
        let session = self.session_manager.get_session(session_id).await?;
        
        if session.status != "completed" {
            debug!("Session {} is not completed, status: {}", session_id, session.status);
            return Ok(None);
        }

        // 检查是否需要刷新token
        if !self.should_refresh_token(&session, &policy)? {
            // token仍然有效，直接返回
            return Ok(session.access_token);
        }

        debug!("Token for session {} needs refresh", session_id);
        
        // 检查是否有refresh_token
        if session.refresh_token.is_none() {
            warn!("Session {} has no refresh token, cannot auto-refresh", session_id);
            return Ok(None);
        }

        // 执行自动刷新
        match self.auto_refresh_token(session_id, &policy).await {
            Ok(token_response) => {
                debug!("Successfully auto-refreshed token for session {}", session_id);
                Ok(Some(token_response.access_token))
            }
            Err(e) => {
                error!("Failed to auto-refresh token for session {}: {}", session_id, e);
                // 刷新失败，返回原token（可能已过期，由调用者处理）
                Ok(session.access_token)
            }
        }
    }

    /// 批量智能刷新多个会话的token
    pub async fn batch_refresh_tokens(
        &self,
        session_ids: Vec<String>,
        policy: Option<RefreshPolicy>,
    ) -> Vec<(String, OAuthResult<Option<String>>)> {
        let policy = policy.unwrap_or_default();
        let mut results = Vec::new();

        // 并发处理多个会话
        let futures = session_ids.into_iter().map(|session_id| {
            let session_id_clone = session_id.clone();
            let policy_clone = policy.clone();
            async move {
                let result = self.get_valid_access_token(&session_id, Some(policy_clone)).await;
                (session_id_clone, result)
            }
        });

        for future in futures {
            results.push(future.await);
        }
        results
    }

    /// 获取所有用户即将过期的会话并刷新
    pub async fn refresh_expiring_sessions_for_user(
        &self,
        user_id: i32,
        policy: Option<RefreshPolicy>,
    ) -> OAuthResult<Vec<(String, OAuthResult<OAuthTokenResponse>)>> {
        let policy = policy.unwrap_or_default();
        
        // 获取用户的所有完成会话
        let sessions = self.session_manager.list_user_active_sessions_flexible(
            user_id, 
            None, 
            None
        ).await?;
        
        let mut results = Vec::new();
        
        for session in sessions {
            if self.should_refresh_token(&session, &policy)? && session.refresh_token.is_some() {
                let result = self.auto_refresh_token(&session.session_id, &policy).await;
                results.push((session.session_id, result));
            }
        }
        
        Ok(results)
    }

    // 私有方法

    /// 判断是否需要刷新token
    fn should_refresh_token(
        &self,
        session: &oauth_client_sessions::Model,
        policy: &RefreshPolicy,
    ) -> OAuthResult<bool> {
        // 检查会话是否已过期
        if session.is_expired() {
            return Ok(true);
        }

        // 检查是否在刷新阈值范围内
        let now = Utc::now().naive_utc();
        let expires_at = session.expires_at;
        let threshold = Duration::try_seconds(policy.refresh_threshold_seconds)
            .unwrap_or_default();

        // 如果token将在阈值时间内过期，则需要刷新
        Ok(expires_at <= now + threshold)
    }

    /// 执行自动token刷新
    async fn auto_refresh_token(
        &self,
        session_id: &str,
        policy: &RefreshPolicy,
    ) -> OAuthResult<OAuthTokenResponse> {
        // 获取会话专属锁，防止并发刷新同一token
        let lock = {
            let mut locks = self.refresh_locks.lock().await;
            locks.entry(session_id.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        let _session_lock = lock.lock().await;

        // 重新检查会话状态（可能已被其他线程刷新）
        let current_session = self.session_manager.get_session(session_id).await?;
        if !self.should_refresh_token(&current_session, policy)? {
            debug!("Token for session {} was already refreshed", session_id);
            if let Some(token) = current_session.access_token {
                return Ok(OAuthTokenResponse {
                    access_token: token,
                    refresh_token: current_session.refresh_token,
                    id_token: current_session.id_token,
                    token_type: current_session.token_type.unwrap_or("Bearer".to_string()),
                    expires_in: current_session.expires_in,
                    scopes: Vec::new(), // TODO: 从session中解析scopes
                });
            }
        }

        // 执行刷新重试逻辑
        let mut last_error = OAuthError::TokenExchangeFailed("No attempts made".to_string());
        
        for attempt in 1..=policy.max_retry_attempts {
            debug!("Attempting token refresh for session {} (attempt {}/{})", 
                   session_id, attempt, policy.max_retry_attempts);

            match self.token_exchange_client.refresh_token(
                &self.provider_manager,
                &self.session_manager,
                session_id,
            ).await {
                Ok(token_response) => {
                    debug!("Successfully refreshed token for session {} on attempt {}", 
                           session_id, attempt);
                    return Ok(token_response);
                }
                Err(e) => {
                    warn!("Token refresh attempt {} failed for session {}: {}", 
                          attempt, session_id, e);
                    last_error = e;
                    
                    // 如果不是最后一次尝试，则等待重试间隔
                    if attempt < policy.max_retry_attempts {
                        tokio::time::sleep(std::time::Duration::from_secs(policy.retry_interval_seconds)).await;
                    }
                }
            }
        }

        // 清理锁（不再需要）
        {
            let mut locks = self.refresh_locks.lock().await;
            locks.remove(session_id);
        }

        Err(last_error)
    }
}

/// 扩展SessionManager以支持智能token获取
impl SessionManager {
    /// 智能获取有效访问令牌（自动刷新版本）
    /// 
    /// 替代原有的get_valid_access_token方法
    /// 当token即将过期时会自动刷新
    pub async fn get_valid_access_token_auto_refresh(
        &self,
        session_id: &str,
        provider_manager: &OAuthProviderManager,
        token_exchange_client: &TokenExchangeClient,
    ) -> OAuthResult<Option<String>> {
        let auto_refresh_manager = AutoRefreshManager::new(
            self.clone(),
            provider_manager.clone(),
            token_exchange_client.clone(),
        );
        
        auto_refresh_manager.get_valid_access_token(session_id, None).await
    }

    /// 带自定义策略的智能token获取
    pub async fn get_valid_access_token_with_policy(
        &self,
        session_id: &str,
        provider_manager: &OAuthProviderManager,
        token_exchange_client: &TokenExchangeClient,
        policy: RefreshPolicy,
    ) -> OAuthResult<Option<String>> {
        let auto_refresh_manager = AutoRefreshManager::new(
            self.clone(),
            provider_manager.clone(),
            token_exchange_client.clone(),
        );
        
        auto_refresh_manager.get_valid_access_token(session_id, Some(policy)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refresh_policy_default() {
        let policy = RefreshPolicy::default();
        assert_eq!(policy.refresh_threshold_seconds, 300);
        assert_eq!(policy.max_retry_attempts, 3);
        assert_eq!(policy.retry_interval_seconds, 5);
    }

    #[test]
    fn test_refresh_policy_custom() {
        let policy = RefreshPolicy {
            refresh_threshold_seconds: 600, // 10分钟
            max_retry_attempts: 5,
            retry_interval_seconds: 10,
        };
        assert_eq!(policy.refresh_threshold_seconds, 600);
        assert_eq!(policy.max_retry_attempts, 5);
        assert_eq!(policy.retry_interval_seconds, 10);
    }
}