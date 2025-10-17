//! # 统一日志工具模块
//!
//! 提供完整的日志工具链：
//! - 业务日志格式化（proxy模块专用）
//! - 数据库查询日志格式化
//! - 日志系统初始化和配置

use crate::proxy::ProxyContext;
use flate2::read::GzDecoder;
use pingora_core::{Error, ErrorType};
use pingora_http::ResponseHeader;
use pingora_proxy::Session;
use serde_json;
use std::collections::BTreeMap;
use std::env;
use std::io::Read;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

// ================ Proxy 模块业务日志工具 ================

/// 日志阶段枚举
#[derive(Debug, Clone, Copy)]
pub enum LogStage {
    RequestStart,
    Authentication,
    RequestModify,
    UpstreamRequest,
    Response,
    ResponseFailure,
    Error,
    // New stages for non-request contexts
    Startup,
    Shutdown,
    Configuration,
    HealthCheck,
    BackgroundTask,
    Scheduling,
    Cache,
    ExternalApi,
    Internal,
    Db,
    Codec,
}

impl LogStage {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::RequestStart => "request_start",
            Self::Authentication => "authentication",
            Self::RequestModify => "request_modify",
            Self::UpstreamRequest => "upstream_request",
            Self::Response => "response",
            Self::ResponseFailure => "response_failure",
            Self::Error => "error",
            Self::Startup => "startup",
            Self::Shutdown => "shutdown",
            Self::Configuration => "configuration",
            Self::HealthCheck => "health_check",
            Self::BackgroundTask => "background_task",
            Self::Scheduling => "scheduling",
            Self::Cache => "cache",
            Self::ExternalApi => "external_api",
            Self::Internal => "internal",
            Self::Db => "db",
            Self::Codec => "codec",
        }
    }
}

/// 组件枚举
#[derive(Debug, Clone, Copy)]
pub enum LogComponent {
    // --- System Components ---
    Main,
    ServerSetup,
    Config,
    Database,
    Cache,
    // --- Proxy Core Components ---
    Proxy,
    Builder,
    // --- Proxy Services ---
    Auth,
    ApiKey,
    OAuth,
    Upstream,
    RequestTransform,
    ResponseTransform,
    Statistics,
    Tracing,
    TracingService,
    // --- Business Logic Components ---
    KeyPool,
    HealthChecker,
    SmartApiKeyProvider,
    // --- External Clients ---
    GeminiClient,
    // --- Provider Strategies ---
    GeminiStrategy,
    OpenAIStrategy,
    Sse,
    ClaudeStrategy,
}

impl LogComponent {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::ServerSetup => "server_setup",
            Self::Config => "config",
            Self::Database => "database",
            Self::Cache => "cache",
            Self::Proxy => "proxy",
            Self::Builder => "builder",
            Self::Auth => "auth",
            Self::ApiKey => "api_key",
            Self::OAuth => "oauth",
            Self::Upstream => "upstream",
            Self::RequestTransform => "request_transform",
            Self::ResponseTransform => "response_transform",
            Self::Statistics => "statistics",
            Self::Tracing => "tracing",
            Self::TracingService => "tracing_service",
            Self::KeyPool => "key_pool",
            Self::HealthChecker => "health_checker",
            Self::SmartApiKeyProvider => "smart_api_key_provider",
            Self::GeminiClient => "gemini_client",
            Self::GeminiStrategy => "gemini_strategy",
            Self::OpenAIStrategy => "openai_strategy",
            Self::Sse => "sse",
            Self::ClaudeStrategy => "claude_strategy",
        }
    }
}

/// 标准日志宏 - 信息级别
#[macro_export]
macro_rules! linfo {
    ($request_id:expr, $stage:expr, $component:expr, $operation:expr, $description:expr, $($rest:tt)*) => {
        tracing::info!(
            request_id = %$request_id,
            stage = $stage.as_str(),
            operation = $operation,
            component = $component.as_str(),
            message = %$description,
            $($rest)*
        )
    };
    ($request_id:expr, $stage:expr, $component:expr, $operation:expr, $description:expr) => {
        tracing::info!(
            request_id = %$request_id,
            stage = $stage.as_str(),
            operation = $operation,
            component = $component.as_str(),
            message = %$description,
        )
    };
}

