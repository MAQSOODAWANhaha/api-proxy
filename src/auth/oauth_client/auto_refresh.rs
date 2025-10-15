//! # `OAuth令牌自动刷新模块`
//!
//! 实现智能token生命周期管理，当token即将过期时自动刷新
//! 提供对调用者透明的token获取接口

use super::providers::OAuthProviderManager;
use super::session_manager::SessionManager;
use super::token_exchange::TokenExchangeClient;
use super::{OAuthError, OAuthTokenResponse};
use crate::error::{AuthResult, ProxyError};
use crate::auth::types::AuthStatus;
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, lerror, linfo, lwarn};
use chrono::{Duration, Utc};
use entity::{oauth_client_sessions, user_provider_keys};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Token自动刷新管理器
#[derive(Debug)]
pub struct AutoRefreshManager {
    session_manager: SessionManager,
    provider_manager: OAuthProviderManager,
    token_exchange_client: TokenExchangeClient,
    /// 防止同一session并发刷新的锁
    refresh_locks: Arc<Mutex<std::collections::HashMap<String, Arc<Mutex<()>>>>>,
    /// 数据库连接（用于验证关联）
    db: Arc<sea_orm::DatabaseConnection>,
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
    #[must_use]
    pub fn new(
        session_manager: SessionManager,
        provider_manager: OAuthProviderManager,
        token_exchange_client: TokenExchangeClient,
        db: Arc<sea_orm::DatabaseConnection>,
    ) -> Self {
        Self {
            session_manager,
            provider_manager,
            token_exchange_client,
            refresh_locks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            db,
        }
    }

    /// 智能获取有效的访问令牌
    /// 如果token即将过期，会自动刷新后返回新token
    #[allow(clippy::cognitive_complexity)]
    pub async fn get_valid_access_token(
        &self,
        session_id: &str,
        policy: Option<RefreshPolicy>,
    ) -> AuthResult<Option<String>> {
        let policy = policy.unwrap_or_default();

        // 获取会话信息
        let session = self.session_manager.get_session(session_id).await?;

        if session.status != AuthStatus::Authorized.to_string() {
            ldebug!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "session_not_authorized",
                &format!(
                    "Session {} is not authorized, status: {}",
                    session_id, session.status
                )
            );
            return Ok(None);
        }

        // 🔒 提前进行孤立检查：对于创建超过10分钟的会话，检查是否有关联
        if !self.validate_session_association(&session).await? {
            // 会话已被删除或无关联
            return Ok(None);
        }

        // 检查是否需要刷新token
        if !Self::should_refresh_token(&session, &policy) {
            // token仍然有效，直接返回
            return Ok(session.access_token);
        }

        ldebug!(
            "system",
            LogStage::Authentication,
            LogComponent::OAuth,
            "token_needs_refresh",
            &format!("Token for session {session_id} needs refresh")
        );

