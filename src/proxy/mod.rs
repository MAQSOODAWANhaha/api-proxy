//! # Pingora 代理服务模块
//!
//! 实现基于 Pingora 的高性能代理服务

pub mod forwarding;
pub mod server;
pub mod service;
pub mod statistics;
pub mod upstream;
// pub mod middleware;  // TODO: 修复中间件实现
pub mod ai_handler;
pub mod builder;
pub mod pingora_proxy;
pub mod provider_resolver;

pub use ai_handler::{AIProxyHandler, ProxyContext};
pub use builder::{ProxyServerBuilder, ProxyServerComponents};
pub use forwarding::{ForwardingConfig, ForwardingContext, ForwardingResult, RequestForwarder};
pub use pingora_proxy::PingoraProxyServer;
pub use provider_resolver::ProviderResolver;
pub use server::ProxyServer;
pub use service::ProxyService;
pub use statistics::{StatisticsCollector, StatisticsConfig, StatsSummary};
