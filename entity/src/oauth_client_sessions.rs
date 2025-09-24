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
    pub status: String, // pending, authorized, error, expired, revoked
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


/// OAuth客户端会话辅助方法
impl Model {
    /// 检查会话是否已过期
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().naive_utc() > self.expires_at
    }

    /// 检查会话是否已授权
    pub fn is_authorized(&self) -> bool {
        self.status == "authorized" && self.access_token.is_some()
    }

    /// 检查会话是否仍然待处理
    pub fn is_pending(&self) -> bool {
        self.status == "pending" && !self.is_expired()
    }

    /// 获取会话状态字符串
    pub fn get_status_str(&self) -> &str {
        &self.status
    }

    /// 设置会话状态字符串
    pub fn set_status_str(&mut self, status: &str) {
        self.status = status.to_string();
        if status == "authorized" {
            self.completed_at = Some(chrono::Utc::now().naive_utc());
        }
    }

    /// 检查是否有有效的访问令牌
    pub fn has_valid_token(&self) -> bool {
        self.access_token.is_some() && self.is_authorized() && !self.is_expired()
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
