//! # OAuth会话实体定义
//!
//! OAuth认证会话表的 Sea-ORM 实体模型

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// OAuth会话实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "oauth_sessions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub session_id: String,
    pub user_id: i32,
    pub provider_type_id: i32,
    pub auth_type: String,
    pub state: String,
    pub code_verifier: Option<String>,
    pub code_challenge: Option<String>,
    pub redirect_uri: String,
    pub scopes: Option<String>,
    pub created_at: DateTime,
    pub expires_at: DateTime,
    pub completed_at: Option<DateTime>,
    pub error_message: Option<String>,
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
        from = "Column::ProviderTypeId",
        to = "super::provider_types::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    ProviderType,
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
        Self {
            id: 0,
            session_id: String::new(),
            user_id: 0,
            provider_type_id: 0,
            auth_type: "oauth2".to_string(),
            state: String::new(),
            code_verifier: None,
            code_challenge: None,
            redirect_uri: String::new(),
            scopes: None,
            created_at: chrono::Utc::now().naive_utc(),
            expires_at: chrono::Utc::now().naive_utc(),
            completed_at: None,
            error_message: None,
        }
    }
}