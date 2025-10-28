//! OAuth 会话状态与调度服务
//!
//! 提供 OAuth 会话的持久化访问、调度策略计算以及清理逻辑，
//! 负责 Token 刷新策略所需的全部数据库交互。

use crate::auth::api_key_refresh_service::TokenRefreshResult;
use crate::auth::types::AuthStatus;
use crate::error::Result;
use crate::{ensure, error};
use chrono::{DateTime, Duration, Utc};
use entity::{oauth_client_sessions, user_provider_keys};
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set};
use std::collections::HashSet;
use std::convert::TryFrom;
use std::sync::Arc;

/// 距离过期多久开始预刷新
pub const REFRESH_LEAD_TIME: Duration = Duration::seconds(60);
/// 刷新失败后的退避间隔
const RETRY_INTERVAL_SECS: i64 = 60;
/// 允许的最大重试次数
const MAX_RETRY_ATTEMPTS: u32 = 3;
/// pending 会话的保留时长（分钟）
const PENDING_EXPIRE_MINUTES: i64 = 30;
/// expired 会话的保留天数
const EXPIRED_RETENTION_DAYS: i64 = 7;

/// OAuth 会话状态管理服务
#[derive(Debug, Clone)]
pub struct ApiKeyOAuthStateService {
    db: Arc<DatabaseConnection>,
}

/// 调度用的刷新任务描述
#[derive(Debug, Clone)]
pub struct ScheduledTokenRefresh {
    pub session_id: String,
    pub next_refresh_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub retry_attempts: u32,
}

/// 会话清理产出的统计数据
#[derive(Debug, Clone, Default)]
pub struct CleanupReport {
    pub removed_expired: usize,
    pub removed_orphaned: usize,
}

impl ApiKeyOAuthStateService {
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// 初始化所有已授权会话的刷新计划
    pub async fn init_refresh_schedules(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<ScheduledTokenRefresh>> {
        let _ = self.prune_stale_sessions(now).await?;
        let sessions = self.list_authorized_sessions().await?;
        let mut schedules = Vec::with_capacity(sessions.len());

        for session in sessions {
            if let Some(mut schedule) = self.build_session_schedule(&session) {
                if Self::is_refresh_due(&session, now) {
                    schedule.next_refresh_at = now;
                }
                schedules.push(schedule);
            }
        }

        Ok(schedules)
    }

    /// 注册单个会话进入刷新计划
    pub async fn schedule_session_refresh(
        &self,
        session_id: &str,
        now: DateTime<Utc>,
    ) -> Result<ScheduledTokenRefresh> {
        let session = self.fetch_session(session_id).await?.ok_or_else(|| {
            error!(
                Authentication,
                format!("OAuth session not found: {session_id}")
            )
        })?;

        ensure!(
            session.status == AuthStatus::Authorized.to_string(),
            Authentication,
            format!("OAuth session {session_id} is not authorized")
        );
        ensure!(
            session.access_token.is_some() && session.refresh_token.is_some(),
            Authentication,
            format!("OAuth session {session_id} is missing refresh credentials")
        );

        let mut schedule = self.build_session_schedule(&session).ok_or_else(|| {
            error!(
                Internal,
                format!("Unable to build refresh schedule for {session_id}")
            )
        })?;

        if Self::is_refresh_due(&session, now) {
            schedule.next_refresh_at = now;
        }

        Ok(schedule)
    }

    /// 列出所有授权并具备刷新条件的 OAuth 会话
    pub async fn list_authorized_sessions(&self) -> Result<Vec<oauth_client_sessions::Model>> {
        let linked_session_ids: Vec<String> = user_provider_keys::Entity::find()
            .filter(user_provider_keys::Column::AuthType.eq("oauth"))
            .select_only()
            .column(user_provider_keys::Column::ApiKey)
            .into_tuple::<String>()
            .all(&*self.db)
            .await
            .map_err(|e| error!(Database, Query(e)))?;

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
            .map_err(|e| error!(Database, Query(e)))
    }

    /// 加载指定会话
    pub async fn fetch_session(
        &self,
        session_id: &str,
    ) -> Result<Option<oauth_client_sessions::Model>> {
        oauth_client_sessions::Entity::find()
            .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
            .one(&*self.db)
            .await
            .map_err(|e| error!(Database, Query(e)))
    }

    /// 在刷新成功后更新会话的 token 信息
    pub async fn update_session_tokens(
        &self,
        session_id: &str,
        access_token: String,
        expires_at: DateTime<Utc>,
        new_refresh_token: Option<String>,
    ) -> Result<()> {
        let session = self.fetch_session(session_id).await?.ok_or_else(|| {
            error!(
                Authentication,
                format!("OAuth session not found: {session_id}")
            )
        })?;

        let mut active: oauth_client_sessions::ActiveModel = session.into();
        active.access_token = Set(Some(access_token));
        active.expires_at = Set(expires_at.naive_utc());
        if let Some(refresh_token) = new_refresh_token {
            active.refresh_token = Set(Some(refresh_token));
        }
        active.updated_at = Set(Utc::now().naive_utc());
        active
            .update(&*self.db)
            .await
            .map_err(|e| error!(Database, Query(e)))?;
        Ok(())
    }

    /// 判断会话是否需要刷新
    #[must_use]
    pub fn is_refresh_due(session: &oauth_client_sessions::Model, now: DateTime<Utc>) -> bool {
        if session.access_token.is_none() || session.refresh_token.is_none() {
            return false;
        }

        let expires_at = DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc);
        now >= expires_at - REFRESH_LEAD_TIME
    }

