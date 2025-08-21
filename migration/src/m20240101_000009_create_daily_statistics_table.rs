use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(DailyStatistics::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DailyStatistics::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(DailyStatistics::UserId).integer().not_null())
                    .col(ColumnDef::new(DailyStatistics::UserServiceApiId).integer())
                    .col(
                        ColumnDef::new(DailyStatistics::ProviderTypeId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(DailyStatistics::Date).date().not_null())
                    .col(
                        ColumnDef::new(DailyStatistics::TotalRequests)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(DailyStatistics::SuccessfulRequests)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(DailyStatistics::FailedRequests)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(DailyStatistics::TotalTokens)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(DailyStatistics::AvgResponseTime)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(DailyStatistics::MaxResponseTime)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(DailyStatistics::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(DailyStatistics::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_daily_statistics_user_id")
                            .from(DailyStatistics::Table, DailyStatistics::UserId)
                            .to(Users::Table, Users::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_daily_statistics_user_service_api_id")
                            .from(DailyStatistics::Table, DailyStatistics::UserServiceApiId)
                            .to(UserServiceApis::Table, UserServiceApis::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_daily_statistics_provider_type_id")
                            .from(DailyStatistics::Table, DailyStatistics::ProviderTypeId)
                            .to(ProviderTypes::Table, ProviderTypes::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建唯一约束
        manager
            .create_index(
                Index::create()
                    .name("idx_daily_statistics_unique")
                    .table(DailyStatistics::Table)
                    .col(DailyStatistics::UserId)
                    .col(DailyStatistics::UserServiceApiId)
                    .col(DailyStatistics::ProviderTypeId)
                    .col(DailyStatistics::Date)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // 创建索引
        manager
            .create_index(
                Index::create()
                    .name("idx_daily_statistics_user_date")
                    .table(DailyStatistics::Table)
                    .col(DailyStatistics::UserId)
                    .col(DailyStatistics::Date)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_daily_statistics_service_date")
                    .table(DailyStatistics::Table)
                    .col(DailyStatistics::UserServiceApiId)
                    .col(DailyStatistics::Date)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(DailyStatistics::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum DailyStatistics {
    Table,
    Id,
    UserId,
    UserServiceApiId,
    ProviderTypeId,
    Date,
    TotalRequests,
    SuccessfulRequests,
    FailedRequests,
    TotalTokens,
    AvgResponseTime,
    MaxResponseTime,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum UserServiceApis {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ProviderTypes {
    Table,
    Id,
}
