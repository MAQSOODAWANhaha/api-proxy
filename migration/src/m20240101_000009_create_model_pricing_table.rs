use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ModelPricing::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ModelPricing::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ModelPricing::ProviderTypeId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModelPricing::ModelName)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ModelPricing::Description).text())
                    .col(
                        ColumnDef::new(ModelPricing::CostCurrency)
                            .string_len(10)
                            .not_null()
                            .default("USD"),
                    )
                    .col(
                        ColumnDef::new(ModelPricing::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ModelPricing::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_model_pricing_provider_type_id")
                            .from(ModelPricing::Table, ModelPricing::ProviderTypeId)
                            .to(ProviderTypes::Table, ProviderTypes::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建唯一复合索引确保每个提供商的每个模型只有一个定价记录
        manager
            .create_index(
                Index::create()
                    .name("idx_model_pricing_provider_model")
                    .table(ModelPricing::Table)
                    .col(ModelPricing::ProviderTypeId)
                    .col(ModelPricing::ModelName)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // 创建查询优化索引
        manager
            .create_index(
                Index::create()
                    .name("idx_model_pricing_model_name")
                    .table(ModelPricing::Table)
                    .col(ModelPricing::ModelName)
                    .to_owned(),
            )
            .await?;

        // 模型数据将在单独的协调数据插入迁移中处理 (m20240101_000011)

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ModelPricing::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ModelPricing {
    Table,
    Id,
    ProviderTypeId,
    ModelName,
    Description,
    CostCurrency,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ProviderTypes {
    Table,
    Id,
}
