//! # OAuth会话管理器
//!
//! 提供OAuth认证流程中的会话管理功能

use chrono::{DateTime, Duration, Utc};
use entity::oauth_sessions;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::auth::types::{AuthType, AuthError};
use crate::error::Result;

/// OAuth会话管理器
#[derive(Clone)]
pub struct OAuthSessionManager {
    db: Arc<DatabaseConnection>,
}

/// OAuth会话创建请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    /// 用户ID
    pub user_id: i32,
    /// 服务提供商类型ID
    pub provider_type_id: i32,
    /// 认证类型
    pub auth_type: AuthType,
    /// 重定向URI
    pub redirect_uri: String,
    /// OAuth范围
    pub scopes: Option<Vec<String>>,
    /// 会话过期时间（分钟）
    pub expires_in_minutes: Option<i32>,
    /// PKCE代码验证器
    pub code_verifier: Option<String>,
    /// PKCE代码质询
    pub code_challenge: Option<String>,
}

/// OAuth会话完成请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteSessionRequest {
    /// 会话ID
    pub session_id: String,
    /// OAuth状态参数
    pub state: String,
    /// 授权码
    pub code: String,
}

/// OAuth会话查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// 会话ID
    pub session_id: String,
    /// 用户ID
    pub user_id: i32,
    /// 服务提供商类型ID
    pub provider_type_id: i32,
    /// 认证类型
    pub auth_type: String,
    /// OAuth状态
    pub state: String,
    /// PKCE代码验证器
    pub code_verifier: Option<String>,
    /// 重定向URI
    pub redirect_uri: String,
    /// OAuth范围
    pub scopes: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 过期时间
    pub expires_at: DateTime<Utc>,
    /// 完成时间
    pub completed_at: Option<DateTime<Utc>>,
    /// 错误信息
    pub error_message: Option<String>,
    /// 是否已过期
    pub is_expired: bool,
    /// 是否已完成
    pub is_completed: bool,
}

