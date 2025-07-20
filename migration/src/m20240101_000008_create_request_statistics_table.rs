use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RequestStatistics::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RequestStatistics::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::UserServiceApiId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::UserProviderKeyId)
                            .integer(),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::RequestId)
                            .string_len(36),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::Method)
                            .string_len(10)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::Path)
                            .string_len(500),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::StatusCode)
                            .integer(),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::ResponseTimeMs)
                            .integer(),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::RequestSize)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::ResponseSize)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::TokensPrompt)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::TokensCompletion)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::TokensTotal)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::ModelUsed)
                            .string_len(100),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::ClientIp)
                            .string_len(45),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::UserAgent)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::ErrorType)
                            .string_len(50),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::ErrorMessage)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::RetryCount)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(RequestStatistics::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_request_statistics_user_service_api_id")
                            .from(RequestStatistics::Table, RequestStatistics::UserServiceApiId)
                            .to(UserServiceApis::Table, UserServiceApis::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_request_statistics_user_provider_key_id")
                            .from(RequestStatistics::Table, RequestStatistics::UserProviderKeyId)
                            .to(UserProviderKeys::Table, UserProviderKeys::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建索引
        manager
            .create_index(
                Index::create()
                    .name("idx_request_statistics_user_service_time")
                    .table(RequestStatistics::Table)
                    .col(RequestStatistics::UserServiceApiId)
                    .col(RequestStatistics::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_request_statistics_status_time")
                    .table(RequestStatistics::Table)
                    .col(RequestStatistics::StatusCode)
                    .col(RequestStatistics::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_request_statistics_request_time")
                    .table(RequestStatistics::Table)
                    .col(RequestStatistics::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_request_statistics_request_id")
                    .table(RequestStatistics::Table)
                    .col(RequestStatistics::RequestId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RequestStatistics::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum RequestStatistics {
    Table,
    Id,
    UserServiceApiId,
    UserProviderKeyId,
    RequestId,
    Method,
    Path,
    StatusCode,
    ResponseTimeMs,
    RequestSize,
    ResponseSize,
    TokensPrompt,
    TokensCompletion,
    TokensTotal,
    ModelUsed,
    ClientIp,
    UserAgent,
    ErrorType,
    ErrorMessage,
    RetryCount,
    CreatedAt,
}

#[derive(DeriveIden)]
enum UserServiceApis {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum UserProviderKeys {
    Table,
    Id,
}