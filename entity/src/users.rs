//! # 用户实体定义
//!
//! 用户基础信息表的 Sea-ORM 实体模型

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 用户实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub username: String,
    #[sea_orm(unique)]
    pub email: String,
    pub password_hash: String,
    pub salt: String,
    pub is_active: bool,
    pub is_admin: bool,
    pub last_login: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::user_sessions::Entity")]
    UserSessions,
    #[sea_orm(has_many = "super::user_audit_logs::Entity")]
    UserAuditLogs,
    #[sea_orm(has_many = "super::user_provider_keys::Entity")]
    UserProviderKeys,
    #[sea_orm(has_many = "super::user_service_apis::Entity")]
    UserServiceApis,
    #[sea_orm(has_many = "super::daily_statistics::Entity")]
    DailyStatistics,
}

impl Related<super::user_sessions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserSessions.def()
    }
}

impl Related<super::user_audit_logs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserAuditLogs.def()
    }
}

impl Related<super::user_provider_keys::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserProviderKeys.def()
    }
}

impl Related<super::user_service_apis::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserServiceApis.def()
    }
}

impl Related<super::daily_statistics::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DailyStatistics.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
