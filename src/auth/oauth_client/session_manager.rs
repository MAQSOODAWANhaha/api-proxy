//! # OAuth会话管理器
//!
//! 管理OAuth客户端会话的完整生命周期，包括创建、更新、查询和删除
//! 提供会话状态跟踪、自动过期清理和安全验证功能

use super::pkce::PkceParams;
use super::{OAuthError, OAuthProviderConfig, OAuthResult, OAuthSessionInfo, OAuthTokenResponse};
use crate::auth::types::AuthStatus;
use chrono::{Duration, Utc};
use entity::{OAuthClientSessions, oauth_client_sessions};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 会话创建参数
#[derive(Debug, Clone)]
pub struct CreateSessionParams {
    pub user_id: i32,
    pub provider_name: String,
    pub provider_type_id: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub expires_in_minutes: Option<i32>,
}

/// 会话管理器
#[derive(Debug, Clone)]
pub struct SessionManager {
    db: DatabaseConnection,
}

impl SessionManager {
    /// 创建新的会话管理器
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// 获取数据库连接的引用
    pub fn get_db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// 创建新的OAuth会话
    pub async fn create_session(
        &self,
        user_id: i32,
        provider_name: &str,
        provider_type_id: Option<i32>,
        name: &str,
        description: Option<&str>,
        _config: &OAuthProviderConfig,
    ) -> OAuthResult<oauth_client_sessions::Model> {
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
            description: Set(description.map(|s| s.to_string())),
            status: Set(AuthStatus::Pending.to_string()),
            expires_at: Set(expires_at),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        };

