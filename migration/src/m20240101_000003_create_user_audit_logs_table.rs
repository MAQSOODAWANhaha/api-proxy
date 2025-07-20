use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserAuditLogs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserAuditLogs::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserAuditLogs::UserId)
                            .integer(),
                    )
                    .col(
                        ColumnDef::new(UserAuditLogs::Action)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserAuditLogs::ResourceType)
                            .string_len(50),
                    )
                    .col(
                        ColumnDef::new(UserAuditLogs::ResourceId)
                            .integer(),
                    )
                    .col(
                        ColumnDef::new(UserAuditLogs::IpAddress)
                            .string_len(45),
                    )
                    .col(
                        ColumnDef::new(UserAuditLogs::UserAgent)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(UserAuditLogs::Details)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(UserAuditLogs::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_audit_logs_user_id")
                            .from(UserAuditLogs::Table, UserAuditLogs::UserId)
                            .to(Users::Table, Users::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建索引
        manager
            .create_index(
                Index::create()
                    .name("idx_user_audit_logs_user_id")
                    .table(UserAuditLogs::Table)
                    .col(UserAuditLogs::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_audit_logs_action")
                    .table(UserAuditLogs::Table)
                    .col(UserAuditLogs::Action)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_user_audit_logs_created_at")
                    .table(UserAuditLogs::Table)
                    .col(UserAuditLogs::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserAuditLogs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UserAuditLogs {
    Table,
    Id,
    UserId,
    Action,
    ResourceType,
    ResourceId,
    IpAddress,
    UserAgent,
    Details,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}