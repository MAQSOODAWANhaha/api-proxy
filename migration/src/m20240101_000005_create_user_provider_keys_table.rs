use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserProviderKeys::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserProviderKeys::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserProviderKeys::UserId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserProviderKeys::ProviderTypeId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserProviderKeys::ApiKey)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserProviderKeys::Name)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserProviderKeys::Weight)
                            .integer()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(UserProviderKeys::MaxRequestsPerMinute)
                            .integer()
                            .default(100),
                    )
                    .col(
                        ColumnDef::new(UserProviderKeys::MaxTokensPromptPerMinute)
                            .integer()
                            .default(1000),
                    )
                    .col(
                        ColumnDef::new(UserProviderKeys::MaxRequestsPerDay)
                            .integer()
                            .default(10000),
                    )
                    .col(
                        ColumnDef::new(UserProviderKeys::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(UserProviderKeys::HealthStatus)
                            .string_len(20)
                            .not_null()
                            .default("healthy"),
                    )
                    .col(
                        ColumnDef::new(UserProviderKeys::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(UserProviderKeys::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_provider_keys_user_id")
                            .from(UserProviderKeys::Table, UserProviderKeys::UserId)
                            .to(Users::Table, Users::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_provider_keys_provider_type_id")
                            .from(UserProviderKeys::Table, UserProviderKeys::ProviderTypeId)
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
                    .name("idx_user_provider_keys_unique_name")
                    .table(UserProviderKeys::Table)
                    .col(UserProviderKeys::UserId)
                    .col(UserProviderKeys::ProviderTypeId)
                    .col(UserProviderKeys::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // 创建索引
        manager
            .create_index(
                Index::create()
                    .name("idx_user_provider_keys_user_provider")
                    .table(UserProviderKeys::Table)
                    .col(UserProviderKeys::UserId)
                    .col(UserProviderKeys::ProviderTypeId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_provider_keys_active")
                    .table(UserProviderKeys::Table)
                    .col(UserProviderKeys::IsActive)
                    .col(UserProviderKeys::ProviderTypeId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserProviderKeys::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserProviderKeys {
    Table,
    Id,
    UserId,
    ProviderTypeId,
    ApiKey,
    Name,
    Weight,
    MaxRequestsPerMinute,
    MaxTokensPromptPerMinute,
    MaxRequestsPerDay,
    IsActive,
    HealthStatus,
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