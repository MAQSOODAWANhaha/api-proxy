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
    #[error(
        "Invalid authentication header format: '{0}'. Expected format: 'Header-Name: header-value'"
    )]
    /// 认证头格式无效
    InvalidFormat(String),

    #[error("Empty header name in format: '{0}'")]
    /// 认证头名称为空
    EmptyHeaderName(String),

    #[error("Empty header value template in format: '{0}'")]
    /// 认证头值模板为空
    EmptyHeaderValue(String),

    #[error("Missing key placeholder '{{key}}' in header value: '{0}'")]
    /// 缺少密钥占位符
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
            return Err(AuthParseError::MissingKeyPlaceholder(
                value_template.to_string(),
            ));
        }

        // 替换占位符
        let header_value = value_template.replace("{key}", api_key);

        Ok(AuthHeader::new(header_name.to_string(), header_value))
    }

    /// 分割头格式为名称和值模板
    fn split_header_format(format: &str) -> Result<(&str, &str), AuthParseError> {
        let trimmed_format = format.trim();
        if let Some((name, value)) = trimmed_format.split_once(": ") {
            Ok((name.trim(), value))
        } else {
            Err(AuthParseError::InvalidFormat(format.to_string()))
        }
    }


    /// 提取头名称（不包含值）
    pub fn extract_header_name(format: &str) -> Result<String, AuthParseError> {
        let (header_name, _) = Self::split_header_format(format)?;

        if header_name.trim().is_empty() {
            return Err(AuthParseError::EmptyHeaderName(format.to_string()));
        }

        Ok(header_name.to_lowercase())
    }


    /// 解析认证头格式数组，支持多种认证头格式
    ///
    /// # 参数
    /// - `formats_json`: JSON数组格式的认证头模板，如 `["Authorization: Bearer {key}", "X-goog-api-key: {key}"]`
    /// - `api_key`: 实际的API密钥
    ///
    /// # 返回
    /// - `Ok(Vec<AuthHeader>)`: 解析成功，返回所有认证头
    /// - `Err(AuthParseError)`: JSON解析失败或格式错误
    pub fn parse_multiple(formats_json: &str, api_key: &str) -> Result<Vec<AuthHeader>, AuthParseError> {
        // 尝试解析为JSON数组
        let formats: Vec<String> = serde_json::from_str(formats_json)
            .map_err(|_| AuthParseError::InvalidFormat(format!("Invalid JSON array: {}", formats_json)))?;

        let mut headers = Vec::new();
        for format in formats {
            let header = Self::parse(&format, api_key)?;
            headers.push(header);
        }

        Ok(headers)
    }

    /// 从JSON格式数组中提取所有头名称（用于请求解析）
    ///
    /// # 参数  
    /// - `formats_json`: JSON数组格式的认证头模板
    ///
    /// # 返回
    /// - `Ok(Vec<String>)`: 所有头名称（小写）
    /// - `Err(AuthParseError)`: JSON解析失败或格式错误
    pub fn extract_header_names_from_array(formats_json: &str) -> Result<Vec<String>, AuthParseError> {
        // 尝试解析为JSON数组
        let formats: Vec<String> = serde_json::from_str(formats_json)
            .map_err(|_| AuthParseError::InvalidFormat(format!("Invalid JSON array: {}", formats_json)))?;

        let mut header_names = Vec::new();
        for format in formats {
            let header_name = Self::extract_header_name(&format)?;
            header_names.push(header_name);
        }

        Ok(header_names)
    }

    /// 智能解析：自动检测是单一格式还是数组格式
    ///
    /// # 参数
    /// - `format_or_array`: 单一格式字符串或JSON数组
    /// - `api_key`: 实际的API密钥
    ///
    /// # 返回
    /// - `Ok(Vec<AuthHeader>)`: 解析成功的认证头列表
    /// - `Err(AuthParseError)`: 解析失败
    pub fn parse_smart(format_or_array: &str, api_key: &str) -> Result<Vec<AuthHeader>, AuthParseError> {
        // 先尝试作为JSON数组解析
        if let Ok(headers) = Self::parse_multiple(format_or_array, api_key) {
            return Ok(headers);
        }

        // 如果不是JSON数组，尝试作为单一格式解析
        let header = Self::parse(format_or_array, api_key)?;
        Ok(vec![header])
    }

    /// 从入站认证头值中解析API密钥（反向解析）
    ///
    /// 根据认证头格式模板，从实际的HTTP头值中提取API密钥
    /// 
    /// # 参数
    /// - `format`: 认证头格式模板，如 "Authorization: Bearer {key}"
    /// - `header_value`: 实际的HTTP头值，如 "Bearer sk-123456"
    ///
    /// # 返回
    /// - `Ok(String)`: 成功提取的API密钥
    /// - `Err(AuthParseError)`: 解析失败
    ///
    /// # 示例
    /// ```rust
    /// let api_key = AuthHeaderParser::parse_api_key_from_inbound_header_value(
    ///     "Authorization: Bearer {key}",
    ///     "Bearer sk-123456"
    /// ).unwrap();
    /// assert_eq!(api_key, "sk-123456");
    /// ```
    pub fn parse_api_key_from_inbound_header_value(
        format: &str,
        header_value: &str,
    ) -> Result<String, AuthParseError> {
        let (_, value_template) = Self::split_header_format(format)?;

        // 直接替换模式：{key}
        if value_template.trim() == "{key}" {
            return Ok(header_value.to_string());
        }

        // 前缀模式：处理 "Bearer {key}", "Token {key}" 等格式
        if let Some(prefix) = value_template.strip_suffix("{key}") {
            let prefix = prefix.trim();
            if let Some(key) = header_value.strip_prefix(prefix) {
                return Ok(key.trim().to_string()); // 去除提取的密钥两边的空格
            }
        }

        // 后缀模式：处理 "{key} suffix" 格式  
        if let Some(suffix) = value_template.strip_prefix("{key}") {
            let suffix = suffix.trim();
            if let Some(key) = header_value.strip_suffix(suffix) {
                return Ok(key.trim().to_string()); // 去除提取的密钥两边的空格
            }
        }

        // 中间模式：处理 "prefix {key} suffix" 格式
        if let Some(key_start) = value_template.find("{key}") {
            let prefix = &value_template[..key_start].trim();
            let suffix = &value_template[key_start + 5..].trim(); // 5 = len("{key}")
            
            if header_value.starts_with(prefix) && header_value.ends_with(suffix) {
                let key_start_pos = prefix.len();
                let key_end_pos = header_value.len() - suffix.len();
                if key_end_pos > key_start_pos {
                    return Ok(header_value[key_start_pos..key_end_pos].trim().to_string()); // 去除提取的密钥两边的空格
                }
            }
        }

        Err(AuthParseError::InvalidFormat(format!(
            "Could not extract API key from header value '{}' using format '{}'",
            header_value, format
        )))
    }

    /// 智能反向解析：从多种格式中提取API密钥
    ///
    /// 支持JSON数组格式和单一格式，自动匹配头名称并提取API密钥
    ///
    /// # 参数
    /// - `formats_json`: JSON数组或单一格式字符串
    /// - `header_name`: 实际的HTTP头名称（小写）
    /// - `header_value`: 实际的HTTP头值
    ///
    /// # 返回
    /// - `Ok(String)`: 成功提取的API密钥
    /// - `Err(AuthParseError)`: 解析失败
    pub fn parse_api_key_from_inbound_headers_smart(
        formats_json: &str,
        header_name: &str,
        header_value: &str,
    ) -> Result<String, AuthParseError> {
        // 尝试解析为JSON数组格式
        let formats: Vec<String> = match serde_json::from_str(formats_json) {
            Ok(formats) => formats,
            Err(_) => vec![formats_json.to_string()], // 单一格式回退
        };

        // 遍历所有格式，找到匹配的格式并提取密钥
        for format in formats {
            if let Ok(format_header_name) = Self::extract_header_name(&format) {
                if format_header_name == header_name {
                    // 找到匹配格式，进行反向解析
                    if let Ok(api_key) = Self::parse_api_key_from_inbound_header_value(&format, header_value) {
                        return Ok(api_key);
                    }
                }
            }
        }

        Err(AuthParseError::InvalidFormat(format!(
            "No matching auth format found for header '{}' in configured formats: {}",
            header_name, formats_json
        )))
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
        assert!(matches!(
            result,
            Err(AuthParseError::MissingKeyPlaceholder(_))
        ));
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
    fn test_multiple_key_replacements() {
        // 测试多个 {key} 占位符的情况
        let result =
            AuthHeaderParser::parse("X-Custom: prefix-{key}-suffix-{key}", "token123").unwrap();
        assert_eq!(result.name, "x-custom");
        assert_eq!(result.value, "prefix-token123-suffix-token123");
    }

    #[test]
    fn test_whitespace_handling() {
        let result = AuthHeaderParser::parse("  Authorization  :  Bearer {key}  ", "test").unwrap();
        assert_eq!(result.name, "authorization");
        assert_eq!(result.value, "Bearer test  "); // 保持值中的尾随空格
    }

    // 反向解析功能测试
    #[test]
    fn test_parse_api_key_from_inbound_header_value_direct() {
        let api_key = AuthHeaderParser::parse_api_key_from_inbound_header_value(
            "Authorization: {key}",
            "sk-123456789"
        ).unwrap();
        assert_eq!(api_key, "sk-123456789");
    }

    #[test]
    fn test_parse_api_key_from_inbound_header_value_bearer() {
        let api_key = AuthHeaderParser::parse_api_key_from_inbound_header_value(
            "Authorization: Bearer {key}",
            "Bearer sk-abcdef123"
        ).unwrap();
        assert_eq!(api_key, "sk-abcdef123");
    }

    #[test]
    fn test_parse_api_key_from_inbound_header_value_google() {
        let api_key = AuthHeaderParser::parse_api_key_from_inbound_header_value(
            "X-goog-api-key: {key}",
            "AIza_google_key_xyz"
        ).unwrap();
        assert_eq!(api_key, "AIza_google_key_xyz");
    }

    #[test]
    fn test_parse_api_key_from_inbound_header_value_custom_prefix() {
        let api_key = AuthHeaderParser::parse_api_key_from_inbound_header_value(
            "X-API-Key: Token {key}",
            "Token custom_token_456"
        ).unwrap();
        assert_eq!(api_key, "custom_token_456");
    }

    #[test]
    fn test_parse_api_key_from_inbound_headers_smart_single() {
        let api_key = AuthHeaderParser::parse_api_key_from_inbound_headers_smart(
            "Authorization: Bearer {key}",
            "authorization",
            "Bearer sk-test123"
        ).unwrap();
        assert_eq!(api_key, "sk-test123");
    }

    #[test]
    fn test_parse_api_key_from_inbound_headers_smart_json_array() {
        let formats_json = r#"["Authorization: Bearer {key}", "X-goog-api-key: {key}"]"#;
        
        // 测试第一种格式
        let api_key1 = AuthHeaderParser::parse_api_key_from_inbound_headers_smart(
            formats_json,
            "authorization",
            "Bearer sk-test456"
        ).unwrap();
        assert_eq!(api_key1, "sk-test456");
        
        // 测试第二种格式
        let api_key2 = AuthHeaderParser::parse_api_key_from_inbound_headers_smart(
            formats_json,
            "x-goog-api-key",
            "AIza_google_789"
        ).unwrap();
        assert_eq!(api_key2, "AIza_google_789");
    }

    #[test]
    fn test_parse_api_key_from_inbound_header_value_invalid_format() {
        let result = AuthHeaderParser::parse_api_key_from_inbound_header_value(
            "Authorization: Bearer {key}",
            "Token sk-123456" // 不匹配格式
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_api_key_from_inbound_headers_smart_no_matching_header() {
        let result = AuthHeaderParser::parse_api_key_from_inbound_headers_smart(
            "Authorization: Bearer {key}",
            "x-api-key", // 不匹配的头名称
            "Bearer sk-123456"
        );
        assert!(result.is_err());
    }
}
