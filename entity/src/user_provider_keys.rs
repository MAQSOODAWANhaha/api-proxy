//! # 用户内部代理商API密钥池实体定义
//!
//! 用户的内部代理商API密钥池表的 Sea-ORM 实体模型

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 用户内部代理商API密钥池实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "user_provider_keys")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub provider_type_id: i32,
    pub api_key: String,
    pub name: String,
    pub weight: Option<i32>,
    pub max_requests_per_minute: Option<i32>,
    pub max_tokens_prompt_per_minute: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub is_active: bool,
    pub health_status: String,
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
    #[sea_orm(has_many = "super::api_health_status::Entity")]
    ApiHealthStatus,
    #[sea_orm(has_many = "super::proxy_tracing::Entity")]
    ProxyTracing,
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

impl Related<super::api_health_status::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ApiHealthStatus.def()
    }
}

impl Related<super::proxy_tracing::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProxyTracing.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
