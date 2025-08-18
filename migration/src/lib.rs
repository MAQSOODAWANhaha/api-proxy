pub use sea_orm_migration::prelude::*;

mod m20240101_000001_create_users_table;
mod m20240101_000002_create_user_sessions_table;
mod m20240101_000003_create_user_audit_logs_table;
mod m20240101_000004_create_provider_types_table;
mod m20240101_000005_create_user_provider_keys_table;
mod m20240101_000006_create_user_service_apis_table;
mod m20240101_000007_create_api_health_status_table;
mod m20240101_000008_create_proxy_tracing_table;
mod m20240101_000009_create_daily_statistics_table;
mod m20240101_000010_insert_default_admin_data;
mod m20240101_000011_create_user_service_api_providers_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_create_users_table::Migration),
            Box::new(m20240101_000002_create_user_sessions_table::Migration),
            Box::new(m20240101_000003_create_user_audit_logs_table::Migration),
            Box::new(m20240101_000004_create_provider_types_table::Migration),
            Box::new(m20240101_000005_create_user_provider_keys_table::Migration),
            Box::new(m20240101_000006_create_user_service_apis_table::Migration),
            Box::new(m20240101_000007_create_api_health_status_table::Migration),
            Box::new(m20240101_000008_create_proxy_tracing_table::Migration),
            Box::new(m20240101_000009_create_daily_statistics_table::Migration),
            Box::new(m20240101_000010_insert_default_admin_data::Migration),
            Box::new(m20240101_000011_create_user_service_api_providers_table::Migration),
        ]
    }
}