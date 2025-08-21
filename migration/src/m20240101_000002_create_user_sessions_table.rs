use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserSessions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserSessions::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UserSessions::UserId).integer().not_null())
                    .col(
                        ColumnDef::new(UserSessions::TokenHash)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(UserSessions::RefreshTokenHash).string_len(255))
                    .col(
                        ColumnDef::new(UserSessions::ExpiresAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserSessions::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_sessions_user_id")
                            .from(UserSessions::Table, UserSessions::UserId)
                            .to(Users::Table, Users::Id)
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
                    .name("idx_user_sessions_user_id")
                    .table(UserSessions::Table)
                    .col(UserSessions::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_sessions_token_hash")
                    .table(UserSessions::Table)
                    .col(UserSessions::TokenHash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_sessions_expires_at")
                    .table(UserSessions::Table)
                    .col(UserSessions::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserSessions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserSessions {
    Table,
    Id,
    UserId,
    TokenHash,
    RefreshTokenHash,
    ExpiresAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