/// 标准日志宏 - 调试级别
#[macro_export]
macro_rules! ldebug {
    ($request_id:expr, $stage:expr, $component:expr, $operation:expr, $description:expr, $($rest:tt)*) => {
        tracing::debug!(
            request_id = %$request_id,
            stage = $stage.as_str(),
            operation = $operation,
            component = $component.as_str(),
            message = %$description,
            $($rest)*
        )
    };
    ($request_id:expr, $stage:expr, $component:expr, $operation:expr, $description:expr) => {
        tracing::debug!(
            request_id = %$request_id,
            stage = $stage.as_str(),
            operation = $operation,
            component = $component.as_str(),
            message = %$description,
        )
    };
}

/// 标准日志宏 - 警告级别
#[macro_export]
macro_rules! lwarn {
    ($request_id:expr, $stage:expr, $component:expr, $operation:expr, $description:expr, $($rest:tt)*) => {
        tracing::warn!(
            request_id = %$request_id,
            stage = $stage.as_str(),
            operation = $operation,
            component = $component.as_str(),
            message = %$description,
            $($rest)*
        )
    };
    ($request_id:expr, $stage:expr, $component:expr, $operation:expr, $description:expr) => {
        tracing::warn!(
            request_id = %$request_id,
            stage = $stage.as_str(),
            operation = $operation,
            component = $component.as_str(),
            message = %$description,
        )
    };
}

/// 标准日志宏 - 错误级别
#[macro_export]
macro_rules! lerror {
    ($request_id:expr, $stage:expr, $component:expr, $operation:expr, $description:expr, $($rest:tt)*) => {
        tracing::error!(
            request_id = %$request_id,
            stage = $stage.as_str(),
            operation = $operation,
            component = $component.as_str(),
            message = %$description,
            $($rest)*
        )
    };
    ($request_id:expr, $stage:expr, $component:expr, $operation:expr, $description:expr) => {
        tracing::error!(
            request_id = %$request_id,
            stage = $stage.as_str(),
            operation = $operation,
            component = $component.as_str(),
            message = %$description,
        )
    };
}

/// 格式化请求头为人类可读的字符串（带脱敏处理）
pub fn format_request_headers(headers: &pingora_http::RequestHeader) -> String {
    let mut formatted = Vec::new();
    for (name, value) in &headers.headers {
        let name_str = name.as_str();
        let value_str = std::str::from_utf8(value.as_bytes()).unwrap_or("<binary>");

        let masked = match name_str.to_ascii_lowercase().as_str() {
            "authorization"
            | "proxy-authorization"
            | "x-api-key"
            | "api-key"
            | "x-goog-api-key"
            | "set-cookie"
            | "cookie" => {
                // 只保留前后少量字符，避免日志泄露敏感信息
                if value_str.len() > 16 {
                    format!(
                        "{}: {}...{}",
                        name_str,
                        &value_str[..8],
                        &value_str[value_str.len().saturating_sub(4)..]
                    )
                } else {
                    format!("{name_str}: ****")
                }
            }
            _ => format!("{name_str}: {value_str}"),
        };
        formatted.push(masked);
    }
    formatted.join(
        "
  ",
    )
}

/// 将请求头转为 JSON 映射（键小写，按字母序）
/// 注意：按当前仓库约定，此函数不做脱敏。
pub fn headers_json_map_request(headers: &pingora_http::RequestHeader) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (name, value) in &headers.headers {
        let key = name.as_str().to_ascii_lowercase();
        let value_str = std::str::from_utf8(value.as_bytes()).unwrap_or("<binary>");
        map.insert(key, value_str.to_string());
    }
    map
}

