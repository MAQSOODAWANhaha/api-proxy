use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProxyTracing::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ProxyTracing::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    // === 基础请求信息（兼容request_statistics） ===
                    .col(
                        ColumnDef::new(ProxyTracing::UserServiceApiId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::UserProviderKeyId)
                            .integer(),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::RequestId)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::Method)
                            .string_len(10)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::Path)
                            .string_len(1000),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::StatusCode)
                            .integer(),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::ResponseTimeMs)
                            .integer(),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::RequestSize)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::ResponseSize)
                            .integer()
                            .default(0),
                    )
                    // === Token使用统计 ===
                    .col(
                        ColumnDef::new(ProxyTracing::TokensPrompt)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::TokensCompletion)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::TokensTotal)
                            .integer()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::TokenEfficiencyRatio)
                            .double(),
                    )
                    // === 业务信息 ===
                    .col(
                        ColumnDef::new(ProxyTracing::ModelUsed)
                            .string_len(100),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::ClientIp)
                            .string_len(45),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::UserAgent)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::ErrorType)
                            .string_len(50),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::ErrorMessage)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::RetryCount)
                            .integer()
                            .default(0),
                    )
                    // === 追踪控制 ===
                    .col(
                        ColumnDef::new(ProxyTracing::TraceLevel)
                            .integer()
                            .not_null()
                            .default(0), // 0=基础, 1=详细, 2=完整
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::SamplingRate)
                            .double()
                            .default(1.0),
                    )
                    // === 提供商信息 ===
                    .col(
                        ColumnDef::new(ProxyTracing::ProviderTypeId)
                            .integer(),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::ProviderName)
                            .string_len(255),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::BackendKeyId)
                            .integer(),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::UpstreamAddr)
                            .string_len(255),
                    )
                    // === 详细时间追踪 ===
                    .col(
                        ColumnDef::new(ProxyTracing::StartTime)
                            .timestamp(),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::EndTime)
                            .timestamp(),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::DurationMs)
                            .big_integer(),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::IsSuccess)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    // === 阶段追踪数据（JSON） ===
                    .col(
                        ColumnDef::new(ProxyTracing::PhasesData)
                            .text(), // JSON: 各阶段详细信息
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::PerformanceMetrics)
                            .text(), // JSON: 性能指标
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::Labels)
                            .text(), // JSON: 自定义标签
                    )
                    // === 健康状态评估 ===
                    .col(
                        ColumnDef::new(ProxyTracing::HealthImpactScore)
                            .double()
                            .default(0.0),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::IsAnomaly)
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(ProxyTracing::QualityMetrics)
                            .text(), // JSON: 质量指标
                    )
                    // === 创建时间 ===
                    .col(
                        ColumnDef::new(ProxyTracing::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_proxy_tracing_user_service_api_id")
                            .from(ProxyTracing::Table, ProxyTracing::UserServiceApiId)
                            .to(UserServiceApis::Table, UserServiceApis::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_proxy_tracing_user_provider_key_id")
                            .from(ProxyTracing::Table, ProxyTracing::UserProviderKeyId)
                            .to(UserProviderKeys::Table, UserProviderKeys::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_proxy_tracing_provider_type_id")
                            .from(ProxyTracing::Table, ProxyTracing::ProviderTypeId)
                            .to(ProviderTypes::Table, ProviderTypes::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建核心索引
        manager
            .create_index(
                Index::create()
                    .name("idx_proxy_tracing_user_service_time")
                    .table(ProxyTracing::Table)
                    .col(ProxyTracing::UserServiceApiId)
                    .col(ProxyTracing::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_proxy_tracing_provider_time")
                    .table(ProxyTracing::Table)
                    .col(ProxyTracing::ProviderTypeId)
                    .col(ProxyTracing::StartTime)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_proxy_tracing_request_id")
                    .table(ProxyTracing::Table)
                    .col(ProxyTracing::RequestId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_proxy_tracing_status_time")
                    .table(ProxyTracing::Table)
                    .col(ProxyTracing::StatusCode)
                    .col(ProxyTracing::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_proxy_tracing_health_time")
                    .table(ProxyTracing::Table)
                    .col(ProxyTracing::IsSuccess)
                    .col(ProxyTracing::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_proxy_tracing_trace_level")
                    .table(ProxyTracing::Table)
                    .col(ProxyTracing::TraceLevel)
                    .col(ProxyTracing::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ProxyTracing::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ProxyTracing {
    Table,
    // 基础请求信息
    Id,
    UserServiceApiId,
    UserProviderKeyId,
    RequestId,
    Method,
    Path,
    StatusCode,
    ResponseTimeMs,
    RequestSize,
    ResponseSize,
    // Token统计
    TokensPrompt,
    TokensCompletion,
    TokensTotal,
    TokenEfficiencyRatio,
    // 业务信息
    ModelUsed,
    ClientIp,
    UserAgent,
    ErrorType,
    ErrorMessage,
    RetryCount,
    // 追踪控制
    TraceLevel,
    SamplingRate,
    // 提供商信息
    ProviderTypeId,
    ProviderName,
    BackendKeyId,
    UpstreamAddr,
    // 详细时间追踪
    StartTime,
    EndTime,
    DurationMs,
    IsSuccess,
    // 阶段追踪数据
    PhasesData,
    PerformanceMetrics,
    Labels,
    // 健康状态评估
    HealthImpactScore,
    IsAnomaly,
    QualityMetrics,
    // 时间戳
    CreatedAt,
}

#[derive(DeriveIden)]
enum UserServiceApis {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum UserProviderKeys {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ProviderTypes {
    Table,
    Id,
}