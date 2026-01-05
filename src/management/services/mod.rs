//! # 管理端服务层
//!
//! 聚合各管理域的业务逻辑，供 HTTP handler、定时任务或其它入口复用。
//! 此模块不仅暴露各领域 service，还统一导出常用的共享工具，便于调用方组合使用。

pub mod auth;
pub mod logs;
pub mod oauth_v2;
pub mod provider_keys;
pub mod provider_types;
pub mod service_apis;
pub mod shared;
pub mod statistics;
pub mod stats_public;
pub mod system;
pub mod users;

pub use auth::AuthManagementService;
pub use logs::LogsService;
pub use oauth_v2::{
    OAuthProviderSummary, OAuthSessionInfoWithTimezone, OAuthV2AuthorizeRequest,
    OAuthV2ExchangeRequest, OAuthV2PollQuery, OAuthV2Service,
};
pub use provider_keys::ProviderKeyService;
pub use provider_keys::{
    CreateProviderKeyRequest, ProviderKeysListQuery, TrendQuery, UpdateProviderKeyRequest,
    UserProviderKeyQuery,
};
pub use provider_types::{
    CreateProviderTypeRequest, ProviderTypesCrudService, UpdateProviderTypeRequest,
};
pub use service_apis::ServiceApiService;
pub use statistics::StatisticsService;
pub use stats_public::StatsService;
pub use users::UsersService;

pub use shared::{
    PaginationInfo, PaginationParams, ServiceResponse, TimeRangeBounds, TimeRangeDefault,
    build_page, resolve_range,
};
