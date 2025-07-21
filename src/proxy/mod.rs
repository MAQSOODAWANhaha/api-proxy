//! # Pingora 代理服务模块
//!
//! 实现基于 Pingora 的高性能代理服务

pub mod server;
pub mod service;
pub mod upstream;
pub mod router;
pub mod forwarding;
pub mod statistics;
// pub mod middleware;  // TODO: 修复中间件实现
pub mod pingora_proxy;

pub use server::ProxyServer;
pub use service::ProxyService;
pub use router::{SmartRouter, RouteRule, RouteDecision};
pub use forwarding::{RequestForwarder, ForwardingContext, ForwardingConfig, ForwardingResult};
pub use statistics::{StatisticsCollector, StatisticsConfig, StatsSummary};
pub use pingora_proxy::PingoraProxyServer;
