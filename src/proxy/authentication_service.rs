//! # 代理端认证服务
//!
//! 职责：作为认证与授权中心，全权负责所有认证、授权、凭证管理和限流逻辑。

use crate::auth::{
    api_key_usage_limit_service::ApiKeyUsageLimitService,
    service::ApiKeyAuthenticationService,
    types::{AuthStatus, AuthType},
};
use crate::cache::CacheManager;
use crate::error::{
    Context, ProxyError, Result,
    auth::{AuthError, OAuthError, UsageLimitInfo, UsageLimitKind},
    config::ConfigError,
};
use crate::key_pool::{ApiKeySchedulerService, SelectionContext};
use crate::logging::{LogComponent, LogStage};
use crate::proxy::context::{ProxyContext, ResolvedCredential};
use crate::proxy::response::format_rate_limit_message;
use crate::types::ProviderTypeId;
use crate::{ldebug, linfo, lwarn};
use entity::{
    oauth_client_sessions::{self, Entity as OAuthClientSessions},
    provider_types::{self, Entity as ProviderTypes},
    user_provider_keys,
    user_service_apis::{self},
};
use pingora_proxy::Session;
use sea_orm::prelude::Decimal;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::sync::Arc;
use std::time::Duration;

/// 认证信息来源类型
#[derive(Debug, Clone)]
pub enum AuthSource {
    Query,  // 查询参数
    Header, // 头部
}

impl std::fmt::Display for AuthSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Query => write!(f, "query"),
            Self::Header => write!(f, "header"),
        }
    }
}

/// 检测到的认证信息
#[derive(Debug, Clone)]
pub struct Authorization {
    /// 检测到的认证值
    pub auth_value: String,
    /// 认证信息来源
    pub source: AuthSource,
    /// 具体位置(header名称/query参数/json路径)
    pub location: String,
}

/// 代理端认证服务
///
/// 职责：作为认证与授权中心，全权负责所有认证、授权、凭证管理和限流逻辑。
pub struct AuthenticationService {
    auth_service: Arc<ApiKeyAuthenticationService>,
    db: Arc<DatabaseConnection>,
    cache: Arc<CacheManager>,
    api_key_scheduler_service: Arc<ApiKeySchedulerService>,
    rate_limiter: Arc<ApiKeyUsageLimitService>,
}

impl AuthenticationService {
    /// 创建新的认证服务
    pub const fn new(
        auth_service: Arc<ApiKeyAuthenticationService>,
        db: Arc<DatabaseConnection>,
        cache: Arc<CacheManager>,
        api_key_pool: Arc<ApiKeySchedulerService>,
        rate_limiter: Arc<ApiKeyUsageLimitService>,
    ) -> Self {
        Self {
            auth_service,
            db,
            cache,
            api_key_scheduler_service: api_key_pool,
            rate_limiter,
        }
    }

    #[must_use]
    pub fn db(&self) -> Arc<DatabaseConnection> {
        self.db.clone()
    }

    /// 执行完整的认证和授权流程, 直接填充 `ProxyContext`
    pub async fn authenticate_and_authorize(
        &self,
        session: &mut Session,
        ctx: &mut ProxyContext,
    ) -> Result<()> {
        // 1. 认证入口API Key
        let user_api = self
            .authenticate_entry_api(session, &ctx.request_id)
            .await?;

        // 2. 检查速率限制和配额
        self.check_limits(&user_api, &ctx.request_id).await?;

        // 3. 获取提供商配置
        let provider_type = self.get_provider_type(user_api.provider_type_id).await?;

        // 4. 选择后端密钥
        let route_group = session.req_header().uri.path().to_string();
        let selected_backend = self
            .select_api_key(&user_api, &ctx.request_id, route_group)
            .await?;

        // 5. 解析最终凭证
        let resolved_credential = self
            .resolve_credential(&selected_backend, &ctx.request_id)
            .await?;

        // 6. 填充上下文
        ctx.user_service_api = Some(user_api);
        ctx.provider_type = Some(provider_type);
        ctx.selected_backend = Some(selected_backend);
        ctx.resolved_credential = Some(resolved_credential);

        Ok(())
    }

    /// 1. 仅进行入口 API Key 认证
    async fn authenticate_entry_api(
        &self,
        session: &Session,
        request_id: &str,
    ) -> Result<user_service_apis::Model> {
        let user_auth = Self::detect_user_auth_from_request(session)?;
        let proxy_auth_result = self
            .auth_service
            .authenticate_proxy_request(&user_auth.auth_value)
            .await?;

        linfo!(
            request_id,
            LogStage::Authentication,
            LogComponent::Auth,
            "entry_auth_success",
            "入口API认证成功",
            user_id = proxy_auth_result.user_id,
            user_service_api_id = proxy_auth_result.user_api.id
        );

        Ok(proxy_auth_result.user_api)
    }

