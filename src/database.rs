//! # 数据库模块
//!
//! 数据库连接和迁移管理

use crate::error::ProxyError;
use crate::logging::{LogComponent, LogStage};
use crate::{ldebug, lerror, linfo, lwarn};
use entity::{model_pricing, model_pricing_tiers, provider_types};
use sea_orm::{
    ColumnTrait, Database, DatabaseConnection, DatabaseTransaction, DbErr, EntityTrait,
    PaginatorTrait, QueryFilter, Set, TransactionTrait,
};
use sea_orm_migration::MigratorTrait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

/// 初始化数据库连接
pub async fn init_database(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "db_connect",
        &format!(
            "正在连接数据库: {}",
            if database_url.starts_with("sqlite:") {
                &database_url[..std::cmp::min(database_url.len(), 50)]
            } else {
                database_url
            }
        )
    );

    // 对于SQLite数据库，确保数据库文件的目录和文件存在
    if database_url.starts_with("sqlite:") {
        let db_path = database_url
            .strip_prefix("sqlite://")
            .unwrap_or(database_url.strip_prefix("sqlite:").unwrap_or(database_url));
        let db_file_path = Path::new(db_path);

        // 确保父目录存在
        if let Some(parent_dir) = db_file_path.parent() {
            if parent_dir.exists() {
                ldebug!(
                    "system",
                    LogStage::Startup,
                    LogComponent::Database,
                    "db_dir_exists",
                    &format!("数据库目录已存在: {}", parent_dir.display())
                );
            } else {
                ldebug!(
                    "system",
                    LogStage::Startup,
                    LogComponent::Database,
                    "create_db_dir",
                    &format!("创建数据库目录: {}", parent_dir.display())
                );
                std::fs::create_dir_all(parent_dir).map_err(|e| {
                    DbErr::Custom(format!(
                        "无法创建数据库目录 {}: {}",
                        parent_dir.display(),
                        e
                    ))
                })?;
                linfo!(
                    "system",
                    LogStage::Startup,
                    LogComponent::Database,
                    "create_db_dir_ok",
                    &format!("数据库目录创建成功: {}", parent_dir.display())
                );
            }
        }

        // 确保数据库文件存在（如果不存在则创建空文件）
        if db_file_path.exists() {
            ldebug!(
                "system",
                LogStage::Startup,
                LogComponent::Database,
                "db_file_exists",
                &format!("数据库文件已存在: {}", db_file_path.display())
            );
        } else {
            ldebug!(
                "system",
                LogStage::Startup,
                LogComponent::Database,
                "create_db_file",
                &format!("创建数据库文件: {}", db_file_path.display())
            );
            std::fs::File::create(db_file_path).map_err(|e| {
                DbErr::Custom(format!(
                    "无法创建数据库文件 {}: {}",
                    db_file_path.display(),
                    e
                ))
            })?;
            linfo!(
                "system",
                LogStage::Startup,
                LogComponent::Database,
                "create_db_file_ok",
                &format!("数据库文件创建成功: {}", db_file_path.display())
            );
        }
    }

    let db = Database::connect(database_url).await?;

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "db_connect_ok",
        "数据库连接成功"
    );
    Ok(db)
}

/// 运行数据库迁移
pub async fn run_migrations(db: &DatabaseConnection) -> Result<(), DbErr> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "migration_start",
        "开始运行数据库迁移..."
    );

    match ::migration::Migrator::up(db, None).await {
        Ok(()) => {
            linfo!(
                "system",
                LogStage::Startup,
                LogComponent::Database,
                "migration_ok",
                "数据库迁移完成"
            );
            Ok(())
        }
        Err(e) => {
            lerror!(
                "system",
                LogStage::Startup,
                LogComponent::Database,
                "migration_fail",
                &format!("数据库迁移失败: {e}")
            );
            Err(e)
        }
    }
}

