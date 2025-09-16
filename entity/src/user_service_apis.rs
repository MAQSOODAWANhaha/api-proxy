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
    /// 关联的用户提供商密钥ID列表(JSON数组)
    #[sea_orm(column_type = "Json")]
    pub user_provider_keys_ids: sea_orm::prelude::Json,
    #[sea_orm(unique)]
    pub api_key: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub scheduling_strategy: Option<String>,
    pub retry_count: Option<i32>,
    pub timeout_seconds: Option<i32>,
    pub max_request_per_min: Option<i32>,
    pub max_requests_per_day: Option<i32>,
    pub max_tokens_per_day: Option<i64>,
    pub max_cost_per_day: Option<Decimal>,
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

impl Related<super::proxy_tracing::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProxyTracing.def()
    }
}


impl ActiveModelBehavior for ActiveModel {}
