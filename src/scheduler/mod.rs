//! # 负载均衡调度器模块
//! 
//! 实现多种负载均衡算法，包括轮询、权重和健康度最佳调度

pub mod balancer;
pub mod algorithms;
pub mod types;
pub mod manager;

pub use balancer::LoadBalancer;
pub use algorithms::{SchedulingAlgorithm, RoundRobinScheduler, WeightedScheduler, HealthBasedScheduler};
pub use types::{ServerMetrics, SchedulingStrategy};
pub use manager::LoadBalancerManager;