        // 检查是否有refresh_token
        if session.refresh_token.is_none() {
            lwarn!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "no_refresh_token",
                &format!("Session {session_id} has no refresh token, cannot auto-refresh")
            );
            return Ok(None);
        }

        ldebug!(
            "system",
            LogStage::Authentication,
            LogComponent::OAuth,
            "start_token_refresh",
            &format!("Session {session_id} 通过关联验证，开始执行token刷新")
        );

        // 执行自动刷新
        match self.auto_refresh_token(session_id, &policy).await {
            Ok(token_response) => {
                ldebug!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::OAuth,
                    "token_refresh_ok",
                    &format!("Successfully auto-refreshed token for session {session_id}")
                );
                Ok(Some(token_response.access_token))
            }
            Err(e) => {
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::OAuth,
                    "token_refresh_fail",
                    &format!("Failed to auto-refresh token for session {session_id}: {e}")
                );
                // 刷新失败：如已过期则返回None，否则返回当前token
                let now = Utc::now().naive_utc();
                if session.expires_at <= now {
                    Ok(None)
                } else {
                    Ok(session.access_token)
                }
            }
        }
    }

    /// 批量智能刷新多个会话的token
    pub async fn batch_refresh_tokens(
        &self,
        session_ids: Vec<String>,
        policy: Option<RefreshPolicy>,
    ) -> Vec<(String, AuthResult<Option<String>>)> {
        let policy = policy.unwrap_or_default();
        let mut results = Vec::new();

        // 并发处理多个会话
        let futures = session_ids.into_iter().map(|session_id| {
            let session_id_clone = session_id.clone();
            let policy_clone = policy.clone();
            async move {
                let result = self
                    .get_valid_access_token(&session_id, Some(policy_clone))
                    .await;
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
    ) -> AuthResult<Vec<(String, AuthResult<OAuthTokenResponse>)>> {
        let policy = policy.unwrap_or_default();

        // 获取用户的所有完成会话
        let sessions = self
            .session_manager
            .list_user_active_sessions_flexible(user_id, None, None)
            .await?;

        let mut results = Vec::new();

        for session in sessions {
            // 跳过新创建的会话（10分钟内）或验证关联失败的会话
            if !self
                .validate_session_association(&session)
                .await
                .unwrap_or(false)
            {
                continue;
            }

            if Self::should_refresh_token(&session, &policy) && session.refresh_token.is_some() {
                let result = self.auto_refresh_token(&session.session_id, &policy).await;
                results.push((session.session_id, result));
            }
        }

        Ok(results)
    }

    // 私有方法

    /// 判断是否需要刷新token
    fn should_refresh_token(
        session: &oauth_client_sessions::Model,
        policy: &RefreshPolicy,
    ) -> bool {
        // 检查会话是否已过期
        if session.is_expired() {
            return true;
        }

        // 检查是否在刷新阈值范围内
        let now = Utc::now().naive_utc();
        let expires_at = session.expires_at;
        let threshold = Duration::try_seconds(policy.refresh_threshold_seconds).unwrap_or_default();

        // 如果token将在阈值时间内过期，则需要刷新
        expires_at <= now + threshold
    }

    /// 执行自动token刷新
    #[allow(clippy::cognitive_complexity)]
    #[allow(clippy::too_many_lines)]
    async fn auto_refresh_token(
        &self,
        session_id: &str,
        policy: &RefreshPolicy,
    ) -> AuthResult<OAuthTokenResponse> {
        // 获取会话专属锁，防止并发刷新同一token
        let lock = {
            let mut locks = self.refresh_locks.lock().await;
            locks
                .entry(session_id.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        let _session_lock = lock.lock().await;

        // 重新检查会话状态（可能已被其他线程刷新）
        let current_session = self.session_manager.get_session(session_id).await?;

        // 🔥 关键检查：验证该会话是否还有对应的user_provider_keys关联
        if !self.validate_session_association(&current_session).await? {
            lwarn!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "session_orphaned",
                &format!("Session {session_id} 没有对应的user_provider_keys关联，跳过刷新")
            );
            // 不在刷新路径进行删除，交由后台清理任务处理
            return Err(
                OAuthError::InvalidSession(format!("Session {session_id} is orphaned")).into(),
            );
        }

        if !Self::should_refresh_token(&current_session, policy) {
            ldebug!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "token_already_refreshed",
                &format!("Token for session {session_id} was already refreshed")
            );
            if let Some(token) = current_session.access_token {
                // 清理锁映射
                {
                    let mut locks = self.refresh_locks.lock().await;
                    locks.remove(session_id);
                }
                return Ok(OAuthTokenResponse {
                    session_id: session_id.to_string(),
                    access_token: token,
                    refresh_token: current_session.refresh_token,
                    id_token: current_session.id_token,
                    token_type: current_session
                        .token_type
                        .unwrap_or_else(|| "Bearer".to_string()),
                    expires_in: current_session.expires_in,
                    scopes: Vec::new(), // TODO: 从session中解析scopes
                });
            }
        }

        // 执行刷新重试逻辑
        let mut last_error =
            ProxyError::from(OAuthError::TokenExchangeFailed("No attempts made".to_string()));

        for attempt in 1..=policy.max_retry_attempts {
            ldebug!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "token_refresh_attempt",
                &format!(
                    "Attempting token refresh for session {} (attempt {}/{})",
                    session_id, attempt, policy.max_retry_attempts
                )
            );

            match self
                .token_exchange_client
                .refresh_token(&self.provider_manager, &self.session_manager, session_id)
                .await
            {
                Ok(token_response) => {
                    ldebug!(
                        "system",
                        LogStage::Authentication,
                        LogComponent::OAuth,
                        "token_refresh_ok",
                        &format!(
                            "Successfully refreshed token for session {session_id} on attempt {attempt}"
                        )
                    );
                    // 成功后清理锁映射
                    {
                        let mut locks = self.refresh_locks.lock().await;
                        locks.remove(session_id);
                    }
                    return Ok(token_response);
                }
                Err(e) => {
                    lwarn!(
                        "system",
                        LogStage::Authentication,
                        LogComponent::OAuth,
                        "token_refresh_attempt_fail",
                        &format!(
                            "Token refresh attempt {attempt} failed for session {session_id}: {e}"
                        )
                    );
                    last_error = e;

                    // 如果不是最后一次尝试，则等待重试间隔
                    if attempt < policy.max_retry_attempts {
                        tokio::time::sleep(std::time::Duration::from_secs(
                            policy.retry_interval_seconds,
                        ))
                        .await;
                    }
                }
            }
        }

        // 清理锁（失败路径）
        {
            let mut locks = self.refresh_locks.lock().await;
            locks.remove(session_id);
        }

        Err(last_error)
    }

    /// `验证会话是否有对应的user_provider_keys关联`
    /// 如果没有关联且创建超过5分钟，说明这是一个孤立的会话，会被自动删除
    #[allow(clippy::cognitive_complexity)]
    async fn validate_session_association(
        &self,
        session: &oauth_client_sessions::Model,
    ) -> AuthResult<bool> {
        // 🔒 安全检查：只处理创建超过5分钟的会话，避免误删正在处理的新会话
        let now = Utc::now().naive_utc();
        let session_age = now.signed_duration_since(session.created_at);
        let min_age_threshold = Duration::try_minutes(5).unwrap_or_default();

        if session_age < min_age_threshold {
            ldebug!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "skip_orphan_check",
                &format!(
                    "Session {} 创建时间不足5分钟 ({}分钟)，跳过孤立检查",
                    session.session_id,
                    session_age.num_minutes()
                )
            );
            return Ok(true);
        }

        // 查找是否有user_provider_keys记录引用了这个session_id
        let associated_key = user_provider_keys::Entity::find()
            .filter(user_provider_keys::Column::UserId.eq(session.user_id))
            .filter(user_provider_keys::Column::AuthType.eq("oauth"))
            .filter(user_provider_keys::Column::ApiKey.eq(&session.session_id)) // OAuth类型的api_key存储session_id
            .one(self.db.as_ref())
            .await
            .map_err(|e| OAuthError::DatabaseError(format!("验证会话关联失败: {e}")))?;

        let has_association = associated_key.is_some();

        if has_association {
            ldebug!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "session_association_ok",
                &format!(
                    "Session {} 有有效的user_provider_keys关联",
                    session.session_id
                )
            );
        } else {
            linfo!(
                "system",
                LogStage::Authentication,
                LogComponent::OAuth,
                "orphan_session_cleanup",
                &format!(
                    "Session {} 创建 {} 分钟后仍无user_provider_keys关联，判定为孤立会话，开始清理",
                    session.session_id,
                    session_age.num_minutes()
                )
            );

            // 删除孤立会话
            if let Err(e) = self
                .session_manager
                .delete_session(&session.session_id, session.user_id)
                .await
            {
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::OAuth,
                    "orphan_session_delete_fail",
                    &format!("删除孤立会话失败 {}: {}", session.session_id, e)
                );
            } else {
                linfo!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::OAuth,
                    "orphan_session_delete_ok",
                    &format!("成功删除孤立会话 {}", session.session_id)
                );
            }
        }

        Ok(has_association)
    }
}

