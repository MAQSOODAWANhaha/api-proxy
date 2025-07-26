//! # Entity 模块
//!
//! 包含所有 Sea-ORM 实体定义

pub mod users;
pub mod user_sessions;
pub mod user_audit_logs;
pub mod provider_types;
pub mod user_provider_keys;
pub mod user_service_apis;
pub mod api_health_status;
pub mod proxy_tracing;
pub mod daily_statistics;

pub use users::Entity as Users;
pub use user_sessions::Entity as UserSessions;
pub use user_audit_logs::Entity as UserAuditLogs;
pub use provider_types::Entity as ProviderTypes;
pub use user_provider_keys::Entity as UserProviderKeys;
pub use user_service_apis::Entity as UserServiceApis;
pub use api_health_status::Entity as ApiHealthStatus;
pub use proxy_tracing::Entity as ProxyTracing;
pub use daily_statistics::Entity as DailyStatistics;

#[cfg(test)]
mod tests;