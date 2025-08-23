//! # 数据库模块
//!
//! 数据库连接和迁移管理

use crate::error::ProxyError;
use entity::{model_pricing, model_pricing_tiers, provider_types};
use sea_orm::{
    ColumnTrait, Database, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait, QueryFilter, Set,
};
use sea_orm_migration::MigratorTrait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::{debug, error, info, warn};

/// 初始化数据库连接
pub async fn init_database(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    info!(
        "正在连接数据库: {}",
        if database_url.starts_with("sqlite:") {
            &database_url[..std::cmp::min(database_url.len(), 50)]
        } else {
            database_url
        }
    );

    // 对于SQLite数据库，确保数据库文件的目录和文件存在
    if database_url.starts_with("sqlite:") {
        let db_path = database_url
            .strip_prefix("sqlite://")
            .unwrap_or(database_url.strip_prefix("sqlite:").unwrap_or(database_url));
        let db_file_path = Path::new(db_path);

        // 确保父目录存在
        if let Some(parent_dir) = db_file_path.parent() {
            if !parent_dir.exists() {
                debug!("创建数据库目录: {}", parent_dir.display());
                std::fs::create_dir_all(parent_dir).map_err(|e| {
                    DbErr::Custom(format!(
                        "无法创建数据库目录 {}: {}",
                        parent_dir.display(),
                        e
                    ))
                })?;
                info!("数据库目录创建成功: {}", parent_dir.display());
            } else {
                debug!("数据库目录已存在: {}", parent_dir.display());
            }
        }

        // 确保数据库文件存在（如果不存在则创建空文件）
        if !db_file_path.exists() {
            debug!("创建数据库文件: {}", db_file_path.display());
            std::fs::File::create(db_file_path).map_err(|e| {
                DbErr::Custom(format!(
                    "无法创建数据库文件 {}: {}",
                    db_file_path.display(),
                    e
                ))
            })?;
            info!("数据库文件创建成功: {}", db_file_path.display());
        } else {
            debug!("数据库文件已存在: {}", db_file_path.display());
        }
    }

    let db = Database::connect(database_url).await?;

    info!("数据库连接成功");
    Ok(db)
}

/// 运行数据库迁移
pub async fn run_migrations(db: &DatabaseConnection) -> Result<(), DbErr> {
    info!("开始运行数据库迁移...");

    match ::migration::Migrator::up(db, None).await {
        Ok(_) => {
            info!("数据库迁移完成");
            Ok(())
        }
        Err(e) => {
            error!("数据库迁移失败: {}", e);
            Err(e)
        }
    }
}

/// 检查数据库状态
pub async fn check_database_status(db: &DatabaseConnection) -> Result<(), DbErr> {
    info!("检查数据库状态...");

    let status = ::migration::Migrator::get_pending_migrations(db).await?;

    if status.is_empty() {
        info!("所有迁移都已应用");
    } else {
        warn!("有 {} 个待应用的迁移", status.len());
    }

    Ok(())
}

/// 从 JSON 解析出的模型定价信息
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelPriceInfo {
    // 基础定价字段
    #[serde(rename = "input_cost_per_token")]
    input_cost_per_token: Option<f64>,
    #[serde(rename = "output_cost_per_token")]
    output_cost_per_token: Option<f64>,
    
    // 缓存相关定价
    #[serde(rename = "cache_creation_input_token_cost")]
    cache_creation_input_token_cost: Option<f64>,
    #[serde(rename = "cache_read_input_token_cost")]
    cache_read_input_token_cost: Option<f64>,
    
    // 阶梯定价字段
    #[serde(rename = "input_cost_per_token_above_200k_tokens")]
    input_cost_per_token_above_200k: Option<f64>,
    #[serde(rename = "output_cost_per_token_above_200k_tokens")]
    output_cost_per_token_above_200k: Option<f64>,
    #[serde(rename = "input_cost_per_token_above_128k_tokens")]
    input_cost_per_token_above_128k: Option<f64>,
    #[serde(rename = "output_cost_per_token_above_128k_tokens")]
    output_cost_per_token_above_128k: Option<f64>,
    
    // Provider信息
    litellm_provider: Option<String>,
    
    // 其他字段（忽略，使用 flatten 来捕获所有其他字段）
    #[serde(flatten)]
    _other: serde_json::Map<String, serde_json::Value>,
}

