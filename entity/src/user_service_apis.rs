//! # 用户对外服务API密钥实体定义
//!
//! 用户对外服务API密钥表的 Sea-ORM 实体模型

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 用户对外服务API密钥实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "user_service_apis")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub provider_type_id: i32,
    #[sea_orm(unique)]
    pub api_key: String,
    pub api_secret: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub scheduling_strategy: Option<String>,
    pub retry_count: Option<i32>,
    pub timeout_seconds: Option<i32>,
    pub rate_limit: Option<i32>,
    pub max_tokens_per_day: Option<i32>,
    pub used_tokens_today: Option<i32>,
    pub total_requests: Option<i32>,
    pub successful_requests: Option<i32>,
    pub last_used: Option<DateTime>,
    pub expires_at: Option<DateTime>,
    pub is_active: bool,
    pub created_at: DateTime,
    pub updated_at: DateTime,
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
        on_delete = "Restrict"
    )]
    ProviderType,
    #[sea_orm(has_many = "super::request_statistics::Entity")]
    RequestStatistics,
    #[sea_orm(has_many = "super::daily_statistics::Entity")]
    DailyStatistics,
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

impl Related<super::request_statistics::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RequestStatistics.def()
    }
}

impl Related<super::daily_statistics::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DailyStatistics.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}