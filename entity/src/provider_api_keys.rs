//\! # Provider API Keys 实体占位符

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "provider_api_keys")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub provider_type_id: i32,
    pub api_key: String,
    pub weight: i32,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
