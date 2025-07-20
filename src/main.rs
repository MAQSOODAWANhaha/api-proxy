//! # AI Proxy System
//!
//! Enterprise-grade AI service proxy platform built with Rust and Pingora.
//!
//! This is the main entry point for the AI proxy system, which provides
//! unified access to multiple AI service providers with load balancing,
//! monitoring, and security features.

/// Main entry point for the AI proxy system.
///
/// Currently a placeholder implementation - will be replaced with
/// the full Pingora-based proxy service in Phase 2.
#[tokio::main]
async fn main() -> api_proxy::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    println!("AI Proxy System v0.1.0");
    println!("Starting development server...");
    
    // 初始化配置管理器
    let config_manager = match api_proxy::config::ConfigManager::new().await {
        Ok(manager) => {
            println!("✅ 配置管理器初始化成功");
            manager
        }
        Err(e) => {
            eprintln!("❌ 配置管理器初始化失败: {e}");
            return Err(e);
        }
    };
    
    // 获取当前配置
    let config = config_manager.get_config().await;
    println!("✅ 配置加载成功:");
    println!("  服务器地址: {}:{}", config.server.host, config.server.port);
    println!("  HTTPS端口: {}", config.server.https_port);
    println!("  工作线程: {}", config.server.workers);
    println!("  数据库URL: {}", config.database.url);
    
    // 初始化数据库
    let db = match api_proxy::database::init_database(&config.database.url).await {
        Ok(db) => {
            println!("✅ 数据库连接成功");
            db
        }
        Err(e) => {
            eprintln!("❌ 数据库连接失败: {e}");
            return Err(api_proxy::error::ProxyError::database_with_source(
                "数据库连接失败",
                e
            ));
        }
    };
    
    // 运行数据库迁移
    if let Err(e) = api_proxy::database::run_migrations(&db).await {
        eprintln!("❌ 数据库迁移失败: {e}");
        return Err(api_proxy::error::ProxyError::database_with_source(
            "数据库迁移失败",
            e
        ));
    } else {
        println!("✅ 数据库迁移完成");
    }
    
    // 检查数据库状态
    if let Err(e) = api_proxy::database::check_database_status(&db).await {
        eprintln!("⚠️ 数据库状态检查失败: {e}");
    }
    
    // 订阅配置变更事件（如果支持热重载）
    if let Some(mut event_receiver) = config_manager.subscribe_changes() {
        println!("✅ 配置热重载已启用");
        
        // 启动配置变更监听任务
        tokio::spawn(async move {
            while let Ok(event) = event_receiver.recv().await {
                match event {
                    api_proxy::config::ConfigEvent::Reloaded(_) => {
                        println!("🔄 配置已重新加载");
                    }
                    api_proxy::config::ConfigEvent::ReloadFailed(error) => {
                        eprintln!("❌ 配置重载失败: {}", error);
                    }
                    api_proxy::config::ConfigEvent::FileDeleted => {
                        eprintln!("⚠️ 配置文件被删除");
                    }
                }
            }
        });
        
        // 保持程序运行以测试热重载功能
        println!("🔄 程序正在运行中，可以修改配置文件测试热重载功能...");
        println!("按 Ctrl+C 退出");
        
        // 等待中断信号
        tokio::signal::ctrl_c().await.map_err(|e| {
            api_proxy::error::ProxyError::internal_with_source("等待中断信号失败", e)
        })?;
        
        println!("\n👋 程序正在退出...");
    } else {
        println!("ℹ️ 配置热重载已禁用");
    }
    
    Ok(())
}
