//! # AI Proxy 主程序
//!
//! 企业级 AI 服务代理平台 - 基于 Pingora 的高性能代理服务

use api_proxy::{ProxyError, Result, config::ConfigManager, dual_port_setup, logging};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志系统
    logging::init_optimized_logging(None);

    // 初始化管理端系统启动时间（用于 /api/system/metrics uptime）
    // 确保在进程启动时即记录，而非在首次 API 调用时懒初始化
    api_proxy::management::handlers::system::init_start_time();

    // 执行数据初始化（数据库迁移等）
    run_data_initialization()
        .await
        .map_err(|e| ProxyError::Database {
            message: format!("数据初始化失败: {}", e),
            source: Some(e),
        })?;

    // 启动服务
    info!(component = "main", "服务启动");
    if let Err(e) = dual_port_setup::run_dual_port_servers().await {
        error!(component = "main", "服务启动失败: {:?}", e);
        std::process::exit(1);
    }

    info!(component = "main", "服务正常关闭");
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