/// 检查数据库状态
pub async fn check_database_status(db: &DatabaseConnection) -> Result<(), DbErr> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "check_db_status",
        "检查数据库状态..."
    );

    let status = ::migration::Migrator::get_pending_migrations(db).await?;

    if status.is_empty() {
        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::Database,
            "migrations_applied",
            "所有迁移都已应用"
        );
    } else {
        lwarn!(
            "system",
            LogStage::Startup,
            LogComponent::Database,
            "pending_migrations",
            &format!("有 {} 个待应用的迁移", status.len())
        );
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

/// 确保模型定价数据的完整性（启动时初始化一次，远程优先，增量更新）
/// 始终尝试拉取并增量更新，失败时使用本地文件回退；如果都失败且已有数据，则保留现状。
pub async fn ensure_model_pricing_data(db: &DatabaseConnection) -> Result<(), ProxyError> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "ensure_pricing_data",
        "🔍 检查模型定价数据完整性..."
    );
    // 始终尝试远程优先的增量更新
    match initialize_model_pricing_from_remote_or_local(db).await {
        Ok(()) => Ok(()),
        Err(e) => {
            // 如果已经有数据，保留现状；否则向上抛出错误
            let pricing_count = model_pricing::Entity::find()
                .count(db)
                .await
                .map_err(|err| ProxyError::database(format!("查询模型定价数据失败: {err}")))?;
            if pricing_count > 0 {
                lerror!(
                    "system",
                    LogStage::Startup,
                    LogComponent::Database,
                    "pricing_init_fail",
                    "远程与本地初始化均失败，保留现有定价数据",
                    error = %e
                );
                Ok(())
            } else {
                Err(e)
            }
        }
    }
}

/// 强制重新初始化模型定价数据
pub async fn force_initialize_model_pricing_data(
    db: &DatabaseConnection,
) -> Result<(), ProxyError> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "force_init_pricing",
        "🔄 强制重新初始化模型定价数据..."
    );

    // 清理现有数据
    model_pricing_tiers::Entity::delete_many()
        .exec(db)
        .await
        .map_err(|e| ProxyError::database(format!("清理定价层级数据失败: {e}")))?;

    model_pricing::Entity::delete_many()
        .exec(db)
        .await
        .map_err(|e| ProxyError::database(format!("清理模型定价数据失败: {e}")))?;

    // 重新初始化
    initialize_model_pricing_from_json(db).await?;

    Ok(())
}

/// 从 JSON 文件初始化数据（完全数据驱动，旧逻辑，仅在空表或强制清理后使用）
async fn initialize_model_pricing_from_json(db: &DatabaseConnection) -> Result<(), ProxyError> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "load_pricing_from_json",
        "📂 从JSON文件读取模型定价数据..."
    );

    // 1. 读取并解析JSON文件
    let json_data = load_json_data().await?;
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "parse_pricing_ok",
        &format!("✅ 成功解析了 {} 个模型的定价数据", json_data.len())
    );

    // 2. 应用数据驱动的模型过滤
    let filtered_models = filter_target_models(&json_data);
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "filter_models_ok",
        &format!("🎯 根据过滤规则选择了 {} 个目标模型", filtered_models.len())
    );

    // 3. 动态获取所需的provider映射
    let provider_mappings = get_provider_mappings(db, &filtered_models).await?;
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "provider_mapping_ok",
        &format!("🗺️  构建了 {} 个provider映射", provider_mappings.len())
    );

    // 4. 批量插入模型定价数据
    let mut success_count = 0;
    for model in filtered_models {
        if let Some(&provider_id) = provider_mappings.get(&model.provider_name) {
            match insert_model_with_pricing(db, &model, provider_id).await {
                Ok(()) => success_count += 1,
                Err(e) => {
                    lerror!(
                        "system",
                        LogStage::Startup,
                        LogComponent::Database,
                        "insert_model_pricing_fail",
                        &format!("插入模型 {} 失败: {}", model.name, e)
                    );
                }
            }
        } else {
            lwarn!(
                "system",
                LogStage::Startup,
                LogComponent::Database,
                "skip_model_no_provider",
                &format!(
                    "⚠️  跳过模型: {} - provider '{}' 在数据库中不存在",
                    model.name, model.provider_name
                )
            );
        }
    }

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "init_pricing_ok",
        &format!("✅ 数据初始化完成! 成功处理了 {success_count} 个模型")
    );
    Ok(())
}