/// 将响应头转为 JSON 映射（键小写，按字母序）
/// 注意：按当前仓库约定，此函数不做脱敏。
#[must_use]
pub fn headers_json_map_response(
    headers: &pingora_http::ResponseHeader,
) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (name, value) in &headers.headers {
        let key = name.as_str().to_ascii_lowercase();
        let value_str = std::str::from_utf8(value.as_bytes()).unwrap_or("<binary>");
        map.insert(key, value_str.to_string());
    }
    map
}

/// 将请求头直接序列化为 JSON 字符串（稳定字段顺序）
pub fn headers_json_string_request(headers: &pingora_http::RequestHeader) -> String {
    serde_json::to_string(&headers_json_map_request(headers)).unwrap_or_else(|_| "{}".to_string())
}

/// 将响应头直接序列化为 JSON 字符串（稳定字段顺序）
#[must_use]
pub fn headers_json_string_response(headers: &pingora_http::ResponseHeader) -> String {
    serde_json::to_string(&headers_json_map_response(headers)).unwrap_or_else(|_| "{}".to_string())
}

/// 脱敏API密钥
#[must_use]
pub fn sanitize_api_key(api_key: &str) -> String {
    if api_key.len() > 8 {
        format!(
            "{}...{}",
            &api_key[..4],
            &api_key[api_key.len().saturating_sub(4)..]
        )
    } else if !api_key.is_empty() {
        "***".to_string()
    } else {
        "<empty>".to_string()
    }
}

/// 构建详细信息的字符串
#[must_use]
pub fn build_details_string(details: &[(&str, String)]) -> String {
    details
        .iter()
        .map(|(key, value)| format!("  {key}: {value}"))
        .collect::<Vec<_>>()
        .join(
            "
",
        )
}

/// 构建请求信息的详细信息
#[must_use]
pub fn build_request_details(method: &str, url: &str, headers: &str) -> String {
    let details = vec![
        ("方法", method.to_string()),
        ("URL", url.to_string()),
        ("请求头", headers.to_string()),
    ];
    build_details_string(&details)
}

/// 构建响应信息的详细信息
#[must_use]
pub fn build_response_details(status_code: u16, headers: &str, duration_ms: u64) -> String {
    let details = vec![
        ("状态码", status_code.to_string()),
        ("响应头", headers.to_string()),
        ("处理时间", format!("{duration_ms}ms")),
    ];
    build_details_string(&details)
}

/// 构建错误信息的详细信息
#[must_use]
pub fn build_error_details(error_message: &str, error_type: &str, context: &str) -> String {
    let details = vec![
        ("错误类型", error_type.to_string()),
        ("错误消息", error_message.to_string()),
        ("错误上下文", context.to_string()),
    ];
    build_details_string(&details)
}

// ================ 数据库查询日志工具 ================

/// 优化的数据库查询日志格式化器
pub struct DbQueryFormatter;

impl DbQueryFormatter {
    /// `格式化SQLx查询日志`
    #[must_use]
    pub fn format_sqlx_query(
        statement: &str,
        _summary: &str,
        elapsed: f64,
        rows_affected: Option<u64>,
        rows_returned: Option<u64>,
    ) -> String {
        // 清理和格式化SQL语句
        let clean_sql = Self::clean_sql_statement(statement);

        // 根据操作类型选择图标
        let operation_icon = Self::get_operation_icon(&clean_sql);

        // 格式化执行时间
        let time_str = if elapsed >= 1.0 {
            format!("{:.2}s", elapsed / 1000.0)
        } else if elapsed >= 0.1 {
            format!("{elapsed:.1}ms")
        } else {
            format!("{elapsed:.2}ms")
        };

        // 构建结果信息
        let mut result_parts = Vec::new();
        if let Some(affected) = rows_affected
            && affected > 0
        {
            result_parts.push(format!("{affected}行受影响"));
        }
        if let Some(returned) = rows_returned
            && returned > 0
        {
            result_parts.push(format!("{returned}行返回"));
        }
        let result_str = if result_parts.is_empty() {
            String::new()
        } else {
            format!(" → {}", result_parts.join(", "))
        };

        format!("{operation_icon} {clean_sql} (⏱ {time_str}){result_str}")
    }

