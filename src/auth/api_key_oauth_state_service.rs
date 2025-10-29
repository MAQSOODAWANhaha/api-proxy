//! OAuth 会话状态与调度服务
//!
//! 提供 OAuth 会话的持久化访问、调度策略计算以及清理逻辑，
//! 负责 Token 刷新策略所需的全部数据库交互。

use crate::auth::api_key_oauth_refresh_service::ApiKeyOAuthRefreshResult;
use crate::auth::types::AuthStatus;
use crate::error::{AuthResult, Result};
use crate::key_pool::types::ApiKeyHealthStatus;
use crate::types::ProviderTypeId;
use crate::{ensure, error};
use chrono::{DateTime, Duration, Utc};
use entity::{OAuthClientSessions, oauth_client_sessions, user_provider_keys};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json, map::Map as JsonMap};
use std::collections::HashSet;
use std::convert::TryFrom;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::oauth_client::{
    OAuthProviderConfig, OAuthSessionInfo, OAuthTokenResponse, pkce::PkceParams,
};
use crate::error::auth::OAuthError;

/// 距离过期多久开始预刷新（默认提前2分钟）
pub const REFRESH_LEAD_TIME: Duration = Duration::seconds(120);
/// 刷新失败后的退避间隔
const RETRY_INTERVAL_SECS: i64 = 60;
/// 允许的最大重试次数
const MAX_RETRY_ATTEMPTS: u32 = 3;
/// pending 会话的保留时长（分钟）
const PENDING_EXPIRE_MINUTES: i64 = 30;
/// expired 会话的保留天数
const EXPIRED_RETENTION_DAYS: i64 = 7;

/// 会话创建参数
#[derive(Debug, Clone)]
pub struct CreateSessionParams {
    pub user_id: i32,
    pub provider_name: String,
    pub provider_type_id: Option<ProviderTypeId>,
    pub name: String,
    pub description: Option<String>,
    pub expires_in_minutes: Option<i32>,
}

/// OAuth 会话状态管理服务
#[derive(Debug, Clone)]
pub struct ApiKeyOAuthStateService {
    db: Arc<DatabaseConnection>,
    refresh_slots: Arc<RwLock<HashSet<String>>>,
}

/// 调度用的刷新任务描述
#[derive(Debug, Clone)]
pub struct ScheduledTokenRefresh {
    pub session_id: String,
    pub next_refresh_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub retry_attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RefreshStatusDetail {
    #[serde(default)]
    refresh_attempts: u32,
    #[serde(default)]
    last_error: Option<String>,
    #[serde(default)]
    next_retry_at: Option<DateTime<Utc>>,
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
        Self {
            db,
            refresh_slots: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// 尝试获取刷新锁，防止同一会话并发刷新
    pub async fn acquire_refresh_slot(&self, session_id: &str) -> bool {
        let mut slots = self.refresh_slots.write().await;
        slots.insert(session_id.to_string())
    }

    /// 释放刷新锁
    pub async fn release_refresh_slot(&self, session_id: &str) {
        let mut slots = self.refresh_slots.write().await;
        slots.remove(session_id);
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
                let refresh_state = self.load_refresh_state(&session.session_id).await?;
                let is_due = Self::is_refresh_due(&session, now);
                schedule.retry_attempts = refresh_state.refresh_attempts;
                schedule.next_refresh_at = if let Some(next_retry_at) = refresh_state.next_retry_at
                {
                    if next_retry_at <= now {
                        now
                    } else {
                        next_retry_at
                    }
                } else if is_due {
                    now
                } else {
                    schedule.next_refresh_at
                };
                schedules.push(schedule);
            }
        }

        Ok(schedules)
    }

