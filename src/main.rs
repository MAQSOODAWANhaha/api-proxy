//! # AI Proxy 主程序
//!
//! 企业级 AI 服务代理平台 - 基于 Pingora 的高性能代理服务

use std::env;
use std::process;
use tracing::{error, info};
use api_proxy::{
    config::ConfigManager,
    proxy::PingoraProxyServer,
    error::Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志系统
    init_logging();

    info!("Starting AI Proxy Service v{}", env!("CARGO_PKG_VERSION"));

    // 处理命令行参数
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--version" | "-v" => {
                println!("AI Proxy v{}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            _ => {
                eprintln!("Unknown argument: {}", args[1]);
                print_help();
                process::exit(1);
            }
        }
    }

    // 启动服务器
    if let Err(e) = run_server().await {
        error!("Failed to start server: {}", e);
        process::exit(1);
    }

    Ok(())
}

/// 初始化日志系统
fn init_logging() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let log_level = env::var("RUST_LOG")
        .unwrap_or_else(|_| "info,api_proxy=debug".to_string());

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_level.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// 运行服务器
async fn run_server() -> Result<()> {
    // 加载配置
    info!("Loading configuration...");
    let config_manager = ConfigManager::new().await?;
    let config = config_manager.get_config().await;

    info!("Configuration loaded successfully");
    info!("Server will listen on {}:{}", config.server.host, config.server.port);

    // 初始化数据库连接
    info!("Initializing database connection...");
    let db = api_proxy::database::init_database(&config.database.url).await?;
    
    // 运行数据库迁移
    info!("Running database migrations...");
    api_proxy::database::run_migrations(&db).await?;
    info!("Database migrations completed");

    // 创建并启动代理服务器
    let proxy_server = PingoraProxyServer::new(config);
    
    // 设置信号处理
    setup_signal_handlers();

    // 启动服务器
    info!("Starting Pingora proxy server...");
    proxy_server.start().await?;

    Ok(())
}

/// 设置信号处理器
fn setup_signal_handlers() {
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                info!("Received Ctrl+C, shutting down gracefully...");
            }
            Err(e) => {
                error!("Failed to listen for shutdown signal: {}", e);
            }
        }

        // 这里可以添加优雅关闭逻辑
        info!("Graceful shutdown completed");
        process::exit(0);
    });
}

/// 打印帮助信息
fn print_help() {
    println!("AI Proxy - Enterprise-grade AI service proxy platform");
    println!();
    println!("USAGE:");
    println!("    {} [OPTIONS]", env!("CARGO_PKG_NAME"));
    println!();
    println!("OPTIONS:");
    println!("    -h, --help       Print this help message");
    println!("    -v, --version    Print version information");
    println!();
    println!("ENVIRONMENT VARIABLES:");
    println!("    RUST_LOG         Set logging level (default: info,api_proxy=debug)");
    println!("    CONFIG_FILE      Configuration file path (default: config/config.toml)");
    println!();
    println!("EXAMPLES:");
    println!("    {}                    # Start with default configuration", env!("CARGO_PKG_NAME"));
    println!("    RUST_LOG=debug {}     # Start with debug logging", env!("CARGO_PKG_NAME"));
}
