//\! # Health Status 实体占位符

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "health_status")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub provider_api_key_id: i32,
    pub is_healthy: bool,
    pub response_time_ms: Option<i32>,
    pub last_check_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
