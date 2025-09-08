//! 认证模块重构测试
//!
//! 验证重构后的AuthenticationService和AuthHeaderParser集成工作正常

use api_proxy::auth::{AuthHeaderParser, AuthParseError};
use api_proxy::error::ProxyError;

/// 测试AuthParseError到ProxyError的自动转换
#[test]
fn test_auth_parse_error_conversion() {
    let parse_error = AuthParseError::InvalidFormat("invalid format".to_string());
    let proxy_error: ProxyError = parse_error.into();
    
    match proxy_error {
        ProxyError::Authentication { message, .. } => {
            assert!(message.contains("认证头解析失败"));
            assert!(message.contains("invalid format"));
        }
        _ => panic!("Expected Authentication error variant"),
    }
}

/// 测试AuthHeaderParser的基本功能
#[test]
fn test_auth_header_parser_basic() {
    // 测试标准Bearer格式
    let result = AuthHeaderParser::parse("Authorization: Bearer {key}", "test-api-key");
    assert!(result.is_ok());
    
    let header = result.unwrap();
    assert_eq!(header.name, "authorization");
    assert_eq!(header.value, "Bearer test-api-key");
}

/// 测试反向解析功能
#[test]
fn test_reverse_parsing() {
    // 测试从实际HTTP头值中提取API密钥 (注意：头名称应该是小写)
    let result = AuthHeaderParser::parse_api_key_from_inbound_headers_smart(
        r#"["Authorization: Bearer {key}"]"#,
        "authorization",  // 头名称必须小写
        "Bearer actual-api-key-123"
    );
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "actual-api-key-123");
}

/// 测试错误处理
#[test]
fn test_error_handling() {
    // 测试无效格式
    let result = AuthHeaderParser::parse("invalid format", "test-key");
    assert!(result.is_err());
    
    // 测试反向解析错误
    let result = AuthHeaderParser::parse_api_key_from_inbound_headers_smart(
        r#"["Authorization: Bearer {key}"]"#,
        "Authorization", 
        "invalid header value"  // 不匹配Bearer格式
    );
    assert!(result.is_err());
}

/// 测试多格式支持
#[test]
fn test_multiple_formats() {
    // 测试JSON数组格式解析
    let formats = r#"["Authorization: Bearer {key}", "X-API-Key: {key}"]"#;
    let result = AuthHeaderParser::extract_header_names_from_array(formats);
    
    assert!(result.is_ok());
    let header_names = result.unwrap();
    assert_eq!(header_names.len(), 2);
    assert!(header_names.contains(&"authorization".to_string()));
    assert!(header_names.contains(&"x-api-key".to_string()));
}

/// 集成测试：模拟完整的认证头解析流程
#[test]
fn test_integration_flow() {
    let auth_formats = r#"["Authorization: Bearer {key}", "X-API-Key: {key}"]"#;
    
    // 测试Authorization头
    let result = AuthHeaderParser::parse_api_key_from_inbound_headers_smart(
        auth_formats,
        "authorization",
        "Bearer integration-test-key"
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "integration-test-key");
    
    // 测试X-API-Key头
    let result = AuthHeaderParser::parse_api_key_from_inbound_headers_smart(
        auth_formats,
        "x-api-key", 
        "direct-api-key"
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "direct-api-key");
}