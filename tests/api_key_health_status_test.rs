//! # API密钥健康状态统一枚举测试
//!
//! 测试健康状态枚举的统一性和相关功能

use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// API密钥健康状态枚举（复制定义用于测试）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ApiKeyHealthStatus {
    /// 健康可用
    Healthy,
    /// 限流中
    RateLimited,
    /// 不健康 (包含原来的 unknown 和 error)
    Unhealthy,
}

impl fmt::Display for ApiKeyHealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Healthy => "healthy",
            Self::RateLimited => "rate_limited",
            Self::Unhealthy => "unhealthy",
        };
        f.write_str(text)
    }
}

impl FromStr for ApiKeyHealthStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "healthy" => Ok(Self::Healthy),
            "rate_limited" => Ok(Self::RateLimited),
            "unhealthy" => Ok(Self::Unhealthy),
            _ => Err(format!("Invalid health status: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_parsing() {
        // 测试有效状态解析
        assert_eq!(
            ApiKeyHealthStatus::from_str("healthy"),
            Ok(ApiKeyHealthStatus::Healthy)
        );
        assert_eq!(
            ApiKeyHealthStatus::from_str("rate_limited"),
            Ok(ApiKeyHealthStatus::RateLimited)
        );
        assert_eq!(
            ApiKeyHealthStatus::from_str("unhealthy"),
            Ok(ApiKeyHealthStatus::Unhealthy)
        );

        // 测试无效状态解析（unknown 和 error 都应该无法解析）
        assert!(ApiKeyHealthStatus::from_str("unknown").is_err());
        assert!(ApiKeyHealthStatus::from_str("error").is_err());
        assert!(ApiKeyHealthStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_health_status_to_string() {
        assert_eq!(ApiKeyHealthStatus::Healthy.to_string(), "healthy");
        assert_eq!(ApiKeyHealthStatus::RateLimited.to_string(), "rate_limited");
        assert_eq!(ApiKeyHealthStatus::Unhealthy.to_string(), "unhealthy");
    }

    #[test]
    fn test_health_status_roundtrip() {
        // 测试字符串和枚举之间的往返转换
        let statuses = vec![
            ApiKeyHealthStatus::Healthy,
            ApiKeyHealthStatus::RateLimited,
            ApiKeyHealthStatus::Unhealthy,
        ];

        for status in statuses {
            let string = status.to_string();
            let parsed = ApiKeyHealthStatus::from_str(&string).unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn test_legacy_status_mapping() {
        // 测试旧状态到新状态的映射
        let legacy_to_new = vec![
            ("healthy", Ok(ApiKeyHealthStatus::Healthy)),
            ("rate_limited", Ok(ApiKeyHealthStatus::RateLimited)),
            ("unhealthy", Ok(ApiKeyHealthStatus::Unhealthy)),
            ("error", Err("Invalid health status: error".to_string())), // error 归类为无法解析（实际归类为 unhealthy）
            ("unknown", Err("Invalid health status: unknown".to_string())), // unknown 归类为无法解析（实际归类为 unhealthy）
        ];

        for (legacy, expected) in legacy_to_new {
            let result = ApiKeyHealthStatus::from_str(legacy);
            match expected {
                Ok(expected_status) => {
                    assert_eq!(result.unwrap(), expected_status);
                }
                Err(expected_err) => {
                    assert_eq!(result.unwrap_err(), expected_err);
                }
            }
        }
    }

    #[test]
    fn test_status_filtering() {
        // 测试状态筛选逻辑
        let test_keys = vec![
            ("key1", "healthy", true),
            ("key2", "rate_limited", false),
            ("key3", "unhealthy", false),
            ("key4", "error", false),   // error 归类为不健康
            ("key5", "unknown", false), // unknown 归类为不健康
        ];

        for (key_name, health_status, expected_healthy) in test_keys {
            let status_result = ApiKeyHealthStatus::from_str(health_status);
            // 仅 Healthy 视为真正健康，其余（含限流、错误与未知）均视为不健康
            let is_healthy = matches!(status_result, Ok(ApiKeyHealthStatus::Healthy));

            assert_eq!(
                is_healthy, expected_healthy,
                "Key {key_name} with status {health_status} should be healthy: {expected_healthy}"
            );
        }
    }

    #[test]
    fn test_status_count() {
        // 确保我们有且只有三个状态
        let statuses = [
            ApiKeyHealthStatus::Healthy,
            ApiKeyHealthStatus::RateLimited,
            ApiKeyHealthStatus::Unhealthy,
        ];
        assert_eq!(statuses.len(), 3);
    }

    #[test]
    fn test_status_serialization() {
        // 测试序列化和反序列化
        let statuses = vec![
            ApiKeyHealthStatus::Healthy,
            ApiKeyHealthStatus::RateLimited,
            ApiKeyHealthStatus::Unhealthy,
        ];

        for status in statuses {
            // 序列化为 JSON
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: ApiKeyHealthStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, deserialized);

            // 序列化为字符串再解析
            let string = status.to_string();
            let parsed = ApiKeyHealthStatus::from_str(&string).unwrap();
            assert_eq!(status, parsed);
        }
    }
}