    /// 清理SQL语句，移除多余的空白和换行
    fn clean_sql_statement(statement: &str) -> String {
        statement
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .collect::<String>()
            .replace("  ", " ") // 移除多余空格
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

// ================ 日志系统初始化和配置 ================

/// 日志系统配置
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// 默认日志级别
    pub default_level: String,
    /// 应用程序日志级别
    pub app_level: String,
    /// 数据库查询日志级别
    pub db_query_level: String,
    /// Sea ORM 查询日志级别
    pub sea_orm_level: String,
    /// `SQLx` 通用日志级别
    pub sqlx_level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            default_level: "info".to_string(),
            app_level: "debug".to_string(),
            db_query_level: "off".to_string(),
            sea_orm_level: "warn".to_string(),
            sqlx_level: "warn".to_string(),
        }
    }
}

impl LoggingConfig {
    /// 创建生产环境配置
    #[must_use]
    pub fn production() -> Self {
        Self {
            default_level: "info".to_string(),
            app_level: "info".to_string(),
            db_query_level: "off".to_string(),
            sea_orm_level: "warn".to_string(),
            sqlx_level: "warn".to_string(),
        }
    }

    /// 创建开发环境配置
    #[must_use]
    pub fn development() -> Self {
        Self {
            default_level: "debug".to_string(),
            app_level: "trace".to_string(),
            db_query_level: "info".to_string(),
            sea_orm_level: "debug".to_string(),
            sqlx_level: "debug".to_string(),
        }
    }

    /// 创建测试环境配置
    #[must_use]
    pub fn testing() -> Self {
        Self {
            default_level: "warn".to_string(),
            app_level: "debug".to_string(),
            db_query_level: "off".to_string(),
            sea_orm_level: "off".to_string(),
            sqlx_level: "warn".to_string(),
        }
    }

    /// 构建日志过滤器字符串
    #[must_use]
    pub fn build_filter(&self) -> String {
        format!(
            "{},api_proxy={},sqlx::query={},sea_orm::query={},sqlx={}",
            self.default_level,
            self.app_level,
            self.db_query_level,
            self.sea_orm_level,
            self.sqlx_level
        )
    }

    /// 从环境变量创建配置
    ///
    /// 支持通过 `LOG_MODE` 环境变量选择预设模式：
    /// - "production": 生产环境（性能优先，关闭数据库查询日志）
    /// - "development": 开发环境（详细日志，启用数据库查询）
    /// - "testing": 测试环境（最小日志）
    /// - 未设置时默认使用 "production"
    ///
    /// Docker Compose 使用示例：
    /// ```yaml
    /// environment:
    ///   - LOG_MODE=production    # 生产模式
    ///   - LOG_MODE=development  # 开发模式
    ///   - LOG_MODE=testing      # 测试模式
    /// ```
    #[must_use]
    pub fn from_env() -> Self {
        match env::var("LOG_MODE").ok().as_deref() {
            Some("development") => Self::development(),
            Some("testing") => Self::testing(),
            _ => Self::production(), // 默认生产模式
        }
    }
}

