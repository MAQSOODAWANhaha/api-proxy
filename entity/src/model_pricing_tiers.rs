//! # 模型定价阶梯实体定义
//!
//! 模型定价阶梯表的 Sea-ORM 实体模型，用于存储支持阶梯定价的token价格配置

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// 模型定价阶梯实体
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "model_pricing_tiers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    /// 模型定价ID (外键)
    pub model_pricing_id: i32,
    /// Token类型: 'prompt', 'completion', 'cache_create', 'cache_read'
    pub token_type: String,
    /// 阈值下限 (tokens数量)，0表示基础价格
    pub min_tokens: i32,
    /// 阈值上限 (tokens数量)，NULL表示无上限
    pub max_tokens: Option<i32>,
    /// 该阶梯的每token价格 (USD/token)
    pub price_per_token: f64,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::model_pricing::Entity",
        from = "Column::ModelPricingId",
        to = "super::model_pricing::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    ModelPricing,
}

impl Related<super::model_pricing::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ModelPricing.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// 检查给定的token数量是否在此阶梯的范围内
    pub fn is_in_range(&self, tokens: i32) -> bool {
        if tokens < self.min_tokens {
            return false;
        }

        match self.max_tokens {
            Some(max) => tokens <= max,
            None => true, // 无上限
        }
    }

    /// 计算在此阶梯内使用的token数量
    pub fn calculate_tokens_in_tier(&self, total_tokens: i32) -> i32 {
        if total_tokens <= self.min_tokens {
            return 0;
        }

        let tokens_above_min = total_tokens - self.min_tokens;

        match self.max_tokens {
            Some(max) => {
                let tier_capacity = max - self.min_tokens + 1;
                tokens_above_min.min(tier_capacity)
            }
            None => tokens_above_min, // 无上限，返回所有超过最小值的tokens
        }
    }

    /// 计算此阶梯的成本
    pub fn calculate_cost(&self, tokens_in_tier: i32) -> f64 {
        tokens_in_tier as f64 * self.price_per_token
    }
}
