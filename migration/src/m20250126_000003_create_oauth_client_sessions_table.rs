use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 创建 oauth_client_sessions 表 - 存储客户端OAuth会话
        manager
            .create_table(
                Table::create()
                    .table(OAuthClientSessions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OAuthClientSessions::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::SessionId)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::UserId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::ProviderName)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::ProviderTypeId)
                            .integer(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::CodeVerifier)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::CodeChallenge)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::State)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::Name)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::Description)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::Status)
                            .string_len(20)
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::AccessToken)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::RefreshToken)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::IdToken)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::TokenType)
                            .string_len(20)
                            .default("Bearer"),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::ExpiresIn)
                            .integer(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::ExpiresAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::ErrorMessage)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(OAuthClientSessions::CompletedAt)
                            .timestamp(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_client_sessions_user_id")
                            .from(OAuthClientSessions::Table, OAuthClientSessions::UserId)
                            .to(Users::Table, Users::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_client_sessions_provider_name")
                            .from(OAuthClientSessions::Table, OAuthClientSessions::ProviderName)
                            .to(ProviderTypes::Table, ProviderTypes::Name)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_client_sessions_provider_type_id")
                            .from(OAuthClientSessions::Table, OAuthClientSessions::ProviderTypeId)
                            .to(ProviderTypes::Table, ProviderTypes::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建索引
        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_client_sessions_session_id")
                    .table(OAuthClientSessions::Table)
                    .col(OAuthClientSessions::SessionId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_client_sessions_user_provider")
                    .table(OAuthClientSessions::Table)
                    .col(OAuthClientSessions::UserId)
                    .col(OAuthClientSessions::ProviderName)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_client_sessions_status")
                    .table(OAuthClientSessions::Table)
                    .col(OAuthClientSessions::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_client_sessions_expires_at")
                    .table(OAuthClientSessions::Table)
                    .col(OAuthClientSessions::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_client_sessions_state")
                    .table(OAuthClientSessions::Table)
                    .col(OAuthClientSessions::State)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_client_sessions_provider_type_id")
                    .table(OAuthClientSessions::Table)
                    .col(OAuthClientSessions::ProviderTypeId)
                    .to_owned(),
            )
            .await?;

        // 为user_provider_keys表添加oauth_session_id字段，建立与oauth_client_sessions的关联
        manager
            .alter_table(
                Table::alter()
                    .table(UserProviderKeys::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(UserProviderKeys::OAuthSessionId)
                            .string_len(64)
                            .null()
                    )
                    .to_owned(),
            )
            .await?;

        // 为oauth_session_id字段创建索引
        manager
            .create_index(
                Index::create()
                    .name("idx_user_provider_keys_oauth_session_id")
                    .table(UserProviderKeys::Table)
                    .col(UserProviderKeys::OAuthSessionId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 删除user_provider_keys表的oauth_session_id字段
        manager
            .alter_table(
                Table::alter()
                    .table(UserProviderKeys::Table)
                    .drop_column(UserProviderKeys::OAuthSessionId)
                    .to_owned(),
            )
            .await?;

        // 删除oauth_client_sessions表
        manager
            .drop_table(Table::drop().table(OAuthClientSessions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum OAuthClientSessions {
    #[sea_orm(iden = "oauth_client_sessions")]
    Table,
    Id,
    SessionId,
    UserId,
    ProviderName,
    ProviderTypeId,
    CodeVerifier,
    CodeChallenge,
    State,
    Name,
    Description,
    Status,
    AccessToken,
    RefreshToken,
    IdToken,
    TokenType,
    ExpiresIn,
    ExpiresAt,
    ErrorMessage,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum Users {
    #[sea_orm(iden = "users")]
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ProviderTypes {
    #[sea_orm(iden = "provider_types")]
    Table,
    Id,
    Name,
}

#[derive(DeriveIden)]
enum UserProviderKeys {
    #[sea_orm(iden = "user_provider_keys")]
    Table,
    #[sea_orm(iden = "oauth_session_id")]
    OAuthSessionId,
}