/// 初始化优化的日志系统
pub fn init_optimized_logging(log_level: Option<&String>) {
    // 使用配置构建日志过滤器
    let config = LoggingConfig::from_env();

    // 如果有传入的日志级别，覆盖默认级别
    let final_config = if let Some(level) = log_level {
        let mut config = config;
        config.default_level.clone_from(level);
        config
    } else {
        config
    };

    // 构建过滤器字符串
    let filter_string = final_config.build_filter();

    // 从环境变量获取覆盖配置，如果没有则使用构建的配置
    let log_filter = env::var("RUST_LOG").unwrap_or(filter_string);

    // 创建多层级订阅者
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::try_new(&log_filter).unwrap_or_else(|_| EnvFilter::default())
    });

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .with_filter(tracing_subscriber::filter::FilterFn::new(|metadata| {
            // 过滤掉一些噪音日志
            !metadata.target().starts_with("h2::client")
                && !metadata.target().starts_with("hyper::")
                && !metadata.target().starts_with("tokio::runtime")
                && !metadata.target().starts_with("pingora::upstreams::peer")
        }));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    // 打印启动信息
    print_startup_info(&final_config, &log_filter);
}

/// 打印启动信息
fn print_startup_info(config: &LoggingConfig, actual_filter: &str) {
    let db_enabled = matches!(config.db_query_level.as_str(), "info" | "debug" | "trace");

    if db_enabled {
        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::Main,
            "log_init",
            &format!(
                "🔍 日志系统已启动 - 模式: 开发 | 数据库查询日志: 启用 | 过滤器: {actual_filter}"
            )
        );
    } else {
        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::Main,
            "log_init",
            &format!(
                "📋 日志系统已启动 - 模式: 生产 | 数据库查询日志: 禁用 | 过滤器: {actual_filter}"
            )
        );
    }

    // 打印配置信息（仅在调试级别）
    ldebug!(
        "system",
        LogStage::Startup,
        LogComponent::Main,
        "log_config",
        "日志配置详情",
        default_level = %config.default_level,
        app_level = %config.app_level,
        db_query_level = %config.db_query_level,
        sea_orm_level = %config.sea_orm_level,
        sqlx_level = %config.sqlx_level
    );
}

/// 环境变量设置指南
// ================ 日志格式验证机制 ================
/// 日志格式验证器
///
/// 提供日志格式的一致性检查和验证功能
pub struct LogFormatValidator;

impl LogFormatValidator {
    fn log_validation_error(message: &str) {
        lerror!(
            "system",
            LogStage::Internal,
            LogComponent::Config,
            "log_validation_fail",
            message
        );
    }

    fn ensure_non_empty(field: &str, value: &str) -> bool {
        if value.is_empty() {
            Self::log_validation_error(&format!("日志格式验证失败: {field} 不能为空"));
            return false;
        }
        true
    }

    fn ensure_valid_stage(stage: LogStage) -> bool {
        if matches!(
            stage,
            LogStage::RequestStart
                | LogStage::Authentication
                | LogStage::RequestModify
                | LogStage::UpstreamRequest
                | LogStage::Response
                | LogStage::ResponseFailure
                | LogStage::Error
        ) {
            true
        } else {
            Self::log_validation_error(&format!("日志格式验证失败: 非法阶段 {stage:?}"));
            false
        }
    }

    fn ensure_valid_component(component: LogComponent) -> bool {
        if matches!(
            component,
            LogComponent::Proxy
                | LogComponent::Auth
                | LogComponent::Tracing
                | LogComponent::Upstream
                | LogComponent::Builder
                | LogComponent::GeminiStrategy
                | LogComponent::Database
        ) {
            true
        } else {
            Self::log_validation_error(&format!("日志格式验证失败: 非法组件 {component:?}"));
            false
        }
    }

    /// 验证日志格式是否符合统一标准
    ///
    /// 检查点：
    /// 1. 是否包含必需的 `request_id` 字段
    /// 2. 是否包含 stage 字段
    /// 3. 是否包含 component 字段
    /// 4. 是否包含 operation 字段
    /// 5. 是否包含描述信息
    #[must_use]
    pub fn validate_log_format(
        request_id: &str,
        stage: LogStage,
        component: LogComponent,
        operation: &str,
        description: &str,
    ) -> bool {
        Self::ensure_non_empty("request_id", request_id)
            && Self::ensure_non_empty("operation", operation)
            && Self::ensure_non_empty("description", description)
            && Self::ensure_valid_stage(stage)
            && Self::ensure_valid_component(component)
    }

