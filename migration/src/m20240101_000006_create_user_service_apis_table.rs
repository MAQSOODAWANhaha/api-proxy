use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserServiceApis::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserServiceApis::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::UserId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::ProviderTypeId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::ApiKey)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::ApiSecret)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::Name)
                            .string_len(100),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::Description)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::SchedulingStrategy)
                            .string_len(20)
                            .default("round_robin"),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::RetryCount)
                            .integer()
                            .default(3),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::TimeoutSeconds)
                            .integer()
                            .default(30),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::RateLimit)
                            .integer()
                            .default(1000),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::MaxTokensPerDay)
                            .integer()
                            .default(10000000),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::UsedTokensToday)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::TotalRequests)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::SuccessfulRequests)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::LastUsed)
                            .timestamp(),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::ExpiresAt)
                            .timestamp(),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(UserServiceApis::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_service_apis_user_id")
                            .from(UserServiceApis::Table, UserServiceApis::UserId)
                            .to(Users::Table, Users::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_service_apis_provider_type_id")
                            .from(UserServiceApis::Table, UserServiceApis::ProviderTypeId)
                            .to(ProviderTypes::Table, ProviderTypes::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建唯一约束：每个用户每种服务商只能有一个对外API
        manager
            .create_index(
                Index::create()
                    .name("idx_user_service_apis_unique_user_provider")
                    .table(UserServiceApis::Table)
                    .col(UserServiceApis::UserId)
                    .col(UserServiceApis::ProviderTypeId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // 创建索引
        manager
            .create_index(
                Index::create()
                    .name("idx_user_service_apis_api_key")
                    .table(UserServiceApis::Table)
                    .col(UserServiceApis::ApiKey)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_service_apis_user_provider")
                    .table(UserServiceApis::Table)
                    .col(UserServiceApis::UserId)
                    .col(UserServiceApis::ProviderTypeId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserServiceApis::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserServiceApis {
    Table,
    Id,
    UserId,
    ProviderTypeId,
    ApiKey,
    ApiSecret,
    Name,
    Description,
    SchedulingStrategy,
    RetryCount,
    TimeoutSeconds,
    RateLimit,
    MaxTokensPerDay,
    UsedTokensToday,
    TotalRequests,
    SuccessfulRequests,
    LastUsed,
    ExpiresAt,
    IsActive,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ProviderTypes {
    Table,
    Id,
}