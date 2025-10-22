//! # OAuth Token智能刷新服务
//!
//! `实现OAuth` token的智能刷新逻辑，支持主动和被动两种刷新策略：
//! - 被动刷新：在获取token时检查过期状态并自动刷新
//! - 主动刷新：后台定期检查即将过期的token并提前刷新

use crate::auth::oauth_client::OAuthClient;
use crate::auth::types::AuthStatus;
use crate::error::Result;
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, lerror, linfo, lwarn};
use chrono::{DateTime, Duration, Utc};
use entity::{oauth_client_sessions, user_provider_keys};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

const REFRESH_BUFFER_MINUTES: i64 = 5;
const RETRY_INTERVAL_SECONDS: u64 = 30;
const PENDING_EXPIRE_MINUTES: i64 = 30;
const EXPIRED_RETENTION_DAYS: i64 = 7;

/// OAuth Token智能刷新服务
///
/// 核心职责：
/// 1. 被动刷新：使用时检查token是否过期并自动刷新
/// 2. 主动刷新：后台任务定期检查即将过期的token并提前刷新
/// 3. 刷新锁：防止并发刷新同一个token
/// 4. 失败重试：token刷新失败时的智能重试机制
pub struct OAuthTokenRefreshService {
    db: Arc<DatabaseConnection>,
    oauth_client: Arc<OAuthClient>,

    /// `刷新锁：session_id` -> Mutex，防止并发刷新同一个token
    refresh_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,

    /// 刷新统计信息
    refresh_stats: Arc<RwLock<RefreshStats>>,
}

/// 刷新统计信息
#[derive(Debug, Default, Clone)]
pub struct RefreshStats {
    /// 总刷新次数
    pub total_refreshes: u64,

    /// 成功刷新次数
    pub successful_refreshes: u64,

    /// 失败刷新次数
    pub failed_refreshes: u64,

    /// 被动刷新次数（使用时触发）
    pub passive_refreshes: u64,

    /// 主动刷新次数（后台任务触发）
    pub active_refreshes: u64,

    /// 最后刷新时间
    pub last_refresh_time: Option<DateTime<Utc>>,

    /// 最后失败时间
    pub last_failure_time: Option<DateTime<Utc>>,

    /// 当前正在刷新的token数量
    pub refreshing_tokens: u32,
}

/// 计划中的 Token 刷新任务
#[derive(Debug, Clone)]
pub struct ScheduledTokenRefresh {
    /// 要刷新的会话 ID
    pub session_id: String,
    /// 下一次刷新的时间
    pub next_refresh_at: DateTime<Utc>,
    /// 当前已知的过期时间
    pub expires_at: DateTime<Utc>,
}

/// Token刷新结果
#[derive(Debug, Clone)]
pub struct TokenRefreshResult {
    /// 是否成功刷新
    pub success: bool,

    /// 新的访问token（如果刷新成功）
    pub new_access_token: Option<String>,

    /// 新的过期时间（如果刷新成功）
    pub new_expires_at: Option<DateTime<Utc>>,

    /// 错误信息（如果刷新失败）
    pub error_message: Option<String>,

    /// 是否应该重试
    pub should_retry: bool,

    /// 刷新类型
    pub refresh_type: RefreshType,
}

/// 刷新类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshType {
    /// 被动刷新：使用时检查过期并刷新
    Passive,
    /// 主动刷新：后台任务提前刷新
    Active,
}

impl OAuthTokenRefreshService {
    /// `创建新的OAuth` Token智能刷新服务
    #[must_use]
    pub fn new(db: Arc<DatabaseConnection>, oauth_client: Arc<OAuthClient>) -> Self {
        Self {
            db,
            oauth_client,
            refresh_locks: Arc::new(RwLock::new(HashMap::new())),
            refresh_stats: Arc::new(RwLock::new(RefreshStats::default())),
        }
    }

