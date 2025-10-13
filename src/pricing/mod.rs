//! # 费用计算服务
//!
//! 基于模型定价和阶梯定价配置，计算AI请求的token使用费用
#![allow(clippy::float_cmp, clippy::items_after_statements)]

use crate::logging::{LogComponent, LogStage};
use crate::types::{CostValue, ProviderTypeId, TokenCount};
use crate::{ldebug, lerror, linfo, lwarn};
use anyhow::Result;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::HashMap;
use std::sync::Arc;

use entity::{
    model_pricing::{self, Entity as ModelPricing},
    model_pricing_tiers::{self, Entity as ModelPricingTiers},
};

/// 费用计算服务
#[derive(Debug, Clone)]
pub struct PricingCalculatorService {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
}

/// Token使用情况
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    /// 输入tokens
    pub prompt_tokens: Option<TokenCount>,
    /// 输出tokens
    pub completion_tokens: Option<TokenCount>,
    /// 缓存创建tokens
    pub cache_create_tokens: Option<TokenCount>,
    /// 缓存读取tokens
    pub cache_read_tokens: Option<TokenCount>,
}

/// 费用计算结果
#[derive(Debug, Clone)]
pub struct CostCalculationResult {
    /// 总费用
    pub total_cost: CostValue,
    /// 货币单位
    pub currency: String,
    /// 详细费用分解
    pub cost_breakdown: HashMap<String, CostValue>,
    /// 是否使用了fallback定价
    pub used_fallback: bool,
}

impl PricingCalculatorService {
    /// 创建新的费用计算服务
    #[must_use]
    pub const fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// 计算请求费用
    ///
    /// # 参数
    /// - `model_used`: 使用的模型名称
    /// - `provider_type_id`: 提供商类型ID
    /// - `token_usage`: Token使用情况
    /// - `request_id`: 请求ID（用于日志）
    #[allow(clippy::cognitive_complexity)]
    pub async fn calculate_cost(
        &self,
        model_used: &str,
        provider_type_id: ProviderTypeId,
        token_usage: &TokenUsage,
        request_id: &str,
    ) -> Result<CostCalculationResult> {
        // 查找模型定价配置
        let Some(model_pricing) = self
            .find_model_pricing(model_used, provider_type_id)
            .await?
        else {
            lwarn!(
                request_id,
                LogStage::Internal,
                LogComponent::Statistics,
                "no_pricing_config",
                "No pricing configuration found for model, using fallback",
                model = %model_used,
                provider_type_id = provider_type_id,
            );
            return Ok(Self::create_fallback_result());
        };

        linfo!(
            request_id,
            LogStage::Internal,
            LogComponent::Statistics,
            "pricing_config_found",
            "Found pricing configuration",
            model = %model_used,
            provider_type_id = provider_type_id,
            pricing_id = model_pricing.id,
            currency = %model_pricing.cost_currency,
        );

        // 查找阶梯定价配置
        let pricing_tiers = self.get_pricing_tiers(model_pricing.id).await?;

        if pricing_tiers.is_empty() {
            lwarn!(
                request_id,
                LogStage::Internal,
                LogComponent::Statistics,
                "no_pricing_tiers",
                "No pricing tiers found for model, using fallback",
                model = %model_used,
                pricing_id = model_pricing.id,
            );
            return Ok(Self::create_fallback_result());
        }

        // 计算各类型token的费用
        let mut cost_breakdown: HashMap<String, CostValue> = HashMap::new();
        let mut total_cost: CostValue = 0.0;

        // 计算prompt tokens费用
        if let Some(prompt_tokens) = token_usage.prompt_tokens {
            let cost =
                Self::calculate_tiered_cost("prompt", prompt_tokens, &pricing_tiers, request_id);
            cost_breakdown.insert("prompt_tokens".to_string(), cost);
            total_cost += cost;
        }

        // 计算completion tokens费用
        if let Some(completion_tokens) = token_usage.completion_tokens {
            let cost = Self::calculate_tiered_cost(
                "completion",
                completion_tokens,
                &pricing_tiers,
                request_id,
            );
            cost_breakdown.insert("completion_tokens".to_string(), cost);
            total_cost += cost;
        }

        // 计算cache create tokens费用
        if let Some(cache_create_tokens) = token_usage.cache_create_tokens {
            let cost = Self::calculate_tiered_cost(
                "cache_create",
                cache_create_tokens,
                &pricing_tiers,
                request_id,
            );
            cost_breakdown.insert("cache_create_tokens".to_string(), cost);
            total_cost += cost;
        }

        // 计算cache read tokens费用
        if let Some(cache_read_tokens) = token_usage.cache_read_tokens {
            let cost = Self::calculate_tiered_cost(
                "cache_read",
                cache_read_tokens,
                &pricing_tiers,
                request_id,
            );
            cost_breakdown.insert("cache_read_tokens".to_string(), cost);
            total_cost += cost;
        }

        linfo!(
            request_id,
            LogStage::Internal,
            LogComponent::Statistics,
            "cost_calculated",
            "Successfully calculated cost",
            model = %model_used,
            total_cost = total_cost,
            currency = %model_pricing.cost_currency,
            cost_breakdown = ?cost_breakdown,
        );

        Ok(CostCalculationResult {
            total_cost,
            currency: model_pricing.cost_currency,
            cost_breakdown,
            used_fallback: false,
        })
    }