/// `扩展SessionManager以支持智能token获取`
impl SessionManager {
    /// 智能获取有效访问令牌（自动刷新版本）
    ///
    /// `替代原有的get_valid_access_token方法`
    /// 当token即将过期时会自动刷新
    pub async fn get_valid_access_token_auto_refresh(
        &self,
        session_id: &str,
        provider_manager: &OAuthProviderManager,
        token_exchange_client: &TokenExchangeClient,
    ) -> AuthResult<Option<String>> {
        let auto_refresh_manager = AutoRefreshManager::new(
            self.clone(),
            provider_manager.clone(),
            token_exchange_client.clone(),
            self.db.clone(),
        );

        auto_refresh_manager
            .get_valid_access_token(session_id, None)
            .await
    }

    /// 带自定义策略的智能token获取
    pub async fn get_valid_access_token_with_policy(
        &self,
        session_id: &str,
        provider_manager: &OAuthProviderManager,
        token_exchange_client: &TokenExchangeClient,
        policy: RefreshPolicy,
    ) -> AuthResult<Option<String>> {
        let auto_refresh_manager = AutoRefreshManager::new(
            self.clone(),
            provider_manager.clone(),
            token_exchange_client.clone(),
            self.db.clone(),
        );

        auto_refresh_manager
            .get_valid_access_token(session_id, Some(policy))
            .await
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