/// 处理后的定价层级信息
#[derive(Debug, Clone)]
struct PricingTier {
    token_type: String,
    min_tokens: i32,
    max_tokens: Option<i32>,
    price_per_token: f64,
}

/// 过滤后的目标模型信息
#[derive(Debug)]
struct FilteredModel {
    name: String,
    description: String,
    provider_name: String,
    price_info: ModelPriceInfo,
}

/// 确保模型定价数据的完整性
/// 检查数据库中是否存在模型定价数据，如果不存在则进行初始化
pub async fn ensure_model_pricing_data(db: &DatabaseConnection) -> Result<(), ProxyError> {
    info!("🔍 检查模型定价数据完整性...");
    
    // 检查 model_pricing 表是否为空
    let pricing_count = model_pricing::Entity::find()
        .count(db)
        .await
        .map_err(|e| ProxyError::database(format!("查询模型定价数据失败: {}", e)))?;
        
    if pricing_count == 0 {
        info!("📊 模型定价数据为空，开始初始化...");
        initialize_model_pricing_from_json(db).await?;
    } else {
        info!("✅ 模型定价数据已存在 ({} 条记录)", pricing_count);
    }
    
    Ok(())
}

/// 强制重新初始化模型定价数据
pub async fn force_initialize_model_pricing_data(db: &DatabaseConnection) -> Result<(), ProxyError> {
    info!("🔄 强制重新初始化模型定价数据...");
    
    // 清理现有数据
    model_pricing_tiers::Entity::delete_many()
        .exec(db)
        .await
        .map_err(|e| ProxyError::database(format!("清理定价层级数据失败: {}", e)))?;
        
    model_pricing::Entity::delete_many()
        .exec(db)
        .await
        .map_err(|e| ProxyError::database(format!("清理模型定价数据失败: {}", e)))?;
        
    // 重新初始化
    initialize_model_pricing_from_json(db).await?;
    
    Ok(())
}

/// 从 JSON 文件初始化数据（完全数据驱动）
async fn initialize_model_pricing_from_json(db: &DatabaseConnection) -> Result<(), ProxyError> {
    info!("📂 从JSON文件读取模型定价数据...");
    
    // 1. 读取并解析JSON文件
    let json_data = load_json_data().await?;
    info!("✅ 成功解析了 {} 个模型的定价数据", json_data.len());
    
    // 2. 应用数据驱动的模型过滤
    let filtered_models = filter_target_models(&json_data);
    info!("🎯 根据过滤规则选择了 {} 个目标模型", filtered_models.len());
    
    // 3. 动态获取所需的provider映射
    let provider_mappings = get_provider_mappings(db, &filtered_models).await?;
    info!("🗺️  构建了 {} 个provider映射", provider_mappings.len());
    
    // 4. 批量插入模型定价数据
    let mut success_count = 0;
    for model in filtered_models {
        if let Some(&provider_id) = provider_mappings.get(&model.provider_name) {
            match insert_model_with_pricing(db, &model, provider_id).await {
                Ok(_) => success_count += 1,
                Err(e) => {
                    error!("插入模型 {} 失败: {}", model.name, e);
                }
            }
        } else {
            warn!("⚠️  跳过模型: {} - provider '{}' 在数据库中不存在", 
                 model.name, model.provider_name);
        }
    }
    
    info!("✅ 数据初始化完成! 成功处理了 {} 个模型", success_count);
    Ok(())
}

/// 加载并解析JSON文件
async fn load_json_data() -> Result<HashMap<String, ModelPriceInfo>, ProxyError> {
    let json_path = std::env::current_dir()
        .map_err(|e| ProxyError::config(format!("获取当前目录失败: {}", e)))?
        .join("config")
        .join("model_prices_and_context_window.json");
        
    if !json_path.exists() {
        return Err(ProxyError::config(format!("JSON文件不存在: {:?}", json_path)));
    }
        
    let json_content = tokio::fs::read_to_string(&json_path).await
        .map_err(|e| ProxyError::config(format!("读取JSON文件失败 {:?}: {}", json_path, e)))?;
        
    serde_json::from_str(&json_content)
        .map_err(|e| ProxyError::config(format!("解析JSON失败: {}", e)))
}

