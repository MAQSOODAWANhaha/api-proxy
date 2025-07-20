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
}