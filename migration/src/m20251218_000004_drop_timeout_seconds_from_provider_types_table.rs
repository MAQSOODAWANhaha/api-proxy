use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // provider_types.timeout_seconds 已不再使用：
        // 代理层超时配置统一从 user_service_apis.timeout_seconds 获取，缺省走内部默认值。
        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .drop_column(ProviderTypes::TimeoutSeconds)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .add_column(
                        ColumnDef::new(ProviderTypes::TimeoutSeconds)
                            .integer()
                            .default(30),
                    )
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ProviderTypes {
    Table,
    TimeoutSeconds,
}
