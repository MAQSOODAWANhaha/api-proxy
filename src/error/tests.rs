//! # 错误处理测试

#[cfg(test)]
mod tests {
    use std::error::Error;
    use crate::error::{ProxyError, ErrorContext};

    #[test]
    fn test_config_error_creation() {
        let err = ProxyError::config("测试配置错误");
        assert!(matches!(err, ProxyError::Config { .. }));
        assert_eq!(err.to_string(), "配置错误: 测试配置错误");
    }

    #[test]
    fn test_config_error_with_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件不存在");
        let err = ProxyError::config_with_source("配置文件加载失败", io_err);
        
        assert!(matches!(err, ProxyError::Config { .. }));
        assert!(err.to_string().contains("配置错误: 配置文件加载失败"));
        assert!(err.source().is_some());
    }

    #[test]
    fn test_ai_provider_error() {
        let err = ProxyError::ai_provider("API调用失败", "OpenAI");
        assert!(matches!(err, ProxyError::AiProvider { .. }));
        assert!(err.to_string().contains("AI服务错误: API调用失败"));
    }

    #[test]
    fn test_error_context_trait() {
        let result: Result<(), std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "权限不足"
        ));
        
        let err = result.with_config_context(|| "读取配置文件失败".to_string()).unwrap_err();
        assert!(matches!(err, ProxyError::Config { .. }));
        assert!(err.to_string().contains("配置错误: 读取配置文件失败"));
    }

    #[test]
    fn test_option_error_context() {
        let option: Option<String> = None;
        let err = option.with_database_context(|| "找不到数据库连接".to_string()).unwrap_err();
        
        assert!(matches!(err, ProxyError::Database { .. }));
        assert_eq!(err.to_string(), "数据库错误: 找不到数据库连接");
    }

    #[test]
    fn test_auto_conversion_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "文件不存在");
        let proxy_err: ProxyError = io_err.into();
        
        assert!(matches!(proxy_err, ProxyError::Io { .. }));
        assert!(proxy_err.to_string().contains("IO错误: 文件操作失败"));
    }

    #[test]
    fn test_auto_conversion_from_toml_error() {
        let invalid_toml = "invalid = toml = syntax";
        let toml_err = toml::from_str::<toml::Value>(invalid_toml).unwrap_err();
        let proxy_err: ProxyError = toml_err.into();
        
        assert!(matches!(proxy_err, ProxyError::Config { .. }));
        assert!(proxy_err.to_string().contains("配置错误: TOML解析失败"));
    }

    #[test]
    fn test_business_error() {
        let err = ProxyError::business("用户权限不足");
        assert!(matches!(err, ProxyError::Business { .. }));
        assert_eq!(err.to_string(), "业务错误: 用户权限不足");
    }

    #[test]
    fn test_error_chain() {
        let root_cause = std::io::Error::new(std::io::ErrorKind::NotFound, "文件不存在");
        let config_err = ProxyError::config_with_source("无法读取配置", root_cause);
        
        // 验证错误链
        assert!(config_err.source().is_some());
        let source = config_err.source().unwrap();
        assert!(source.to_string().contains("文件不存在"));
    }

    #[test]
    fn test_new_error_types() {
        let err = ProxyError::load_balancer("负载均衡失败");
        assert!(matches!(err, ProxyError::LoadBalancer { .. }));
        assert_eq!(err.to_string(), "负载均衡错误: 负载均衡失败");

        let err = ProxyError::health_check("健康检查失败");
        assert!(matches!(err, ProxyError::HealthCheck { .. }));
        assert_eq!(err.to_string(), "健康检查错误: 健康检查失败");

        let err = ProxyError::statistics("统计收集失败");
        assert!(matches!(err, ProxyError::Statistics { .. }));
        assert_eq!(err.to_string(), "统计收集错误: 统计收集失败");

        let err = ProxyError::tracing("跟踪系统失败");
        assert!(matches!(err, ProxyError::Tracing { .. }));
        assert_eq!(err.to_string(), "跟踪系统错误: 跟踪系统失败");
    }

    #[test]
    fn test_timeout_errors() {
        let err = ProxyError::connection_timeout("连接超时", 30);
        assert!(matches!(err, ProxyError::ConnectionTimeout { timeout_seconds: 30, .. }));
        assert!(err.to_string().contains("连接超时: 连接超时"));

        let err = ProxyError::read_timeout("读取超时", 30);
        assert!(matches!(err, ProxyError::ReadTimeout { timeout_seconds: 30, .. }));
        assert!(err.to_string().contains("读取超时: 读取超时"));

        let err = ProxyError::write_timeout("写入超时", 30);
        assert!(matches!(err, ProxyError::WriteTimeout { timeout_seconds: 30, .. }));
        assert!(err.to_string().contains("写入超时: 写入超时"));
    }

    #[test]
    fn test_error_macros() {
        let err = crate::config_error!("配置错误");
        assert!(matches!(err, ProxyError::Config { .. }));

        let err = crate::database_error!("数据库错误");
        assert!(matches!(err, ProxyError::Database { .. }));

        let err = crate::network_error!("网络错误");
        assert!(matches!(err, ProxyError::Network { .. }));

        let err = crate::auth_error!("认证错误");
        assert!(matches!(err, ProxyError::Auth { .. }));

        let err = crate::cache_error!("缓存错误");
        assert!(matches!(err, ProxyError::Cache { .. }));

        let err = crate::server_init_error!("服务器初始化错误");
        assert!(matches!(err, ProxyError::ServerInit { .. }));

        let err = crate::server_start_error!("服务器启动错误");
        assert!(matches!(err, ProxyError::ServerStart { .. }));

        let err = crate::authentication_error!("认证错误");
        assert!(matches!(err, ProxyError::Authentication { .. }));

        let err = crate::rate_limit_error!("速率限制错误");
        assert!(matches!(err, ProxyError::RateLimit { .. }));

        let err = crate::bad_gateway_error!("网关错误");
        assert!(matches!(err, ProxyError::BadGateway { .. }));

        let err = crate::upstream_not_found_error!("上游服务器未找到");
        assert!(matches!(err, ProxyError::UpstreamNotFound { .. }));

        let err = crate::upstream_not_available_error!("上游服务器不可用");
        assert!(matches!(err, ProxyError::UpstreamNotAvailable { .. }));

        let err = crate::load_balancer_error!("负载均衡错误");
        assert!(matches!(err, ProxyError::LoadBalancer { .. }));

        let err = crate::health_check_error!("健康检查错误");
        assert!(matches!(err, ProxyError::HealthCheck { .. }));

        let err = crate::statistics_error!("统计收集错误");
        assert!(matches!(err, ProxyError::Statistics { .. }));

        let err = crate::tracing_error!("跟踪系统错误");
        assert!(matches!(err, ProxyError::Tracing { .. }));
    }

    #[test]
    fn test_ensure_macros() -> Result<(), ProxyError> {
        crate::ensure_config!(true, "这不应该触发");
        crate::ensure_business!(true, "这不应该触发");
        crate::ensure_database!(true, "这不应该触发");
        crate::ensure_network!(true, "这不应该触发");
        crate::ensure_auth!(true, "这不应该触发");
        crate::ensure_cache!(true, "这不应该触发");

        // 测试确保宏会正确返回错误
        let result = (|| -> Result<(), ProxyError> {
            crate::ensure_config!(false, "配置错误");
            Ok(())
        })();
        assert!(matches!(result.unwrap_err(), ProxyError::Config { .. }));

        let result = (|| -> Result<(), ProxyError> {
            crate::ensure_business!(false, "业务错误");
            Ok(())
        })();
        assert!(matches!(result.unwrap_err(), ProxyError::Business { .. }));

        let result = (|| -> Result<(), ProxyError> {
            crate::ensure_database!(false, "数据库错误");
            Ok(())
        })();
        assert!(matches!(result.unwrap_err(), ProxyError::Database { .. }));

        let result = (|| -> Result<(), ProxyError> {
            crate::ensure_network!(false, "网络错误");
            Ok(())
        })();
        assert!(matches!(result.unwrap_err(), ProxyError::Network { .. }));

        let result = (|| -> Result<(), ProxyError> {
            crate::ensure_auth!(false, "认证错误");
            Ok(())
        })();
        assert!(matches!(result.unwrap_err(), ProxyError::Auth { .. }));

        let result = (|| -> Result<(), ProxyError> {
            crate::ensure_cache!(false, "缓存错误");
            Ok(())
        })();
        assert!(matches!(result.unwrap_err(), ProxyError::Cache { .. }));

        Ok(())
    }
}