//! # 错误类型定义

use axum::body::Body as AxumBody;
use axum::http::StatusCode;
use axum::response::Response as AxumResponse;
use thiserror::Error;

/// 应用主要错误类型
#[derive(Debug, Error)]
pub enum ProxyError {
    /// 配置相关错误
    #[error("配置错误: {message}")]
    Config {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 数据库相关错误  
    #[error("数据库错误: {message}")]
    Database {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 网络通信错误
    #[error("网络错误: {message}")]
    Network {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// AI服务商错误
    #[error("AI服务错误: {message}")]
    AiProvider {
        message: String,
        provider: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// TLS证书错误
    #[error("TLS证书错误: {message}")]
    Tls {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 业务逻辑错误
    #[error("业务错误: {message}")]
    Business { message: String },

    /// Gemini Code Assist API错误
    #[error("Gemini Code Assist API错误: {message}")]
    GeminiCodeAssistError { message: String },

    /// `Gemini项目ID获取错误`
    #[error("Gemini项目ID获取错误: {message}")]
    GeminiProjectIdError { message: String },

    /// 系统内部错误
    #[error("内部错误: {message}")]
    Internal {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// IO相关错误
    #[error("IO错误: {message}")]
    Io {
        message: String,
        #[source]
        source: std::io::Error,
    },

    /// 序列化/反序列化错误
    #[error("序列化错误: {message}")]
    Serialization {
        message: String,
        #[source]
        source: anyhow::Error,
    },

    /// 缓存相关错误
    #[error("缓存错误: {message}")]
    Cache {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 服务器初始化错误
    #[error("服务器初始化错误: {message}")]
    ServerInit {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 服务器启动错误
    #[error("服务器启动错误: {message}")]
    ServerStart {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 认证错误
    #[error("认证错误: {message}")]
    Authentication {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 上游服务器未找到
    #[error("上游服务器未找到: {message}")]
    UpstreamNotFound {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 上游服务器不可用
    #[error("上游服务器不可用: {message}")]
    UpstreamNotAvailable {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 速率限制错误
    #[error("速率限制: {message}")]
    RateLimit {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 网关错误
    #[error("网关错误: {message}")]
    BadGateway {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 连接超时错误
    #[error("连接超时: {message}")]
    ConnectionTimeout {
        message: String,
        timeout_seconds: u64,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 读取超时错误
    #[error("读取超时: {message}")]
    ReadTimeout {
        message: String,
        timeout_seconds: u64,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 写入超时错误
    #[error("写入超时: {message}")]
    WriteTimeout {
        message: String,
        timeout_seconds: u64,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 负载均衡错误
    #[error("负载均衡错误: {message}")]
    LoadBalancer {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 健康检查错误
    #[error("健康检查错误: {message}")]
    HealthCheck {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 统计收集错误
    #[error("统计收集错误: {message}")]
    Statistics {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 跟踪系统错误
    #[error("跟踪系统错误: {message}")]
    Tracing {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 管理模块认证错误
    #[error("管理认证错误: {message}")]
    ManagementAuth {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 管理模块权限错误
    #[error("管理权限错误: {message}")]
    ManagementPermission {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 管理模块验证错误
    #[error("管理验证错误: {message}")]
    ManagementValidation {
        message: String,
        field: Option<String>,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 管理模块业务错误
    #[error("管理业务错误: {message}")]
    ManagementBusiness {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 管理模块资源未找到错误
    #[error("管理资源未找到: {resource_type} {identifier}")]
    ManagementNotFound {
        resource_type: String,
        identifier: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 管理模块资源冲突错误
    #[error("管理资源冲突: {resource_type} {identifier}")]
    ManagementConflict {
        resource_type: String,
        identifier: String,
        #[source]
        source: Option<anyhow::Error>,
    },

    /// 管理模块速率限制错误
    #[error("管理速率限制: {message}")]
    ManagementRateLimit {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
    },
}

impl ProxyError {
    /// 将错误标准化为 (HTTP 状态码, 标准错误码, 人类可读消息)
    /// 可用于管理端(Axum)与代理端(Pingora)统一输出
    #[must_use]
    pub fn as_http_parts(&self) -> (StatusCode, &'static str, String) {
        let (status, code) = self.to_http_response_parts();
        let message = self.to_string();
        (status, code, message)
    }

    /// 直接转换为 Axum Response（application/json）
    pub fn to_axum_response(&self) -> AxumResponse {
        let (status, body) = self.to_http_status_and_body();
        axum::http::Response::builder()
            .status(status)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(AxumBody::from(body))
            .unwrap_or_else(|_| axum::http::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(AxumBody::from("{\"error\":\"Internal error building response\",\"code\":\"INTERNAL_ERROR\"}"))
                .expect("fallback response"))
    }

    /// 直接转换为 Pingora 错误
    #[must_use]
    pub fn to_pingora_error(&self) -> pingora_core::Error {
        let (status, body) = self.to_http_status_and_body();
        *pingora_core::Error::explain(pingora_core::ErrorType::HTTPStatus(status.as_u16()), body)
    }
    /// 直接生成 (HTTP 状态码, JSON 字符串) 形式的响应体，便于快速返回
    /// JSON 结构: {"error": <message>, "code": <code>, [extras...]}
    #[must_use]
    pub fn to_http_status_and_body(&self) -> (StatusCode, String) {
        let (status, code) = self.to_http_response_parts();

        // 附加可选字段（如超时配置）
        match self {
            Self::ConnectionTimeout {
                timeout_seconds, ..
            } => (
                status,
                format!(
                    "{{\"error\":\"{self}\",\"code\":\"{code}\",\"timeout_configured\":{timeout_seconds}}}"
                ),
            ),
            Self::ReadTimeout {
                timeout_seconds, ..
            } => (
                status,
                format!(
                    "{{\"error\":\"{self}\",\"code\":\"{code}\",\"timeout_configured\":{timeout_seconds}}}"
                ),
            ),
            Self::WriteTimeout {
                timeout_seconds, ..
            } => (
                status,
                format!(
                    "{{\"error\":\"{self}\",\"code\":\"{code}\",\"timeout_configured\":{timeout_seconds}}}"
                ),
            ),
            _ => (
                status,
                format!("{{\"error\":\"{self}\",\"code\":\"{code}\"}}"),
            ),
        }
    }

    /// 将错误转换为HTTP状态码和错误代码
    #[must_use]
    pub const fn to_http_response_parts(&self) -> (StatusCode, &'static str) {
        match self {
            Self::Config { .. } => (StatusCode::BAD_REQUEST, "CONFIG_ERROR"),
            Self::Database { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR"),
            Self::Network { .. } => (StatusCode::BAD_GATEWAY, "NETWORK_ERROR"),

            Self::AiProvider { .. } => (StatusCode::BAD_GATEWAY, "AI_PROVIDER_ERROR"),
            Self::Tls { .. } => (StatusCode::BAD_REQUEST, "TLS_ERROR"),
            Self::Business { .. } => (StatusCode::BAD_REQUEST, "BUSINESS_ERROR"),
            Self::Internal { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
            Self::Io { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "IO_ERROR"),
            Self::Serialization { .. } => (StatusCode::BAD_REQUEST, "SERIALIZATION_ERROR"),
            Self::Cache { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "CACHE_ERROR"),
            Self::ServerInit { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "SERVER_INIT_ERROR"),
            Self::ServerStart { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "SERVER_START_ERROR"),
            Self::Authentication { .. } => (StatusCode::UNAUTHORIZED, "AUTH_ERROR"),
            Self::UpstreamNotFound { .. } => (StatusCode::NOT_FOUND, "UPSTREAM_NOT_FOUND"),
            Self::UpstreamNotAvailable { .. } => {
                (StatusCode::SERVICE_UNAVAILABLE, "UPSTREAM_UNAVAILABLE")
            }
            Self::RateLimit { .. } => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMIT"),
            Self::BadGateway { .. } => (StatusCode::BAD_GATEWAY, "BAD_GATEWAY"),
            Self::ConnectionTimeout { .. } => (StatusCode::GATEWAY_TIMEOUT, "CONNECTION_TIMEOUT"),
            Self::ReadTimeout { .. } => (StatusCode::GATEWAY_TIMEOUT, "READ_TIMEOUT"),
            Self::WriteTimeout { .. } => (StatusCode::GATEWAY_TIMEOUT, "WRITE_TIMEOUT"),
            Self::LoadBalancer { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "LOAD_BALANCER_ERROR"),
            Self::HealthCheck { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "HEALTH_CHECK_ERROR"),
            Self::Statistics { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "STATISTICS_ERROR"),
            Self::Tracing { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "TRACING_ERROR"),
            Self::ManagementAuth { .. } => (StatusCode::UNAUTHORIZED, "AUTH_ERROR"),
            Self::ManagementPermission { .. } => (StatusCode::FORBIDDEN, "PERMISSION_ERROR"),
            Self::ManagementValidation { .. } => (StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
            Self::ManagementBusiness { .. } => (StatusCode::BAD_REQUEST, "BUSINESS_ERROR"),
            Self::ManagementNotFound { .. } => (StatusCode::NOT_FOUND, "RESOURCE_NOT_FOUND"),
            Self::ManagementConflict { .. } => (StatusCode::CONFLICT, "RESOURCE_CONFLICT"),
            Self::ManagementRateLimit { .. } => {
                (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMIT_EXCEEDED")
            }
            Self::GeminiCodeAssistError { .. } => {
                (StatusCode::BAD_GATEWAY, "GEMINI_CODE_ASSIST_ERROR")
            }
            Self::GeminiProjectIdError { .. } => {
                (StatusCode::BAD_GATEWAY, "GEMINI_PROJECT_ID_ERROR")
            }
        }
    }

    /// 创建配置错误
    pub fn config<T: Into<String>>(message: T) -> Self {
        Self::Config {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的配置错误
    pub fn config_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Config {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建数据库错误
    pub fn database<T: Into<String>>(message: T) -> Self {
        Self::Database {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的数据库错误
    pub fn database_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Database {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建网络错误
    pub fn network<T: Into<String>>(message: T) -> Self {
        Self::Network {
            message: message.into(),
            source: None,
        }
    }

    /// ��建带来源的网络错误
    pub fn network_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Network {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建AI服务商错误
    pub fn ai_provider<T: Into<String>, P: Into<String>>(message: T, provider: P) -> Self {
        Self::AiProvider {
            message: message.into(),
            provider: provider.into(),
            source: None,
        }
    }

    /// 创建业务错误
    pub fn business<T: Into<String>>(message: T) -> Self {
        Self::Business {
            message: message.into(),
        }
    }

    /// 创建内部错误
    pub fn internal<T: Into<String>>(message: T) -> Self {
        Self::Internal {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的内部错误
    pub fn internal_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Internal {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建缓存错误
    pub fn cache<T: Into<String>>(message: T) -> Self {
        Self::Cache {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的缓存错误
    pub fn cache_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Cache {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建服务器初始化错误
    pub fn server_init<T: Into<String>>(message: T) -> Self {
        Self::ServerInit {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的服务器初始化错误
    pub fn server_init_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::ServerInit {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建服务器启动错误
    pub fn server_start<T: Into<String>>(message: T) -> Self {
        Self::ServerStart {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的服务器启动错误
    pub fn server_start_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::ServerStart {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建认证错误
    pub fn authentication<T: Into<String>>(message: T) -> Self {
        Self::Authentication {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的认证错误
    pub fn authentication_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Authentication {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建速率限制错误
    pub fn rate_limit<T: Into<String>>(message: T) -> Self {
        Self::RateLimit {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的速率限制错误
    pub fn rate_limit_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::RateLimit {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建网关错误
    pub fn bad_gateway<T: Into<String>>(message: T) -> Self {
        Self::BadGateway {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的网关错误
    pub fn bad_gateway_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::BadGateway {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建上游服务器未找到错误
    pub fn upstream_not_found<T: Into<String>>(message: T) -> Self {
        Self::UpstreamNotFound {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的上游服务���未找到错误
    pub fn upstream_not_found_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::UpstreamNotFound {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建上游服务器不可用错误
    pub fn upstream_not_available<T: Into<String>>(message: T) -> Self {
        Self::UpstreamNotAvailable {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的上游服务器不可用错误
    pub fn upstream_not_available_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::UpstreamNotAvailable {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建连接超时错误
    pub fn connection_timeout<T: Into<String>>(message: T, timeout_seconds: u64) -> Self {
        Self::ConnectionTimeout {
            message: message.into(),
            timeout_seconds,
            source: None,
        }
    }

    /// 创建带来源的连接超时错误
    pub fn connection_timeout_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        timeout_seconds: u64,
        source: E,
    ) -> Self {
        Self::ConnectionTimeout {
            message: message.into(),
            timeout_seconds,
            source: Some(source.into()),
        }
    }

    /// 创建读取超时错误
    pub fn read_timeout<T: Into<String>>(message: T, timeout_seconds: u64) -> Self {
        Self::ReadTimeout {
            message: message.into(),
            timeout_seconds,
            source: None,
        }
    }

    /// 创建带来源的读取超时错误
    pub fn read_timeout_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        timeout_seconds: u64,
        source: E,
    ) -> Self {
        Self::ReadTimeout {
            message: message.into(),
            timeout_seconds,
            source: Some(source.into()),
        }
    }

    /// 创建写入超时错误
    pub fn write_timeout<T: Into<String>>(message: T, timeout_seconds: u64) -> Self {
        Self::WriteTimeout {
            message: message.into(),
            timeout_seconds,
            source: None,
        }
    }

    /// 创建带来源的写入超时错误
    pub fn write_timeout_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        timeout_seconds: u64,
        source: E,
    ) -> Self {
        Self::WriteTimeout {
            message: message.into(),
            timeout_seconds,
            source: Some(source.into()),
        }
    }

    /// 创建负载均衡错误
    pub fn load_balancer<T: Into<String>>(message: T) -> Self {
        Self::LoadBalancer {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的负载均衡错误
    pub fn load_balancer_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::LoadBalancer {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建健康检查错误
    pub fn health_check<T: Into<String>>(message: T) -> Self {
        Self::HealthCheck {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的健康检查错误
    pub fn health_check_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::HealthCheck {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建统计收集错误
    pub fn statistics<T: Into<String>>(message: T) -> Self {
        Self::Statistics {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的统计收集错误
    pub fn statistics_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Statistics {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建跟踪系统错误
    pub fn tracing<T: Into<String>>(message: T) -> Self {
        Self::Tracing {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的跟踪系统错误
    pub fn tracing_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::Tracing {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建管理模块认证错误
    pub fn management_auth<T: Into<String>>(message: T) -> Self {
        Self::ManagementAuth {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的管理模块认证错误
    pub fn management_auth_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::ManagementAuth {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建管理模块权限错误
    pub fn management_permission<T: Into<String>>(message: T) -> Self {
        Self::ManagementPermission {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的管理模块权限错误
    pub fn management_permission_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::ManagementPermission {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建管理模块验证错误
    pub fn management_validation<T: Into<String>>(message: T, field: Option<String>) -> Self {
        Self::ManagementValidation {
            message: message.into(),
            field,
            source: None,
        }
    }

    /// 创建带来源的管理模块验证错误
    pub fn management_validation_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        field: Option<String>,
        source: E,
    ) -> Self {
        Self::ManagementValidation {
            message: message.into(),
            field,
            source: Some(source.into()),
        }
    }

    /// 创建管理模块业务错误
    pub fn management_business<T: Into<String>>(message: T) -> Self {
        Self::ManagementBusiness {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的管理模块业务错误
    pub fn management_business_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::ManagementBusiness {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建管理模块资源未找到错误
    pub fn management_not_found<T: Into<String>, I: Into<String>>(
        resource_type: T,
        identifier: I,
    ) -> Self {
        Self::ManagementNotFound {
            resource_type: resource_type.into(),
            identifier: identifier.into(),
            source: None,
        }
    }

    /// 创建带来源的管理模块资源未找到错误
    pub fn management_not_found_with_source<
        T: Into<String>,
        I: Into<String>,
        E: Into<anyhow::Error>,
    >(
        resource_type: T,
        identifier: I,
        source: E,
    ) -> Self {
        Self::ManagementNotFound {
            resource_type: resource_type.into(),
            identifier: identifier.into(),
            source: Some(source.into()),
        }
    }

    /// 创建管理模块资源冲突错误
    pub fn management_conflict<T: Into<String>, I: Into<String>>(
        resource_type: T,
        identifier: I,
    ) -> Self {
        Self::ManagementConflict {
            resource_type: resource_type.into(),
            identifier: identifier.into(),
            source: None,
        }
    }

    /// 创建带来源的管理模块资源冲突错误
    pub fn management_conflict_with_source<
        T: Into<String>,
        I: Into<String>,
        E: Into<anyhow::Error>,
    >(
        resource_type: T,
        identifier: I,
        source: E,
    ) -> Self {
        Self::ManagementConflict {
            resource_type: resource_type.into(),
            identifier: identifier.into(),
            source: Some(source.into()),
        }
    }

    /// 创建管理模块速率限制错误
    pub fn management_rate_limit<T: Into<String>>(message: T) -> Self {
        Self::ManagementRateLimit {
            message: message.into(),
            source: None,
        }
    }

    /// 创建带来源的管理模块速率限制错误
    pub fn management_rate_limit_with_source<T: Into<String>, E: Into<anyhow::Error>>(
        message: T,
        source: E,
    ) -> Self {
        Self::ManagementRateLimit {
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// 创建Gemini Code Assist API错误
    pub fn gemini_code_assist<T: Into<String>>(message: T) -> Self {
        Self::GeminiCodeAssistError {
            message: message.into(),
        }
    }

    /// `创建Gemini项目ID获取错误`
    pub fn gemini_project_id<T: Into<String>>(message: T) -> Self {
        Self::GeminiProjectIdError {
            message: message.into(),
        }
    }
}

// 自动转换常见错误类型
impl From<std::io::Error> for ProxyError {
    fn from(err: std::io::Error) -> Self {
        Self::Io {
            message: "文件操作失败".to_string(),
            source: err,
        }
    }
}

impl From<toml::de::Error> for ProxyError {
    fn from(err: toml::de::Error) -> Self {
        Self::config_with_source("TOML解析失败", err)
    }
}

impl From<serde_json::Error> for ProxyError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization {
            message: "JSON处理失败".to_string(),
            source: err.into(),
        }
    }
}

impl From<sea_orm::error::DbErr> for ProxyError {
    fn from(err: sea_orm::error::DbErr) -> Self {
        Self::database_with_source("数据库操作失败", err)
    }
}

// Redis错误转换
impl From<redis::RedisError> for ProxyError {
    fn from(err: redis::RedisError) -> Self {
        Self::cache_with_source("Redis操作失败", err)
    }
}

// Reqwest错误转换
impl From<reqwest::Error> for ProxyError {
    fn from(err: reqwest::Error) -> Self {
        Self::network_with_source("HTTP请求失败", err)
    }
}

// Bcrypt错误转换
impl From<bcrypt::BcryptError> for ProxyError {
    fn from(err: bcrypt::BcryptError) -> Self {
        Self::authentication_with_source("密码处理失败", err)
    }
}

// JWT错误转换
impl From<jsonwebtoken::errors::Error> for ProxyError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        Self::authentication_with_source("JWT处理失败", err)
    }
}

// Pingora错误转换
impl From<pingora_core::Error> for ProxyError {
    fn from(err: pingora_core::Error) -> Self {
        Self::network_with_source("Pingora操作失败", err)
    }
}

// AuthParseError错误转换
impl From<crate::auth::header_parser::AuthParseError> for ProxyError {
    fn from(err: crate::auth::header_parser::AuthParseError) -> Self {
        Self::Authentication {
            message: format!("认证头解析失败: {err}"),
            source: Some(anyhow::Error::new(err)),
        }
    }
}

// 空实现 - 不需要自引用转换
