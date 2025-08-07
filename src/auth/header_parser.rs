//! # 通用认证头解析器
//!
//! 解析各种HTTP认证头格式，支持标准的 "Header-Name: Header-Value" 格式

use serde::{Deserialize, Serialize};

/// 解析后的认证头信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthHeader {
    /// HTTP头名称（小写格式）
    pub name: String,
    /// HTTP头值
    pub value: String,
}

impl AuthHeader {
    /// 创建新的认证头
    pub fn new(name: String, value: String) -> Self {
        Self {
            name: name.to_lowercase(),
            value,
        }
    }
}

/// 认证头解析错误
#[derive(Debug, thiserror::Error)]
pub enum AuthParseError {
    #[error("Invalid authentication header format: '{0}'. Expected format: 'Header-Name: header-value'")]
    InvalidFormat(String),
    
    #[error("Empty header name in format: '{0}'")]
    EmptyHeaderName(String),
    
    #[error("Empty header value template in format: '{0}'")]
    EmptyHeaderValue(String),
    
    #[error("Missing key placeholder '{{key}}' in header value: '{0}'")]
    MissingKeyPlaceholder(String),
}

/// 通用认证头解析器
pub struct AuthHeaderParser;

impl AuthHeaderParser {
    /// 解析认证头格式并替换API密钥
    ///
    /// # 支持的格式
    /// - `"Authorization: Bearer {key}"` -> `AuthHeader { name: "authorization", value: "Bearer sk-123" }`
    /// - `"Authorization: {key}"` -> `AuthHeader { name: "authorization", value: "sk-456" }`
    /// - `"X-goog-api-key: {key}"` -> `AuthHeader { name: "x-goog-api-key", value: "sk-789" }`
    /// - `"X-API-Key: Token {key}"` -> `AuthHeader { name: "x-api-key", value: "Token sk-abc" }`
    ///
    /// # 参数
    /// - `format`: HTTP头格式模板，必须包含 `{key}` 占位符
    /// - `api_key`: 实际的API密钥
    ///
    /// # 错误
    /// - `InvalidFormat`: 格式不符合 "Header-Name: header-value" 标准
    /// - `EmptyHeaderName`: 头名称为空
    /// - `EmptyHeaderValue`: 头值模板为空
    /// - `MissingKeyPlaceholder`: 头值中缺少 `{key}` 占位符
    pub fn parse(format: &str, api_key: &str) -> Result<AuthHeader, AuthParseError> {
        // 分割头名称和值模板
        let (header_name, value_template) = Self::split_header_format(format)?;
        
        // 验证头名称
        if header_name.trim().is_empty() {
            return Err(AuthParseError::EmptyHeaderName(format.to_string()));
        }
        
        // 验证值模板
        if value_template.trim().is_empty() {
            return Err(AuthParseError::EmptyHeaderValue(format.to_string()));
        }
        
        // 检查是否包含 {key} 占位符
        if !value_template.contains("{key}") {
            return Err(AuthParseError::MissingKeyPlaceholder(value_template.to_string()));
        }
        
        // 替换占位符
        let header_value = value_template.replace("{key}", api_key);
        
        Ok(AuthHeader::new(header_name.to_string(), header_value))
    }
    
    /// 分割头格式为名称和值模板
    fn split_header_format(format: &str) -> Result<(&str, &str), AuthParseError> {
        format.split_once(": ")
            .ok_or_else(|| AuthParseError::InvalidFormat(format.to_string()))
    }
    
    /// 验证认证头格式是否有效（不替换密钥）
    pub fn validate_format(format: &str) -> Result<(), AuthParseError> {
        let (_header_name, value_template) = Self::split_header_format(format)?;
        
        if !value_template.contains("{key}") {
            return Err(AuthParseError::MissingKeyPlaceholder(value_template.to_string()));
        }
        
        Ok(())
    }
    
    /// 提取头名称（不包含值）
    pub fn extract_header_name(format: &str) -> Result<String, AuthParseError> {
        let (header_name, _) = Self::split_header_format(format)?;
        
        if header_name.trim().is_empty() {
            return Err(AuthParseError::EmptyHeaderName(format.to_string()));
        }
        
        Ok(header_name.to_lowercase())
    }
    
