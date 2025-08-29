use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. 扩展 provider_types 表支持多认证类型 - SQLite需要分开添加字段
        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .add_column(ColumnDef::new(ProviderTypes::SupportedAuthTypes).json())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .add_column(ColumnDef::new(ProviderTypes::AuthConfigJson).json())
                    .to_owned(),
            )
            .await?;

        // 创建索引支持 JSON 查询
        manager
            .create_index(
                Index::create()
                    .name("idx_provider_types_supported_auth")
                    .table(ProviderTypes::Table)
                    .col(ProviderTypes::SupportedAuthTypes)
                    .to_owned(),
            )
            .await?;

        // 2. 扩展 user_provider_keys 表支持多认证 - SQLite需要分开添加字段
        manager
            .alter_table(
                Table::alter()
                    .table(UserProviderKeys::Table)
                    .add_column(
                        ColumnDef::new(UserProviderKeys::AuthType)
                            .string_len(30)
                            .not_null()
                            .default("api_key"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserProviderKeys::Table)
                    .add_column(ColumnDef::new(UserProviderKeys::AuthConfigJson).json())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserProviderKeys::Table)
                    .add_column(
                        ColumnDef::new(UserProviderKeys::AuthStatus)
                            .string_len(20)
                            .default("pending"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserProviderKeys::Table)
                    .add_column(ColumnDef::new(UserProviderKeys::ExpiresAt).timestamp())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserProviderKeys::Table)
                    .add_column(ColumnDef::new(UserProviderKeys::LastAuthCheck).timestamp())
                    .to_owned(),
            )
            .await?;

        // 创建认证相关索引
        manager
            .create_index(
                Index::create()
                    .name("idx_user_provider_keys_auth_type")
                    .table(UserProviderKeys::Table)
                    .col(UserProviderKeys::AuthType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_provider_keys_auth_status")
                    .table(UserProviderKeys::Table)
                    .col(UserProviderKeys::AuthStatus)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_provider_keys_expires_at")
                    .table(UserProviderKeys::Table)
                    .col(UserProviderKeys::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        // 3. 创建 oauth_sessions 表
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
                            .to(ProviderTypes::Table, ProviderTypes::Id)
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
        // 删除 OAuth sessions 表
        manager
            .drop_table(Table::drop().table(OAuthSessions::Table).to_owned())
            .await?;

        // 删除 user_provider_keys 表的新字段 - SQLite需要分开删除
        manager
            .alter_table(
                Table::alter()
                    .table(UserProviderKeys::Table)
                    .drop_column(UserProviderKeys::LastAuthCheck)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserProviderKeys::Table)
                    .drop_column(UserProviderKeys::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserProviderKeys::Table)
                    .drop_column(UserProviderKeys::AuthStatus)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserProviderKeys::Table)
                    .drop_column(UserProviderKeys::AuthConfigJson)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserProviderKeys::Table)
                    .drop_column(UserProviderKeys::AuthType)
                    .to_owned(),
            )
            .await?;

        // 删除 provider_types 表的新字段 - SQLite需要分开删除
        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .drop_column(ProviderTypes::AuthConfigJson)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .drop_column(ProviderTypes::SupportedAuthTypes)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum ProviderTypes {
    Table,
    Id,
    SupportedAuthTypes,
    AuthConfigJson,
}

#[derive(DeriveIden)]
enum UserProviderKeys {
    Table,
    AuthType,
    AuthConfigJson,
    AuthStatus,
    ExpiresAt,
    LastAuthCheck,
}

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