/// 远程优先的初始化与增量更新（不删除未出现在数据源中的旧模型）
async fn initialize_model_pricing_from_remote_or_local(
    db: &DatabaseConnection,
) -> Result<(), ProxyError> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "remote_pricing_fetch",
        "尝试从远程获取最新模型定价（失败则回退本地）..."
    );

    // 读取远程或本地 JSON
    let json_data = load_json_data_remote_or_local().await?;
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "remote_pricing_fetched",
        "已获取模型定价原始数据",
        models = json_data.len()
    );

    // 过滤并标准化
    let filtered_models = filter_target_models(&json_data);
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "filter_models",
        "根据规则筛选目标模型",
        count = filtered_models.len()
    );

    // provider 映射
    let provider_mappings = get_provider_mappings(db, &filtered_models).await?;
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "provider_mapping_complete",
        "构建 provider 映射完成",
        mappings = provider_mappings.len()
    );

    // 事务内增量 upsert
    let txn = db
        .begin()
        .await
        .map_err(|e| ProxyError::database(format!("开启事务失败: {e}")))?;
    let mut inserted = 0usize;
    let mut updated = 0usize;
    let mut tiers_written = 0usize;

    for model in filtered_models {
        if let Some(&provider_id) = provider_mappings.get(&model.provider_name) {
            // 查找是否存在同 provider + model_name 的记录
            let existing = model_pricing::Entity::find()
                .filter(model_pricing::Column::ProviderTypeId.eq(provider_id))
                .filter(model_pricing::Column::ModelName.eq(&model.name))
                .one(&txn)
                .await
                .map_err(|e| ProxyError::database(format!("查询现有定价记录失败: {e}")))?;

            if let Some(existing_model) = existing {
                // 更新基础字段
                let id = existing_model.id;
                let mut am: model_pricing::ActiveModel = existing_model.into();
                am.description = Set(Some(model.description.clone()));
                am.cost_currency = Set("USD".to_string());
                model_pricing::Entity::update(am)
                    .exec(&txn)
                    .await
                    .map_err(|e| ProxyError::database(format!("更新模型定价失败: {e}")))?;

                // 替换 tiers
                model_pricing_tiers::Entity::delete_many()
                    .filter(model_pricing_tiers::Column::ModelPricingId.eq(id))
                    .exec(&txn)
                    .await
                    .map_err(|e| ProxyError::database(format!("清理旧定价层级失败: {e}")))?;

                let pricing_tiers = parse_pricing_tiers(&model.price_info);
                for tier in pricing_tiers {
                    let tier_model = model_pricing_tiers::ActiveModel {
                        model_pricing_id: Set(id),
                        token_type: Set(tier.token_type),
                        min_tokens: Set(tier.min_tokens),
                        max_tokens: Set(tier.max_tokens),
                        price_per_token: Set(tier.price_per_token),
                        ..Default::default()
                    };
                    model_pricing_tiers::Entity::insert(tier_model)
                        .exec(&txn)
                        .await
                        .map_err(|e| ProxyError::database(format!("插入定价层级失败: {e}")))?;
                    tiers_written += 1;
                }
                updated += 1;
            } else {
                // 新增
                insert_model_with_pricing_txn(&txn, &model, provider_id).await?;
                let tiers = parse_pricing_tiers(&model.price_info);
                tiers_written += tiers.len();
                inserted += 1;
            }
        } else {
            lwarn!(
                "system",
                LogStage::Startup,
                LogComponent::Database,
                "skip_model_no_provider",
                "跳过：provider 在数据库中不存在",
                provider = %model.provider_name,
                model = %model.name
            );
        }
    }

    txn.commit()
        .await
        .map_err(|e| ProxyError::database(format!("提交模型定价事务失败: {e}")))?;

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "pricing_upsert_complete",
        "模型定价增量更新完成",
        inserted = inserted,
        updated = updated,
        tiers_written = tiers_written
    );

    Ok(())
}

/// 远程优先：先拉取远程 JSON，失败则回退本地文件
async fn load_json_data_remote_or_local() -> Result<HashMap<String, ModelPriceInfo>, ProxyError> {
    match fetch_remote_json().await {
        Ok(map) => {
            linfo!(
                "system",
                LogStage::Startup,
                LogComponent::Database,
                "use_remote_pricing",
                "使用远程模型定价数据",
                source = "remote"
            );
            Ok(map)
        }
        Err(e) => {
            lwarn!("system", LogStage::Startup, LogComponent::Database, "remote_pricing_fail", "远程获取失败，回退到本地JSON", error = %e);
            load_json_data().await
        }
    }
}

