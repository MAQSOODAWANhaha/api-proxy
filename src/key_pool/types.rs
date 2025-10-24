//! # API密钥调度器类型定义

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// 调度策略枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SchedulingStrategy {
    /// 轮询调度
    RoundRobin,
    /// 权重调度
    Weighted,
}

/// API密钥健康状态枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ApiKeyHealthStatus {
    /// 健康可用
    Healthy,
    /// 限流中
    RateLimited,
    /// 不健康 (包含原来的 unknown 和 error)
    Unhealthy,
}

impl<'de> serde::Deserialize<'de> for ApiKeyHealthStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ApiKeyHealthStatusVisitor;

        impl serde::de::Visitor<'_> for ApiKeyHealthStatusVisitor {
            type Value = ApiKeyHealthStatus;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string representing API key health status")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match s {
                    "healthy" => Ok(ApiKeyHealthStatus::Healthy),
                    "rate_limited" => Ok(ApiKeyHealthStatus::RateLimited),
                    "unhealthy" => Ok(ApiKeyHealthStatus::Unhealthy),
                    _ => Err(E::custom(format!("unknown health status: {s}"))),
                }
            }
        }

        deserializer.deserialize_str(ApiKeyHealthStatusVisitor)
    }
}

impl serde::Serialize for ApiKeyHealthStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            Self::Healthy => "healthy",
            Self::RateLimited => "rate_limited",
            Self::Unhealthy => "unhealthy",
        };
        serializer.serialize_str(s)
    }
}

impl fmt::Display for ApiKeyHealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::RateLimited => write!(f, "rate_limited"),
            Self::Unhealthy => write!(f, "unhealthy"),
        }
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

impl Default for SchedulingStrategy {
    fn default() -> Self {
        Self::RoundRobin
    }
}

impl FromStr for SchedulingStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "round_robin" | "roundrobin" | "rr" => Ok(Self::RoundRobin),
            "weighted" | "weight" | "w" => Ok(Self::Weighted),
            _ => Err(format!("Unknown scheduling strategy: {s}")),
        }
    }
}

impl SchedulingStrategy {
    /// 从字符串解析调度策略
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    /// 转换为字符串
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::RoundRobin => "round_robin",
            Self::Weighted => "weighted",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduling_strategy_parsing() {
        assert_eq!(
            SchedulingStrategy::parse("round_robin"),
            Some(SchedulingStrategy::RoundRobin)
        );
        assert_eq!(
            SchedulingStrategy::parse("weighted"),
            Some(SchedulingStrategy::Weighted)
        );
        assert_eq!(SchedulingStrategy::parse("unknown"), None);
    }

    #[test]
    fn test_scheduling_strategy_as_str() {
        assert_eq!(SchedulingStrategy::RoundRobin.as_str(), "round_robin");
        assert_eq!(SchedulingStrategy::Weighted.as_str(), "weighted");
    }

    #[test]
    fn test_api_key_health_status_parsing() {
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
        assert!(ApiKeyHealthStatus::from_str("unknown").is_err());
        assert!(ApiKeyHealthStatus::from_str("error").is_err());
    }

    #[test]
    fn test_api_key_health_status_to_string() {
        assert_eq!(ApiKeyHealthStatus::Healthy.to_string(), "healthy");
        assert_eq!(ApiKeyHealthStatus::RateLimited.to_string(), "rate_limited");
        assert_eq!(ApiKeyHealthStatus::Unhealthy.to_string(), "unhealthy");
    }

    #[test]
    fn test_api_key_health_status_serde_deserialization() {
        // 测试反序列化：只支持 rate_limited
        let json1 = r#""rate_limited""#;
        let status1: ApiKeyHealthStatus = serde_json::from_str(json1).unwrap();
        assert_eq!(status1, ApiKeyHealthStatus::RateLimited);

        let json2 = r#""healthy""#;
        let status2: ApiKeyHealthStatus = serde_json::from_str(json2).unwrap();
        assert_eq!(status2, ApiKeyHealthStatus::Healthy);

        let json3 = r#""unhealthy""#;
        let status3: ApiKeyHealthStatus = serde_json::from_str(json3).unwrap();
        assert_eq!(status3, ApiKeyHealthStatus::Unhealthy);

        // 测试不支持 ratelimited
        let json4 = r#""ratelimited""#;
        let result4: Result<ApiKeyHealthStatus, _> = serde_json::from_str(json4);
        assert!(result4.is_err());
    }

    #[test]
    fn test_api_key_health_status_serde_serialization() {
        // 测试序列化：总是输出 rate_limited
        let status = ApiKeyHealthStatus::RateLimited;
        let serialized = serde_json::to_string(&status).unwrap();
        assert_eq!(serialized, "\"rate_limited\"");

        let status = ApiKeyHealthStatus::Healthy;
        let serialized = serde_json::to_string(&status).unwrap();
        assert_eq!(serialized, "\"healthy\"");

        let status = ApiKeyHealthStatus::Unhealthy;
        let serialized = serde_json::to_string(&status).unwrap();
        assert_eq!(serialized, "\"unhealthy\"");
    }
}
