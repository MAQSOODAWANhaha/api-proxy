//! # API健康状态实体定义
//!
//! API健康状态表的 Sea-ORM 实体模型

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// API健康状态实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "api_health_status")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_provider_key_id: i32,
    pub is_healthy: bool,
    pub response_time_ms: Option<i32>,
    pub success_rate: Option<f32>,
    pub last_success: Option<DateTime>,
    pub last_failure: Option<DateTime>,
    pub consecutive_failures: Option<i32>,
    pub total_checks: Option<i32>,
    pub successful_checks: Option<i32>,
    pub last_error_message: Option<String>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user_provider_keys::Entity",
        from = "Column::UserProviderKeyId",
        to = "super::user_provider_keys::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    UserProviderKey,
}

impl Related<super::user_provider_keys::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserProviderKey.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