    /// 构建标准格式的认证头字符串（用于配置迁移）
    pub fn build_standard_format(header_name: &str, value_template: &str) -> String {
        format!("{}: {}", header_name, value_template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_authorization_bearer() {
        let result = AuthHeaderParser::parse("Authorization: Bearer {key}", "sk-test123").unwrap();
        assert_eq!(result.name, "authorization");
        assert_eq!(result.value, "Bearer sk-test123");
    }

    #[test]
    fn test_parse_authorization_direct() {
        let result = AuthHeaderParser::parse("Authorization: {key}", "sk-direct456").unwrap();
        assert_eq!(result.name, "authorization");
        assert_eq!(result.value, "sk-direct456");
    }

    #[test]
    fn test_parse_google_api_key() {
        let result = AuthHeaderParser::parse("X-goog-api-key: {key}", "AIza_google_key").unwrap();
        assert_eq!(result.name, "x-goog-api-key");
        assert_eq!(result.value, "AIza_google_key");
    }

    #[test]
    fn test_parse_custom_header() {
        let result = AuthHeaderParser::parse("X-API-Key: Token {key}", "custom_token_789").unwrap();
        assert_eq!(result.name, "x-api-key");
        assert_eq!(result.value, "Token custom_token_789");
    }

    #[test]
    fn test_case_insensitive_header_name() {
        let result = AuthHeaderParser::parse("Authorization: Bearer {key}", "test").unwrap();
        assert_eq!(result.name, "authorization"); // 转换为小写

        let result2 = AuthHeaderParser::parse("X-GOOG-API-KEY: {key}", "test").unwrap();
        assert_eq!(result2.name, "x-goog-api-key"); // 转换为小写
    }

    #[test]
    fn test_invalid_format_no_colon() {
        let result = AuthHeaderParser::parse("Bearer {key}", "test");
        assert!(matches!(result, Err(AuthParseError::InvalidFormat(_))));
    }

    #[test]
    fn test_invalid_format_no_space() {
        let result = AuthHeaderParser::parse("Authorization:{key}", "test");
        assert!(matches!(result, Err(AuthParseError::InvalidFormat(_))));
    }

    #[test]
    fn test_empty_header_name() {
        let result = AuthHeaderParser::parse(": Bearer {key}", "test");
        assert!(matches!(result, Err(AuthParseError::EmptyHeaderName(_))));
    }

    #[test]
    fn test_empty_header_value() {
        let result = AuthHeaderParser::parse("Authorization: ", "test");
        assert!(matches!(result, Err(AuthParseError::EmptyHeaderValue(_))));
    }

    #[test]
    fn test_missing_key_placeholder() {
        let result = AuthHeaderParser::parse("Authorization: Bearer token", "test");
        assert!(matches!(result, Err(AuthParseError::MissingKeyPlaceholder(_))));
    }

    #[test]
    fn test_validate_format() {
        assert!(AuthHeaderParser::validate_format("Authorization: Bearer {key}").is_ok());
        assert!(AuthHeaderParser::validate_format("X-goog-api-key: {key}").is_ok());
        assert!(AuthHeaderParser::validate_format("Bearer {key}").is_err());
        assert!(AuthHeaderParser::validate_format("Authorization: Bearer").is_err());
    }

    #[test]
    fn test_extract_header_name() {
        assert_eq!(
            AuthHeaderParser::extract_header_name("Authorization: Bearer {key}").unwrap(),
            "authorization"
        );
        assert_eq!(
            AuthHeaderParser::extract_header_name("X-goog-api-key: {key}").unwrap(),
            "x-goog-api-key"
        );
    }

    #[test]
    fn test_build_standard_format() {
        assert_eq!(
            AuthHeaderParser::build_standard_format("Authorization", "Bearer {key}"),
            "Authorization: Bearer {key}"
        );
        assert_eq!(
            AuthHeaderParser::build_standard_format("X-goog-api-key", "{key}"),
            "X-goog-api-key: {key}"
        );
    }

    #[test]
    fn test_multiple_key_replacements() {
        // 测试多个 {key} 占位符的情况
        let result = AuthHeaderParser::parse("X-Custom: prefix-{key}-suffix-{key}", "token123").unwrap();
        assert_eq!(result.name, "x-custom");
        assert_eq!(result.value, "prefix-token123-suffix-token123");
    }

    #[test]
    fn test_whitespace_handling() {
        let result = AuthHeaderParser::parse("  Authorization  :  Bearer {key}  ", "test").unwrap();
        assert_eq!(result.name, "authorization");
        assert_eq!(result.value, "Bearer test  "); // 保持值中的尾随空格
    }
}