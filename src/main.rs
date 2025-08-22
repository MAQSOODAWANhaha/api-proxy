//! # AI Proxy 主程序
//!
//! 企业级 AI 服务代理平台 - 基于 Pingora 的高性能代理服务

use api_proxy::{config::ConfigManager, dual_port_setup};
use clap::{Arg, ArgMatches, Command};
use std::env;
use std::process;
use std::result::Result as StdResult;
use tracing::{error, info};

fn main() -> StdResult<(), Box<dyn std::error::Error>> {
    let matches = build_cli().get_matches();

    // 初始化日志系统
    let log_level = matches.get_one::<String>("log_level");
    init_logging_with_level(log_level);

    info!("Starting AI Proxy Service v{}", env!("CARGO_PKG_VERSION"));

    // 处理配置检查命令
    if matches.get_flag("check") {
        return run_config_check(&matches);
    }

    // 启动双端口分离架构服务器
    if let Err(e) = dual_port_setup::run_dual_port_servers(&matches) {
        error!("Failed to start servers: {}", e);
        process::exit(1);
    }

    Ok(())
}

/// 构建CLI命令定义
fn build_cli() -> Command {
    Command::new("api-proxy")
        .version(env!("CARGO_PKG_VERSION"))
        .author("AI Proxy Team")
        .about("Enterprise-grade AI service proxy platform")
        .long_about("A high-performance AI service proxy platform built with Rust and Pingora.\nSupports multiple AI providers with load balancing, authentication, and monitoring.")
        .arg(Arg::new("config")
            .short('c')
            .long("config")
            .help("Configuration file path")
            .value_name("FILE")
            .default_value("config/config.toml"))
        .arg(Arg::new("log_level")
            .short('l')
            .long("log-level")
            .help("Set logging level")
            .value_name("LEVEL")
            .value_parser(["error", "warn", "info", "debug", "trace"])
            .default_value("info"))
        .arg(Arg::new("port")
            .short('p')
            .long("port")
            .help("Override proxy server port")
            .value_name("PORT")
            .value_parser(clap::value_parser!(u16)))
        .arg(Arg::new("host")
            .long("host")
            .help("Override proxy server host")
            .value_name("HOST")
            .default_value("127.0.0.1"))
        .arg(Arg::new("https_port")
            .long("https-port")
            .help("Override HTTPS port")
            .value_name("PORT")
            .value_parser(clap::value_parser!(u16)))
        .arg(Arg::new("database_url")
            .short('d')
            .long("database-url")
            .help("Override database URL")
            .value_name("URL"))
        .arg(Arg::new("workers")
            .short('w')
            .long("workers")
            .help("Number of worker threads")
            .value_name("COUNT")
            .value_parser(clap::value_parser!(u16)))
        .arg(Arg::new("check")
            .long("check")
            .help("Check configuration and exit")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("daemon")
            .long("daemon")
            .help("Run as daemon (background process)")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("enable_trace")
            .long("enable-trace")
            .help("Enable request tracing system")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("disable_trace")
            .long("disable-trace")
            .help("Disable request tracing system")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("trace_level")
            .long("trace-level")
            .help("Set tracing level (0=basic, 1=detailed, 2=full)")
            .value_name("LEVEL")
            .value_parser(clap::value_parser!(i32)))
        .arg(Arg::new("trace_sampling_rate")
            .long("trace-sampling-rate")
            .help("Set tracing sampling rate (0.0-1.0)")
            .value_name("RATE")
            .value_parser(clap::value_parser!(f64)))
}

/// 带日志级别的初始化函数
fn init_logging_with_level(log_level: Option<&String>) {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let level = log_level.map_or("info", std::string::String::as_str);
    let log_filter = env::var("RUST_LOG").unwrap_or_else(|_| format!("{level},api_proxy=debug"));

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_filter.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// 配置检查函数
fn run_config_check(matches: &ArgMatches) -> StdResult<(), Box<dyn std::error::Error>> {
    info!("Checking configuration...");

    let config_path = matches.get_one::<String>("config").unwrap();
    info!("Using configuration file: {}", config_path);

    // 创建Tokio运行时进行异步操作
    let rt = tokio::runtime::Runtime::new().map_err(|e| {
        api_proxy::error::ProxyError::server_init(format!("Failed to create Tokio runtime: {e}"))
    })?;

    rt.block_on(async {
        // 验证配置文件
        let config_manager = ConfigManager::new().await?;
        let config = config_manager.get_config().await;
        info!("✓ Configuration file is valid");
        info!(
            "  Server: {}:{}",
            config.server.as_ref().map_or("0.0.0.0", |s| &s.host),
            config.server.as_ref().map_or(8080, |s| s.port)
        );
        if let Some(server) = &config.server {
            if server.https_port > 0 {
                info!("  HTTPS: {}:{}", server.host, server.https_port);
            }
        }
        info!("  Database: {}", config.database.url);
        info!("  Redis: {}", config.redis.url);
        info!(
            "  Workers: {}",
            config.server.as_ref().map_or(1, |s| s.workers)
        );

        // 测试数据库连接
        info!("Testing database connection...");
        let _db = api_proxy::database::init_database(&config.database.url).await?;
        info!("✓ Database connection successful");

        info!("✓ All configuration checks passed");
        Ok::<_, api_proxy::error::ProxyError>(())
    })?;

    Ok(())
}
