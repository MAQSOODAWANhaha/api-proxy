use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 清理 provider_types 表中不再使用的配置字段：
        // - api_format / default_model：现已由策略层与请求转换逻辑接管，不再依赖静态字段
        // - max_tokens / rate_limit：限流与配额在 user_service_apis / user_provider_keys 层实现
        // - health_check_path：健康检查逻辑不再依赖静态 path
        // 注意：SQLite 不支持在一次 ALTER TABLE 中包含多个操作（sea-query 会直接 panic）。
        // 为了兼容 `sqlite::memory:` 的单元测试，这里拆分为多次 ALTER TABLE。
        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .drop_column(ProviderTypes::ApiFormat)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .drop_column(ProviderTypes::DefaultModel)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .drop_column(ProviderTypes::MaxTokens)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .drop_column(ProviderTypes::RateLimit)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .drop_column(ProviderTypes::HealthCheckPath)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 回滚时仅恢复列定义；历史数据无法恢复，使用合理 default 保证 NOT NULL 约束可通过。
        // 同样为了兼容 SQLite，拆分为多次 ALTER TABLE。
        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .add_column(
                        ColumnDef::new(ProviderTypes::ApiFormat)
                            .string_len(50)
                            .not_null()
                            .default("unknown"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .add_column(ColumnDef::new(ProviderTypes::DefaultModel).string_len(100))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .add_column(
                        ColumnDef::new(ProviderTypes::MaxTokens)
                            .integer()
                            .default(4096),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .add_column(
                        ColumnDef::new(ProviderTypes::RateLimit)
                            .integer()
                            .default(100),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProviderTypes::Table)
                    .add_column(
                        ColumnDef::new(ProviderTypes::HealthCheckPath)
                            .string_len(255)
                            .default("/models"),
                    )
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ProviderTypes {
    Table,
    ApiFormat,
    DefaultModel,
    MaxTokens,
    RateLimit,
    HealthCheckPath,
}
