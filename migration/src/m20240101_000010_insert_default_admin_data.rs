use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 插入默认管理员用户
        // 密码: *** (bcrypt hash)
        manager
            .exec_stmt(
                Query::insert()
                    .into_table(Users::Table)
                    .columns([
                        Users::Username,
                        Users::Email,
                        Users::PasswordHash,
                        Users::Salt,
                        Users::IsActive,
                        Users::IsAdmin,
                    ])
                    .values_panic([
                        "admin".into(),
                        "admin@api-proxy.local".into(),
                        "$2b$12$LMURIch2lHkm1y1uhuh1HOJ/RDlGjddn6NCiAOCuvsjjmHMXiGTn2".into(),
                        "default_salt_32_chars_long_12345".into(),
                        true.into(),
                        true.into(),
                    ])
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 删除默认admin用户及相关数据（级联删除会自动处理）
        manager
            .exec_stmt(
                Query::delete()
                    .from_table(Users::Table)
                    .and_where(Expr::col(Users::Username).eq("admin"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

// 表定义枚举
#[derive(DeriveIden)]
#[allow(dead_code)]
enum Users {
    Table,
    Id,
    Username,
    Email,
    PasswordHash,
    Salt,
    IsActive,
    IsAdmin,
}

#[derive(DeriveIden)]
#[allow(dead_code)]
enum UserServiceApis {
    Table,
    UserId,
    ProviderTypeId,
    ApiKey,
    ApiSecret,
    Name,
    Description,
    SchedulingStrategy,
    RateLimit,
    MaxTokensPerDay,
}

#[derive(DeriveIden)]
#[allow(dead_code)]
enum UserProviderKeys {
    Table,
    UserId,
    ProviderTypeId,
    ApiKey,
    Name,
    Weight,
    MaxRequestsPerMinute,
    MaxTokensPerDay,
}