    /// 提供明确语义的启动加载接口
    #[inline]
    pub async fn load_initial_plans(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<ScheduledTokenRefresh>> {
        self.init_refresh_schedules(now).await
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

        let refresh_state = self.load_refresh_state(session_id).await?;
        let is_due = Self::is_refresh_due(&session, now);
        schedule.retry_attempts = refresh_state.refresh_attempts;
        schedule.next_refresh_at = if let Some(next_retry_at) = refresh_state.next_retry_at {
            if next_retry_at <= now {
                now
            } else {
                next_retry_at
            }
        } else if is_due {
            now
        } else {
            schedule.next_refresh_at
        };

        Ok(schedule)
    }

    /// 根据会话ID获取会话
    pub async fn get_session(&self, session_id: &str) -> AuthResult<oauth_client_sessions::Model> {
        let session = OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
            .one(self.db.as_ref())
            .await?;

        session.ok_or_else(|| {
            crate::error!(
                Authentication,
                OAuth(OAuthError::InvalidSession(format!(
                    "Session {session_id} not found"
                )))
            )
        })
    }

    /// 获取会话信息，允许返回 None 以表示不存在
    pub async fn fetch_session(
        &self,
        session_id: &str,
    ) -> Result<Option<oauth_client_sessions::Model>> {
        OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
            .one(self.db.as_ref())
            .await
            .map_err(|e| error!(Database, Query(e)))
    }

    async fn find_provider_key(
        &self,
        session_id: &str,
    ) -> Result<Option<user_provider_keys::Model>> {
        user_provider_keys::Entity::find()
            .filter(user_provider_keys::Column::AuthType.eq("oauth"))
            .filter(user_provider_keys::Column::ApiKey.eq(session_id))
            .one(self.db.as_ref())
            .await
            .map_err(|e| error!(Database, Query(e)))
    }

    pub async fn refresh_target_exists(&self, session_id: &str) -> Result<bool> {
        if self.find_provider_key(session_id).await?.is_none() {
            return Ok(false);
        }
        if let Some(session) = self.fetch_session(session_id).await? {
            Ok(session.status == AuthStatus::Authorized.to_string())
        } else {
            Ok(false)
        }
    }

    async fn load_refresh_state(&self, session_id: &str) -> Result<RefreshStatusDetail> {
        let detail = match self.find_provider_key(session_id).await? {
            Some(key) => key.health_status_detail,
            None => return Ok(RefreshStatusDetail::default()),
        };
        Ok(Self::parse_refresh_state(detail.as_deref()))
    }

    /// 判断 OAuth 会话是否仍绑定到用户密钥
    pub async fn has_oauth_association(&self, user_id: i32, session_id: &str) -> AuthResult<bool> {
        let record = user_provider_keys::Entity::find()
            .filter(user_provider_keys::Column::UserId.eq(user_id))
            .filter(user_provider_keys::Column::AuthType.eq("oauth"))
            .filter(user_provider_keys::Column::ApiKey.eq(session_id))
            .one(self.db.as_ref())
            .await
            .map_err(|e| OAuthError::DatabaseError(format!("验证会话关联失败: {e}")))?;

        Ok(record.is_some())
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

    /// 使用令牌信息更新会话
    pub async fn update_session_tokens(
        &self,
        session_id: &str,
        token_response: &OAuthTokenResponse,
    ) -> AuthResult<()> {
        let session = self.get_session(session_id).await?;

        // 计算令牌过期时间
        let expires_at = token_response.expires_in.map_or_else(
            || Utc::now().naive_utc() + Duration::try_hours(1).unwrap_or_default(),
            |expires_in| {
                Utc::now().naive_utc()
                    + Duration::try_seconds(i64::from(expires_in)).unwrap_or_default()
            },
        );

        // 先保存会话中的原 refresh_token，再转换为 ActiveModel
        let existing_refresh = session.refresh_token.clone();
        let mut active_model: oauth_client_sessions::ActiveModel = session.into();
        active_model.access_token = Set(Some(token_response.access_token.clone()));
        // 仅当响应中包含新的 refresh_token 时才覆盖；否则保留会话中已有的 refresh_token
        let effective_refresh_token = if token_response.refresh_token.is_some() {
            token_response.refresh_token.clone()
        } else {
            existing_refresh
        };
        active_model.refresh_token = Set(effective_refresh_token);
        active_model.id_token = Set(token_response.id_token.clone());
        active_model.token_type = Set(Some(token_response.token_type.clone()));
        active_model.expires_in = Set(token_response.expires_in);
        active_model.expires_at = Set(expires_at);
        active_model.status = Set(AuthStatus::Authorized.to_string());
        active_model.completed_at = Set(Some(Utc::now().naive_utc()));
        active_model.updated_at = Set(Utc::now().naive_utc());

        active_model.update(self.db.as_ref()).await?;
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

    fn parse_refresh_state(raw: Option<&str>) -> RefreshStatusDetail {
        let Some(text) = raw else {
            return RefreshStatusDetail::default();
        };

        if let Ok(value) = serde_json::from_str::<Value>(text)
            && let Some(obj) = value.as_object()
        {
            if let Some(state) = obj.get("refresh_state").and_then(Value::as_object) {
                return Self::extract_refresh_state(state);
            }
            return Self::extract_refresh_state(obj);
        }
        RefreshStatusDetail::default()
    }

    fn extract_refresh_state(map: &JsonMap<String, Value>) -> RefreshStatusDetail {
        let mut detail = RefreshStatusDetail::default();
        if let Some(attempts) = map.get("refresh_attempts").and_then(Value::as_u64) {
            detail.refresh_attempts = u32::try_from(attempts).unwrap_or(u32::MAX);
        }
        if let Some(err) = map.get("last_error").and_then(Value::as_str) {
            detail.last_error = Some(err.to_string());
        }
        if let Some(next) = map.get("next_retry_at").and_then(Value::as_str)
            && let Ok(parsed) = DateTime::parse_from_rfc3339(next)
        {
            detail.next_retry_at = Some(parsed.with_timezone(&Utc));
        }
        detail
    }

    fn encode_refresh_state(detail: &RefreshStatusDetail) -> String {
        let mut state = JsonMap::new();
        state.insert(
            "refresh_attempts".to_string(),
            json!(detail.refresh_attempts),
        );
        if let Some(err) = &detail.last_error {
            state.insert("last_error".to_string(), json!(err));
        }
        if let Some(next) = detail.next_retry_at {
            state.insert("next_retry_at".to_string(), json!(next.to_rfc3339()));
        }
        json!({ "refresh_state": state }).to_string()
    }

    async fn update_refresh_state(
        &self,
        session_id: &str,
        detail: RefreshStatusDetail,
        desired_status: Option<ApiKeyHealthStatus>,
    ) -> Result<()> {
        let Some(key) = self.find_provider_key(session_id).await? else {
            return Ok(());
        };

        let now = Utc::now().naive_utc();
        let mut active: user_provider_keys::ActiveModel = key.into();

        if let Some(status) = desired_status {
            active.health_status = Set(status.to_string());
        }

        let detail_json = if detail.refresh_attempts == 0
            && detail.last_error.is_none()
            && detail.next_retry_at.is_none()
        {
            None
        } else {
            Some(Self::encode_refresh_state(&detail))
        };
        active.health_status_detail = Set(detail_json);
        if detail.last_error.is_some() {
            active.last_error_time = Set(Some(now));
        } else {
            active.last_error_time = Set(None);
        }
        active.updated_at = Set(now);

        active.update(self.db.as_ref()).await?;
        Ok(())
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

    /// 刷新成功后落库并生成下一次调度计划
    pub async fn complete_refresh(
        &self,
        result: &ApiKeyOAuthRefreshResult,
    ) -> Result<ScheduledTokenRefresh> {
        let session_id = result.session_id.as_str();
        self.update_session_tokens(session_id, &result.token_response)
            .await?;
        self.update_refresh_state(
            session_id,
            RefreshStatusDetail::default(),
            Some(ApiKeyHealthStatus::Healthy),
        )
        .await?;
        self.release_refresh_slot(session_id).await;

        let now = Utc::now();
        let mut next_refresh_at = Self::compute_refresh_deadline(result.expires_at, now);
        if next_refresh_at <= now {
            next_refresh_at = now + Duration::seconds(RETRY_INTERVAL_SECS);
        }

        Ok(ScheduledTokenRefresh {
            session_id: result.session_id.clone(),
            next_refresh_at,
            expires_at: result.expires_at,
            retry_attempts: 0,
        })
    }

    /// 刷新失败后更新重试状态
    pub async fn fail_refresh(
        &self,
        session_id: &str,
        previous_attempts: u32,
        error_message: &str,
    ) -> Result<Option<ScheduledTokenRefresh>> {
        let attempts = previous_attempts.saturating_add(1);
        let now = Utc::now();
        let next_retry_at = if attempts >= MAX_RETRY_ATTEMPTS {
            None
        } else {
            Some(now + Duration::seconds(RETRY_INTERVAL_SECS))
        };

        let detail = RefreshStatusDetail {
            refresh_attempts: attempts,
            last_error: Some(error_message.to_string()),
            next_retry_at,
        };

        let desired_status = if attempts >= MAX_RETRY_ATTEMPTS {
            Some(ApiKeyHealthStatus::Unhealthy)
        } else {
            None
        };
        self.update_refresh_state(session_id, detail, desired_status)
            .await?;
        self.release_refresh_slot(session_id).await;

        if attempts >= MAX_RETRY_ATTEMPTS {
            return Ok(None);
        }

        let expires_at = if let Some(session) = self.fetch_session(session_id).await? {
            DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc)
        } else {
            now + Duration::seconds(RETRY_INTERVAL_SECS)
        };

        Ok(Some(ScheduledTokenRefresh {
            session_id: session_id.to_string(),
            next_refresh_at: next_retry_at.unwrap_or(now),
            expires_at,
            retry_attempts: attempts,
        }))
    }

    /// 停止调度某个会话的刷新计划
    pub async fn stop_refresh(&self, session_id: &str, reason: Option<&str>) -> Result<()> {
        let detail = RefreshStatusDetail {
            refresh_attempts: 0,
            last_error: reason.map(ToOwned::to_owned),
            next_retry_at: None,
        };
        self.update_refresh_state(session_id, detail, Some(ApiKeyHealthStatus::Unhealthy))
            .await?;
        self.release_refresh_slot(session_id).await;
        Ok(())
    }

    /// 创建新的刷新计划
    pub async fn create_refresh_plan(
        &self,
        session_id: &str,
        now: DateTime<Utc>,
    ) -> Result<ScheduledTokenRefresh> {
        let schedule = self.schedule_session_refresh(session_id, now).await?;
        self.update_refresh_state(
            session_id,
            RefreshStatusDetail::default(),
            Some(ApiKeyHealthStatus::Healthy),
        )
        .await?;
        Ok(schedule)
    }

    /// 更新已存在的刷新计划
    pub async fn update_refresh_plan(
        &self,
        session_id: &str,
        now: DateTime<Utc>,
    ) -> Result<ScheduledTokenRefresh> {
        self.schedule_session_refresh(session_id, now).await
    }

    /// 删除刷新计划
    pub async fn delete_refresh_plan(&self, session_id: &str) -> Result<()> {
        self.update_refresh_state(session_id, RefreshStatusDetail::default(), None)
            .await?;
        self.release_refresh_slot(session_id).await;
        Ok(())
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

    /// `创建新的OAuth会话`
    pub async fn create_session(
        &self,
        user_id: i32,
        provider_name: &str,
        provider_type_id: Option<ProviderTypeId>,
        name: &str,
        description: Option<&str>,
        _config: &OAuthProviderConfig,
    ) -> AuthResult<oauth_client_sessions::Model> {
        // 生成PKCE参数
        let pkce = PkceParams::new();

        // 生成唯一的会话ID和状态参数
        let session_id = Uuid::new_v4().to_string();
        let state = Uuid::new_v4().to_string();

        // 计算过期时间（默认15分钟）
        let now = Utc::now().naive_utc();
        let expires_at = now + Duration::try_minutes(15).unwrap_or_default();

        // 创建会话记录
        let session = oauth_client_sessions::ActiveModel {
            session_id: Set(session_id),
            user_id: Set(user_id),
            provider_name: Set(provider_name.to_string()),
            provider_type_id: Set(provider_type_id),
            code_verifier: Set(pkce.verifier.into_string()),
            code_challenge: Set(pkce.challenge.as_str().to_string()),
            state: Set(state),
            name: Set(name.to_string()),
            description: Set(description.map(std::string::ToString::to_string)),
            status: Set(AuthStatus::Pending.to_string()),
            expires_at: Set(expires_at),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let inserted_session = session.insert(self.db.as_ref()).await?;
        Ok(inserted_session)
    }

    /// `使用参数结构创建OAuth会话`
    pub async fn create_session_with_params(
        &self,
        params: &CreateSessionParams,
        config: &OAuthProviderConfig,
    ) -> AuthResult<oauth_client_sessions::Model> {
        self.create_session(
            params.user_id,
            &params.provider_name,
            params.provider_type_id,
            &params.name,
            params.description.as_deref(),
            config,
        )
        .await
    }

    /// 根据状态参数获取会话
    pub async fn get_session_by_state(
        &self,
        state: &str,
    ) -> AuthResult<oauth_client_sessions::Model> {
        let session = OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::State.eq(state))
            .one(self.db.as_ref())
            .await?;

        session.ok_or_else(|| {
            crate::error!(
                Authentication,
                OAuth(OAuthError::InvalidSession(format!(
                    "Session with state {state} not found"
                )))
            )
        })
    }

    /// 更新会话状态
    pub async fn update_session_status(
        &self,
        session_id: &str,
        status: AuthStatus,
        error_message: Option<&str>,
    ) -> AuthResult<()> {
        let session = self.get_session(session_id).await?;

        let mut active_model: oauth_client_sessions::ActiveModel = session.into();
        active_model.status = Set(status.to_string());
        active_model.updated_at = Set(Utc::now().naive_utc());

        if let Some(error) = error_message {
            active_model.error_message = Set(Some(error.to_string()));
        }

        if status == AuthStatus::Authorized {
            active_model.completed_at = Set(Some(Utc::now().naive_utc()));
        }

        active_model.update(self.db.as_ref()).await?;
        Ok(())
    }

    /// 更新会话的授权码
    pub async fn update_session_authorization_code(
        &self,
        session_id: &str,
        _authorization_code: &str,
    ) -> AuthResult<()> {
        let session = self.get_session(session_id).await?;

        let mut active_model: oauth_client_sessions::ActiveModel = session.into();
        // authorization_code 字段已删除，不再持久化授权码（安全最佳实践）
        active_model.status = Set(AuthStatus::Pending.to_string()); // 临时状态，使用pending
        active_model.updated_at = Set(Utc::now().naive_utc());

        active_model.update(self.db.as_ref()).await?;
        Ok(())
    }

    /// 获取用户的所有会话
    pub async fn list_user_sessions(&self, user_id: i32) -> AuthResult<Vec<OAuthSessionInfo>> {
        let sessions = OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::UserId.eq(user_id))
            .order_by_desc(oauth_client_sessions::Column::CreatedAt)
            .all(self.db.as_ref())
            .await?;

        let session_infos = sessions
            .into_iter()
            .map(|session| OAuthSessionInfo {
                session_id: session.session_id,
                user_id: session.user_id,
                provider_name: session.provider_name,
                name: session.name,
                description: session.description,
                status: session.status,
                created_at: session.created_at,
                expires_at: session.expires_at,
                completed_at: session.completed_at,
            })
            .collect();

        Ok(session_infos)
    }

    /// 获取用户在特定提供商的活跃会话
    pub async fn list_user_active_sessions(
        &self,
        user_id: i32,
        provider_name: &str,
    ) -> AuthResult<Vec<oauth_client_sessions::Model>> {
        let sessions = OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::UserId.eq(user_id))
            .filter(oauth_client_sessions::Column::ProviderName.eq(provider_name))
            .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
            .filter(oauth_client_sessions::Column::ExpiresAt.gt(Utc::now().naive_utc()))
            .order_by_desc(oauth_client_sessions::Column::CreatedAt)
            .all(self.db.as_ref())
            .await?;

        Ok(sessions)
    }

    /// `根据provider_type_id获取用户的活跃会话`
    pub async fn list_user_active_sessions_by_provider_id(
        &self,
        user_id: i32,
        provider_type_id: ProviderTypeId,
    ) -> AuthResult<Vec<oauth_client_sessions::Model>> {
        let sessions = OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::UserId.eq(user_id))
            .filter(oauth_client_sessions::Column::ProviderTypeId.eq(provider_type_id))
            .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
            .filter(oauth_client_sessions::Column::ExpiresAt.gt(Utc::now().naive_utc()))
            .order_by_desc(oauth_client_sessions::Column::CreatedAt)
            .all(self.db.as_ref())
            .await?;

        Ok(sessions)
    }

    /// `根据provider_name或provider_type_id获取用户的活跃会话`
    pub async fn list_user_active_sessions_flexible(
        &self,
        user_id: i32,
        provider_name: Option<&str>,
        provider_type_id: Option<ProviderTypeId>,
    ) -> AuthResult<Vec<oauth_client_sessions::Model>> {
        let mut query = OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::UserId.eq(user_id))
            .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
            .filter(oauth_client_sessions::Column::ExpiresAt.gt(Utc::now().naive_utc()));

        // 优先使用provider_type_id查询
        if let Some(provider_id) = provider_type_id {
            query = query.filter(oauth_client_sessions::Column::ProviderTypeId.eq(provider_id));
        } else if let Some(provider_name) = provider_name {
            query = query.filter(oauth_client_sessions::Column::ProviderName.eq(provider_name));
        }

        let sessions = query
            .order_by_desc(oauth_client_sessions::Column::CreatedAt)
            .all(self.db.as_ref())
            .await?;

        Ok(sessions)
    }