        let inserted_session = session.insert(&self.db).await?;
        Ok(inserted_session)
    }

    /// 使用参数结构创建OAuth会话
    pub async fn create_session_with_params(
        &self,
        params: &CreateSessionParams,
        config: &OAuthProviderConfig,
    ) -> OAuthResult<oauth_client_sessions::Model> {
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

    /// 根据会话ID获取会话
    pub async fn get_session(&self, session_id: &str) -> OAuthResult<oauth_client_sessions::Model> {
        let session = OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
            .one(&self.db)
            .await?;

        match session {
            Some(session) => Ok(session),
            None => Err(OAuthError::InvalidSession(format!(
                "Session {} not found",
                session_id
            ))),
        }
    }

    /// 根据状态参数获取会话
    pub async fn get_session_by_state(
        &self,
        state: &str,
    ) -> OAuthResult<oauth_client_sessions::Model> {
        let session = OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::State.eq(state))
            .one(&self.db)
            .await?;

        match session {
            Some(session) => Ok(session),
            None => Err(OAuthError::InvalidSession(format!(
                "Session with state {} not found",
                state
            ))),
        }
    }

    /// 更新会话状态
    pub async fn update_session_status(
        &self,
        session_id: &str,
        status: AuthStatus,
        error_message: Option<&str>,
    ) -> OAuthResult<()> {
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

        active_model.update(&self.db).await?;
        Ok(())
    }

    /// 更新会话的授权码
    pub async fn update_session_authorization_code(
        &self,
        session_id: &str,
        _authorization_code: &str,
    ) -> OAuthResult<()> {
        let session = self.get_session(session_id).await?;

        let mut active_model: oauth_client_sessions::ActiveModel = session.into();
        // authorization_code 字段已删除，不再持久化授权码（安全最佳实践）
        active_model.status = Set(AuthStatus::Pending.to_string()); // 临时状态，使用pending
        active_model.updated_at = Set(Utc::now().naive_utc());

        active_model.update(&self.db).await?;
        Ok(())
    }

    /// 使用令牌信息更新会话
    pub async fn update_session_with_tokens(
        &self,
        session_id: &str,
        token_response: &OAuthTokenResponse,
    ) -> OAuthResult<()> {
        let session = self.get_session(session_id).await?;

        // 计算令牌过期时间
        let expires_at = if let Some(expires_in) = token_response.expires_in {
            Utc::now().naive_utc() + Duration::try_seconds(expires_in as i64).unwrap_or_default()
        } else {
            // 默认1小时过期
            Utc::now().naive_utc() + Duration::try_hours(1).unwrap_or_default()
        };

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

        active_model.update(&self.db).await?;
        Ok(())
    }

    /// 获取用户的所有会话
    pub async fn list_user_sessions(&self, user_id: i32) -> OAuthResult<Vec<OAuthSessionInfo>> {
        let sessions = OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::UserId.eq(user_id))
            .order_by_desc(oauth_client_sessions::Column::CreatedAt)
            .all(&self.db)
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
    ) -> OAuthResult<Vec<oauth_client_sessions::Model>> {
        let sessions = OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::UserId.eq(user_id))
            .filter(oauth_client_sessions::Column::ProviderName.eq(provider_name))
            .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
            .filter(oauth_client_sessions::Column::ExpiresAt.gt(Utc::now().naive_utc()))
            .order_by_desc(oauth_client_sessions::Column::CreatedAt)
            .all(&self.db)
            .await?;

        Ok(sessions)
    }

    /// 根据provider_type_id获取用户的活跃会话
    pub async fn list_user_active_sessions_by_provider_id(
        &self,
        user_id: i32,
        provider_type_id: i32,
    ) -> OAuthResult<Vec<oauth_client_sessions::Model>> {
        let sessions = OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::UserId.eq(user_id))
            .filter(oauth_client_sessions::Column::ProviderTypeId.eq(provider_type_id))
            .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
            .filter(oauth_client_sessions::Column::ExpiresAt.gt(Utc::now().naive_utc()))
            .order_by_desc(oauth_client_sessions::Column::CreatedAt)
            .all(&self.db)
            .await?;

        Ok(sessions)
    }

    /// 根据provider_name或provider_type_id获取用户的活跃会话
    pub async fn list_user_active_sessions_flexible(
        &self,
        user_id: i32,
        provider_name: Option<&str>,
        provider_type_id: Option<i32>,
    ) -> OAuthResult<Vec<oauth_client_sessions::Model>> {
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
            .all(&self.db)
            .await?;

        Ok(sessions)
    }

    /// 删除会话
    pub async fn delete_session(&self, session_id: &str, user_id: i32) -> OAuthResult<()> {
        let session = self.get_session(session_id).await?;

        // 验证会话所有权
        if session.user_id != user_id {
            return Err(OAuthError::InvalidSession(
                "Session does not belong to user".to_string(),
            ));
        }

        let active_model: oauth_client_sessions::ActiveModel = session.into();
        active_model.delete(&self.db).await?;

        Ok(())
    }

    /// 清理过期会话
    pub async fn cleanup_expired_sessions(&self) -> OAuthResult<u64> {
        let now = Utc::now().naive_utc();

        // 查找过期会话
        let expired_sessions = OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::ExpiresAt.lt(now))
            .filter(oauth_client_sessions::Column::Status.ne(AuthStatus::Authorized.to_string()))
            .all(&self.db)
            .await?;

        let mut deleted_count = 0;

        // 使用事务删除过期会话
        let txn = self.db.begin().await?;

        for session in expired_sessions {
            let active_model: oauth_client_sessions::ActiveModel = session.into();
            active_model.delete(&txn).await?;
            deleted_count += 1;
        }

        txn.commit().await?;

        Ok(deleted_count)
    }

    /// 获取会话统计信息
    pub async fn get_session_statistics(
        &self,
        user_id: Option<i32>,
    ) -> OAuthResult<SessionStatistics> {
        let mut query = OAuthClientSessions::find();

        if let Some(uid) = user_id {
            query = query.filter(oauth_client_sessions::Column::UserId.eq(uid));
        }

        let sessions = query.all(&self.db).await?;

        let mut stats = SessionStatistics::default();
        stats.total_sessions = sessions.len() as u64;

        for session in sessions {
            match session.status.as_str() {
                s if s == AuthStatus::Pending.to_string() => stats.pending_sessions += 1,
                s if s == AuthStatus::Authorized.to_string() => stats.completed_sessions += 1,
                s if s == AuthStatus::Error.to_string() => stats.failed_sessions += 1,
                s if s == AuthStatus::Expired.to_string() => stats.expired_sessions += 1,
                _ => {}
            }

            // 统计各提供商
            *stats
                .provider_counts
                .entry(session.provider_name)
                .or_insert(0) += 1;
        }

        stats.last_updated = Utc::now();
        Ok(stats)
    }

    /// 验证会话访问权限
    pub async fn validate_session_access(
        &self,
        session_id: &str,
        user_id: i32,
    ) -> OAuthResult<bool> {
        let session = self.get_session(session_id).await?;
        Ok(session.user_id == user_id)
    }

    /// 获取有效的访问令牌
    pub async fn get_valid_access_token(&self, session_id: &str) -> OAuthResult<Option<String>> {
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
    ) -> OAuthResult<()> {
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

/// 会话统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatistics {
    pub total_sessions: u64,
    pub pending_sessions: u64,
    pub completed_sessions: u64,
    pub failed_sessions: u64,
    pub expired_sessions: u64,
    pub provider_counts: std::collections::HashMap<String, u64>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl Default for SessionStatistics {
    fn default() -> Self {
        Self {
            total_sessions: 0,
            pending_sessions: 0,
            completed_sessions: 0,
            failed_sessions: 0,
            expired_sessions: 0,
            provider_counts: std::collections::HashMap::new(),
            last_updated: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_status_conversion() {
        let status = AuthStatus::Pending;
        let status_str = status.to_string();
        assert_eq!(status_str, "pending");

        let status = AuthStatus::Authorized;
        let status_str = status.to_string();
        assert_eq!(status_str, "authorized");
    }

    #[test]
    fn test_session_statistics_default() {
        let stats = SessionStatistics::default();
        assert_eq!(stats.total_sessions, 0);
        assert_eq!(stats.pending_sessions, 0);
        assert_eq!(stats.completed_sessions, 0);
        assert!(stats.provider_counts.is_empty());
    }

    #[test]
    fn test_create_session_params() {
        let params = CreateSessionParams {
            user_id: 1,
            provider_name: "google".to_string(),
            provider_type_id: Some(1),
            name: "Test Session".to_string(),
            description: Some("Test description".to_string()),
            expires_in_minutes: Some(30),
        };

        assert_eq!(params.user_id, 1);
        assert_eq!(params.provider_name, "google");
        assert_eq!(params.provider_type_id, Some(1));
        assert_eq!(params.name, "Test Session");
    }
}