/// 完全数据驱动的模型过滤
/// 基于模型名称模式匹配选择目标模型
fn filter_target_models(json_data: &HashMap<String, ModelPriceInfo>) -> Vec<FilteredModel> {
    // 定义目标模型的过滤规则（基于用户需求）
    let target_patterns = [
        ("gemini-2.5", "gemini", "Gemini 2.5 系列模型"),
        ("gpt-4o", "openai", "GPT-4o 系列模型"), 
        ("claude-sonnet-4", "claude", "Claude Sonnet 4 系列模型"),
        ("claude-opus-4", "claude", "Claude Opus 4 系列模型"),
    ];
    
    let mut filtered_models = Vec::new();
    
    for (model_name, price_info) in json_data {
        // 检查模型名是否匹配任何目标模式
        if let Some((_pattern, default_provider, description)) = target_patterns
            .iter()
            .find(|(pattern, _, _)| model_name.contains(pattern))
        {
            let provider_name = price_info
                .litellm_provider
                .clone()
                .unwrap_or_else(|| default_provider.to_string());
                
            filtered_models.push(FilteredModel {
                name: model_name.clone(),
                description: format!("{} ({})", description, model_name),
                provider_name: provider_name.clone(),
                price_info: price_info.clone(),
            });
            
            info!("🎯 选择模型: {} (provider: {})", model_name, provider_name);
        }
    }
    
    filtered_models
}

/// 动态获取provider映射关系
/// 从数据库查询所有活跃的provider，构建name -> id映射
async fn get_provider_mappings(
    db: &DatabaseConnection, 
    models: &[FilteredModel]
) -> Result<HashMap<String, i32>, ProxyError> {
    // 提取所有需要的provider名称
    let required_providers: HashSet<String> = models
        .iter()
        .map(|m| m.provider_name.clone())
        .collect();
        
    info!("📋 需要查询的providers: {:?}", required_providers);
    
    // 查询数据库中所有活跃的provider
    let providers = provider_types::Entity::find()
        .filter(provider_types::Column::IsActive.eq(true))
        .all(db)
        .await
        .map_err(|e| ProxyError::database(format!("查询provider类型失败: {}", e)))?;
        
    // 构建映射关系
    let mut mappings = HashMap::new();
    for provider in providers {
        if required_providers.contains(&provider.name) {
            mappings.insert(provider.name.clone(), provider.id);
            info!("🔗 Provider映射: {} -> {}", provider.name, provider.id);
        }
    }
    
    // 检查是否有缺失的provider
    for required in &required_providers {
        if !mappings.contains_key(required) {
            warn!("⚠️  Provider '{}' 在数据库中不存在", required);
        }
    }
    
    Ok(mappings)
}

/// 插入单个模型及其定价数据
async fn insert_model_with_pricing(
    db: &DatabaseConnection,
    model: &FilteredModel, 
    provider_id: i32
) -> Result<(), ProxyError> {
    info!("💰 插入模型定价: {} (provider_id: {})", model.name, provider_id);
    
    // 1. 插入model_pricing记录
    let pricing_model = model_pricing::ActiveModel {
        provider_type_id: Set(provider_id),
        model_name: Set(model.name.clone()),
        description: Set(Some(model.description.clone())),
        cost_currency: Set("USD".to_string()),
        ..Default::default()
    };
    
    let pricing_result = model_pricing::Entity::insert(pricing_model)
        .exec(db)
        .await
        .map_err(|e| ProxyError::database(format!("插入模型定价记录失败: {}", e)))?;
        
    let model_pricing_id = pricing_result.last_insert_id;
    
    // 2. 解析并插入定价层级
    let pricing_tiers = parse_pricing_tiers(&model.price_info);
    info!("🎯 为模型 {} 解析出 {} 个定价层级", model.name, pricing_tiers.len());
    
    for tier in pricing_tiers {
        let tier_model = model_pricing_tiers::ActiveModel {
            model_pricing_id: Set(model_pricing_id),
            token_type: Set(tier.token_type),
            min_tokens: Set(tier.min_tokens),
            max_tokens: Set(tier.max_tokens),
            price_per_token: Set(tier.price_per_token),
            ..Default::default()
        };
        
        model_pricing_tiers::Entity::insert(tier_model)
            .exec(db)
            .await
            .map_err(|e| ProxyError::database(format!("插入定价层级失败: {}", e)))?;
    }
    
    Ok(())
}