    /// 验证并记录日志（安全包装）
    ///
    /// 在记录日志前进行格式验证，确保日志格式的一致性
    pub fn validate_and_log_info(
        request_id: &str,
        stage: LogStage,
        component: LogComponent,
        operation: &str,
        description: &str,
        fields: &[(&str, String)],
    ) {
        if Self::validate_log_format(request_id, stage, component, operation, description) {
            // 使用标准 tracing 记录（验证通过）
            let field_str = fields
                .iter()
                .map(|(key, value)| format!("{key} = {value}"))
                .collect::<Vec<_>>()
                .join(", ");

            linfo!(
                request_id,
                stage,
                component,
                operation,
                &format!("=== {description} ===, {field_str}")
            );
        } else {
            lwarn!(
                "system",
                LogStage::Internal,
                LogComponent::Config,
                "log_validation_fail",
                &format!(
                    "日志格式验证失败，跳过记录: request_id={request_id}, operation={operation}"
                )
            );
        }
    }

    /// 获取日志格式统计信息
    ///
    /// 返回当前系统中各种日志格式的使用情况
    #[must_use]
    pub fn get_format_stats() -> String {
        "📊 日志格式统计:
  - 统一日志宏: proxy_info!, proxy_debug!, proxy_warn!, proxy_error!
  - 日志阶段: 7种 (RequestStart, Authentication, RequestModify, UpstreamRequest, Response, ResponseFailure, Error)
  - 组件类型: 8种 (Proxy, AuthService, RequestHandler, TracingService, Upstream, Builder, GeminiStrategy, Database)
  - 优化文件: 6个 (authentication_service.rs, request_handler.rs, tracing_service.rs, builder.rs, pingora_proxy.rs, provider_strategy_gemini.rs)".to_string()
    }

    /// 检查日志字段是否包含敏感信息
    ///
    /// 自动检测并警告潜在的敏感信息泄露
    #[must_use]
    pub fn check_sensitive_fields(fields: &[(&str, String)]) -> Vec<String> {
        let sensitive_keywords = vec![
            "password",
            "secret",
            "token",
            "key",
            "auth",
            "credential",
            "api_key",
            "authorization",
            "signature",
            "private",
        ];

        let mut warnings = Vec::new();

        for (key, value) in fields {
            let key_lower = key.to_lowercase();
            for keyword in &sensitive_keywords {
                if key_lower.contains(keyword) {
                    warnings.push(format!("潜在敏感字段: {} (值长度: {})", key, value.len()));
                }
            }
        }

        warnings
    }
}

// ================ Gemini Provider 日志工具 ================