/// 拉取远程 JSON 模型定价
async fn fetch_remote_json() -> Result<HashMap<String, ModelPriceInfo>, ProxyError> {
    const REMOTE_URL: &str = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

    let url = REMOTE_URL
        .parse::<reqwest::Url>()
        .map_err(|e| ProxyError::config(format!("远程URL非法: {e}")))?;
    if url.scheme() != "https" {
        return Err(ProxyError::config("仅允许HTTPS的远程URL".to_string()));
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(5000))
        .build()
        .map_err(|e| ProxyError::config(format!("创建HTTP客户端失败: {e}")))?;

    let resp = client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            format!("api-proxy/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .await
        .map_err(|e| ProxyError::config(format!("请求远程模型定价失败: {e}")))?;

    if !resp.status().is_success() {
        return Err(ProxyError::config(format!(
            "远程定价响应非成功状态: {}",
            resp.status()
        )));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| ProxyError::config(format!("读取远程响应失败: {e}")))?;

    serde_json::from_str::<HashMap<String, ModelPriceInfo>>(&text)
        .map_err(|e| ProxyError::config(format!("解析远程JSON失败: {e}")))
}
/// 加载并解析JSON文件
async fn load_json_data() -> Result<HashMap<String, ModelPriceInfo>, ProxyError> {
    let json_path = std::env::current_dir()
        .map_err(|e| ProxyError::config(format!("获取当前目录失败: {e}")))?
        .join("config")
        .join("model_prices_and_context_window.json");

    if !json_path.exists() {
        return Err(ProxyError::config(format!("JSON文件不存在: {json_path:?}")));
    }

    let json_content = tokio::fs::read_to_string(&json_path)
        .await
        .map_err(|e| ProxyError::config(format!("读取JSON文件失败 {json_path:?}: {e}")))?;

    serde_json::from_str(&json_content)
        .map_err(|e| ProxyError::config(format!("解析JSON失败: {e}")))
}

/// 完全数据驱动的模型过滤
/// 基于 `litellm_provider` 字段选择目标提供商的所有模型
fn filter_target_models(json_data: &HashMap<String, ModelPriceInfo>) -> Vec<FilteredModel> {
    // 定义目标提供商映射：JSON中的provider名称 -> 数据库中的provider名称
    let provider_mappings = [
        ("gemini", "gemini"),
        ("anthropic", "anthropic"),
        ("openai", "openai"),
    ];

    let mut filtered_models = Vec::new();

    for (model_name, price_info) in json_data {
        // 基于 litellm_provider 字段进行过滤
        if let Some(litellm_provider) = &price_info.litellm_provider {
            // 查找匹配的提供商映射
            if let Some((_, db_provider_name)) = provider_mappings
                .iter()
                .find(|(json_provider, _)| litellm_provider == json_provider)
            {
                // 标准化模型名称：去除提供商前缀
                let normalized_model_name = normalize_model_name(model_name, litellm_provider);

                // 生成描述信息
                let description = match litellm_provider.as_str() {
                    "gemini" => format!("Google Gemini 模型 ({normalized_model_name})"),
                    "anthropic" => format!("Anthropic Claude 模型 ({normalized_model_name})"),
                    "openai" => format!("OpenAI 模型 ({normalized_model_name})"),
                    _ => format!("AI 模型 ({normalized_model_name})"),
                };

                filtered_models.push(FilteredModel {
                    name: normalized_model_name.clone(),
                    description,
                    provider_name: (*db_provider_name).to_string(),
                    price_info: price_info.clone(),
                });

                linfo!(
                    "system",
                    LogStage::Startup,
                    LogComponent::Database,
                    "select_model",
                    &format!(
                        "🎯 选择模型: {model_name} -> {normalized_model_name} (litellm_provider: {litellm_provider} -> db_provider: {db_provider_name})"
                    )
                );
            }
        }
    }

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "filter_models_result",
        &format!(
            "📊 过滤结果: 从 {} 个模型中选择了 {} 个目标模型",
            json_data.len(),
            filtered_models.len()
        )
    );

    filtered_models
}

/// 标准化模型名称，去除提供商前缀
///
/// `根据litellm_provider字段动态确定前缀，如果模型名称以"provider/"开头则去除`
/// # 示例
/// - `"gemini/gemini-2.5-flash"` (`litellm_provider="gemini`") -> `"gemini-2.5-flash"`
/// - `"anthropic/claude-3.5-sonnet"` (`litellm_provider="anthropic`") -> `"claude-3.5-sonnet"`
/// - `"openai/gpt-4"` (`litellm_provider="openai`") -> `"gpt-4"`
/// - `"gemini-2.5-flash"` (`litellm_provider="gemini`") -> `"gemini-2.5-flash"` (无前缀保持不变)
fn normalize_model_name(model_name: &str, litellm_provider: &str) -> String {
    // 构建基于litellm_provider的前缀
    let provider_prefix = format!("{litellm_provider}/");

    // 检查模型名称是否以该provider前缀开头
    if model_name.starts_with(&provider_prefix) {
        let normalized = model_name
            .strip_prefix(&provider_prefix)
            .unwrap_or(model_name);
        ldebug!(
            "system",
            LogStage::Startup,
            LogComponent::Database,
            "normalize_model_name",
            &format!(
                "标准化模型名称: {model_name} -> {normalized} (移除前缀: {provider_prefix} 基于litellm_provider: {litellm_provider})"
            )
        );
        return normalized.to_string();
    }

    // 无匹配前缀，保持原名称
    ldebug!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "normalize_model_name_skip",
        &format!("模型名称无需标准化: {model_name} (litellm_provider: {litellm_provider})")
    );
    model_name.to_string()
}

