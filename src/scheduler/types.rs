//! # API密钥调度器类型定义

use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// 调度策略枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SchedulingStrategy {
    /// 轮询调度
    RoundRobin,
    /// 权重调度
    Weighted,
    /// 健康度最佳调度
    HealthBest,
}

/// API密钥健康状态枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ApiKeyHealthStatus {
    /// 健康可用
    Healthy,
    /// 限流中
    RateLimited,
    /// 不健康 (包含原来的 unknown 和 error)
    Unhealthy,
}

impl ToString for ApiKeyHealthStatus {
    fn to_string(&self) -> String {
        match self {
            ApiKeyHealthStatus::Healthy => "healthy",
            ApiKeyHealthStatus::RateLimited => "rate_limited",
            ApiKeyHealthStatus::Unhealthy => "unhealthy",
        }.to_string()
    }
}

impl FromStr for ApiKeyHealthStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "healthy" => Ok(ApiKeyHealthStatus::Healthy),
            "rate_limited" => Ok(ApiKeyHealthStatus::RateLimited),
            "unhealthy" => Ok(ApiKeyHealthStatus::Unhealthy),
            _ => Err(format!("Invalid health status: {}", s)),
        }
    }
}

impl Default for SchedulingStrategy {
    fn default() -> Self {
        Self::RoundRobin
    }
}

impl SchedulingStrategy {
    /// 从字符串解析调度策略
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "round_robin" | "roundrobin" | "rr" => Some(Self::RoundRobin),
            "weighted" | "weight" | "w" => Some(Self::Weighted),
            "health_best" | "healthbest" | "health" | "hb" | "health_based" | "healthbased" => Some(Self::HealthBest),
            _ => None,
        }
    }

    /// 转换为字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RoundRobin => "round_robin",
            Self::Weighted => "weighted",
            Self::HealthBest => "health_best",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduling_strategy_parsing() {
        assert_eq!(
            SchedulingStrategy::from_str("round_robin"),
            Some(SchedulingStrategy::RoundRobin)
        );
        assert_eq!(
            SchedulingStrategy::from_str("weighted"),
            Some(SchedulingStrategy::Weighted)
        );
        assert_eq!(
            SchedulingStrategy::from_str("health_best"),
            Some(SchedulingStrategy::HealthBest)
        );
        assert_eq!(SchedulingStrategy::from_str("unknown"), None);
    }

    #[test]
    fn test_scheduling_strategy_as_str() {
        assert_eq!(SchedulingStrategy::RoundRobin.as_str(), "round_robin");
        assert_eq!(SchedulingStrategy::Weighted.as_str(), "weighted");
        assert_eq!(SchedulingStrategy::HealthBest.as_str(), "health_best");
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
}