/// 分析错误类型和分类
fn analyze_error_type(
    error: Option<&Error>,
    status_code: u16,
    _ctx: &ProxyContext,
) -> (String, String, String) {
    error.map_or_else(
        || {
            if status_code >= 400 {
                match status_code {
                    400..=499 => (
                        "client_error".to_string(),
                        "客户端错误".to_string(),
                        format!("HTTP客户端错误 {status_code}"),
                    ),
                    500..=599 => (
                        "server_error".to_string(),
                        "服务器错误".to_string(),
                        format!("HTTP服务器错误 {status_code}"),
                    ),
                    _ => (
                        "http_error".to_string(),
                        "HTTP错误".to_string(),
                        format!("HTTP错误 {status_code}"),
                    ),
                }
            } else {
                (
                    "unknown_failure".to_string(),
                    "未知失败".to_string(),
                    "请求失败但具体原因不明".to_string(),
                )
            }
        },
        |err| match err.etype {
            ErrorType::ConnectionClosed => (
                "connection_failure".to_string(),
                "连接关闭".to_string(),
                format!("连接关闭: {err}"),
            ),
            ErrorType::ConnectTimedout => (
                "connection_timeout".to_string(),
                "连接超时".to_string(),
                format!("连接上游服务器超时: {err}"),
            ),
            ErrorType::ReadTimedout => (
                "read_timeout".to_string(),
                "读取超时".to_string(),
                format!("读取响应数据超时: {err}"),
            ),
            ErrorType::WriteTimedout => (
                "write_timeout".to_string(),
                "写入超时".to_string(),
                format!("发送请求数据超时: {err}"),
            ),
            ErrorType::HTTPStatus(code) => {
                if code == 0 {
                    (
                        "connection_error".to_string(),
                        "连接错误".to_string(),
                        format!("连接中断，未收到HTTP响应: {err}"),
                    )
                } else {
                    (
                        "http_error".to_string(),
                        "HTTP错误响应".to_string(),
                        format!("上游返回HTTP错误 {code}: {err}"),
                    )
                }
            }
            ErrorType::CustomCode(_, code) => (
                "custom_error".to_string(),
                "自定义错误".to_string(),
                format!("自定义错误 {code}: {err}"),
            ),
            _ => (
                "unknown_error".to_string(),
                "未知错误".to_string(),
                format!("未知错误类型: {:?}", err.etype),
            ),
        },
    )
}

/// 记录详细的代理失败信息
#[allow(clippy::cognitive_complexity)]
pub fn log_proxy_failure_details(
    request_id: &str,
    status_code: u16,
    error: Option<&Error>,
    ctx: &ProxyContext,
) {
    // 分析错误类型
    let (error_category, error_type_cn, error_message) =
        analyze_error_type(error, status_code, ctx);

    // 获取Pingora错误详情
    let pingora_error_details = error.map_or_else(|| "无Pingora错误".to_string(), Error::to_string);

    // 安全获取请求体
    let request_body = String::from_utf8_lossy(&ctx.request_body);
    let request_body_preview = if request_body.len() > 4096 {
        format!("{}...", &request_body[..4096])
    } else {
        request_body.to_string()
    };

    // 处理响应体
    let response_body_preview = if ctx
        .response_details
        .content_encoding
        .as_deref()
        .is_some_and(|encoding| encoding.contains("gzip"))
    {
        let mut decoder = GzDecoder::new(ctx.response_body.as_ref());
        let mut decompressed = Vec::new();
        if decoder.read_to_end(&mut decompressed).is_ok() {
            let body_str = String::from_utf8_lossy(&decompressed);
            if body_str.len() > 4096 {
                format!("{}...", &body_str[..4096])
            } else {
                body_str.to_string()
            }
        } else {
            String::from_utf8_lossy(&ctx.response_body).to_string()
        }
    } else {
        let body_str = String::from_utf8_lossy(&ctx.response_body);
        if body_str.len() > 4096 {
            format!("{}...", &body_str[..4096])
        } else {
            body_str.to_string()
        }
    };

    // 检测状态码不一致问题
    let context_status_code = ctx.response_details.status_code;
    let status_code_consistent = context_status_code.is_none_or(|ctx_code| ctx_code == status_code);

    // 记录详细的错误信息
    lerror!(
        request_id,
        LogStage::ResponseFailure,
        LogComponent::Proxy,
        "proxy_request_failed",
        "代理请求失败 - 详细分析",
        status_code = status_code,
        context_status_code = ?context_status_code,
        status_code_consistent = status_code_consistent,
        error_category = %error_category,
        error_type_cn = %error_type_cn,
        error_message = %error_message,
        pingora_error_details = %pingora_error_details,
        path = %ctx.request_details.path,
        method = %ctx.request_details.method,
        client_ip = %ctx.request_details.client_ip,
        user_agent = ?ctx.request_details.user_agent,
        request_body_size = ctx.request_body.len(),
        response_body_size = ctx.response_body.len(),
        request_body_preview = %request_body_preview,
        response_body_preview = %response_body_preview,
        has_selected_backend = ctx.selected_backend.is_some(),
        provider_type = ?ctx.provider_type.as_ref().map(|p| &p.name),
        request_duration_ms = ctx.start_time.elapsed().as_millis()
    );

    // 针对连接失败的特殊日志
    if matches!(
        error_category.as_str(),
        "connection_failure" | "connection_timeout" | "connection_error"
    ) {
        lwarn!(
            request_id,
            LogStage::ResponseFailure,
            LogComponent::Proxy,
            "connection_failure_analysis",
            "连接失败分析 - 可能需要检查网络或上游服务状态",
            error_category = %error_category,
            client_ip = %ctx.request_details.client_ip,
            selected_backend_id = ?ctx.selected_backend.as_ref().map(|b| b.id),
            provider_name = ?ctx.provider_type.as_ref().map(|p| &p.name)
        );
    }

    // 针对状态码不一致的警告
    if !status_code_consistent {
        lwarn!(
            request_id,
            LogStage::ResponseFailure,
            LogComponent::Proxy,
            "status_code_inconsistency",
            "检测到状态码不一致 - 可能存在部分响应接收问题",
            resolved_status_code = status_code,
            context_status_code = ?context_status_code,
            error_category = %error_category
        );
    }
}

