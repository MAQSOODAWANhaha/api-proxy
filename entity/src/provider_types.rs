//! # AI 服务提供商类型实体定义
//!
//! AI 服务提供商类型表的 Sea-ORM 实体模型

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// AI 服务提供商类型实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "provider_types")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub name: String,
    pub display_name: String,
    pub base_url: String,
    pub api_format: String,
    pub default_model: Option<String>,
    pub max_tokens: Option<i32>,
    pub rate_limit: Option<i32>,
    pub timeout_seconds: Option<i32>,
    pub health_check_path: Option<String>,
    pub auth_header_format: Option<String>,
    pub is_active: bool,
    pub config_json: Option<String>, // JSON 字符串
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::user_provider_keys::Entity")]
    UserProviderKeys,
    #[sea_orm(has_many = "super::user_service_apis::Entity")]
    UserServiceApis,
    #[sea_orm(has_many = "super::daily_statistics::Entity")]
    DailyStatistics,
}

impl Related<super::user_provider_keys::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserProviderKeys.def()
    }
}

impl Related<super::user_service_apis::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserServiceApis.def()
    }
}

impl Related<super::daily_statistics::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DailyStatistics.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Default for Model {
    fn default() -> Self {
        Self {
            id: 0,
            name: "unknown".to_string(),
            display_name: "Unknown Provider".to_string(),
            base_url: "".to_string(),
            api_format: "".to_string(),
            default_model: None,
            max_tokens: None,
            rate_limit: None,
            timeout_seconds: None,
            health_check_path: None,
            auth_header_format: None,
            is_active: false,
            config_json: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        }
    }
}
