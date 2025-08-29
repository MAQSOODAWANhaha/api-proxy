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
    pub is_active: bool,
    pub config_json: Option<String>,           // JSON 字符串
    pub token_mappings_json: Option<String>,   // Token字段映射配置
    pub model_extraction_json: Option<String>, // 模型提取规则配置
    // 认证配置字段
    pub auth_type: String,                     // 认证类型 (api_key, oauth2, etc.)
    pub auth_header_format: String,            // 认证头格式模板
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::user_provider_keys::Entity")]
    UserProviderKeys,
    #[sea_orm(has_many = "super::user_service_apis::Entity")]
    UserServiceApis,
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
            is_active: false,
            config_json: None,
            token_mappings_json: None,
            model_extraction_json: None,
            auth_type: "api_key".to_string(),
            auth_header_format: "Authorization: Bearer {key}".to_string(),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        }
    }
}