impl OAuthSessionManager {
    /// 创建新的OAuth会话管理器
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// 创建新的OAuth会话
    pub async fn create_session(&self, request: CreateSessionRequest) -> Result<SessionInfo> {
        let session_id = Uuid::new_v4().to_string();
        let state = Uuid::new_v4().to_string();
        let now = Utc::now().naive_utc();
        let expires_at = now + Duration::minutes(request.expires_in_minutes.unwrap_or(15) as i64);

        debug!(
            "Creating OAuth session: session_id={}, user_id={}, provider_type_id={}",
            session_id, request.user_id, request.provider_type_id
        );

        let new_session = oauth_sessions::ActiveModel {
            session_id: Set(session_id.clone()),
            user_id: Set(request.user_id),
            provider_type_id: Set(request.provider_type_id),
            auth_type: Set(request.auth_type.to_string()),
            state: Set(state.clone()),
            code_verifier: Set(request.code_verifier.clone()),
            code_challenge: Set(request.code_challenge.clone()),
            redirect_uri: Set(request.redirect_uri.clone()),
            scopes: Set(request.scopes.map(|s| s.join(","))),
            created_at: Set(now),
            expires_at: Set(expires_at),
            completed_at: Set(None),
            error_message: Set(None),
            ..Default::default()
        };

        match new_session.insert(&*self.db).await {
            Ok(session) => {
                info!(
                    "OAuth session created successfully: session_id={}",
                    session_id
                );
                Ok(SessionInfo {
                    session_id: session.session_id,
                    user_id: session.user_id,
                    provider_type_id: session.provider_type_id,
                    auth_type: session.auth_type,
                    state: session.state,
                    code_verifier: session.code_verifier,
                    redirect_uri: session.redirect_uri,
                    scopes: session.scopes,
                    created_at: DateTime::<Utc>::from_naive_utc_and_offset(session.created_at, Utc),
                    expires_at: DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc),
                    completed_at: session.completed_at.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
                    error_message: session.error_message,
                    is_expired: DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc) < Utc::now(),
                    is_completed: session.completed_at.is_some(),
                })
            }
            Err(e) => {
                error!("Failed to create OAuth session: {}", e);
                Err(AuthError::ConfigError(format!("Failed to create session: {}", e)).into())
            }
        }
    }

    /// 根据会话ID获取会话信息
    pub async fn get_session_by_id(&self, session_id: &str) -> Result<Option<SessionInfo>> {
        debug!("Getting OAuth session by ID: {}", session_id);

        match oauth_sessions::Entity::find()
            .filter(oauth_sessions::Column::SessionId.eq(session_id))
            .one(&*self.db)
            .await
        {
            Ok(Some(session)) => Ok(Some(self.model_to_session_info(session))),
            Ok(None) => {
                debug!("OAuth session not found: {}", session_id);
                Ok(None)
            }
            Err(e) => {
                error!("Failed to get OAuth session: {}", e);
                Err(AuthError::ConfigError(format!("Failed to get session: {}", e)).into())
            }
        }
    }

    /// 根据state参数获取会话信息
    pub async fn get_session_by_state(&self, state: &str) -> Result<Option<SessionInfo>> {
        debug!("Getting OAuth session by state: {}", state);

        match oauth_sessions::Entity::find()
            .filter(oauth_sessions::Column::State.eq(state))
            .one(&*self.db)
            .await
        {
            Ok(Some(session)) => Ok(Some(self.model_to_session_info(session))),
            Ok(None) => {
                debug!("OAuth session not found for state: {}", state);
                Ok(None)
            }
            Err(e) => {
                error!("Failed to get OAuth session by state: {}", e);
                Err(AuthError::ConfigError(format!("Failed to get session: {}", e)).into())
            }
        }
    }

    /// 完成OAuth会话
    pub async fn complete_session(&self, request: CompleteSessionRequest) -> Result<SessionInfo> {
        debug!(
            "Completing OAuth session: session_id={}, state={}",
            request.session_id, request.state
        );

        // 验证会话存在且state匹配
        let session = match self.get_session_by_id(&request.session_id).await? {
            Some(session) => session,
            None => {
                warn!("OAuth session not found: {}", request.session_id);
                return Err(AuthError::InvalidAuthType("Session not found".to_string()).into());
            }
        };

        if session.state != request.state {
            warn!("OAuth state mismatch: expected={}, got={}", session.state, request.state);
            return Err(AuthError::InvalidAuthType("Invalid state parameter".to_string()).into());
        }

        if session.is_expired {
            warn!("OAuth session expired: {}", request.session_id);
            return Err(AuthError::Expired.into());
        }

        if session.is_completed {
            warn!("OAuth session already completed: {}", request.session_id);
            return Err(AuthError::InvalidAuthType("Session already completed".to_string()).into());
        }

        // 更新会话状态
        let now = Utc::now().naive_utc();
        let update_result = oauth_sessions::Entity::update_many()
            .filter(oauth_sessions::Column::SessionId.eq(&request.session_id))
            .set(oauth_sessions::ActiveModel {
                completed_at: Set(Some(now)),
                ..Default::default()
            })
            .exec(&*self.db)
            .await;

        match update_result {
            Ok(_) => {
                info!("OAuth session completed successfully: {}", request.session_id);
                // 返回更新后的会话信息
                self.get_session_by_id(&request.session_id).await.map(|opt| {
                    opt.expect("Session should exist after successful update")
                })
            }
            Err(e) => {
                error!("Failed to complete OAuth session: {}", e);
                Err(AuthError::ConfigError(format!("Failed to complete session: {}", e)).into())
            }
        }
    }

    /// 标记会话为失败
    pub async fn fail_session(&self, session_id: &str, error_message: &str) -> Result<()> {
        debug!("Marking OAuth session as failed: session_id={}", session_id);

        let update_result = oauth_sessions::Entity::update_many()
            .filter(oauth_sessions::Column::SessionId.eq(session_id))
            .set(oauth_sessions::ActiveModel {
                error_message: Set(Some(error_message.to_string())),
                completed_at: Set(Some(Utc::now().naive_utc())),
                ..Default::default()
            })
            .exec(&*self.db)
            .await;

        match update_result {
            Ok(_) => {
                info!("OAuth session marked as failed: {}", session_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to mark OAuth session as failed: {}", e);
                Err(AuthError::ConfigError(format!("Failed to fail session: {}", e)).into())
            }
        }
    }

    /// 清理过期的会话
    pub async fn cleanup_expired_sessions(&self) -> Result<u64> {
        debug!("Cleaning up expired OAuth sessions");

        let now = Utc::now().naive_utc();
        
        match oauth_sessions::Entity::delete_many()
            .filter(oauth_sessions::Column::ExpiresAt.lt(now))
            .exec(&*self.db)
            .await
        {
            Ok(result) => {
                let deleted_count = result.rows_affected;
                if deleted_count > 0 {
                    info!("Cleaned up {} expired OAuth sessions", deleted_count);
                } else {
                    debug!("No expired OAuth sessions to clean up");
                }
                Ok(deleted_count)
            }
            Err(e) => {
                error!("Failed to clean up expired OAuth sessions: {}", e);
                Err(AuthError::ConfigError(format!("Failed to cleanup sessions: {}", e)).into())
            }
        }
    }

    /// 获取用户的活跃会话
    pub async fn get_user_active_sessions(&self, user_id: i32) -> Result<Vec<SessionInfo>> {
        debug!("Getting active OAuth sessions for user: {}", user_id);

        let now = Utc::now().naive_utc();

        match oauth_sessions::Entity::find()
            .filter(oauth_sessions::Column::UserId.eq(user_id))
            .filter(oauth_sessions::Column::ExpiresAt.gt(now))
            .filter(oauth_sessions::Column::CompletedAt.is_null())
            .all(&*self.db)
            .await
        {
            Ok(sessions) => Ok(sessions.into_iter().map(|s| self.model_to_session_info(s)).collect()),
            Err(e) => {
                error!("Failed to get user active sessions: {}", e);
                Err(AuthError::ConfigError(format!("Failed to get user sessions: {}", e)).into())
            }
        }
    }

    /// 撤销用户的所有活跃会话
    pub async fn revoke_user_sessions(&self, user_id: i32) -> Result<u64> {
        debug!("Revoking all OAuth sessions for user: {}", user_id);

        let now = Utc::now().naive_utc();
        
        match oauth_sessions::Entity::update_many()
            .filter(oauth_sessions::Column::UserId.eq(user_id))
            .filter(oauth_sessions::Column::CompletedAt.is_null())
            .set(oauth_sessions::ActiveModel {
                error_message: Set(Some("Session revoked by user".to_string())),
                completed_at: Set(Some(now)),
                ..Default::default()
            })
            .exec(&*self.db)
            .await
        {
            Ok(result) => {
                let revoked_count = result.rows_affected;
                if revoked_count > 0 {
                    info!("Revoked {} OAuth sessions for user {}", revoked_count, user_id);
                } else {
                    debug!("No active OAuth sessions to revoke for user {}", user_id);
                }
                Ok(revoked_count)
            }
            Err(e) => {
                error!("Failed to revoke OAuth sessions: {}", e);
                Err(AuthError::ConfigError(format!("Failed to revoke sessions: {}", e)).into())
            }
        }
    }

    /// 验证会话状态
    pub async fn validate_session(&self, session_id: &str, state: &str) -> Result<bool> {
        match self.get_session_by_id(session_id).await? {
            Some(session) => {
                if session.state != state {
                    debug!("OAuth session state mismatch");
                    return Ok(false);
                }
                if session.is_expired {
                    debug!("OAuth session expired");
                    return Ok(false);
                }
                if session.is_completed {
                    debug!("OAuth session already completed");
                    return Ok(false);
                }
                Ok(true)
            }
            None => {
                debug!("OAuth session not found");
                Ok(false)
            }
        }
    }

    /// 内部辅助方法：将数据库模型转换为会话信息
    fn model_to_session_info(&self, session: oauth_sessions::Model) -> SessionInfo {
        let expires_at = DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc);
        let completed_at = session.completed_at.map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc));
        
        SessionInfo {
            session_id: session.session_id,
            user_id: session.user_id,
            provider_type_id: session.provider_type_id,
            auth_type: session.auth_type,
            state: session.state,
            code_verifier: session.code_verifier,
            redirect_uri: session.redirect_uri,
            scopes: session.scopes,
            created_at: DateTime::<Utc>::from_naive_utc_and_offset(session.created_at, Utc),
            expires_at,
            completed_at,
            error_message: session.error_message,
            is_expired: expires_at < Utc::now(),
            is_completed: completed_at.is_some(),
        }
    }
}

impl AuthType {
    /// 将认证类型转换为字符串
    pub fn to_string(&self) -> String {
        match self {
            AuthType::OAuth2 => "oauth2".to_string(),
            AuthType::GoogleOAuth => "google_oauth".to_string(),
            AuthType::BearerToken => "bearer_token".to_string(),
            AuthType::ApiKey => "api_key".to_string(),
            AuthType::ServiceAccount => "service_account".to_string(),
            AuthType::ApplicationDefaultCredentials => "application_default_credentials".to_string(),
        }
    }
}