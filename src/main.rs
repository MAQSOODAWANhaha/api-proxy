//! # AI Proxy 主程序
//!
//! 企业级 AI 服务代理平台 - 基于 Pingora 的高性能代理服务

use std::env;
use std::process;
use tracing::{error, info};
use clap::{Arg, Command, ArgMatches};
use api_proxy::{
    config::{ConfigManager, TraceConfig},
    error::Result,
    dual_port_setup,
};

fn main() -> Result<()> {
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

    let level = log_level.map(|s| s.as_str()).unwrap_or("info");
    let log_filter = env::var("RUST_LOG")
        .unwrap_or_else(|_| format!("{},api_proxy=debug", level));

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_filter.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// 配置检查函数
fn run_config_check(matches: &ArgMatches) -> Result<()> {
    info!("Checking configuration...");
    
    let config_path = matches.get_one::<String>("config").unwrap();
    info!("Using configuration file: {}", config_path);
    
    // 创建Tokio运行时进行异步操作
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| api_proxy::error::ProxyError::server_init(format!("Failed to create Tokio runtime: {}", e)))?;

    rt.block_on(async {
        // 验证配置文件
        let config_manager = ConfigManager::new().await?;
        let mut config = config_manager.get_config().await;
        
        // 应用CLI参数覆盖以显示正确的配置信息
        apply_trace_overrides(&mut config, matches);
        
        info!("✓ Configuration file is valid");
        info!("  Server: {}:{}", config.server.as_ref().map_or("0.0.0.0", |s| &s.host), config.server.as_ref().map_or(8080, |s| s.port));
        if let Some(server) = &config.server {
            if server.https_port > 0 {
                info!("  HTTPS: {}:{}", server.host, server.https_port);
            }
        }
        info!("  Database: {}", config.database.url);
        info!("  Redis: {}", config.redis.url);
        info!("  Workers: {}", config.server.as_ref().map_or(1, |s| s.workers));
        
        // 显示追踪配置信息
        if let Some(trace_config) = &config.trace {
            info!("  Tracing: {} (level={})", 
                  if trace_config.enabled { "enabled" } else { "disabled" },
                  trace_config.default_trace_level);
            info!("  Trace Sampling: {}", trace_config.sampling_rate);
            info!("  Trace Batch Size: {}", trace_config.max_batch_size);
            info!("  Trace Flush Interval: {}s", trace_config.flush_interval);
            if trace_config.enable_phases {
                info!("  Trace Features: phases=✓ health={}  performance={}", 
                      if trace_config.enable_health_metrics { "✓" } else { "✗" },
                      if trace_config.enable_performance_metrics { "✓" } else { "✗" });
            }
        } else {
            info!("  Tracing: not configured");
        }
        
        // 测试数据库连接
        info!("Testing database connection...");
        let _db = api_proxy::database::init_database(&config.database.url).await?;
        info!("✓ Database connection successful");
        
        info!("✓ All configuration checks passed");
        Ok::<_, api_proxy::error::ProxyError>(())
    })?;

    Ok(())
}

/// 应用追踪相关的命令行参数覆盖
fn apply_trace_overrides(config: &mut api_proxy::config::AppConfig, matches: &ArgMatches) {
    let mut trace_modified = false;
    
    // 确保有追踪配置
    if config.trace.is_none() {
        config.trace = Some(TraceConfig::default());
    }
    
    let trace_config = config.trace.as_mut().unwrap();
    
    // 处理启用/禁用追踪
    if matches.get_flag("enable_trace") {
        info!("🔧 Enabling tracing system from CLI");
        trace_config.enabled = true;
        trace_modified = true;
    }
    
    if matches.get_flag("disable_trace") {
        info!("🔧 Disabling tracing system from CLI");
        trace_config.enabled = false;
        trace_modified = true;
    }
    
    // 处理追踪级别
    if let Some(level) = matches.get_one::<i32>("trace_level") {
        if *level >= 0 && *level <= 2 {
            info!("🔧 Overriding trace level from CLI: {}", level);
            trace_config.default_trace_level = *level;
            trace_modified = true;
        } else {
            error!("❌ Invalid trace level: {}. Must be 0-2", level);
            process::exit(1);
        }
    }
    
    // 处理采样率
    if let Some(rate) = matches.get_one::<f64>("trace_sampling_rate") {
        if *rate >= 0.0 && *rate <= 1.0 {
            info!("🔧 Overriding trace sampling rate from CLI: {}", rate);
            trace_config.sampling_rate = *rate;
            trace_modified = true;
        } else {
            error!("❌ Invalid sampling rate: {}. Must be 0.0-1.0", rate);
            process::exit(1);
        }
    }
    
    if trace_modified {
        info!("✅ Trace configuration updated from CLI arguments");
    }
}