    /// 删除会话
    pub async fn delete_session(&self, session_id: &str, user_id: i32) -> AuthResult<()> {
        let session = self.get_session(session_id).await?;

        // 验证会话所有权
        if session.user_id != user_id {
            return Err(
                OAuthError::InvalidSession("Session does not belong to user".to_string()).into(),
            );
        }

        let active_model: oauth_client_sessions::ActiveModel = session.into();
        active_model.delete(self.db.as_ref()).await?;

        Ok(())
    }

    /// 验证会话访问权限
    pub async fn validate_session_access(
        &self,
        session_id: &str,
        user_id: i32,
    ) -> AuthResult<bool> {
        let session = self.get_session(session_id).await?;
        Ok(session.user_id == user_id)
    }

    /// 获取有效的访问令牌
    pub async fn get_valid_access_token(&self, session_id: &str) -> AuthResult<Option<String>> {
        let session = self.get_session(session_id).await?;

        if session.status != AuthStatus::Authorized.to_string() {
            return Ok(None);
        }

        if session.is_expired() {
            return Ok(None);
        }

        Ok(session.access_token)
    }

    /// 批量更新会话状态
    pub async fn batch_update_sessions(
        &self,
        updates: Vec<(String, AuthStatus, Option<String>)>,
    ) -> AuthResult<()> {
        let txn = self.db.begin().await?;

        for (session_id, status, error_message) in updates {
            let session = OAuthClientSessions::find()
                .filter(oauth_client_sessions::Column::SessionId.eq(&session_id))
                .one(&txn)
                .await?;

            if let Some(session) = session {
                let mut active_model: oauth_client_sessions::ActiveModel = session.into();
                active_model.status = Set(status.to_string());
                active_model.updated_at = Set(Utc::now().naive_utc());

                if let Some(error) = error_message {
                    active_model.error_message = Set(Some(error));
                }

                active_model.update(&txn).await?;
            }
        }

        txn.commit().await?;
        Ok(())
    }
}
