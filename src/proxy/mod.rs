//! # Proxy 模块（Pingora 代理）
//!
//! 职责清晰、最小可用：
//! - `service`: 实现 Pingora `ProxyHttp` 全流程（请求阶段编排、上游处理、响应收集）
//! - `server`/`pingora_proxy` + `builder`: 服务装配与启动
//! - `request_handler`: 纯业务能力（选择上游、请求/响应改写、统计提取等）
//! - `authentication_service`/`tracing_service`：关注点分离的协作服务（统计服务已迁移至 `src/statistics/`）
//! - `provider_strategy`: 提供商特定的最小策略扩展（如 Gemini 注入）
//! - `types`: 轻量通用类型
//! - `logging`: 统一日志工具和格式标准

pub mod service;
pub mod types;
// 统一通过 `ProxyService` 调度；`RequestHandler` 提供纯业务方法
pub mod request_handler;
// 旧式适配器/转发器模块暂不编译，待全面评估后决定是否删除
pub mod authentication_service;
pub mod builder;
pub mod pingora_proxy;
pub mod provider_strategy;
pub mod tracing_service;
// 执行流：通过多个步骤 Service 组合，由总服务顺序编排
pub use crate::statistics::service::{RequestStats, ResponseStats, StatisticsService};
pub use authentication_service::AuthenticationService;
pub use builder::{ProxyServerBuilder, ProxyServerComponents};
pub use pingora_proxy::PingoraProxyServer;
pub use request_handler::{
    DetailedRequestStats, RequestDetails, ResponseDetails, SerializableResponseDetails, TokenUsage,
};
pub use request_handler::{ProxyContext, RequestHandler};
pub use service::ProxyService;
pub use tracing_service::{TracingContextHelper, TracingService};
pub use types::{ForwardingContext, ForwardingResult, ProviderId};

/// 统一导入集合（建议上层使用）
pub mod prelude {
    pub use super::authentication_service::AuthenticationService;
    pub use super::provider_strategy::{ProviderRegistry, ProviderStrategy, make_strategy};
    pub use super::request_handler::{
        DetailedRequestStats, RequestDetails, ResponseDetails, SerializableResponseDetails,
        TokenUsage,
    };
    pub use super::request_handler::{ProxyContext, RequestHandler};
    pub use super::tracing_service::{TracingContextHelper, TracingService};
    pub use super::types::{ForwardingContext, ForwardingResult, ProviderId};
    pub use super::{PingoraProxyServer, ProxyService};
    pub use crate::statistics::service::{RequestStats, ResponseStats, StatisticsService};
}
