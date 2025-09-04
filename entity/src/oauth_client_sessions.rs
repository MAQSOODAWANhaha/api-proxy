//! # OAuth客户端会话实体定义
//!
//! OAuth客户端会话表的 Sea-ORM 实体模型
//! 支持客户端侧OAuth流程，采用轮询模式而非回调模式

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// OAuth客户端会话实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "oauth_client_sessions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub session_id: String,
    pub user_id: i32,
    pub provider_name: String,
    pub provider_type_id: Option<i32>,
    pub code_verifier: String,
    pub code_challenge: String,
    pub state: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String, // pending, completed, failed
    pub authorization_code: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub token_type: Option<String>,
    pub expires_in: Option<i32>,
    pub expires_at: DateTime,
    pub error_message: Option<String>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub completed_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    User,
    #[sea_orm(
        belongs_to = "super::provider_types::Entity",
        from = "Column::ProviderName",
        to = "super::provider_types::Column::Name",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    ProviderType,
    #[sea_orm(
        belongs_to = "super::provider_types::Entity",
        from = "Column::ProviderTypeId",
        to = "super::provider_types::Column::Id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    ProviderTypeById,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::provider_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProviderType.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Default for Model {
    fn default() -> Self {
        let now = chrono::Utc::now().naive_utc();
        Self {
            id: 0,
            session_id: String::new(),
            user_id: 0,
            provider_name: String::new(),
            provider_type_id: None,
            code_verifier: String::new(),
            code_challenge: String::new(),
            state: String::new(),
            name: String::new(),
            description: None,
            status: "pending".to_string(),
            authorization_code: None,
            access_token: None,
            refresh_token: None,
            id_token: None,
            token_type: Some("Bearer".to_string()),
            expires_in: None,
            expires_at: now + chrono::Duration::try_hours(1).unwrap_or_default(),
            error_message: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }
}

/// OAuth会话状态枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "expired")]
    Expired,
}

impl From<SessionStatus> for String {
    fn from(status: SessionStatus) -> Self {
        match status {
            SessionStatus::Pending => "pending".to_string(),
            SessionStatus::Completed => "completed".to_string(),
            SessionStatus::Failed => "failed".to_string(),
            SessionStatus::Expired => "expired".to_string(),
        }
    }
}

impl TryFrom<String> for SessionStatus {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.as_str() {
            "pending" => Ok(SessionStatus::Pending),
            "completed" => Ok(SessionStatus::Completed),
            "failed" => Ok(SessionStatus::Failed),
            "expired" => Ok(SessionStatus::Expired),
            _ => Err(format!("Invalid session status: {}", s)),
        }
    }
}

/// OAuth客户端会话辅助方法
impl Model {
    /// 检查会话是否已过期
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().naive_utc() > self.expires_at
    }

    /// 检查会话是否已完成
    pub fn is_completed(&self) -> bool {
        self.status == "completed" && self.access_token.is_some()
    }

    /// 检查会话是否仍然待处理
    pub fn is_pending(&self) -> bool {
        self.status == "pending" && !self.is_expired()
    }

    /// 获取会话状态枚举
    pub fn get_status(&self) -> Result<SessionStatus, String> {
        SessionStatus::try_from(self.status.clone())
    }

    /// 设置会话状态
    pub fn set_status(&mut self, status: SessionStatus) {
        let is_completed = status == SessionStatus::Completed;
        self.status = status.into();
        if is_completed {
            self.completed_at = Some(chrono::Utc::now().naive_utc());
        }
    }

    /// 检查是否有有效的访问令牌
    pub fn has_valid_token(&self) -> bool {
        self.access_token.is_some() && self.is_completed() && !self.is_expired()
    }

    /// 获取provider_type_id，如果没有则返回None
    pub fn get_provider_type_id(&self) -> Option<i32> {
        self.provider_type_id
    }

    /// 设置provider_type_id
    pub fn set_provider_type_id(&mut self, provider_type_id: Option<i32>) {
        self.provider_type_id = provider_type_id;
    }

    /// 检查是否使用新的provider_type_id关联
    pub fn uses_provider_type_id(&self) -> bool {
        self.provider_type_id.is_some()
    }
}