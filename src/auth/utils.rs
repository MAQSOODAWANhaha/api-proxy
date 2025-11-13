//! # 认证工具函数
//!
//! 提供管理端和代理端共享的认证相关工具函数

use axum::http::HeaderMap;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// 认证工具类 - 提供管理端和代理端共享的基础功能
pub struct AuthUtils;

impl AuthUtils {
    /// 净化API密钥用于日志记录（管理端和代理端共享）
    ///
    /// # 参数
    /// - `api_key`: 原始API密钥
    ///
    /// # 返回
    /// 脱敏后的API密钥字符串，格式: "sk-1***2345"
    #[must_use]
    pub fn sanitize_api_key(api_key: &str) -> String {
        if api_key.len() > 10 {
            format!("{}***{}", &api_key[..4], &api_key[api_key.len() - 4..])
        } else {
            "***".to_string()
        }
    }

    /// `从HTTP头中提取Authorization头的值`
    ///
    /// # 参数
    /// - `headers`: HTTP请求头
    ///
    /// # 返回
    /// - `Some(String)`: Authorization头的值
    /// - `None`: 未找到Authorization头
    #[must_use]
    pub fn extract_authorization_header(headers: &HeaderMap) -> Option<String> {
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
    }

    /// `从Authorization头中提取Bearer` token
    ///
    /// # 参数
    /// - `auth_header`: Authorization头的完整值，如 "Bearer eyJ..."
    ///
    /// # 返回
    /// - `Some(String)`: Bearer token部分
    /// - `None`: 不是Bearer类型的认证头
    #[must_use]
    pub fn extract_bearer_token(auth_header: &str) -> Option<String> {
        if auth_header.starts_with("Bearer ") && auth_header.len() > 7 {
            Some(auth_header[7..].to_string())
        } else {
            None
        }
    }

    /// 从HTTP头中提取各种API Key
    ///
    /// 支持的头格式:
    /// - `Authorization: Bearer <key>`
    /// - `Authorization: <key>` (直接key)
    /// - `X-API-Key: <key>`
    /// - `API-Key: <key>`
    ///
    /// # 参数
    /// - `headers`: HTTP请求头
    ///
    /// # 返回
    /// - `Some(String)`: 提取的API Key
    /// - `None`: 未找到任何API Key
    #[must_use]
    pub fn extract_api_key_from_headers(headers: &HeaderMap) -> Option<String> {
        // 1. 尝试从 Authorization 头提取
        if let Some(auth_value) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
            // Bearer token
            if let Some(token) = Self::extract_bearer_token(auth_value) {
                return Some(token);
            }
            // 直接的API Key
            if !auth_value.is_empty() {
                return Some(auth_value.to_owned());
            }
        }

        // 2. 尝试从 X-API-Key 头提取
        if let Some(api_key) = headers
            .get("x-api-key")
            .and_then(|v| v.to_str().ok())
            .filter(|s| !s.is_empty())
        {
            return Some(api_key.to_owned());
        }

        // 3. 尝试从 API-Key 头提取
        if let Some(api_key) = headers
            .get("api-key")
            .and_then(|v| v.to_str().ok())
            .filter(|s| !s.is_empty())
        {
            return Some(api_key.to_owned());
        }

