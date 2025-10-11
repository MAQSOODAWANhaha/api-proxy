//! # 代理模块 (Pingora Proxy)
//!
//! ## 核心设计理念
//!
//! 本模块采用基于“服务-上下文-策略”的清晰分层架构，遵循单一职责原则，旨在实现一个高性能、可扩展、易于维护的AI服务代理。
//!
//! ### 1. 服务编排 (Orchestration) - `service.rs`
//!
//! - **`ProxyService`**: 作为核心编排器，它实现了 Pingora 的 `ProxyHttp` trait。它不包含任何业务逻辑，
//!   唯一的职责是在请求生命周期的各个阶段（如 `early_request_filter`, `upstream_request_filter`）
//!   调用相应的专有服务。它持有所有服务的实例，并通过依赖注入进行组装。
//!
//! ### 2. 专有服务 (Specialized Services)
//!
//! 每个服务都封装了一组特定的业务能力，职责清晰：
//!
//! - **`authentication_service.rs`**: **认证与授权中心**。负责从请求中提取凭证、验证用户API Key、
//!   选择后端密钥池、解析最终上游凭证（API Key/OAuth）、以及执行速率限制和配额检查。
//!
//! - **`upstream_service.rs`**: **上游管理中心**。负责根据服务商策略选择正确的上游主机地址，
//!   并配置连接参数（如超时、TLS、HTTP/2）。
//!
//! - **`request_transform_service.rs`**: **请求转换器**。负责在请求发往上游前对其进行修改，
//!   包括：注入正确的认证头、根据 `ProviderStrategy` 改写路径或请求体、清理代理痕迹。
//!
//! - **`response_transform_service.rs`**: **响应转换器**。负责修改从上游返回的响应头，
//!   例如添加CORS头、移除敏感信息。
//!
//! - **`statistics_service.rs` (`src/statistics/`)**: **统计与计费中心**。负责从请求和响应中
//!   提取`model`和`token`使用量，并调用 `PricingService` 计算费用。
//!
//! - **`tracing_service.rs`**: **分布式追踪中心**。管理请求从开始到结束的完整链路追踪记录。
//!
//! ### 3. 上下文 (Context) - `context.rs`
//!
//! - **`ProxyContext`**: 一个纯粹的状态传递对象。它像一个“行李箱”，在 `ProxyService` 编排的
//!   各个服务之间传递。每个服务执行完毕后，将其结果（如认证信息、选择的上游等）放入 `ProxyContext`，
//!   供后续服务使用。
//!
//! ### 4. 策略 (Strategy) - `provider_strategy/`
//!
//! - **`ProviderStrategy` Trait**: 定义了服务商特有的行为接口（如 `modify_request`, `build_auth_headers`）。
//! - **具体实现 (e.g., `GeminiStrategy`, `OpenAIStrategy`)**: 封装了针对特定服务商（如Google Gemini, `OpenAI`）
//!   的定制化逻辑，例如请求体注入、特殊错误处理等。这使得核心代理逻辑保持通用，易于扩展以支持新的AI服务。
//!
//! ## 数据流
//!
//! `Client` -> `Pingora` -> `ProxyService` -> `AuthenticationService` -> `UpstreamService` -> `RequestTransformService` -> `Upstream`
//! `Client` <- `Pingora` <- `ProxyService` <- `ResponseTransformService` <- `StatisticsService` <- `TracingService` <- `Upstream`
//!

pub mod context;
pub mod service;
pub mod types;

// 专有服务
pub mod authentication_service;
pub mod builder;
pub mod pingora_proxy;
pub mod provider_strategy;
pub mod request_transform_service;
pub mod response_transform_service;
pub mod tracing_service;
pub mod upstream_service;

// 统一导出
pub use crate::statistics::service::StatisticsService;
pub use authentication_service::AuthenticationService;
pub use builder::{ProxyServerBuilder, ProxyServerComponents};
pub use context::ProxyContext;
pub use pingora_proxy::PingoraProxyServer;
pub use request_transform_service::RequestTransformService;
pub use response_transform_service::ResponseTransformService;
pub use service::ProxyService;
pub use tracing_service::{TracingContextHelper, TracingService};
pub use types::{ForwardingContext, ForwardingResult, ProviderId};
pub use upstream_service::UpstreamService;

/// 统一导入集合（建议上层使用）
pub mod prelude {
    pub use super::authentication_service::AuthenticationService;
    pub use super::context::ProxyContext;
    pub use super::provider_strategy::{ProviderRegistry, ProviderStrategy, make_strategy};
    pub use super::tracing_service::{TracingContextHelper, TracingService};
    pub use super::types::{ForwardingContext, ForwardingResult, ProviderId};
    pub use super::{
        PingoraProxyServer, ProxyService, RequestTransformService, ResponseTransformService,
        UpstreamService,
    };
    pub use crate::statistics::service::StatisticsService;
}
