//! # Pingora 代理服务模块
//!
//! 实现基于 Pingora 的高性能代理服务

pub mod types;
pub mod server;
pub mod service;
pub mod statistics;
// 中间件功能已集成到RequestHandler中
pub mod request_handler;
pub mod provider_adapter;
pub mod request_forwarder;
pub mod builder;
pub mod pingora_proxy;
pub mod authentication_service;
pub mod statistics_service;
pub mod tracing_service;
pub use request_handler::{RequestHandler, ProxyContext};
pub use provider_adapter::ProviderAdapter;
pub use request_forwarder::RequestForwarder;
pub use builder::{ProxyServerBuilder, ProxyServerComponents};
pub use types::{ProviderId, ForwardingContext, ForwardingResult};
pub use pingora_proxy::PingoraProxyServer;
pub use server::ProxyServer;
pub use service::ProxyService;
pub use statistics::{StatisticsCollector, StatisticsConfig, StatsSummary};
pub use authentication_service::AuthenticationService;
pub use statistics_service::{
    StatisticsService, TokenUsage, RequestDetails, ResponseDetails, 
    SerializableResponseDetails, DetailedRequestStats
};
pub use tracing_service::{TracingService, TracingContextHelper};
