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
    pub max_tokens_per_day: Option<i32>,
    pub used_tokens_today: Option<i32>,
    pub last_used: Option<DateTime>,
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
    #[sea_orm(has_many = "super::api_health_status::Entity")]
    ApiHealthStatus,
    #[sea_orm(has_many = "super::proxy_tracing::Entity")]
    ProxyTracing,
    #[sea_orm(has_many = "super::user_service_api_providers::Entity")]
    UserServiceApiProviders,
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

impl Related<super::user_service_api_providers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserServiceApiProviders.def()
    }
}

// 通过中间表与user_service_apis建立多对多关系
impl Related<super::user_service_apis::Entity> for Entity {
    fn to() -> RelationDef {
        super::user_service_api_providers::Relation::UserServiceApi.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::user_service_api_providers::Relation::UserProviderKey.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
