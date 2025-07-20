//! # 请求统计实体定义
//!
//! 请求统计表的 Sea-ORM 实体模型

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 请求统计实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "request_statistics")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_service_api_id: i32,
    pub user_provider_key_id: Option<i32>,
    pub request_id: Option<String>,
    pub method: String,
    pub path: Option<String>,
    pub status_code: Option<i32>,
    pub response_time_ms: Option<i32>,
    pub request_size: Option<i32>,
    pub response_size: Option<i32>,
    pub tokens_prompt: Option<i32>,
    pub tokens_completion: Option<i32>,
    pub tokens_total: Option<i32>,
    pub model_used: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub retry_count: Option<i32>,
    pub created_at: DateTime,
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
        on_delete = "SetNull"
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