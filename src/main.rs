//! # AI Proxy 主程序
//!
//! 企业级 AI 服务代理平台 - 基于 Pingora 的高性能代理服务

use api_proxy::{config::ConfigManager, dual_port_setup};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志系统
    api_proxy::logging::init_optimized_logging(None);

    info!(
        version = env!("CARGO_PKG_VERSION"),
        flow = "service_boot",
        "启动 AI Proxy 服务"
    );

    // 执行数据初始化（数据库迁移等）
    run_data_initialization()
        .await
        .map_err(anyhow::Error::from)?;

    // 启动双端口分离架构服务器
    dual_port_setup::run_dual_port_servers()
        .await
        .map_err(anyhow::Error::from)?;

    Ok(())
}

/// 数据初始化函数
async fn run_data_initialization() -> anyhow::Result<()> {
    info!("🚀 开始数据初始化过程...");

    // 获取配置并初始化数据库连接
    let config_manager = ConfigManager::new()
        .await
        .map_err(|e| anyhow::anyhow!("配置管理器初始化失败: {}", e))?;
    let config = config_manager.get_config().await;

    let db = api_proxy::database::init_database(&config.database.url)
        .await
        .map_err(|e| anyhow::anyhow!("数据库连接失败: {}", e))?;

    // 首先运行数据库迁移，确保表结构存在
    info!("📋 执行数据库迁移...");
    api_proxy::database::run_migrations(&db)
        .await
        .map_err(|e| anyhow::anyhow!("数据库迁移失败: {}", e))?;

    // 检查数据完整性并按需初始化
    info!("🔍 检查数据完整性并按需初始化...");
    api_proxy::database::ensure_model_pricing_data(&db)
        .await
        .map_err(|e| anyhow::anyhow!("数据完整性检查失败: {}", e))?;

    info!("✅ 数据初始化过程完成");
    Ok(())
}
