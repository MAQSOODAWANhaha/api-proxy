//! # 负载均衡调度器模块
//!
//! 实现多种负载均衡算法，包括轮询、权重和健康度最佳调度

pub mod algorithms;
pub mod balancer;
pub mod manager;
pub mod types;

pub use algorithms::{
    HealthBasedScheduler, RoundRobinScheduler, SchedulingAlgorithm, WeightedScheduler,
};
pub use balancer::LoadBalancer;
pub use manager::LoadBalancerManager;
pub use types::{SchedulingStrategy, ServerMetrics};