/// 记录 Gemini 完整请求信息
pub fn log_complete_request(request_id: &str, path: &str, session: &Session, ctx: &ProxyContext) {
    // 读取请求体
    let request_body = if ctx.request_body.is_empty() {
        String::new()
    } else {
        String::from_utf8_lossy(&ctx.request_body).to_string()
    };

    // 过滤 request 字段
    let filtered_body = if path.contains("streamGenerateContent") {
        filter_request_field(&request_body)
    } else {
        request_body
    };

    // 记录请求头
    let headers = headers_json_map_request(session.req_header());

    linfo!(
        request_id,
        LogStage::UpstreamRequest,
        LogComponent::GeminiStrategy,
        "gemini_complete_request",
        "=== GEMINI COMPLETE REQUEST ===",
        route = path,
        method = %session.req_header().method,
        uri = %session.req_header().uri,
        request_headers = %serde_json::to_string_pretty(&headers).unwrap_or_else(|_| "Failed to serialize headers".to_string()),
        request_body = %filtered_body
    );
}

/// 记录 Gemini 完整响应信息
pub fn log_complete_response(
    request_id: &str,
    path: &str,
    response_header: &ResponseHeader,
    response_body: &[u8],
) {
    // 读取响应头
    let response_headers = headers_json_map_response(response_header);

    // 读取响应体
    let body_str = String::from_utf8_lossy(response_body);

    linfo!(
        request_id,
        LogStage::Response,
        LogComponent::GeminiStrategy,
        "gemini_complete_response",
        "=== GEMINI COMPLETE RESPONSE ===",
        route = path,
        status_code = %response_header.status,
        response_headers = %serde_json::to_string_pretty(&response_headers).unwrap_or_else(|_| "Failed to serialize response headers".to_string()),
        response_body = %body_str
    );
}

/// 记录错误响应信息（状态码 >= 400）
pub fn log_error_response(request_id: &str, path: &str, status_code: u16, response_body: &[u8]) {
    linfo!(
        request_id,
        LogStage::ResponseFailure,
        LogComponent::Proxy,
        "error_response",
        "=== ERROR RESPONSE ===",
        target = "error_response",
        path = %path,
        status_code = %status_code,
        response_body = %String::from_utf8_lossy(response_body)
    );
}

/// 过滤 JSON 中的 request 字段
fn filter_request_field(json_str: &str) -> String {
    serde_json::from_str::<serde_json::Value>(json_str).map_or_else(
        |_| json_str.to_string(),
        |mut json| {
            if let Some(obj) = json.as_object_mut() {
                obj.remove("request");
            }
            serde_json::to_string(&json).unwrap_or_else(|_| json_str.to_string())
        },
    )
}