    /// `查找模型定价配置并验证ProviderTypeId匹配`
    async fn find_model_pricing(
        &self,
        model_name: &str,
        expected_provider_type_id: ProviderTypeId,
    ) -> Result<Option<model_pricing::Model>> {
        let pricing = ModelPricing::find()
            .filter(model_pricing::Column::ModelName.eq(model_name))
            .filter(model_pricing::Column::ProviderTypeId.eq(expected_provider_type_id))
            .one(&*self.db)
            .await?;

        // 验证ProviderTypeId匹配
        if let Some(ref pricing_model) = pricing {
            if pricing_model.provider_type_id != expected_provider_type_id {
                lerror!(
                    "system",
                    LogStage::Internal,
                    LogComponent::Statistics,
                    "provider_id_mismatch",
                    "ProviderTypeId mismatch in pricing configuration",
                    model = %model_name,
                    expected_provider_id = expected_provider_type_id,
                    actual_provider_id = pricing_model.provider_type_id,
                );
                return Ok(None);
            }

            ldebug!(
                "system",
                LogStage::Internal,
                LogComponent::Statistics,
                "provider_id_validation_ok",
                "ProviderTypeId validation successful",
                model = %model_name,
                provider_type_id = expected_provider_type_id,
                pricing_id = pricing_model.id,
            );
        }

        Ok(pricing)
    }

    /// 获取模型的阶梯定价配置
    async fn get_pricing_tiers(
        &self,
        model_pricing_id: i32,
    ) -> Result<Vec<model_pricing_tiers::Model>> {
        let tiers = ModelPricingTiers::find()
            .filter(model_pricing_tiers::Column::ModelPricingId.eq(model_pricing_id))
            .all(&*self.db)
            .await?;

        ldebug!(
            "system",
            LogStage::Internal,
            LogComponent::Statistics,
            "pricing_tiers_retrieved",
            "Retrieved pricing tiers",
            model_pricing_id = model_pricing_id,
            tiers_count = tiers.len(),
        );

        Ok(tiers)
    }

    /// 计算特定token类型的阶梯费用
    fn calculate_tiered_cost(
        token_type: &str,
        token_count: TokenCount,
        pricing_tiers: &[model_pricing_tiers::Model],
        request_id: &str,
    ) -> CostValue {
        // 尝试将 TokenCount 转换为 i32，如果失败说明 token 数量异常大
        let token_count = i32::try_from(token_count).unwrap_or_else(|_| {
            lerror!(
                request_id,
                LogStage::Internal,
                LogComponent::Statistics,
                "token_count_overflow",
                "Token count exceeds i32::MAX, this is likely an error",
                token_type = %token_type,
                token_count = token_count,
            );
            0
        });

        // 筛选出对应token类型的阶梯定价
        let relevant_tiers: Vec<_> = pricing_tiers
            .iter()
            .filter(|tier| tier.token_type == token_type)
            .collect();

        if relevant_tiers.is_empty() {
            ldebug!(
                request_id,
                LogStage::Internal,
                LogComponent::Statistics,
                "no_pricing_tiers_for_type",
                "No pricing tiers found for token type, cost = 0",
                token_type = %token_type,
                token_count = token_count,
            );
            return 0.0;
        }

        // 按min_tokens排序，确保阶梯计算正确
        let mut sorted_tiers = relevant_tiers.clone();
        sorted_tiers.sort_by_key(|tier| tier.min_tokens);

        let mut total_cost: CostValue = 0.0;

        for tier in &sorted_tiers {
            // 计算在此阶梯内使用的token数量
            let tokens_in_tier = tier.calculate_tokens_in_tier(token_count);
            if tokens_in_tier > 0 {
                let tier_cost = tier.calculate_cost(tokens_in_tier);
                total_cost += tier_cost;

                ldebug!(
                    request_id,
                    LogStage::Internal,
                    LogComponent::Statistics,
                    "pricing_tier_applied",
                    "Applied pricing tier",
                    token_type = %token_type,
                    tier_min = tier.min_tokens,
                    tier_max = ?tier.max_tokens,
                    tokens_in_tier = tokens_in_tier,
                    price_per_token = tier.price_per_token,
                    tier_cost = tier_cost,
                );
            }
        }

        ldebug!(
            request_id,
            LogStage::Internal,
            LogComponent::Statistics,
            "tiered_cost_calculated",
            "Calculated tiered cost",
            token_type = %token_type,
            token_count = token_count,
            total_cost = total_cost,
            tiers_used = sorted_tiers.len(),
        );

        total_cost
    }

