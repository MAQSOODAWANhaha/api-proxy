use sea_orm_migration::prelude::*;

/// ⚠️ 破坏性变更说明
///
/// `provider_types` 表已改为按 `auth_type` 分行（同一 `name` 可存在 `api_key` / `oauth` 两条记录），并引入
/// `(name, auth_type)` 唯一约束。
///
/// 注意：本仓库当前发布策略为“可重建部署”，不保证对已存在的旧数据库进行原地升级。
/// 如需升级，请重建数据库后重新执行迁移。
///
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
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::DisplayName)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::AuthType)
                            .string_len(50)
                            .not_null()
                            .default("api_key"),
                    )
                    .col(
                        ColumnDef::new(ProviderTypes::BaseUrl)
                            .string_len(255)
                            .not_null(),
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

        // 索引与唯一约束
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
                    .name("idx_provider_types_name")
                    .table(ProviderTypes::Table)
                    .col(ProviderTypes::Name)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_provider_types_name_auth")
                    .table(ProviderTypes::Table)
                    .col(ProviderTypes::Name)
                    .col(ProviderTypes::AuthType)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // 初始化数据：每个 provider 按 auth_type 拆分行
        manager
            .exec_stmt(
                Query::insert()
                    .into_table(ProviderTypes::Table)
                    .columns([
                        ProviderTypes::Name,
                        ProviderTypes::DisplayName,
                        ProviderTypes::AuthType,
                        ProviderTypes::BaseUrl,
                        ProviderTypes::ConfigJson,
                        ProviderTypes::TokenMappingsJson,
                        ProviderTypes::ModelExtractionJson,
                        ProviderTypes::AuthConfigsJson,
                    ])
                    // OpenAI - API Key
                    .values_panic([
                        "openai".into(),
                        "OpenAI ChatGPT".into(),
                        "api_key".into(),
                        "api.openai.com".into(),
                        r#"{"request_stage":{"required_headers":{}},"response_stage":{}}"#.into(),
                        r#"{"tokens_prompt":{"type":"direct","path":"response.usage.input_tokens","fallback":{"type":"default","value":0}},"tokens_completion":{"type":"direct","path":"response.usage.output_tokens","fallback":{"type":"default","value":0}},"tokens_total":{"type":"expression","formula":"response.usage.total_tokens","fallback":{"type":"expression","formula":"response.usage.input_tokens + response.usage.output_tokens"}},"cache_create_tokens":{"type":"default","value":0},"cache_read_tokens":{"type":"default","value":0}}"#.into(),
                        r#"{"extraction_rules":[{"type":"body_json","path":"model","priority":1,"description":"从请求body提取模型名"},{"type":"query_param","parameter":"model","priority":2,"description":"从query参数提取模型名"}],"fallback_model":"gpt-5"}"#.into(),
                        r#"{}"#.into(),
                    ])
                    // OpenAI - OAuth
                    .values_panic([
                        "openai".into(),
                        "OpenAI ChatGPT".into(),
                        "oauth".into(),
                        "chatgpt.com".into(),
                        r#"{"request_stage":{"required_headers":{}},"response_stage":{}}"#.into(),
                        r#"{"tokens_prompt":{"type":"direct","path":"response.usage.input_tokens","fallback":{"type":"default","value":0}},"tokens_completion":{"type":"direct","path":"response.usage.output_tokens","fallback":{"type":"default","value":0}},"tokens_total":{"type":"expression","formula":"response.usage.total_tokens","fallback":{"type":"expression","formula":"response.usage.input_tokens + response.usage.output_tokens"}},"cache_create_tokens":{"type":"default","value":0},"cache_read_tokens":{"type":"default","value":0}}"#.into(),
                        r#"{"extraction_rules":[{"type":"body_json","path":"model","priority":1,"description":"从请求body提取模型名"},{"type":"query_param","parameter":"model","priority":2,"description":"从query参数提取模型名"}],"fallback_model":"gpt-5"}"#.into(),
                        r#"{"client_id": "app_EMoamEEZ73f0CkXaXp7hrann", "authorize_url": "https://auth.openai.com/oauth/authorize", "token_url": "https://auth.openai.com/oauth/token", "redirect_uri": "http://localhost:1455/auth/callback", "scopes": "openid profile email offline_access", "pkce_required": true, "extra_params": {"response_type": "code", "id_token_add_organizations": "true", "codex_cli_simplified_flow": "true", "originator": "codex_cli_rs"}}"#.into(),
                    ])
                    // Gemini - API Key
                    .values_panic([
                        "gemini".into(),
                        "Google Gemini".into(),
                        "api_key".into(),
                        "generativelanguage.googleapis.com".into(),
                        r#"{"request_stage":{"required_headers":{}},"response_stage":{}}"#.into(),
                        r#"{"tokens_prompt":{"type":"direct","path":"response.usageMetadata.promptTokenCount","fallback":{"type":"direct","path":"usageMetadata.promptTokenCount"}},"tokens_completion":{"type":"direct","path":"response.usageMetadata.candidatesTokenCount","fallback":{"type":"direct","path":"usageMetadata.completion_tokens"}},"tokens_total":{"type":"expression","formula":"response.usageMetadata.totalTokenCount","fallback":{"type":"expression","formula":"response.usageMetadata.promptTokenCount + response.usageMetadata.candidatesTokenCount"}},"cache_create_tokens":{"type":"default","value":0},"cache_read_tokens":{"type":"default","value":0}}"#.into(),
                        r#"{"extraction_rules":[{"type":"body_json","path":"model","priority":1,"description":"从请求body提取模型名（最高优先级）"},{"type":"url_regex","pattern":"(?:/gemini)?/v1beta/models/([^:/?]+):(?:stream)?[gG]enerateContent","priority":2,"description":"从URL路径提取模型名（支持流式和非流式端点）"},{"type":"url_regex","pattern":"/v1beta/models/([^:/?]+):generateContent","priority":3,"description":"标准generateContent端点模型提取"},{"type":"url_regex","pattern":"/v1beta/models/([^:/?]+):([^/?]+)","priority":4,"description":"v1beta路径参数提取模型名"},{"type":"query_param","parameter":"model","priority":5,"description":"从query参数提取模型名"}],"fallback_model":"gemini-2.5-pro"}"#.into(),
                        r#"{}"#.into(),
                    ])
                    // Gemini - OAuth
                    .values_panic([
                        "gemini".into(),
                        "Google Gemini".into(),
                        "oauth".into(),
                        "cloudcode-pa.googleapis.com".into(),
                        r#"{"request_stage":{"required_headers":{}},"response_stage":{}}"#.into(),
                        r#"{"tokens_prompt":{"type":"direct","path":"response.usageMetadata.promptTokenCount","fallback":{"type":"direct","path":"usageMetadata.promptTokenCount"}},"tokens_completion":{"type":"direct","path":"response.usageMetadata.candidatesTokenCount","fallback":{"type":"direct","path":"usageMetadata.completion_tokens"}},"tokens_total":{"type":"expression","formula":"response.usageMetadata.totalTokenCount","fallback":{"type":"expression","formula":"response.usageMetadata.promptTokenCount + response.usageMetadata.candidatesTokenCount"}},"cache_create_tokens":{"type":"default","value":0},"cache_read_tokens":{"type":"default","value":0}}"#.into(),
                        r#"{"extraction_rules":[{"type":"body_json","path":"model","priority":1,"description":"从请求body提取模型名（最高优先级）"},{"type":"url_regex","pattern":"(?:/gemini)?/v1beta/models/([^:/?]+):(?:stream)?[gG]enerateContent","priority":2,"description":"从URL路径提取模型名（支持流式和非流式端点）"},{"type":"url_regex","pattern":"/v1beta/models/([^:/?]+):generateContent","priority":3,"description":"标准generateContent端点模型提取"},{"type":"url_regex","pattern":"/v1beta/models/([^:/?]+):([^/?]+)","priority":4,"description":"v1beta路径参数提取模型名"},{"type":"query_param","parameter":"model","priority":5,"description":"从query参数提取模型名"}],"fallback_model":"gemini-2.5-pro"}"#.into(),
                        r#"{"client_id": "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com", "client_secret": "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", "authorize_url": "https://accounts.google.com/o/oauth2/v2/auth", "token_url": "https://oauth2.googleapis.com/token", "redirect_uri": "https://codeassist.google.com/authcode", "scopes": "https://www.googleapis.com/auth/cloud-platform", "pkce_required": true, "extra_params": {"response_type": "code", "access_type": "offline", "prompt": "select_account"}}"#.into(),
                    ])
                    // Claude - API Key
                    .values_panic([
                        "anthropic".into(),
                        "Anthropic Claude".into(),
                        "api_key".into(),
                        "api.anthropic.com".into(),
                        r#"{"request_stage":{"required_headers":{"anthropic-version":"2023-06-01"}},"response_stage":{}}"#.into(),
                        r#"{"tokens_prompt":{"type":"direct","path":"usage.input_tokens","fallback":{"type":"direct","path":"usage.prompt_tokens"}},"tokens_completion":{"type":"direct","path":"usage.output_tokens","fallback":{"type":"direct","path":"usage.completion_tokens"}},"tokens_total":{"type":"expression","formula":"usage.total_tokens","fallback":{"type":"expression","formula":"usage.input_tokens + usage.output_tokens"}},"cache_create_tokens":{"type":"direct","path":"usage.cache_creation_input_tokens","fallback":{"type":"direct","path":"usage.prompt_tokens_details.cached_tokens"}},"cache_read_tokens":{"type":"direct","path":"usage.cache_read_input_tokens","fallback":{"type":"direct","path":"usage.cached_tokens"}}}"#.into(),
                        r#"{"extraction_rules":[{"type":"body_json","path":"model","priority":1,"description":"从请求body提取模型名"}],"fallback_model":"claude-4-sonnet"}"#.into(),
                        r#"{}"#.into(),
                    ])
                    // Claude - OAuth
                    .values_panic([
                        "anthropic".into(),
                        "Anthropic Claude".into(),
                        "oauth".into(),
                        "api.anthropic.com".into(),
                        r#"{"request_stage":{"required_headers":{"anthropic-version":"2023-06-01"}},"response_stage":{}}"#.into(),
                        r#"{"tokens_prompt":{"type":"direct","path":"usage.input_tokens","fallback":{"type":"direct","path":"usage.prompt_tokens"}},"tokens_completion":{"type":"direct","path":"usage.output_tokens","fallback":{"type":"direct","path":"usage.completion_tokens"}},"tokens_total":{"type":"expression","formula":"usage.total_tokens","fallback":{"type":"expression","formula":"usage.input_tokens + usage.output_tokens"}},"cache_create_tokens":{"type":"direct","path":"usage.cache_creation_input_tokens","fallback":{"type":"direct","path":"usage.prompt_tokens_details.cached_tokens"}},"cache_read_tokens":{"type":"direct","path":"usage.cache_read_input_tokens","fallback":{"type":"direct","path":"usage.cached_tokens"}}}"#.into(),
                        r#"{"extraction_rules":[{"type":"body_json","path":"model","priority":1,"description":"从请求body提取模型名"}],"fallback_model":"claude-4-sonnet"}"#.into(),
                        r#"{"client_id": "9d1c250a-e61b-44d9-88ed-5944d1962f5e", "authorize_url": "https://claude.ai/oauth/authorize", "token_url": "https://console.anthropic.com/v1/oauth/token", "redirect_uri": "https://console.anthropic.com/oauth/code/callback", "scopes": "org:create_api_key user:profile user:inference", "pkce_required": true, "extra_params": {"response_type": "code", "code": "true"}}"#.into(),
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
    AuthType,
    BaseUrl,
    IsActive,
    ConfigJson,
    TokenMappingsJson,
    ModelExtractionJson,
    AuthConfigsJson,
    CreatedAt,
    UpdatedAt,
}
