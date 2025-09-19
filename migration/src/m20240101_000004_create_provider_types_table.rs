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
                    .col(ColumnDef::new(ProviderTypes::DefaultModel).string_len(100))
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
                        ColumnDef::new(ProviderTypes::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(ColumnDef::new(ProviderTypes::ConfigJson).json())
                    .col(ColumnDef::new(ProviderTypes::TokenMappingsJson).json())
                    .col(ColumnDef::new(ProviderTypes::ModelExtractionJson).json())
                    // 认证配置字段
                    .col(
                        ColumnDef::new(ProviderTypes::SupportedAuthTypes)
                            .json()
                            .not_null()
                            .default("[\"api_key\"]"),
                    )
                    .col(ColumnDef::new(ProviderTypes::AuthConfigsJson).json())
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

        manager
            .create_index(
                Index::create()
                    .name("idx_provider_types_supported_auth")
                    .table(ProviderTypes::Table)
                    .col(ProviderTypes::SupportedAuthTypes)
                    .to_owned(),
            )
            .await?;

        // 插入初始化数据，包含完整的动态配置支持
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
                        ProviderTypes::SupportedAuthTypes,
                        ProviderTypes::ConfigJson,
                        ProviderTypes::TokenMappingsJson,
                        ProviderTypes::ModelExtractionJson,
                        ProviderTypes::AuthConfigsJson,
                    ])
                    // OpenAI配置 - 支持API Key和OAuth2认证
                    .values_panic([
                        "openai".into(),
                        "OpenAI ChatGPT".into(),
                        "api.openai.com".into(),
                        "openai".into(),
                        "gpt-4.1".into(),
                        "[\"api_key\", \"oauth\"]".into(),
                        r#"{"request_stage":{"required_headers":{}},"response_stage":{}}"#.into(),
                        r#"{"tokens_prompt":{"type":"direct","path":"usage.prompt_tokens"},"tokens_completion":{"type":"direct","path":"usage.completion_tokens"},"tokens_total":{"type":"expression","formula":"usage.prompt_tokens + usage.completion_tokens","fallback":"usage.total_tokens"},"cache_create_tokens":{"type":"fallback","paths":["usage.prompt_tokens_details.cached_tokens","0"]},"cache_read_tokens":{"type":"fallback","paths":["usage.completion_tokens_details.accepted_prediction_tokens","0"]}}"#.into(),
                        r#"{"extraction_rules":[{"type":"body_json","path":"model","priority":1,"description":"从请求body提取模型名"},{"type":"query_param","parameter":"model","priority":2,"description":"从query参数提取模型名"}],"fallback_model":"gpt-3.5-turbo"}"#.into(),
                        r#"{"api_key": {}, "oauth": {"client_id": "app_EMoamEEZ73f0CkXaXp7hrann", "authorize_url": "https://auth.openai.com/oauth/authorize", "token_url": "https://auth.openai.com/oauth/token", "redirect_uri": "http://localhost:1455/auth/callback", "scopes": "openid profile email offline_access", "pkce_required": true, "extra_params": {"response_type": "code", "id_token_add_organizations": "true", "codex_cli_simplified_flow": "true"}}}"#.into(),
                    ])
                    // Gemini配置 - 支持多种认证方式：API Key, Google OAuth, Service Account, ADC
                    .values_panic([
                        "gemini".into(),
                        "Google Gemini".into(),
                        "cloudcode-pa.googleapis.com".into(),
                        "gemini".into(),
                        "gemini-2.5-flash".into(),
                        "[\"api_key\", \"oauth\", \"service_account\", \"adc\"]".into(),
                        r#"{"request_stage":{"required_headers":{}},"response_stage":{}}"#.into(),
                        r#"{"tokens_prompt":{"type":"direct","path":"usageMetadata.promptTokenCount"},"tokens_completion":{"type":"direct","path":"usageMetadata.candidatesTokenCount"},"tokens_total":{"type":"expression","formula":"usageMetadata.promptTokenCount + usageMetadata.candidatesTokenCount","fallback":"usageMetadata.totalTokenCount"},"cache_create_tokens":{"type":"default","value":0},"cache_read_tokens":{"type":"conditional","condition":"exists(usageMetadata.thoughtsTokenCount)","true_value":"usageMetadata.thoughtsTokenCount","false_value":0}}"#.into(),
                        r#"{"extraction_rules":[{"type":"body_json","path":"model","priority":1,"description":"从请求body提取模型名（最高优先级）"},{"type":"url_regex","pattern":"(?:/gemini)?/v1beta/models/([^:/?]+):(?:stream)?[gG]enerateContent","priority":2,"description":"从URL路径提取模型名（支持流式和非流式端点）"},{"type":"url_regex","pattern":"/v1beta/models/([^:/?]+):generateContent","priority":3,"description":"标准generateContent端点模型提取"},{"type":"url_regex","pattern":"/v1beta/models/([^:/?]+):([^/?]+)","priority":4,"description":"v1beta路径参数提取模型名"},{"type":"query_param","parameter":"model","priority":5,"description":"从query参数提取模型名"}],"fallback_model":"gemini-2.5-flash"}"#.into(),
                        r#"{"api_key": {}, "oauth": {"client_id": "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com", "client_secret": "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", "authorize_url": "https://accounts.google.com/o/oauth2/v2/auth", "token_url": "https://oauth2.googleapis.com/token", "redirect_uri": "https://codeassist.google.com/authcode", "scopes": "https://www.googleapis.com/auth/cloud-platform", "pkce_required": true, "extra_params": {"response_type": "code", "access_type": "offline", "prompt": "select_account", "include_granted_scopes": "true"}}, "service_account": {"token_url": "https://oauth2.googleapis.com/token", "scopes": "https://www.googleapis.com/auth/cloud-platform"}, "adc": {"scopes": "https://www.googleapis.com/auth/cloud-platform"}}"#.into(),
                    ])
                    // Claude配置 - 支持API Key和OAuth2认证
                    .values_panic([
                        "claude".into(),
                        "Anthropic Claude".into(),
                        "api.anthropic.com".into(),
                        "anthropic".into(),
                        "claude-3.5-sonnet".into(),
                        "[\"api_key\", \"oauth\"]".into(),
                        r#"{"request_stage":{"required_headers":{"anthropic-version":"2023-06-01"}},"response_stage":{}}"#.into(),
                        r#"{"tokens_prompt":{"type":"direct","path":"usage.input_tokens"},"tokens_completion":{"type":"direct","path":"usage.output_tokens"},"tokens_total":{"type":"expression","formula":"usage.input_tokens + usage.output_tokens","fallback":"usage.total_tokens"},"cache_create_tokens":{"type":"fallback","paths":["usage.cache_creation_input_tokens","0"]},"cache_read_tokens":{"type":"fallback","paths":["usage.cache_read_input_tokens","0"]}}"#.into(),
                        r#"{"extraction_rules":[{"type":"body_json","path":"model","priority":1,"description":"从请求body提取模型名"}],"fallback_model":"claude-3-sonnet"}"#.into(),
                        r#"{"api_key": {}, "oauth": {"client_id": "9d1c250a-e61b-44d9-88ed-5944d1962f5e", "authorize_url": "https://claude.ai/oauth/authorize", "token_url": "https://console.anthropic.com/v1/oauth/token", "redirect_uri": "https://console.anthropic.com/oauth/code/callback", "scopes": "org:create_api_key user:profile user:inference", "pkce_required": true, "extra_params": {"response_type": "code", "code": "true"}}}"#.into(),
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
    IsActive,
    ConfigJson,
    TokenMappingsJson,
    ModelExtractionJson,
    SupportedAuthTypes,
    AuthConfigsJson,
    CreatedAt,
    UpdatedAt,
}