    /// 从请求中检测用户认证信息
    fn detect_user_auth_from_request(session: &Session) -> Result<Authorization> {
        let req_header = session.req_header();

        if let Some(query) = req_header.uri.query() {
            for param_pair in query.split('&') {
                if let Some((key, value)) = param_pair.split_once('=') {
                    match key {
                        "key" | "access_token" | "api_key" | "apikey" => {
                            let decoded = urlencoding::decode(value).map_err(|err| {
                                AuthError::Message(format!(
                                    "Failed to decode query parameter '{key}': {err}"
                                ))
                            })?;
                            return Ok(Authorization {
                                auth_value: decoded.to_string(),
                                source: AuthSource::Query,
                                location: key.to_string(),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }

        let auth_headers = [
            "authorization",
            "x-api-key",
            "x-goog-api-key",
            "x-openai-api-key",
        ];
        for header_name in &auth_headers {
            if let Some(header_value) = req_header.headers.get(*header_name)
                && let Ok(header_str) = std::str::from_utf8(header_value.as_bytes())
            {
                let auth_value = if *header_name == "authorization" {
                    header_str
                        .strip_prefix("Bearer ")
                        .unwrap_or(header_str)
                        .trim()
                        .to_string()
                } else {
                    header_str.trim().to_string()
                };
                if !auth_value.is_empty() {
                    return Ok(Authorization {
                        auth_value,
                        source: AuthSource::Header,
                        location: (*header_name).to_string(),
                    });
                }
            }
        }

        Err(AuthError::ApiKeyMissing.into())
    }

    /// 2. 检查所有限制 (Redis for freq, DB for usage)
    async fn check_limits(
        &self,
        user_api: &user_service_apis::Model,
        request_id: &str,
    ) -> Result<()> {
        if let Some(expires_at) = &user_api.expires_at
            && chrono::Utc::now().naive_utc() > *expires_at
        {
            return Err(
                crate::error::auth::AuthError::Message("该 API Key 已过期".to_string()).into(),
            );
        }

        let endpoint_key = format!("service_api:{}", user_api.id);

        // 1. MaxRequestPerMin
        if let Some(rate_limit) = user_api.max_request_per_min
            && rate_limit > 0
        {
            let outcome = self
                .rate_limiter
                .check_per_minute(user_api.user_id, &endpoint_key, i64::from(rate_limit))
                .await?;
            if !outcome.allowed {
                let resets = u64::try_from(outcome.ttl_seconds)
                    .ok()
                    .map(Duration::from_secs);
                Self::log_rate_limit_hit(
                    request_id,
                    UsageLimitKind::PerMinute,
                    Some(Self::to_f64(outcome.limit)),
                    Some(Self::to_f64(outcome.current)),
                    resets,
                );
                return Err(ApiKeyUsageLimitService::rate_limit_error(
                    UsageLimitKind::PerMinute,
                    Some(Self::to_f64(outcome.limit)),
                    Some(Self::to_f64(outcome.current)),
                    resets,
                ));
            }
        }

        // 2. MaxRequestsPerDay
        if let Some(daily_limit) = user_api.max_requests_per_day
            && daily_limit > 0
        {
            let outcome = self
                .rate_limiter
                .check_per_day(user_api.user_id, &endpoint_key, i64::from(daily_limit))
                .await?;
            if !outcome.allowed {
                let resets = u64::try_from(outcome.ttl_seconds)
                    .ok()
                    .map(Duration::from_secs);
                Self::log_rate_limit_hit(
                    request_id,
                    UsageLimitKind::DailyRequests,
                    Some(Self::to_f64(outcome.limit)),
                    Some(Self::to_f64(outcome.current)),
                    resets,
                );
                return Err(ApiKeyUsageLimitService::rate_limit_error(
                    UsageLimitKind::DailyRequests,
                    Some(Self::to_f64(outcome.limit)),
                    Some(Self::to_f64(outcome.current)),
                    resets,
                ));
            }
        }

        // 3. MaxTokensPerDay
        if let Some(max_tokens) = user_api.max_tokens_per_day
            && max_tokens > 0
            && let Err(err) = self
                .rate_limiter
                .check_daily_token_limit(user_api.id, max_tokens)
                .await
        {
            if let ProxyError::Authentication(AuthError::UsageLimitExceeded(info)) = &err {
                Self::log_rate_limit_hit(
                    request_id,
                    info.kind,
                    info.limit,
                    info.current,
                    info.resets_in,
                );
            }
            return Err(err);
        }

        // 4. MaxCostPerDay
        if let Some(max_cost) = user_api.max_cost_per_day
            && max_cost > Decimal::from(0)
            && let Err(err) = self
                .rate_limiter
                .check_daily_cost_limit(user_api.id, max_cost)
                .await
        {
            if let ProxyError::Authentication(AuthError::UsageLimitExceeded(info)) = &err {
                Self::log_rate_limit_hit(
                    request_id,
                    info.kind,
                    info.limit,
                    info.current,
                    info.resets_in,
                );
            }
            return Err(err);
        }

        Ok(())
    }

    fn log_rate_limit_hit(
        request_id: &str,
        kind: UsageLimitKind,
        limit: Option<f64>,
        current: Option<f64>,
        resets: Option<Duration>,
    ) {
        let kind_label = match kind {
            UsageLimitKind::PerMinute => "每分钟请求",
            UsageLimitKind::DailyRequests => "每日请求次数",
            UsageLimitKind::DailyTokens => "每日 Token 用量",
            UsageLimitKind::DailyCost => "每日成本",
        };
        let info = UsageLimitInfo {
            kind,
            limit,
            current,
            resets_in: resets,
            plan_type: ApiKeyUsageLimitService::PLAN_TYPE.to_string(),
        };
        let message = format_rate_limit_message(&info);
        let resets_secs = resets.map(|d| d.as_secs());
        lwarn!(
            request_id,
            LogStage::Authentication,
            LogComponent::Auth,
            "usage_limit_reached",
            message,
            limit_kind = kind_label,
            limit = limit,
            current = current,
            resets_in_seconds = resets_secs
        );
    }

    #[allow(clippy::cast_precision_loss)]
    const fn to_f64(value: i64) -> f64 {
        value as f64
    }

    /// 3. 获取提供商类型配置
    async fn get_provider_type(
        &self,
        provider_type_id: ProviderTypeId,
    ) -> Result<provider_types::Model> {
        let cache_key = format!("provider_type:{provider_type_id}");
        if let Ok(Some(provider_type)) = self
            .cache
            .provider()
            .get::<provider_types::Model>(&cache_key)
            .await
        {
            return Ok(provider_type);
        }
        let provider_type = ProviderTypes::find_by_id(provider_type_id)
            .one(&*self.db)
            .await
            .context("Failed to fetch provider type")?
            .ok_or_else(|| {
                ConfigError::Load(format!("Provider type not found: {provider_type_id}"))
            })?;
        let _ = self
            .cache
            .provider()
            .set(&cache_key, &provider_type, Some(Duration::from_secs(1800)))
            .await;
        Ok(provider_type)
    }

    /// 4. 根据用户API配置选择合适的API密钥
    async fn select_api_key(
        &self,
        user_service_api: &user_service_apis::Model,
        request_id: &str,
        route_group: String,
    ) -> Result<user_provider_keys::Model> {
        let context = SelectionContext::new(
            request_id.to_string(),
            user_service_api.user_id,
            user_service_api.id,
            user_service_api.provider_type_id,
            route_group,
        );
        let result = self
            .api_key_scheduler_service
            .select_api_key_from_service_api(user_service_api, &context)
            .await?;
        ldebug!(
            request_id,
            LogStage::Authentication,
            LogComponent::Auth,
            "api_key_selected",
            "API密钥选择完成",
            selected_key_id = result.selected_key.id,
            strategy = result.strategy.as_str()
        );
        Ok(result.selected_key)
    }

    /// 5. 解析最终凭证
    async fn resolve_credential(
        &self,
        selected_backend: &user_provider_keys::Model,
        request_id: &str,
    ) -> Result<ResolvedCredential> {
        match AuthType::from(selected_backend.auth_type.as_str()) {
            Some(AuthType::ApiKey) => {
                Ok(ResolvedCredential::ApiKey(selected_backend.api_key.clone()))
            }
            Some(AuthType::OAuth) => {
                let token = self
                    .resolve_oauth_access_token(&selected_backend.api_key, request_id)
                    .await?;
                Ok(ResolvedCredential::OAuthAccessToken(token))
            }
            _ => Err(AuthError::Message(format!(
                "Unsupported auth type: {}",
                selected_backend.auth_type
            ))
            .into()),
        }
    }

    /// 解析 OAuth 会话，返回 `access_token`
    async fn resolve_oauth_access_token(
        &self,
        session_id: &str,
        _request_id: &str,
    ) -> Result<String> {
        let session = OAuthClientSessions::find()
            .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
            .one(self.db.as_ref())
            .await
            .context(format!("Failed to fetch OAuth session '{session_id}'"))?
            .ok_or_else(|| OAuthError::InvalidSession(session_id.to_string()))?;
        if session.status != AuthStatus::Authorized.to_string() {
            return Err(AuthError::Message(format!(
                "OAuth session {session_id} is not authorized"
            ))
            .into());
        }
        let token = session.access_token.clone().ok_or_else(|| {
            crate::error::ProxyError::from(crate::error::auth::AuthError::Message(
                "OAuth session has no access_token".to_string(),
            ))
        })?;
        if session.expires_at <= chrono::Utc::now().naive_utc() {
            return Err(crate::error::auth::AuthError::Message(
                "OAuth access token expired".to_string(),
            )
            .into());
        }
        Ok(token)
    }
}