    /// 列出所有授权且具备刷新条件的会话
    pub async fn list_authorized_sessions(&self) -> Result<Vec<oauth_client_sessions::Model>> {
        let linked_session_ids: Vec<String> = user_provider_keys::Entity::find()
            .filter(user_provider_keys::Column::AuthType.eq("oauth"))
            .select_only()
            .column(user_provider_keys::Column::ApiKey)
            .into_tuple::<String>()
            .all(&*self.db)
            .await
            .map_err(|e| crate::error!(Database, Query(e)))?;

        if linked_session_ids.is_empty() {
            return Ok(Vec::new());
        }

        oauth_client_sessions::Entity::find()
            .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
            .filter(oauth_client_sessions::Column::RefreshToken.is_not_null())
            .filter(oauth_client_sessions::Column::AccessToken.is_not_null())
            .filter(oauth_client_sessions::Column::SessionId.is_in(linked_session_ids))
            .all(&*self.db)
            .await
            .map_err(|e| crate::error!(Database, Query(e)))
    }

    /// 清理过期或孤立的 OAuth 会话记录
    #[allow(clippy::cognitive_complexity)]
    pub async fn cleanup_stale_sessions(&self) -> Result<()> {
        // 删除超时 pending 会话
        let pending_cutoff = Utc::now() - chrono::Duration::minutes(PENDING_EXPIRE_MINUTES);
        let delete_pending = oauth_client_sessions::Entity::delete_many()
            .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Pending.to_string()))
            .filter(oauth_client_sessions::Column::CreatedAt.lt(pending_cutoff.naive_utc()))
            .exec(&*self.db)
            .await
            .map_err(|e| crate::error!(Database, Query(e)))?;

        if delete_pending.rows_affected > 0 {
            linfo!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "cleanup_pending_sessions",
                &format!(
                    "Deleted {} expired pending OAuth sessions",
                    delete_pending.rows_affected
                )
            );
        }

        // 删除长时间保留的 expired 会话
        let expired_cutoff = Utc::now() - chrono::Duration::days(EXPIRED_RETENTION_DAYS);
        let delete_expired = oauth_client_sessions::Entity::delete_many()
            .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Expired.to_string()))
            .filter(oauth_client_sessions::Column::UpdatedAt.lt(expired_cutoff.naive_utc()))
            .exec(&*self.db)
            .await
            .map_err(|e| crate::error!(Database, Query(e)))?;

        if delete_expired.rows_affected > 0 {
            linfo!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "cleanup_expired_sessions",
                &format!(
                    "Deleted {} old expired OAuth sessions",
                    delete_expired.rows_affected
                )
            );
        }

        // 删除孤立会话：未被任何 user_provider_keys (OAuth) 引用
        let linked_session_ids: HashSet<String> = user_provider_keys::Entity::find()
            .filter(user_provider_keys::Column::AuthType.eq("oauth"))
            .select_only()
            .column(user_provider_keys::Column::ApiKey)
            .into_tuple::<String>()
            .all(&*self.db)
            .await
            .map_err(|e| crate::error!(Database, Query(e)))?
            .into_iter()
            .collect();

        let orphan_sessions = oauth_client_sessions::Entity::find()
            .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
            .all(&*self.db)
            .await
            .map_err(|e| crate::error!(Database, Query(e)))?;

        let linked_ids = linked_session_ids;
        let orphan_ids: Vec<String> = orphan_sessions
            .into_iter()
            .filter(|session| !linked_ids.contains(&session.session_id))
            .map(|session| session.session_id)
            .collect();

        if !orphan_ids.is_empty() {
            let deleted = oauth_client_sessions::Entity::delete_many()
                .filter(oauth_client_sessions::Column::SessionId.is_in(orphan_ids))
                .exec(&*self.db)
                .await
                .map_err(|e| crate::error!(Database, Query(e)))?;
            linfo!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "cleanup_orphan_sessions",
                &format!(
                    "Deleted {} orphan OAuth sessions lacking user_provider_keys association",
                    deleted.rows_affected
                )
            );
        }

        Ok(())
    }

    /// 从数据库加载指定会话
    pub async fn load_session(
        &self,
        session_id: &str,
    ) -> Result<Option<oauth_client_sessions::Model>> {
        oauth_client_sessions::Entity::find()
            .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
            .one(&*self.db)
            .await
            .map_err(|e| crate::error!(Database, Query(e)))
    }

    fn should_refresh_session(session: &oauth_client_sessions::Model, now: DateTime<Utc>) -> bool {
        if session.access_token.is_none() || session.refresh_token.is_none() {
            return false;
        }

        let buffer = Duration::minutes(REFRESH_BUFFER_MINUTES);
        let expires_at = DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc);
        now + buffer >= expires_at
    }

    fn compute_next_from_expiry(expires_at: DateTime<Utc>, now: DateTime<Utc>) -> DateTime<Utc> {
        let buffer = Duration::minutes(REFRESH_BUFFER_MINUTES);
        let remaining = expires_at - now;

        let mut target = if remaining <= buffer {
            now
        } else if remaining <= buffer * 2 {
            expires_at - buffer
        } else if remaining <= buffer * 4 {
            expires_at - buffer * 2
        } else {
            expires_at - buffer * 3
        };

        if target <= now {
            target = now + Duration::seconds(1);
        }

        target
    }

    fn compute_next_refresh_at(
        session: &oauth_client_sessions::Model,
        now: DateTime<Utc>,
    ) -> Option<DateTime<Utc>> {
        if session.refresh_token.is_none() || session.access_token.is_none() {
            return None;
        }

        let expires_at = DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc);
        Some(Self::compute_next_from_expiry(expires_at, now))
    }

    /// 基于会话构建下一次刷新计划
    #[must_use]
    pub fn build_schedule_for_session(
        &self,
        session: &oauth_client_sessions::Model,
    ) -> Option<ScheduledTokenRefresh> {
        let now = Utc::now();
        Self::compute_next_refresh_at(session, now).map(|next| ScheduledTokenRefresh {
            session_id: session.session_id.clone(),
            next_refresh_at: next,
            expires_at: DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc),
        })
    }

    /// 构建启动时的刷新计划，并针对需要立即刷新的会话进行刷新
    #[allow(clippy::cognitive_complexity)]
    pub async fn initialize_refresh_schedule(&self) -> Result<Vec<ScheduledTokenRefresh>> {
        if let Err(e) = self.cleanup_stale_sessions().await {
            lwarn!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "cleanup_failed",
                &format!("Failed to cleanup OAuth sessions before scheduling: {e:?}")
            );
        }

        let sessions = self.list_authorized_sessions().await?;
        let mut schedule = Vec::new();

        for mut session in sessions {
            let now = Utc::now();

            if Self::should_refresh_session(&session, now) {
                match self
                    .refresh_token_with_lock(&session.session_id, RefreshType::Active)
                    .await
                {
                    Ok(result) => {
                        if let Some(expires_at) = result.new_expires_at {
                            session.expires_at = expires_at.naive_utc();
                            if let Some(access_token) = result.new_access_token {
                                session.access_token = Some(access_token);
                            }
                        } else if let Some(updated) = self.load_session(&session.session_id).await?
                        {
                            session = updated;
                        }
                    }
                    Err(e) => {
                        lwarn!(
                            "system",
                            LogStage::BackgroundTask,
                            LogComponent::OAuth,
                            "eager_refresh_failed",
                            &format!(
                                "Failed to eagerly refresh session {}: {:?}",
                                session.session_id, e
                            )
                        );
                        let retry_at = now
                            + Duration::seconds(
                                i64::try_from(RETRY_INTERVAL_SECONDS).unwrap_or(i64::MAX),
                            );
                        schedule.push(ScheduledTokenRefresh {
                            session_id: session.session_id.clone(),
                            next_refresh_at: retry_at,
                            expires_at: DateTime::<Utc>::from_naive_utc_and_offset(
                                session.expires_at,
                                Utc,
                            ),
                        });
                        continue;
                    }
                }
            }

            if let Some(next) = Self::compute_next_refresh_at(&session, now) {
                schedule.push(ScheduledTokenRefresh {
                    session_id: session.session_id.clone(),
                    next_refresh_at: next,
                    expires_at: DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc),
                });
            }
        }

        Ok(schedule)
    }

    /// 单独注册会话到刷新计划
    pub async fn register_session_for_refresh(
        &self,
        session_id: &str,
    ) -> Result<ScheduledTokenRefresh> {
        let mut session = self.load_session(session_id).await?.ok_or_else(|| {
            crate::error!(
                Authentication,
                format!("OAuth session not found: {}", session_id)
            )
        })?;

        if session.status != AuthStatus::Authorized.to_string() {
            return Err(crate::error!(
                Authentication,
                format!("OAuth session {} is not authorized", session_id)
            ));
        }

        if session.refresh_token.is_none() || session.access_token.is_none() {
            return Err(crate::error!(
                Authentication,
                format!("OAuth session {} missing refresh credentials", session_id)
            ));
        }

        let now = Utc::now();

        if Self::should_refresh_session(&session, now) {
            match self
                .refresh_token_with_lock(&session.session_id, RefreshType::Active)
                .await
            {
                Ok(result) => {
                    if let Some(expires_at) = result.new_expires_at {
                        session.expires_at = expires_at.naive_utc();
                        if let Some(access_token) = result.new_access_token {
                            session.access_token = Some(access_token);
                        }
                    } else if let Some(updated) = self.load_session(&session.session_id).await? {
                        session = updated;
                    }
                }
                Err(e) => {
                    lwarn!(
                        "system",
                        LogStage::BackgroundTask,
                        LogComponent::OAuth,
                        "eager_refresh_failed",
                        &format!(
                            "Failed to eagerly refresh session {}: {:?}",
                            session.session_id, e
                        )
                    );
                    let retry_at = now
                        + Duration::seconds(
                            i64::try_from(RETRY_INTERVAL_SECONDS).unwrap_or(i64::MAX),
                        );
                    return Ok(ScheduledTokenRefresh {
                        session_id: session.session_id.clone(),
                        next_refresh_at: retry_at,
                        expires_at: DateTime::<Utc>::from_naive_utc_and_offset(
                            session.expires_at,
                            Utc,
                        ),
                    });
                }
            }
        }

        if let Some(next) = Self::compute_next_refresh_at(&session, now) {
            Ok(ScheduledTokenRefresh {
                session_id: session.session_id.clone(),
                next_refresh_at: next,
                expires_at: DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc),
            })
        } else {
            Err(crate::error!(
                Internal,
                format!("Unable to compute refresh schedule for session {session_id}")
            ))
        }
    }

    /// 根据刷新结果确定下一次刷新计划
    pub async fn determine_next_refresh_after(
        &self,
        session_id: &str,
        result: &TokenRefreshResult,
    ) -> Result<Option<ScheduledTokenRefresh>> {
        let now = Utc::now();

        if let Some(expires_at) = result.new_expires_at.filter(|_| result.success) {
            let next = Self::compute_next_from_expiry(expires_at, now);
            return Ok(Some(ScheduledTokenRefresh {
                session_id: session_id.to_string(),
                next_refresh_at: next,
                expires_at,
            }));
        }

        if let Some(session) = self.load_session(session_id).await?
            && let Some(next) = Self::compute_next_refresh_at(&session, now)
        {
            return Ok(Some(ScheduledTokenRefresh {
                session_id: session.session_id.clone(),
                next_refresh_at: next,
                expires_at: DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc),
            }));
        }

        if !result.success && result.should_retry {
            let retry_at =
                now + Duration::seconds(i64::try_from(RETRY_INTERVAL_SECONDS).unwrap_or(i64::MAX));
            return Ok(Some(ScheduledTokenRefresh {
                session_id: session_id.to_string(),
                next_refresh_at: retry_at,
                expires_at: retry_at,
            }));
        }

        Ok(None)
    }

    /// 对外暴露刷新的包装方法
    pub async fn refresh_session(
        &self,
        session_id: &str,
        refresh_type: RefreshType,
    ) -> Result<TokenRefreshResult> {
        self.refresh_token_with_lock(session_id, refresh_type).await
    }

    /// 被动刷新：检查token是否需要刷新，如果需要则刷新
    ///
    /// `这个方法通常在SmartApiKeyProvider中使用时调用`
    pub async fn passive_refresh_if_needed(&self, session_id: &str) -> Result<TokenRefreshResult> {
        ldebug!(
            "system",
            LogStage::BackgroundTask,
            LogComponent::OAuth,
            "passive_refresh_check",
            &format!("Checking passive refresh for session_id: {session_id}")
        );

        // 检查是否需要刷新
        if !self.should_refresh_token(session_id).await? {
            ldebug!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "passive_refresh_not_needed",
                &format!("Token for session_id {session_id} does not need refresh")
            );
            return Ok(TokenRefreshResult {
                success: true,
                new_access_token: None,
                new_expires_at: None,
                error_message: None,
                should_retry: false,
                refresh_type: RefreshType::Passive,
            });
        }

        // 执行被动刷新
        self.refresh_token_with_lock(session_id, RefreshType::Passive)
            .await
    }

    /// 检查token是否需要刷新
    async fn should_refresh_token(&self, session_id: &str) -> Result<bool> {
        let session = oauth_client_sessions::Entity::find()
            .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
            .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
            .one(&*self.db)
            .await
            .map_err(|e| crate::error!(Database, Query(e)))?
            .ok_or_else(|| {
                crate::error!(
                    Authentication,
                    format!("OAuth session not found: {}", session_id)
                )
            })?;

        // 检查是否有有效的访问token
        if session.access_token.is_none() {
            ldebug!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "no_access_token",
                &format!("Session {session_id} has no access token")
            );
            return Ok(false); // 没有token，无需刷新
        }

        let now = Utc::now();
        let should_refresh = Self::should_refresh_session(&session, now);

        ldebug!(
            "system",
            LogStage::BackgroundTask,
            LogComponent::OAuth,
            "should_refresh_check",
            &format!(
                "Session {} expires at {:?}, should refresh: {}",
                session_id, session.expires_at, should_refresh
            )
        );

        Ok(should_refresh)
    }

    /// 使用锁进行token刷新，防止并发刷新
    async fn refresh_token_with_lock(
        &self,
        session_id: &str,
        refresh_type: RefreshType,
    ) -> Result<TokenRefreshResult> {
        // 获取刷新锁
        let refresh_lock = self.get_refresh_lock(session_id).await;
        let _guard = refresh_lock.lock().await;

        // 获得锁后再次检查是否需要刷新（可能其他线程已经刷新了）
        if refresh_type == RefreshType::Passive && !self.should_refresh_token(session_id).await? {
            ldebug!(
                "system",
                LogStage::BackgroundTask,
                LogComponent::OAuth,
                "already_refreshed",
                &format!("Token already refreshed by another thread for session: {session_id}")
            );
            return Ok(TokenRefreshResult {
                success: true,
                new_access_token: None,
                new_expires_at: None,
                error_message: None,
                should_retry: false,
                refresh_type,
            });
        }

        // 更新统计信息
        self.increment_refreshing_count().await;

        // 执行实际的token刷新
        let result = self
            .perform_token_refresh(session_id, refresh_type.clone())
            .await;

        // 更新统计信息
        self.decrement_refreshing_count().await;
        if let Ok(ref refresh_result) = result {
            self.update_refresh_stats(refresh_result).await;
        }

        result
    }

    /// 执行实际的token刷新
    #[allow(clippy::cognitive_complexity)]
    async fn perform_token_refresh(
        &self,
        session_id: &str,
        refresh_type: RefreshType,
    ) -> Result<TokenRefreshResult> {
        ldebug!(
            "system",
            LogStage::BackgroundTask,
            LogComponent::OAuth,
            "perform_token_refresh",
            &format!("Performing token refresh for session: {session_id}, type: {refresh_type:?}")
        );

        // 使用OAuth client进行token刷新
        match self.oauth_client.get_valid_access_token(session_id).await {
            Ok(Some(new_access_token)) => {
                linfo!(
                    "system",
                    LogStage::BackgroundTask,
                    LogComponent::OAuth,
                    "token_refresh_ok",
                    &format!("Successfully refreshed token for session: {session_id}")
                );

                // 获取新的过期时间
                let new_expires_at = self.get_token_expires_at(session_id).await;

                Ok(TokenRefreshResult {
                    success: true,
                    new_access_token: Some(new_access_token),
                    new_expires_at,
                    error_message: None,
                    should_retry: false,
                    refresh_type,
                })
            }

            Ok(None) => {
                lwarn!(
                    "system",
                    LogStage::BackgroundTask,
                    LogComponent::OAuth,
                    "no_valid_token_after_refresh",
                    &format!("No valid access token returned for session: {session_id}")
                );
                Ok(TokenRefreshResult {
                    success: false,
                    new_access_token: None,
                    new_expires_at: None,
                    error_message: Some("No valid access token available".to_string()),
                    should_retry: true,
                    refresh_type,
                })
            }

            Err(e) => {
                lerror!(
                    "system",
                    LogStage::BackgroundTask,
                    LogComponent::OAuth,
                    "token_refresh_fail",
                    &format!("Failed to refresh token for session {session_id}: {e:?}")
                );
                Ok(TokenRefreshResult {
                    success: false,
                    new_access_token: None,
                    new_expires_at: None,
                    error_message: Some(format!("OAuth client error: {e:?}")),
                    should_retry: true,
                    refresh_type,
                })
            }
        }
    }

    /// 获取token的过期时间
    async fn get_token_expires_at(&self, session_id: &str) -> Option<DateTime<Utc>> {
        match oauth_client_sessions::Entity::find()
            .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
            .one(&*self.db)
            .await
        {
            Ok(Some(session)) => Some(DateTime::<Utc>::from_naive_utc_and_offset(
                session.expires_at,
                Utc,
            )),
            _ => None,
        }
    }

    /// 判断是否应该重试刷新
    /// 获取刷新锁
    async fn get_refresh_lock(&self, session_id: &str) -> Arc<Mutex<()>> {
        let mut locks = self.refresh_locks.write().await;
        locks
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// 增加正在刷新的计数
    async fn increment_refreshing_count(&self) {
        let mut stats = self.refresh_stats.write().await;
        stats.refreshing_tokens += 1;
    }

    /// 减少正在刷新的计数
    async fn decrement_refreshing_count(&self) {
        let mut stats = self.refresh_stats.write().await;
        if stats.refreshing_tokens > 0 {
            stats.refreshing_tokens -= 1;
        }
    }

    /// 更新刷新统计信息
    async fn update_refresh_stats(&self, result: &TokenRefreshResult) {
        let mut stats = self.refresh_stats.write().await;

        stats.total_refreshes += 1;
        stats.last_refresh_time = Some(Utc::now());

        if result.success {
            stats.successful_refreshes += 1;
        } else {
            stats.failed_refreshes += 1;
            stats.last_failure_time = Some(Utc::now());
        }

        match result.refresh_type {
            RefreshType::Passive => stats.passive_refreshes += 1,
            RefreshType::Active => stats.active_refreshes += 1,
        }
    }

    /// 获取刷新统计信息
    pub async fn get_refresh_stats(&self) -> RefreshStats {
        self.refresh_stats.read().await.clone()
    }

    #[must_use]
    pub const fn retry_interval_seconds() -> u64 {
        RETRY_INTERVAL_SECONDS
    }
}
