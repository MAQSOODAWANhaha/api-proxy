use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 插入默认管理员用户
        // 密码: admin123 (bcrypt hash)
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
                        "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8.OG7I0dOVFgtNKGl6e".into(), // admin123
                        "default_salt_32_chars_long_12345".into(),
                        true.into(),
                        true.into(),
                    ])
                    .to_owned(),
            )
            .await?;

        // 为admin用户创建OpenAI API配置
        manager
            .exec_stmt(
                Query::insert()
                    .into_table(UserServiceApis::Table)
                    .columns([
                        UserServiceApis::UserId,
                        UserServiceApis::ProviderTypeId,
                        UserServiceApis::ApiKey,
                        UserServiceApis::ApiSecret,
                        UserServiceApis::Name,
                        UserServiceApis::Description,
                        UserServiceApis::SchedulingStrategy,
                        UserServiceApis::RateLimit,
                        UserServiceApis::MaxTokensPerDay,
                    ])
                    .values_panic([
                        1.into(), // admin用户ID
                        1.into(), // OpenAI提供商ID
                        "demo-admin-openai-key-123456789".into(),
                        "demo-secret".into(),
                        "管理员OpenAI服务".into(),
                        "默认管理员OpenAI API配置，用于测试和演示".into(),
                        "round_robin".into(),
                        100.into(), // 每分钟100次请求
                        1000000.into(), // 每天100万tokens
                    ])
                    .to_owned(),
            )
            .await?;

        // 为admin用户创建Gemini API配置
        manager
            .exec_stmt(
                Query::insert()
                    .into_table(UserServiceApis::Table)
                    .columns([
                        UserServiceApis::UserId,
                        UserServiceApis::ProviderTypeId,
                        UserServiceApis::ApiKey,
                        UserServiceApis::ApiSecret,
                        UserServiceApis::Name,
                        UserServiceApis::Description,
                        UserServiceApis::SchedulingStrategy,
                        UserServiceApis::RateLimit,
                        UserServiceApis::MaxTokensPerDay,
                    ])
                    .values_panic([
                        1.into(), // admin用户ID
                        2.into(), // Gemini提供商ID
                        "demo-admin-gemini-key-123456789".into(),
                        "demo-secret".into(),
                        "管理员Gemini服务".into(),
                        "默认管理员Gemini API配置，用于测试和演示".into(),
                        "round_robin".into(),
                        50.into(), // 每分钟50次请求
                        500000.into(), // 每天50万tokens
                    ])
                    .to_owned(),
            )
            .await?;

        // 为admin用户创建Claude API配置
        manager
            .exec_stmt(
                Query::insert()
                    .into_table(UserServiceApis::Table)
                    .columns([
                        UserServiceApis::UserId,
                        UserServiceApis::ProviderTypeId,
                        UserServiceApis::ApiKey,
                        UserServiceApis::ApiSecret,
                        UserServiceApis::Name,
                        UserServiceApis::Description,
                        UserServiceApis::SchedulingStrategy,
                        UserServiceApis::RateLimit,
                        UserServiceApis::MaxTokensPerDay,
                    ])
                    .values_panic([
                        1.into(), // admin用户ID
                        3.into(), // Claude提供商ID
                        "demo-admin-claude-key-123456789".into(),
                        "demo-secret".into(),
                        "管理员Claude服务".into(),
                        "默认管理员Claude API配置，用于测试和演示".into(),
                        "round_robin".into(),
                        80.into(), // 每分钟80次请求
                        800000.into(), // 每天80万tokens
                    ])
                    .to_owned(),
            )
            .await?;

        // 为admin用户的OpenAI服务创建后端API密钥池
        let backend_keys = [
            ("sk-demo-backend-openai-key-1", "后端OpenAI密钥1", 5),
            ("sk-demo-backend-openai-key-2", "后端OpenAI密钥2", 3),
            ("sk-demo-backend-openai-key-3", "后端OpenAI密钥3", 2),
        ];

        for (api_key, name, weight) in backend_keys.iter() {
            manager
                .exec_stmt(
                    Query::insert()
                        .into_table(UserProviderKeys::Table)
                        .columns([
                            UserProviderKeys::UserId,
                            UserProviderKeys::ProviderTypeId,
                            UserProviderKeys::ApiKey,
                            UserProviderKeys::Name,
                            UserProviderKeys::Weight,
                            UserProviderKeys::MaxRequestsPerMinute,
                            UserProviderKeys::MaxTokensPerDay,
                        ])
                        .values_panic([
                            1.into(), // admin用户ID
                            1.into(), // OpenAI提供商ID
                            (*api_key).into(),
                            (*name).into(),
                            (*weight).into(),
                            100.into(), // 每分钟100次请求
                            300000.into(), // 每天30万tokens
                        ])
                        .to_owned(),
                )
                .await?;
        }

        // 为admin用户的Gemini服务创建后端API密钥池
        let gemini_backend_keys = [
            ("demo-backend-gemini-key-1", "后端Gemini密钥1", 4),
            ("demo-backend-gemini-key-2", "后端Gemini密钥2", 3),
        ];

        for (api_key, name, weight) in gemini_backend_keys.iter() {
            manager
                .exec_stmt(
                    Query::insert()
                        .into_table(UserProviderKeys::Table)
                        .columns([
                            UserProviderKeys::UserId,
                            UserProviderKeys::ProviderTypeId,
                            UserProviderKeys::ApiKey,
                            UserProviderKeys::Name,
                            UserProviderKeys::Weight,
                            UserProviderKeys::MaxRequestsPerMinute,
                            UserProviderKeys::MaxTokensPerDay,
                        ])
                        .values_panic([
                            1.into(), // admin用户ID
                            2.into(), // Gemini提供商ID
                            (*api_key).into(),
                            (*name).into(),
                            (*weight).into(),
                            60.into(), // 每分钟60次请求
                            200000.into(), // 每天20万tokens
                        ])
                        .to_owned(),
                )
                .await?;
        }

        // 为admin用户的Claude服务创建后端API密钥池
        let claude_backend_keys = [
            ("sk-ant-demo-backend-claude-key-1", "后端Claude密钥1", 5),
            ("sk-ant-demo-backend-claude-key-2", "后端Claude密钥2", 4),
            ("sk-ant-demo-backend-claude-key-3", "后端Claude密钥3", 3),
        ];

        for (api_key, name, weight) in claude_backend_keys.iter() {
            manager
                .exec_stmt(
                    Query::insert()
                        .into_table(UserProviderKeys::Table)
                        .columns([
                            UserProviderKeys::UserId,
                            UserProviderKeys::ProviderTypeId,
                            UserProviderKeys::ApiKey,
                            UserProviderKeys::Name,
                            UserProviderKeys::Weight,
                            UserProviderKeys::MaxRequestsPerMinute,
                            UserProviderKeys::MaxTokensPerDay,
                        ])
                        .values_panic([
                            1.into(), // admin用户ID
                            3.into(), // Claude提供商ID
                            (*api_key).into(),
                            (*name).into(),
                            (*weight).into(),
                            80.into(), // 每分钟80次请求
                            250000.into(), // 每天25万tokens
                        ])
                        .to_owned(),
                )
                .await?;
        }

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