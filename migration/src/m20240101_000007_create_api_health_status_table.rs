use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ApiHealthStatus::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ApiHealthStatus::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ApiHealthStatus::UserProviderKeyId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ApiHealthStatus::IsHealthy)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(ApiHealthStatus::ResponseTimeMs)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ApiHealthStatus::SuccessRate)
                            .float()
                            .default(1.0),
                    )
                    .col(ColumnDef::new(ApiHealthStatus::LastSuccess).timestamp())
                    .col(ColumnDef::new(ApiHealthStatus::LastFailure).timestamp())
                    .col(
                        ColumnDef::new(ApiHealthStatus::ConsecutiveFailures)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ApiHealthStatus::TotalChecks)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ApiHealthStatus::SuccessfulChecks)
                            .integer()
                            .default(0),
                    )
                    .col(ColumnDef::new(ApiHealthStatus::LastErrorMessage).text())
                    .col(
                        ColumnDef::new(ApiHealthStatus::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ApiHealthStatus::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_api_health_status_user_provider_key_id")
                            .from(ApiHealthStatus::Table, ApiHealthStatus::UserProviderKeyId)
                            .to(UserProviderKeys::Table, UserProviderKeys::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建索引
        manager
            .create_index(
                Index::create()
                    .name("idx_api_health_status_user_provider_key_healthy")
                    .table(ApiHealthStatus::Table)
                    .col(ApiHealthStatus::UserProviderKeyId)
                    .col(ApiHealthStatus::IsHealthy)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_api_health_status_updated_at")
                    .table(ApiHealthStatus::Table)
                    .col(ApiHealthStatus::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApiHealthStatus::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApiHealthStatus {
    Table,
    Id,
    UserProviderKeyId,
    IsHealthy,
    ResponseTimeMs,
    SuccessRate,
    LastSuccess,
    LastFailure,
    ConsecutiveFailures,
    TotalChecks,
    SuccessfulChecks,
    LastErrorMessage,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum UserProviderKeys {
    Table,
    Id,
}
