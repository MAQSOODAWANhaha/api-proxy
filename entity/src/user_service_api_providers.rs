//! # 用户服务API与提供商API密钥关联表实体定义
//!
//! 用户服务API与提供商API密钥多对多关联表的 Sea-ORM 实体模型

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 用户服务API与提供商API密钥关联实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "user_service_api_providers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_service_api_id: i32,
    pub user_provider_key_id: i32,
    pub weight: Option<i32>,
    pub is_active: bool,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user_service_apis::Entity",
        from = "Column::UserServiceApiId",
        to = "super::user_service_apis::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    UserServiceApi,
    #[sea_orm(
        belongs_to = "super::user_provider_keys::Entity",
        from = "Column::UserProviderKeyId",
        to = "super::user_provider_keys::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    UserProviderKey,
}

impl Related<super::user_service_apis::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserServiceApi.def()
    }
}

impl Related<super::user_provider_keys::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserProviderKey.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}