    /// 创建fallback费用结果（当无法找到定价配置时）
    fn create_fallback_result() -> CostCalculationResult {
        CostCalculationResult {
            total_cost: 0.0,
            currency: "USD".to_string(),
            cost_breakdown: HashMap::new(),
            used_fallback: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::Database;

    async fn setup_test_db() -> Arc<DatabaseConnection> {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to test database");
        Migrator::up(&db, None)
            .await
            .expect("Failed to run migrations");
        Arc::new(db)
    }

    #[tokio::test]
    async fn test_fallback_pricing() {
        let db = setup_test_db().await;
        let pricing_service = PricingCalculatorService::new(db.clone());

        // 测试无模型定价配置时的fallback行为
        let token_usage = TokenUsage {
            prompt_tokens: Some(100),
            completion_tokens: Some(50),
            cache_create_tokens: None,
            cache_read_tokens: None,
        };

        let result = pricing_service
            .calculate_cost("nonexistent-model", 999, &token_usage, "test-request-1")
            .await
            .expect("Should return fallback result");

        assert_eq!(result.total_cost, 0.0);
        assert_eq!(result.currency, "USD");
        assert!(result.used_fallback);
        assert!(result.cost_breakdown.is_empty());
    }

    #[tokio::test]
    async fn test_pricing_with_data() {
        use entity::{model_pricing, model_pricing_tiers, provider_types};
        use sea_orm::{NotSet, Set};

        let db = setup_test_db().await;
        let pricing_service = PricingCalculatorService::new(db.clone());

        // 插入provider_types测试数据
        let provider_type = provider_types::ActiveModel {
            id: NotSet, // 让数据库自动生成ID
            name: Set("openai-test1".to_string()),
            display_name: Set("OpenAI Test 1".to_string()),
            base_url: Set("https://api.openai.com".to_string()),
            api_format: Set("openai".to_string()),
            is_active: Set(true),
            timeout_seconds: Set(Some(30)),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };
        let provider_insert_result = entity::provider_types::Entity::insert(provider_type)
            .exec(&*db)
            .await
            .unwrap();
        let provider_type_id = provider_insert_result.last_insert_id;

        // 插入model_pricing测试数据
        let model_pricing_record = model_pricing::ActiveModel {
            id: NotSet, // 让数据库自动生成ID
            provider_type_id: Set(provider_type_id),
            model_name: Set("gpt-4".to_string()),
            description: Set(Some("GPT-4 model".to_string())),
            cost_currency: Set("USD".to_string()),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
        };
        let model_pricing_insert_result =
            entity::model_pricing::Entity::insert(model_pricing_record)
                .exec(&*db)
                .await
                .unwrap();
        let model_pricing_id = model_pricing_insert_result.last_insert_id;

        // 插入model_pricing_tiers测试数据
        let prompt_tier = model_pricing_tiers::ActiveModel {
            id: NotSet, // 让数据库自动生成ID
            model_pricing_id: Set(model_pricing_id),
            token_type: Set("prompt".to_string()),
            min_tokens: Set(0),
            max_tokens: Set(None),
            price_per_token: Set(0.00003), // $0.03 per 1K tokens
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
        };
        entity::model_pricing_tiers::Entity::insert(prompt_tier)
            .exec(&*db)
            .await
            .unwrap();

        let completion_tier = model_pricing_tiers::ActiveModel {
            id: NotSet, // 让数据库自动生成ID
            model_pricing_id: Set(model_pricing_id),
            token_type: Set("completion".to_string()),
            min_tokens: Set(0),
            max_tokens: Set(None),
            price_per_token: Set(0.00006), // $0.06 per 1K tokens
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
        };
        entity::model_pricing_tiers::Entity::insert(completion_tier)
            .exec(&*db)
            .await
            .unwrap();

        // 测试费用计算
        let token_usage = TokenUsage {
            prompt_tokens: Some(1000),    // 1000 prompt tokens
            completion_tokens: Some(500), // 500 completion tokens
            cache_create_tokens: None,
            cache_read_tokens: None,
        };

        let result = pricing_service
            .calculate_cost("gpt-4", provider_type_id, &token_usage, "test-request-2")
            .await
            .expect("Should calculate cost successfully");

        assert!(!result.used_fallback);
        assert_eq!(result.currency, "USD");

        // 期望费用: (1000 * 0.00003) + (500 * 0.00006) = 0.03 + 0.03 = 0.06
        // 使用容差比较来处理浮点精度问题
        const EPSILON: f64 = 1e-10;
        assert!(
            (result.total_cost - 0.06).abs() < EPSILON,
            "Expected total cost ~0.06, got {}",
            result.total_cost
        );

        assert_eq!(result.cost_breakdown.len(), 2);
        assert!(
            (result.cost_breakdown.get("prompt_tokens").unwrap() - 0.03).abs() < EPSILON,
            "Expected prompt tokens cost ~0.03, got {}",
            result.cost_breakdown.get("prompt_tokens").unwrap()
        );
        assert!(
            (result.cost_breakdown.get("completion_tokens").unwrap() - 0.03).abs() < EPSILON,
            "Expected completion tokens cost ~0.03, got {}",
            result.cost_breakdown.get("completion_tokens").unwrap()
        );
    }

    #[tokio::test]
    async fn test_provider_type_id_validation() {
        use entity::{model_pricing, provider_types};
        use sea_orm::{NotSet, Set};

        let db = setup_test_db().await;
        let pricing_service = PricingCalculatorService::new(db.clone());

        // 插入provider_types测试数据
        let provider_type = provider_types::ActiveModel {
            id: NotSet, // 让数据库自动生成ID
            name: Set("openai-test2".to_string()),
            display_name: Set("OpenAI Test 2".to_string()),
            base_url: Set("https://api.openai.com".to_string()),
            api_format: Set("openai".to_string()),
            is_active: Set(true),
            timeout_seconds: Set(Some(30)),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };
        let provider_insert_result = entity::provider_types::Entity::insert(provider_type)
            .exec(&*db)
            .await
            .unwrap();
        let provider_type_id = provider_insert_result.last_insert_id;

        // 插入model_pricing测试数据，但使用不同的provider_type_id
        let model_pricing_record = model_pricing::ActiveModel {
            id: NotSet,                              // 让数据库自动生成ID
            provider_type_id: Set(provider_type_id), // 使用自动生成的provider_type_id
            model_name: Set("gpt-4".to_string()),
            description: Set(Some("GPT-4 model".to_string())),
            cost_currency: Set("USD".to_string()),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
        };
        entity::model_pricing::Entity::insert(model_pricing_record)
            .exec(&*db)
            .await
            .unwrap();

        let token_usage = TokenUsage {
            prompt_tokens: Some(100),
            completion_tokens: Some(50),
            cache_create_tokens: None,
            cache_read_tokens: None,
        };

        // 测试使用错误的provider_type_id，应该fallback
        let result = pricing_service
            .calculate_cost("gpt-4", 999, &token_usage, "test-request-3") // 错误的provider_type_id
            .await
            .expect("Should return fallback result");

        assert!(result.used_fallback);
        assert_eq!(result.total_cost, 0.0);

        // 测试使用正确的provider_type_id，应该找到模型但因为没有pricing tiers而fallback
        let result = pricing_service
            .calculate_cost("gpt-4", provider_type_id, &token_usage, "test-request-4") // 使用自动生成的provider_type_id
            .await
            .expect("Should return fallback result");

        assert!(result.used_fallback); // 因为没有定价阶梯而fallback
        assert_eq!(result.total_cost, 0.0);
    }
}