/// 动态获取provider映射关系
/// 从数据库查询所有活跃的provider，构建name -> id映射
async fn get_provider_mappings(
    db: &DatabaseConnection,
    models: &[FilteredModel],
) -> Result<HashMap<String, i32>, ProxyError> {
    // 提取所有需要的provider名称
    let required_providers: HashSet<String> =
        models.iter().map(|m| m.provider_name.clone()).collect();

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "query_providers",
        &format!("📋 需要查询的providers: {required_providers:?}")
    );

    // 查询数据库中所有活跃的provider
    let providers = provider_types::Entity::find()
        .filter(provider_types::Column::IsActive.eq(true))
        .all(db)
        .await
        .map_err(|e| ProxyError::database(format!("查询provider类型失败: {e}")))?;

    // 构建映射关系
    let mut mappings = HashMap::new();
    for provider in providers {
        if required_providers.contains(&provider.name) {
            mappings.insert(provider.name.clone(), provider.id);
            linfo!(
                "system",
                LogStage::Startup,
                LogComponent::Database,
                "provider_mapping",
                &format!("🔗 Provider映射: {} -> {}", provider.name, provider.id)
            );
        }
    }

    // 检查是否有缺失的provider
    for required in &required_providers {
        if !mappings.contains_key(required) {
            lwarn!(
                "system",
                LogStage::Startup,
                LogComponent::Database,
                "provider_not_found",
                &format!("⚠️  Provider '{required}' 在数据库中不存在")
            );
        }
    }

    Ok(mappings)
}

/// 插入单个模型及其定价数据
async fn insert_model_with_pricing(
    db: &DatabaseConnection,
    model: &FilteredModel,
    provider_id: i32,
) -> Result<(), ProxyError> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "insert_model_pricing",
        &format!(
            "💰 插入模型定价: {} (provider_id: {})",
            model.name, provider_id
        )
    );

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
        .map_err(|e| ProxyError::database(format!("插入模型定价记录失败: {e}")))?;

    let model_pricing_id = pricing_result.last_insert_id;

    // 2. 解析并插入定价层级
    let pricing_tiers = parse_pricing_tiers(&model.price_info);
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "parse_pricing_tiers",
        &format!(
            "🎯 为模型 {} 解析出 {} 个定价层级",
            model.name,
            pricing_tiers.len()
        )
    );

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
            .map_err(|e| ProxyError::database(format!("插入定价层级失败: {e}")))?;
    }

    Ok(())
}

/// 事务版本：插入单个模型及其定价数据
async fn insert_model_with_pricing_txn(
    txn: &DatabaseTransaction,
    model: &FilteredModel,
    provider_id: i32,
) -> Result<(), ProxyError> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Database,
        "insert_model_pricing",
        &format!(
            "💰 插入模型定价: {} (provider_id: {})",
            model.name, provider_id
        )
    );

    let pricing_model = model_pricing::ActiveModel {
        provider_type_id: Set(provider_id),
        model_name: Set(model.name.clone()),
        description: Set(Some(model.description.clone())),
        cost_currency: Set("USD".to_string()),
        ..Default::default()
    };

    let pricing_result = model_pricing::Entity::insert(pricing_model)
        .exec(txn)
        .await
        .map_err(|e| ProxyError::database(format!("插入模型定价记录失败: {e}")))?;

    let model_pricing_id = pricing_result.last_insert_id;

    let pricing_tiers = parse_pricing_tiers(&model.price_info);
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
            .exec(txn)
            .await
            .map_err(|e| ProxyError::database(format!("插入定价层级失败: {e}")))?;
    }

    Ok(())
}

/// `从ModelPriceInfo解析出定价层级`
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