    /// 根据过期时间计算下一次刷新时间
    #[must_use]
    pub fn compute_refresh_deadline(
        expires_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> DateTime<Utc> {
        let candidate = expires_at - REFRESH_LEAD_TIME;
        if candidate <= now { now } else { candidate }
    }

    /// 为给定会话构建调度计划
    #[must_use]
    pub fn build_session_schedule(
        &self,
        session: &oauth_client_sessions::Model,
    ) -> Option<ScheduledTokenRefresh> {
        if session.access_token.is_none() || session.refresh_token.is_none() {
            return None;
        }
        let now = Utc::now();
        let expires_at = DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc);
        let next_refresh_at = Self::compute_refresh_deadline(expires_at, now);

        Some(ScheduledTokenRefresh {
            session_id: session.session_id.clone(),
            next_refresh_at,
            expires_at,
            retry_attempts: 0,
        })
    }

    /// 根据刷新结果计算下一次调度计划
    pub async fn resolve_next_schedule(
        &self,
        session_id: &str,
        previous_attempts: u32,
        result: &TokenRefreshResult,
    ) -> Result<Option<ScheduledTokenRefresh>> {
        let now = Utc::now();

        if result.success {
            let expires_at = if let Some(expires_at) = result.new_expires_at {
                expires_at
            } else if let Some(session) = self.fetch_session(session_id).await? {
                DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc)
            } else {
                return Ok(None);
            };

            return Ok(Some(ScheduledTokenRefresh {
                session_id: session_id.to_string(),
                next_refresh_at: Self::compute_refresh_deadline(expires_at, now),
                expires_at,
                retry_attempts: 0,
            }));
        }

        let attempts = previous_attempts.saturating_add(1);
        if attempts > MAX_RETRY_ATTEMPTS {
            return Ok(None);
        }

        let expires_at = if let Some(session) = self.fetch_session(session_id).await? {
            DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc)
        } else {
            now
        };

        Ok(Some(ScheduledTokenRefresh {
            session_id: session_id.to_string(),
            next_refresh_at: now + Duration::seconds(RETRY_INTERVAL_SECS),
            expires_at,
            retry_attempts: attempts,
        }))
    }

    /// 清理过期、异常或孤立的 OAuth 会话
    pub async fn prune_stale_sessions(&self, now: DateTime<Utc>) -> Result<CleanupReport> {
        let pending_cutoff = now - Duration::minutes(PENDING_EXPIRE_MINUTES);
        let expired_cutoff = now - Duration::days(EXPIRED_RETENTION_DAYS);

        let pending_deleted = {
            let rows = oauth_client_sessions::Entity::delete_many()
                .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Pending.to_string()))
                .filter(oauth_client_sessions::Column::CreatedAt.lt(pending_cutoff.naive_utc()))
                .exec(&*self.db)
                .await
                .map_err(|e| error!(Database, Query(e)))?
                .rows_affected;
            usize::try_from(rows).map_or(usize::MAX, |value| value)
        };

        let expired_deleted = {
            let rows = oauth_client_sessions::Entity::delete_many()
                .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Expired.to_string()))
                .filter(oauth_client_sessions::Column::UpdatedAt.lt(expired_cutoff.naive_utc()))
                .exec(&*self.db)
                .await
                .map_err(|e| error!(Database, Query(e)))?
                .rows_affected;
            usize::try_from(rows).map_or(usize::MAX, |value| value)
        };

        let linked_session_ids: HashSet<String> = user_provider_keys::Entity::find()
            .filter(user_provider_keys::Column::AuthType.eq("oauth"))
            .select_only()
            .column(user_provider_keys::Column::ApiKey)
            .into_tuple::<String>()
            .all(&*self.db)
            .await
            .map_err(|e| error!(Database, Query(e)))?
            .into_iter()
            .collect();

        let orphan_sessions = oauth_client_sessions::Entity::find()
            .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
            .all(&*self.db)
            .await
            .map_err(|e| error!(Database, Query(e)))?;

        let orphan_ids: Vec<String> = orphan_sessions
            .into_iter()
            .filter(|session| !linked_session_ids.contains(&session.session_id))
            .map(|session| session.session_id)
            .collect();

        let orphan_deleted = if orphan_ids.is_empty() {
            0
        } else {
            let rows = oauth_client_sessions::Entity::delete_many()
                .filter(oauth_client_sessions::Column::SessionId.is_in(orphan_ids))
                .exec(&*self.db)
                .await
                .map_err(|e| error!(Database, Query(e)))?
                .rows_affected;
            usize::try_from(rows).map_or(usize::MAX, |value| value)
        };

        Ok(CleanupReport {
            removed_expired: pending_deleted + expired_deleted,
            removed_orphaned: orphan_deleted,
        })
    }

    #[must_use]
    pub const fn retry_interval_secs() -> u64 {
        RETRY_INTERVAL_SECS as u64
    }
}
