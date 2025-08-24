//! # Pingora 代理服务模块
//!
//! 实现基于 Pingora 的高性能代理服务

pub mod types;
pub mod server;
pub mod service;
pub mod statistics;
// pub mod middleware;  // TODO: 修复中间件实现
pub mod ai_handler;
pub mod request_handler;
pub mod provider_adapter;
pub mod request_forwarder;
pub mod builder;
pub mod pingora_proxy;
pub use ai_handler::{AIProxyHandler, ProxyContext};
pub use request_handler::{RequestHandler, RequestContext};
pub use provider_adapter::ProviderAdapter;
pub use request_forwarder::RequestForwarder;
pub use builder::{ProxyServerBuilder, ProxyServerComponents};
pub use types::{ProviderId, ForwardingContext, ForwardingResult};
pub use pingora_proxy::PingoraProxyServer;
pub use server::ProxyServer;
pub use service::ProxyService;
pub use statistics::{StatisticsCollector, StatisticsConfig, StatsSummary};
