//! # 日志配置模块
//!
//! 提供自定义的日志格式化和配置功能，特别针对数据库查询日志的优化显示

use std::env;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, fmt, EnvFilter, Layer};

/// 优化的数据库查询日志格式化器
pub struct DbQueryFormatter;

impl DbQueryFormatter {
    /// 格式化SQLx查询日志
    pub fn format_sqlx_query(statement: &str, summary: &str, elapsed: f64, rows_affected: Option<u64>, rows_returned: Option<u64>) -> String {
        // 清理和格式化SQL语句
        let clean_sql = Self::clean_sql_statement(statement);
        
        // 根据操作类型选择图标
        let operation_icon = Self::get_operation_icon(&clean_sql);
        
        // 格式化执行时间
        let time_str = if elapsed >= 1.0 {
            format!("{:.2}s", elapsed / 1000.0)
        } else if elapsed >= 0.1 {
            format!("{:.1}ms", elapsed)
        } else {
            format!("{:.2}ms", elapsed)
        };

        // 构建结果信息
        let mut result_parts = Vec::new();
        if let Some(affected) = rows_affected {
            if affected > 0 {
                result_parts.push(format!("{}行受影响", affected));
            }
        }
        if let Some(returned) = rows_returned {
            if returned > 0 {
                result_parts.push(format!("{}行返回", returned));
            }
        }
        let result_str = if result_parts.is_empty() {
            String::new()
        } else {
            format!(" → {}", result_parts.join(", "))
        };

        format!(
            "{} {} (⏱ {}){}", 
            operation_icon, 
            clean_sql, 
            time_str, 
            result_str
        )
    }

    /// 清理SQL语句，移除多余的空白和换行
    fn clean_sql_statement(statement: &str) -> String {
        statement
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .collect::<String>()
            .replace("  ", " ")  // 移除多余空格
    }

    /// 根据SQL操作类型获取对应图标
    fn get_operation_icon(sql: &str) -> &'static str {
        let sql_upper = sql.to_uppercase();
        if sql_upper.starts_with("SELECT") {
            "🔍"
        } else if sql_upper.starts_with("INSERT") {
            "➕"
        } else if sql_upper.starts_with("UPDATE") {
            "✏️"
        } else if sql_upper.starts_with("DELETE") {
            "🗑️"
        } else if sql_upper.starts_with("CREATE") {
            "🔨"
        } else if sql_upper.starts_with("DROP") {
            "💥"
        } else if sql_upper.starts_with("ALTER") {
            "🔧"
        } else {
            "📋"
        }
    }
}

/// 自定义日志层，专门处理数据库查询日志
pub struct DatabaseLogLayer;

impl DatabaseLogLayer {
    pub fn new() -> impl Layer<tracing_subscriber::Registry> {
        fmt::layer()
            .with_target(true)
            .with_level(true)
            .with_thread_ids(false)
            .with_thread_names(false)
            .with_file(false)
            .with_line_number(false)
            .compact()
            .with_filter(EnvFilter::new("sqlx::query=info,sea_orm::query=info"))
    }
}

/// 初始化优化的日志系统
pub fn init_optimized_logging(log_level: Option<&String>) {
    let level = log_level.map_or("info", std::string::String::as_str);
    
    // 默认配置：减少数据库查询的冗长日志
    let default_filter = format!(
        "{},api_proxy=debug,sqlx::query=warn,sea_orm::query=warn,sqlx::query::summary=info", 
        level
    );
    
    let log_filter = env::var("RUST_LOG").unwrap_or(default_filter);

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_filter.into()),
        )
        .with(
            fmt::layer()
                .with_target(true)
                .with_level(true)
                .with_thread_ids(false)
                .with_thread_names(false)
                .with_file(false)
                .with_line_number(false)
                .compact()
        )
        .init();

    // 启动提示
    if env::var("RUST_LOG").map_or(false, |v| {
        v.contains("sqlx::query=info") || v.contains("sea_orm::query=info")
    }) {
        tracing::info!("🔍 Enhanced database query logging enabled");
    } else {
        tracing::info!("📋 Database query logging minimized (set RUST_LOG=sqlx::query=info to enable)");
    }
}

/// 环境变量设置指南
pub fn print_logging_help() {
    println!("📋 日志配置指南:");
    println!("  RUST_LOG=info                    # 标准日志级别");
    println!("  RUST_LOG=debug                   # 调试级别");
    println!("  RUST_LOG=sqlx::query=info        # 启用数据库查询日志");
    println!("  RUST_LOG=sea_orm::query=info     # 启用Sea-ORM查询日志");
    println!("  RUST_LOG=api_proxy=trace         # 应用详细追踪");
    println!();
    println!("💡 组合示例:");
    println!("  RUST_LOG=info,sqlx::query=info   # 标准日志+数据库查询");
    println!("  RUST_LOG=debug,sqlx::query=warn  # 调试模式但减少数据库日志");
}