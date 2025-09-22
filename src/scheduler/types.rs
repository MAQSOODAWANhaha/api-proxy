//! # API密钥调度器类型定义

use serde::{Deserialize, Serialize};

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
}