/// 从ModelPriceInfo解析出定价层级
fn parse_pricing_tiers(price_info: &ModelPriceInfo) -> Vec<PricingTier> {
    let mut tiers = Vec::new();
    
    // 处理输入token定价
    if let Some(base_input_cost) = price_info.input_cost_per_token {
        if let Some(above_200k_cost) = price_info.input_cost_per_token_above_200k {
            // 200k阶梯定价
            tiers.push(PricingTier {
                token_type: "prompt".to_string(),
                min_tokens: 0,
                max_tokens: Some(199_999),
                price_per_token: base_input_cost,
            });
            tiers.push(PricingTier {
                token_type: "prompt".to_string(),
                min_tokens: 200_000,
                max_tokens: None,
                price_per_token: above_200k_cost,
            });
        } else if let Some(above_128k_cost) = price_info.input_cost_per_token_above_128k {
            // 128k阶梯定价
            tiers.push(PricingTier {
                token_type: "prompt".to_string(),
                min_tokens: 0,
                max_tokens: Some(127_999),
                price_per_token: base_input_cost,
            });
            tiers.push(PricingTier {
                token_type: "prompt".to_string(),
                min_tokens: 128_000,
                max_tokens: None,
                price_per_token: above_128k_cost,
            });
        } else {
            // 无阶梯，统一价格
            tiers.push(PricingTier {
                token_type: "prompt".to_string(),
                min_tokens: 0,
                max_tokens: None,
                price_per_token: base_input_cost,
            });
        }
    }
    
    // 处理输出token定价
    if let Some(base_output_cost) = price_info.output_cost_per_token {
        if let Some(above_200k_cost) = price_info.output_cost_per_token_above_200k {
            // 200k阶梯定价
            tiers.push(PricingTier {
                token_type: "completion".to_string(),
                min_tokens: 0,
                max_tokens: Some(199_999),
                price_per_token: base_output_cost,
            });
            tiers.push(PricingTier {
                token_type: "completion".to_string(),
                min_tokens: 200_000,
                max_tokens: None,
                price_per_token: above_200k_cost,
            });
        } else if let Some(above_128k_cost) = price_info.output_cost_per_token_above_128k {
            // 128k阶梯定价
            tiers.push(PricingTier {
                token_type: "completion".to_string(),
                min_tokens: 0,
                max_tokens: Some(127_999),
                price_per_token: base_output_cost,
            });
            tiers.push(PricingTier {
                token_type: "completion".to_string(),
                min_tokens: 128_000,
                max_tokens: None,
                price_per_token: above_128k_cost,
            });
        } else {
            // 无阶梯，统一价格
            tiers.push(PricingTier {
                token_type: "completion".to_string(),
                min_tokens: 0,
                max_tokens: None,
                price_per_token: base_output_cost,
            });
        }
    }
    
    // 处理缓存相关定价
    if let Some(cache_create_cost) = price_info.cache_creation_input_token_cost {
        tiers.push(PricingTier {
            token_type: "cache_create".to_string(),
            min_tokens: 0,
            max_tokens: None,
            price_per_token: cache_create_cost,
        });
    }
    
    if let Some(cache_read_cost) = price_info.cache_read_input_token_cost {
        tiers.push(PricingTier {
            token_type: "cache_read".to_string(),
            min_tokens: 0,
            max_tokens: None,
            price_per_token: cache_read_cost,
        });
    }
    
    tiers
}
