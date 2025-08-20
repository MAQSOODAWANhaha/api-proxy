//! # Entity 模块
//!
//! 包含所有 Sea-ORM 实体定义

pub mod api_health_status;
pub mod daily_statistics;
pub mod provider_types;
pub mod proxy_tracing;
// TLS 模块已移除
pub mod user_audit_logs;
pub mod user_provider_keys;
pub mod user_service_apis;
pub mod user_sessions;
pub mod users;

pub use api_health_status::Entity as ApiHealthStatus;
pub use daily_statistics::Entity as DailyStatistics;
pub use provider_types::Entity as ProviderTypes;
pub use proxy_tracing::Entity as ProxyTracing;
// pub use tls_certificates::Entity as TlsCertificates; // 已移除
pub use user_audit_logs::Entity as UserAuditLogs;
pub use user_provider_keys::Entity as UserProviderKeys;
pub use user_service_apis::Entity as UserServiceApis;
pub use user_sessions::Entity as UserSessions;
pub use users::Entity as Users;
