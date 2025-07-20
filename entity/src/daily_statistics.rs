//! # 每日统计汇总实体定义
//!
//! 每日统计汇总表的 Sea-ORM 实体模型

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 每日统计汇总实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "daily_statistics")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub user_service_api_id: Option<i32>,
    pub provider_type_id: i32,
    pub date: Date,
    pub total_requests: Option<i32>,
    pub successful_requests: Option<i32>,
    pub failed_requests: Option<i32>,
    pub total_tokens: Option<i32>,
    pub avg_response_time: Option<i32>,
    pub max_response_time: Option<i32>,
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
        belongs_to = "super::user_service_apis::Entity",
        from = "Column::UserServiceApiId",
        to = "super::user_service_apis::Column::Id",
        on_update = "Cascade",
        on_delete = "SetNull"
    )]
    UserServiceApi,
    #[sea_orm(
        belongs_to = "super::provider_types::Entity",
        from = "Column::ProviderTypeId",
        to = "super::provider_types::Column::Id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    ProviderType,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::user_service_apis::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserServiceApi.def()
    }
}

impl Related<super::provider_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProviderType.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}