        None
    }

    /// `解析URL查询参数为HashMap`
    ///
    /// # 参数
    /// - `query_string`: URL查询字符串，不包含'?'
    ///
    /// # 返回
    /// 解析后的键值对映射
    #[must_use]
    pub fn parse_query_string(query_string: &str) -> HashMap<String, String> {
        let mut params = HashMap::new();

        for param in query_string.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                // URL解码
                let decoded_key = urlencoding::decode(key).unwrap_or_else(|_| key.into());
                let decoded_value = urlencoding::decode(value).unwrap_or_else(|_| value.into());
                params.insert(decoded_key.to_string(), decoded_value.to_string());
            }
        }

        params
    }

    /// `从路径中提取查询参数`
    ///
    /// # 参数
    /// - `path`: 包含查询参数的完整路径，如 "/api/chat?model=gpt-4"
    ///
    /// # 返回
    /// 解析后的查询参数映射
    #[must_use]
    pub fn extract_query_params_from_path(path: &str) -> HashMap<String, String> {
        path.find('?').map_or_else(HashMap::new, |query_start| {
            let query = &path[query_start + 1..];
            Self::parse_query_string(query)
        })
    }

    /// `验证API Key的基本格式`
    ///
    /// # 参数
    /// - `api_key`: 要验证的API Key
    ///
    /// # 返回
    /// - `true`: 格式有效（以sk-开头，长度>=20字符）
    /// - `false`: 格式无效
    #[must_use]
    pub fn is_valid_api_key_format(api_key: &str) -> bool {
        // 基础格式检查：以sk-开头且至少20字符
        api_key.starts_with("sk-") && api_key.len() >= 20
    }

    /// `生成缓存键`
    ///
    /// 为认证相关的数据生成标准化的缓存键
    ///
    /// # 参数
    /// - `prefix`: 缓存键前缀，如 `"jwt"`, `"api_key"`
    /// - `identifier`: 标识符，如用户ID、API Key hash等
    ///
    /// # 返回
    /// 标准化的缓存键
    #[must_use]
    pub fn generate_cache_key(prefix: &str, identifier: &str) -> String {
        format!("auth:{prefix}:{identifier}")
    }

    /// `计算字符串的SHA256哈希值`
    ///
    /// 用于API Key或Token的哈希，用于缓存键或记录
    ///
    /// # 参数
    /// - `input`: 要哈希的字符串
    ///
    /// # 返回
    /// 十六进制格式的哈ashi值
    #[must_use]
    pub fn sha256_hash(input: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// `哈希用户凭据`
    ///
    /// # 参数
    /// - `username`: 用户名
    /// - `password`: 密码
    ///
    /// # 返回
    /// 十六进制格式的哈希值
    #[must_use]
    pub fn hash_credentials(username: &str, password: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{username}:{password}").as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// `从请求中提取客户端IP地址（考虑代理）`
    ///
    /// 按优先级检查以下头部：
    /// 1. X-Forwarded-For (第一个IP)
    /// 2. X-Real-IP
    /// 3. CF-Connecting-IP (Cloudflare)
    /// 4. 直接连接IP
    ///
    /// # 参数
    /// - `headers`: HTTP请求头
    /// - `connection_ip`: 直接连接的客户端IP
    ///
    /// # 返回
    /// 真实的客户端IP地址
    #[must_use]
    pub fn extract_real_client_ip(headers: &HeaderMap, connection_ip: Option<String>) -> String {
        // 1. 优先检查 X-Forwarded-For 头
        if let Some(forwarded_for) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            // X-Forwarded-For 可能包含多个IP，取第一个（最原始的客户端IP）
            if let Some(first_ip) = forwarded_for.split(',').next() {
                let ip = first_ip.trim();
                if !ip.is_empty() && ip != "unknown" {
                    return ip.to_owned();
                }
            }
        }

        // 2. 检查 X-Real-IP 头
        if let Some(real_ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
            let ip = real_ip.trim();
            if !ip.is_empty() && ip != "unknown" {
                return ip.to_owned();
            }
        }

        // 3. 检查 CF-Connecting-IP (Cloudflare)
        if let Some(cf_ip) = headers
            .get("cf-connecting-ip")
            .and_then(|v| v.to_str().ok())
        {
            let ip = cf_ip.trim();
            if !ip.is_empty() && ip != "unknown" {
                return ip.to_owned();
            }
        }

        // 4. 最后使用直接连接的客户端地址
        connection_ip.unwrap_or_else(|| "unknown".to_owned())
    }

    /// `提取User-Agent字符串`
    ///
    /// # 参数
    /// - `headers`: HTTP请求头
    ///
    /// # 返回
    /// - `Some(String)`: User-Agent字符串
    /// - `None`: 未找到User-Agent头
    #[must_use]
    pub fn extract_user_agent(headers: &HeaderMap) -> Option<String> {
        headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    }

    /// `提取Referer字符串`
    ///
    /// 支持 "referer" 和 "referrer" 两种拼写
    ///
    /// # 参数
    /// - `headers`: HTTP请求头
    ///
    /// # 返回
    /// - `Some(String)`: Referer字符串
    /// - `None`: 未找到Referer头
    #[must_use]
    pub fn extract_referer(headers: &HeaderMap) -> Option<String> {
        headers
            .get("referer")
            .or_else(|| headers.get("referrer")) // 支持两种拼写
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    }

    /// `净化用户名用于日志记录（管理端和代理端共享）`
    ///
    /// # 参数
    /// - `username`: 原始用户名
    ///
    /// # 返回
    /// 脱敏后的用户名字符串，用于安全日志记录
    #[must_use]
    pub fn sanitize_username(username: &str) -> String {
        if username.len() > 6 {
            format!("{}***{}", &username[..2], &username[username.len() - 2..])
        } else if username.len() > 2 {
            format!("{}***", &username[..1])
        } else {
            "***".to_string()
        }
    }

    /// Sanitize token for logging
    #[must_use]
    pub fn sanitize_token_for_logging(token: &str) -> String {
        if token.len() > 20 {
            format!("{}***{}", &token[..8], &token[token.len() - 8..])
        } else if token.len() > 8 {
            format!("{}***", &token[..4])
        } else {
            "***".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderName, HeaderValue};

    fn create_test_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_str("Bearer sk-test1234567890abcdef").unwrap(),
        );
        headers.insert(
            HeaderName::from_static("x-forwarded-for"),
            HeaderValue::from_str("203.0.113.1, 198.51.100.1").unwrap(),
        );
        headers.insert(
            HeaderName::from_static("user-agent"),
            HeaderValue::from_str("TestClient/1.0").unwrap(),
        );
        headers
    }

    #[test]
    fn test_sanitize_api_key() {
        assert_eq!(
            AuthUtils::sanitize_api_key("sk-1234567890abcdef12345"),
            "sk-1***2345"
        );
        assert_eq!(AuthUtils::sanitize_api_key("short"), "***");
        assert_eq!(AuthUtils::sanitize_api_key(""), "***");
    }

    #[test]
    fn test_extract_bearer_token() {
        assert_eq!(
            AuthUtils::extract_bearer_token("Bearer sk-test123"),
            Some("sk-test123".to_string())
        );
        assert_eq!(AuthUtils::extract_bearer_token("Basic user:pass"), None);
        assert_eq!(AuthUtils::extract_bearer_token("Bearer "), None);
        assert_eq!(AuthUtils::extract_bearer_token(""), None);
    }

    #[test]
    fn test_extract_api_key_from_headers() {
        let headers = create_test_headers();
        let api_key = AuthUtils::extract_api_key_from_headers(&headers);
        assert_eq!(api_key, Some("sk-test1234567890abcdef".to_string()));
    }

    #[test]
    fn test_parse_query_string() {
        let params = AuthUtils::parse_query_string("model=gpt-4&stream=true&temperature=0.7");

        assert_eq!(params.get("model"), Some(&"gpt-4".to_string()));
        assert_eq!(params.get("stream"), Some(&"true".to_string()));
        assert_eq!(params.get("temperature"), Some(&"0.7".to_string()));
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_extract_query_params_from_path() {
        let params = AuthUtils::extract_query_params_from_path("/api/chat?model=gpt-4&stream=true");

        assert_eq!(params.get("model"), Some(&"gpt-4".to_string()));
        assert_eq!(params.get("stream"), Some(&"true".to_string()));

        let no_params = AuthUtils::extract_query_params_from_path("/api/chat");
        assert!(no_params.is_empty());
    }

    #[test]
    fn test_is_valid_api_key_format() {
        assert!(AuthUtils::is_valid_api_key_format(
            "sk-1234567890abcdef12345"
        ));
        assert!(!AuthUtils::is_valid_api_key_format("invalid-key"));
        assert!(!AuthUtils::is_valid_api_key_format("sk-short"));
        assert!(!AuthUtils::is_valid_api_key_format(
            "ak-1234567890abcdef12345"
        ));
    }

    #[test]
    fn test_generate_cache_key() {
        let key = AuthUtils::generate_cache_key("jwt", "user123");
        assert_eq!(key, "auth:jwt:user123");

        let api_key = AuthUtils::generate_cache_key("api_key", "sk-test123");
        assert_eq!(api_key, "auth:api_key:sk-test123");
    }

    #[test]
    fn test_sha256_hash() {
        let hash1 = AuthUtils::sha256_hash("test");
        let hash2 = AuthUtils::sha256_hash("test");
        let hash3 = AuthUtils::sha256_hash("different");

        // 相同输入应产生相同哈希
        assert_eq!(hash1, hash2);
        // 不同输入应产生不同哈希
        assert_ne!(hash1, hash3);
        // 哈希长度应为64字符（256位的十六进制表示）
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_extract_real_client_ip() {
        let headers = create_test_headers();
        let ip = AuthUtils::extract_real_client_ip(&headers, Some("127.0.0.1".to_string()));
        // 应该返回X-Forwarded-For头中的第一个IP
        assert_eq!(ip, "203.0.113.1");

        // 测试没有代理头的情况
        let empty_headers = HeaderMap::new();
        let direct_ip =
            AuthUtils::extract_real_client_ip(&empty_headers, Some("192.168.1.1".to_string()));
        assert_eq!(direct_ip, "192.168.1.1");
    }

    #[test]
    fn test_extract_user_agent() {
        let headers = create_test_headers();
        let user_agent = AuthUtils::extract_user_agent(&headers);
        assert_eq!(user_agent, Some("TestClient/1.0".to_string()));

        let empty_headers = HeaderMap::new();
        let no_user_agent = AuthUtils::extract_user_agent(&empty_headers);
        assert_eq!(no_user_agent, None);
    }

    #[test]
    fn test_sha256_hash_consistency() {
        let token = "test_token_123";
        let hash1 = AuthUtils::sha256_hash(token);
        let hash2 = AuthUtils::sha256_hash(token);

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA256 produces a 64-character hex string
    }

    #[test]
    fn test_hash_credentials_consistency() {
        let username = "user";
        let password = "pass";

        let hash1 = AuthUtils::hash_credentials(username, password);
        let hash2 = AuthUtils::hash_credentials(username, password);

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);

        // Different credentials should produce different hashes
        let different_hash = AuthUtils::hash_credentials("other", "pass");
        assert_ne!(hash1, different_hash);
    }
}
