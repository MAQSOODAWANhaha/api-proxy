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
                    .col(
                        ColumnDef::new(ProviderTypes::AuthHeaderFormat)
                            .string_len(255)
                            .not_null()
                            .default("Authorization: Bearer {token}"),
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
                        ProviderTypes::AuthHeaderFormat,
                        ProviderTypes::ConfigJson,
                        ProviderTypes::TokenMappingsJson,
                        ProviderTypes::ModelExtractionJson,
                        ProviderTypes::AuthConfigsJson,
                    ])
                    // OpenAI配置 - 标准OpenAI兼容格式，使用最新GPT-4.1
                    .values_panic([
                        "openai".into(),
                        "OpenAI ChatGPT".into(),
                        "api.openai.com".into(),
                        "openai".into(),
                        "gpt-4.1".into(),
                        "[\"api_key\"]".into(),
                        "Authorization: Bearer {key}".into(),
                        r#"{"streaming":{"supported":true,"content_type":"text/event-stream","chunk_prefix":"data: ","end_marker":"data: [DONE]"},"request_transform":{"default_parameters":{"stream":false,"max_tokens":4096,"temperature":0.7}},"response_transform":{"extract_content":"choices[0].message.content","extract_usage":"usage"},"supported_models":["gpt-4.1","gpt-4.1-mini","gpt-4.1-nano","gpt-4o","gpt-4-turbo","gpt-3.5-turbo","o4-mini","o3","o3-pro"],"field_mappings":{"input_tokens":"usage.prompt_tokens","output_tokens":"usage.completion_tokens","total_tokens":"usage.total_tokens","model_name":"model","content":"choices[0].message.content","finish_reason":"choices[0].finish_reason","cost":"usage.total_cost","cache_create_tokens":"usage.prompt_tokens_details.cached_tokens","cache_read_tokens":"usage.completion_tokens_details.accepted_prediction_tokens","error_type":"error.type","error_message":"error.message"},"default_values":{"cost_currency":"USD","cache_create_tokens":0,"cache_read_tokens":0},"transformations":{"cost":"divide:1000000"}}"#.into(),
                        r#"{"tokens_prompt":{"type":"direct","path":"usage.prompt_tokens"},"tokens_completion":{"type":"direct","path":"usage.completion_tokens"},"tokens_total":{"type":"expression","formula":"usage.prompt_tokens + usage.completion_tokens","fallback":"usage.total_tokens"},"cache_create_tokens":{"type":"fallback","paths":["usage.prompt_tokens_details.cached_tokens","0"]},"cache_read_tokens":{"type":"fallback","paths":["usage.completion_tokens_details.accepted_prediction_tokens","0"]}}"#.into(),
                        r#"{"extraction_rules":[{"type":"body_json","path":"model","priority":1,"description":"从请求body提取模型名"},{"type":"query_param","parameter":"model","priority":2,"description":"从query参数提取模型名"}],"fallback_model":"gpt-3.5-turbo"}"#.into(),
                        r#"{"api_key": {}}"#.into(),
                    ])
                    // Gemini配置 - 支持多种认证方式：API Key, Google OAuth, Service Account, ADC
                    .values_panic([
                        "gemini".into(),
                        "Google Gemini".into(),
                        "generativelanguage.googleapis.com".into(),
                        "gemini".into(),
                        "gemini-2.5-flash".into(),
                        "[\"api_key\", \"google_oauth\", \"service_account\", \"adc\"]".into(),
                        "X-goog-api-key: {key}".into(),
                        r#"{"streaming":{"supported":true,"content_type":"text/event-stream","chunk_format":"gemini_sse"},"request_transform":{"message_format":"contents","default_parameters":{"generationConfig":{"maxOutputTokens":4096,"temperature":0.7},"safetySettings":[{"category":"HARM_CATEGORY_HARASSMENT","threshold":"BLOCK_MEDIUM_AND_ABOVE"}]}},"response_transform":{"extract_content":"candidates[0].content.parts[0].text","extract_usage":"usageMetadata"},"supported_models":["gemini-2.5-flash","gemini-2.5-flash-lite","gemini-2.5-pro","gemini-2.0-flash","gemini-2.0-pro","gemini-1.5-pro","gemini-1.5-flash","gemini-pro"],"field_mappings":{"input_tokens":"usageMetadata.promptTokenCount","output_tokens":"usageMetadata.candidatesTokenCount","total_tokens":"usageMetadata.totalTokenCount","model_name":"model","content":"candidates[0].content.parts[0].text","finish_reason":"candidates[0].finishReason","cost":"usageMetadata.totalCost","error_type":"error.code","error_message":"error.message"},"default_values":{"cost_currency":"USD","cache_create_tokens":0,"cache_read_tokens":0,"input_tokens":0,"output_tokens":0},"transformations":{}}"#.into(),
                        r#"{"tokens_prompt":{"type":"direct","path":"usageMetadata.promptTokenCount"},"tokens_completion":{"type":"direct","path":"usageMetadata.candidatesTokenCount"},"tokens_total":{"type":"expression","formula":"usageMetadata.promptTokenCount + usageMetadata.candidatesTokenCount","fallback":"usageMetadata.totalTokenCount"},"cache_create_tokens":{"type":"default","value":0},"cache_read_tokens":{"type":"conditional","condition":"exists(usageMetadata.thoughtsTokenCount)","true_value":"usageMetadata.thoughtsTokenCount","false_value":0}}"#.into(),
                        r#"{"extraction_rules":[{"type":"url_regex","pattern":"/v1beta/models/([^:]+):generateContent","priority":1,"description":"从URL路径提取模型名"},{"type":"body_json","path":"model","priority":2,"description":"从请求body提取模型名"}],"fallback_model":"gemini-pro"}"#.into(),
                        r#"{"api_key": {}, "google_oauth": {"client_id": "60955708087-hvqh1eo54rqin4bafg42sbsfl6fgc4hn.apps.googleusercontent.com", "authorize_url": "https://accounts.google.com/o/oauth2/v2/auth", "token_url": "https://oauth2.googleapis.com/token", "scopes": "https://www.googleapis.com/auth/generative-language openid email profile", "pkce_required": true, "access_type": "offline", "prompt": "select_account"}, "service_account": {"token_url": "https://oauth2.googleapis.com/token", "scopes": "https://www.googleapis.com/auth/generative-language"}, "adc": {"scopes": "https://www.googleapis.com/auth/generative-language"}}"#.into(),
                    ])
                    // Claude配置 - 支持API Key和OAuth2认证
                    .values_panic([
                        "claude".into(),
                        "Anthropic Claude".into(),
                        "api.anthropic.com".into(),
                        "anthropic".into(),
                        "claude-3.5-sonnet".into(),
                        "[\"api_key\", \"oauth2\"]".into(),
                        "Authorization: Bearer {key}".into(),
                        r#"{"streaming":{"supported":true,"content_type":"text/event-stream","chunk_format":"anthropic_sse"},"request_transform":{"message_format":"messages","default_parameters":{"max_tokens":4096,"anthropic_version":"2023-06-01"},"required_headers":{"anthropic-version":"2023-06-01"}},"response_transform":{"extract_content":"content[0].text","extract_usage":"usage"},"supported_models":["claude-4.1","claude-4","claude-3.7-sonnet","claude-3.5-sonnet","claude-3.5-haiku","claude-3-opus-20240229","claude-3-sonnet-20240229","claude-3-haiku-20240307"],"field_mappings":{"input_tokens":"usage.input_tokens","output_tokens":"usage.output_tokens","total_tokens":"usage.total_tokens","model_name":"model","content":"content[0].text","finish_reason":"stop_reason","cost":"billing.subtotal","cache_create_tokens":"usage.cache_creation_input_tokens","cache_read_tokens":"usage.cache_read_input_tokens","error_type":"error.type","error_message":"error.message"},"default_values":{"cost_currency":"USD","cache_create_tokens":0,"cache_read_tokens":0},"transformations":{"cost":"divide:1000"}}"#.into(),
                        r#"{"tokens_prompt":{"type":"direct","path":"usage.input_tokens"},"tokens_completion":{"type":"direct","path":"usage.output_tokens"},"tokens_total":{"type":"expression","formula":"usage.input_tokens + usage.output_tokens","fallback":"usage.total_tokens"},"cache_create_tokens":{"type":"fallback","paths":["usage.cache_creation_input_tokens","0"]},"cache_read_tokens":{"type":"fallback","paths":["usage.cache_read_input_tokens","0"]}}"#.into(),
                        r#"{"extraction_rules":[{"type":"body_json","path":"model","priority":1,"description":"从请求body提取模型名"}],"fallback_model":"claude-3-sonnet"}"#.into(),
                        r#"{"api_key": {}, "oauth2": {"client_id": "9d1c250a-e61b-44d9-88ed-5944d1962f5e", "authorize_url": "https://claude.ai/oauth/authorize", "token_url": "https://console.anthropic.com/oauth/token", "scopes": "user:inference", "pkce_required": true, "response_type": "code", "grant_type": "authorization_code"}}"#.into(),
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
    AuthHeaderFormat,
    AuthConfigsJson,
    CreatedAt,
    UpdatedAt,
}
