use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(UserServiceApis::Table)
                    .add_column(
                        ColumnDef::new(UserServiceApis::LogMode)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(UserServiceApis::Table)
                    .drop_column(UserServiceApis::LogMode)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum UserServiceApis {
    Table,
    LogMode,
}
