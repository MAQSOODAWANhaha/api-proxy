use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 创建 oauth_sessions 表 - user_provider_keys的OAuth字段已在原始创建文件中定义
        manager
            .create_table(
                Table::create()
                    .table(OAuthSessions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OAuthSessions::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(OAuthSessions::SessionId)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(OAuthSessions::UserId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthSessions::ProviderTypeId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthSessions::AuthType)
                            .string_len(30)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthSessions::State)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(OAuthSessions::CodeVerifier).string_len(128))
                    .col(ColumnDef::new(OAuthSessions::CodeChallenge).string_len(128))
                    .col(ColumnDef::new(OAuthSessions::RedirectUri).text().not_null())
                    .col(ColumnDef::new(OAuthSessions::Scopes).text())
                    .col(
                        ColumnDef::new(OAuthSessions::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(OAuthSessions::ExpiresAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OAuthSessions::CompletedAt).timestamp())
                    .col(ColumnDef::new(OAuthSessions::ErrorMessage).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_sessions_user_id")
                            .from(OAuthSessions::Table, OAuthSessions::UserId)
                            .to(Users::Table, Users::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_sessions_provider_type_id")
                            .from(OAuthSessions::Table, OAuthSessions::ProviderTypeId)
                            .to(ProviderTypesTable::Table, ProviderTypesTable::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建 OAuth 会话索引
        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_sessions_session_id")
                    .table(OAuthSessions::Table)
                    .col(OAuthSessions::SessionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_sessions_user_provider")
                    .table(OAuthSessions::Table)
                    .col(OAuthSessions::UserId)
                    .col(OAuthSessions::ProviderTypeId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_sessions_expires_at")
                    .table(OAuthSessions::Table)
                    .col(OAuthSessions::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 删除 OAuth sessions 表 - user_provider_keys的OAuth字段由原始创建文件管理
        manager
            .drop_table(Table::drop().table(OAuthSessions::Table).to_owned())
            .await
    }
}

// user_provider_keys的OAuth字段已合并到原始创建文件中

#[derive(DeriveIden)]
enum OAuthSessions {
    Table,
    Id,
    SessionId,
    UserId,
    ProviderTypeId,
    AuthType,
    State,
    CodeVerifier,
    CodeChallenge,
    RedirectUri,
    Scopes,
    CreatedAt,
    ExpiresAt,
    CompletedAt,
    ErrorMessage,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ProviderTypesTable {
    Table,
    Id,
}