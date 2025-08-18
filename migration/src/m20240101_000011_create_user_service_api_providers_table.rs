use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserServiceApiProviders::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserServiceApiProviders::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserServiceApiProviders::UserServiceApiId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserServiceApiProviders::UserProviderKeyId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserServiceApiProviders::Weight)
                            .integer()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(UserServiceApiProviders::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(UserServiceApiProviders::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(UserServiceApiProviders::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_service_api_providers_service_api_id")
                            .from(UserServiceApiProviders::Table, UserServiceApiProviders::UserServiceApiId)
                            .to(UserServiceApis::Table, UserServiceApis::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_service_api_providers_provider_key_id")
                            .from(UserServiceApiProviders::Table, UserServiceApiProviders::UserProviderKeyId)
                            .to(UserProviderKeys::Table, UserProviderKeys::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建唯一约束：同一个API不能重复关联同一个提供商密钥
        manager
            .create_index(
                Index::create()
                    .name("idx_user_service_api_providers_unique")
                    .table(UserServiceApiProviders::Table)
                    .col(UserServiceApiProviders::UserServiceApiId)
                    .col(UserServiceApiProviders::UserProviderKeyId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // 创建索引用于查询
        manager
            .create_index(
                Index::create()
                    .name("idx_user_service_api_providers_service_api")
                    .table(UserServiceApiProviders::Table)
                    .col(UserServiceApiProviders::UserServiceApiId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_service_api_providers_provider_key")
                    .table(UserServiceApiProviders::Table)
                    .col(UserServiceApiProviders::UserProviderKeyId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserServiceApiProviders::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserServiceApiProviders {
    Table,
    Id,
    UserServiceApiId,
    UserProviderKeyId,
    Weight,
    IsActive,
    CreatedAt,
    UpdatedAt,
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