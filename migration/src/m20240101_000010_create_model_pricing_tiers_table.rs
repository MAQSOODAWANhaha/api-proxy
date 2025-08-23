use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ModelPricingTiers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ModelPricingTiers::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ModelPricingTiers::ModelPricingId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModelPricingTiers::TokenType)
                            .string_len(20)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModelPricingTiers::MinTokens)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ModelPricingTiers::MaxTokens)
                            .integer(),
                    )
                    .col(
                        ColumnDef::new(ModelPricingTiers::PricePerToken)
                            .double()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModelPricingTiers::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ModelPricingTiers::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_model_pricing_tiers_model_pricing_id")
                            .from(ModelPricingTiers::Table, ModelPricingTiers::ModelPricingId)
                            .to(ModelPricing::Table, ModelPricing::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建唯一复合索引确保同一模型的同一token类型不能有重叠的阈值范围
        manager
            .create_index(
                Index::create()
                    .name("idx_model_pricing_tiers_unique")
                    .table(ModelPricingTiers::Table)
                    .col(ModelPricingTiers::ModelPricingId)
                    .col(ModelPricingTiers::TokenType)
                    .col(ModelPricingTiers::MinTokens)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // 创建查询优化索引
        manager
            .create_index(
                Index::create()
                    .name("idx_model_pricing_tiers_lookup")
                    .table(ModelPricingTiers::Table)
                    .col(ModelPricingTiers::ModelPricingId)
                    .col(ModelPricingTiers::TokenType)
                    .to_owned(),
            )
            .await?;

        // 阶梯定价数据将在单独的协调数据插入迁移中处理 (m20240101_000011)

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ModelPricingTiers::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ModelPricingTiers {
    Table,
    Id,
    ModelPricingId,
    TokenType,
    MinTokens,
    MaxTokens,
    PricePerToken,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ModelPricing {
    Table,
    Id,
}