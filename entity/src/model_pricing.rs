//! # 模型定价实体定义
//!
//! 模型定价表的 Sea-ORM 实体模型，用于存储每个提供商的模型token定价配置

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 模型定价实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "model_pricing")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// 提供商类型ID (外键)
    pub provider_type_id: i32,
    /// 模型名称
    pub model_name: String,
    /// 模型描述
    pub description: Option<String>,
    /// 货币单位
    pub cost_currency: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::provider_types::Entity",
        from = "Column::ProviderTypeId",
        to = "super::provider_types::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    ProviderType,
    #[sea_orm(has_many = "super::model_pricing_tiers::Entity")]
    ModelPricingTiers,
}

impl Related<super::provider_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProviderType.def()
    }
}

impl Related<super::model_pricing_tiers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ModelPricingTiers.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}