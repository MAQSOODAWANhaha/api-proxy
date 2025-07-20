use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProviderTypes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProviderTypes::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::Name)
                            .string_len(50)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::DisplayName)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::BaseUrl)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::ApiFormat)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::DefaultModel)
                            .string_len(100),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::MaxTokens)
                            .integer()
                            .default(4096),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::RateLimit)
                            .integer()
                            .default(100),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::TimeoutSeconds)
                            .integer()
                            .default(30),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::HealthCheckPath)
                            .string_len(255)
                            .default("/models"),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::AuthHeaderFormat)
                            .string_len(100)
                            .default("Bearer {key}"),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::ConfigJson)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建索引
        manager
            .create_index(
                Index::create()
                    .name("idx_provider_types_name")
                    .table(ProviderTypes::Table)
                    .col(ProviderTypes::Name)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_provider_types_active")
                    .table(ProviderTypes::Table)
                    .col(ProviderTypes::IsActive)
                    .to_owned(),
            )
            .await?;

        // 插入初始化数据
        manager
            .exec_stmt(
                Query::insert()
                    .into_table(ProviderTypes::Table)
                    .columns([
                        ProviderTypes::Name,
                        ProviderTypes::DisplayName,
                        ProviderTypes::BaseUrl,
                        ProviderTypes::ApiFormat,
                        ProviderTypes::DefaultModel,
                    ])
                    .values_panic([
                        "openai".into(),
                        "OpenAI ChatGPT".into(),
                        "api.openai.com".into(),
                        "openai".into(),
                        "gpt-3.5-turbo".into(),
                    ])
                    .values_panic([
                        "gemini".into(),
                        "Google Gemini".into(),
                        "generativelanguage.googleapis.com".into(),
                        "gemini_rest".into(),
                        "gemini-pro".into(),
                    ])
                    .values_panic([
                        "claude".into(),
                        "Anthropic Claude".into(),
                        "api.anthropic.com".into(),
                        "anthropic".into(),
                        "claude-3-sonnet".into(),
                    ])
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ProviderTypes::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ProviderTypes {
    Table,
    Id,
    Name,
    DisplayName,
    BaseUrl,
    ApiFormat,
    DefaultModel,
    MaxTokens,
    RateLimit,
    TimeoutSeconds,
    HealthCheckPath,
    AuthHeaderFormat,
    IsActive,
    ConfigJson,
    CreatedAt,
    UpdatedAt,
}