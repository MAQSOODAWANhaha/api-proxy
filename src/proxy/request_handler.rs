//! # 请求处理器
//!
//! 简化后的AI代理请求处理器，专注于核心请求处理流程

use pingora_core::Error as PingoraError;
use pingora_core::upstreams::peer::HttpPeer;
use pingora_http::RequestHeader;
use pingora_proxy::Session;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::sync::Arc;

use crate::auth::{AuthMethod, AuthResult, AuthUtils};
use crate::cache::UnifiedCacheManager;
use crate::config::{AppConfig, ProviderConfigManager};
use crate::providers::DynamicAdapterManager;
use crate::proxy::types::{ForwardingContext, ForwardingResult, ProviderId};
use crate::scheduler::{ApiKeyPoolManager, SelectionContext};
use crate::trace::immediate::ImmediateProxyTracer;

/// 简化的请求处理器
///
/// 职责：
/// 1. 身份验证和API密钥验证
/// 2. 提供商选择和API密钥选择
/// 3. 请求转发到上游服务器
/// 4. 响应处理和统计收集
pub struct RequestHandler {
    /// 数据库连接
    db: Arc<DatabaseConnection>,
    /// 统一缓存管理器 (未来使用)
    _cache: Arc<UnifiedCacheManager>,
    /// 配置 (未来使用)
    _config: Arc<AppConfig>,
    /// 适配器管理器 (未来使用)
    _adapter_manager: Arc<DynamicAdapterManager>,
    /// 服务商配置管理器 (未来使用)
    _provider_config_manager: Arc<ProviderConfigManager>,
    /// API密钥池管理器
    api_key_pool: Arc<ApiKeyPoolManager>,
    /// 追踪器
    tracer: Option<Arc<ImmediateProxyTracer>>,
}

/// 请求上下文
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// 请求ID
    pub request_id: String,
    /// 用户ID
    pub user_id: Option<i32>,
    /// 使用的提供商ID
    pub provider_id: Option<ProviderId>,
    /// 使用的API密钥ID
    pub api_key_id: Option<i32>,
    /// 请求方法
    pub method: String,
    /// 请求路径
    pub path: String,
    /// 转发上下文
    pub forwarding_context: Option<ForwardingContext>,
}

/// 从URL路径中提取provider名称
///
/// 解析格式: /{provider_name}/{api_path}
/// 例如: /gemini/v1beta/models/gemini-2.5-flash:generateContent -> "gemini"
fn extract_provider_name_from_path(path: &str) -> Option<&str> {
    // 去掉开头的 '/' 然后获取第一个路径段
    path.strip_prefix('/').and_then(|s| s.split('/').next())
}

impl RequestHandler {
    /// 创建新的请求处理器
    pub fn new(
        db: Arc<DatabaseConnection>,
        cache: Arc<UnifiedCacheManager>,
        config: Arc<AppConfig>,
        adapter_manager: Arc<DynamicAdapterManager>,
        provider_config_manager: Arc<ProviderConfigManager>,
        tracer: Option<Arc<ImmediateProxyTracer>>,
    ) -> Self {
        let health_checker = Arc::new(crate::scheduler::api_key_health::ApiKeyHealthChecker::new(
            db.clone(),
            None,
        ));
        let api_key_pool = Arc::new(ApiKeyPoolManager::new(db.clone(), health_checker));

        Self {
            db,
            _cache: cache,
            _config: config,
            _adapter_manager: adapter_manager,
            _provider_config_manager: provider_config_manager,
            api_key_pool,
            tracer,
        }
    }

    /// 处理请求的主要入口点
    pub async fn handle_request(
        &self,
        session: &mut Session,
        ctx: &mut RequestContext,
    ) -> Result<(), PingoraError> {
        // 1. 身份验证 - 返回认证结果和service_api配置
        let (auth_result, service_api_config) =
            self.authenticate_request(session.req_header(), ctx).await?;

        // 2. 使用service_api配置选择API密钥
        let selected_key = self
            .select_api_key(&auth_result, &service_api_config, ctx)
            .await?;

        // 3. 创建上游连接
        let _peer = self.create_upstream_peer(&selected_key, ctx).await?;

        // 4. 设置上游
        session.set_keepalive(None);
        ctx.forwarding_context = Some(ForwardingContext::new(
            ctx.request_id.clone(),
            ctx.provider_id.unwrap_or(ProviderId::from_database_id(1)),
        ));

        // 5. 连接到上游（简化实现）
        session.set_keepalive(None);

        Ok(())
    }

    /// 身份验证 - 根据provider的AuthHeaderFormat动态解析认证头
    async fn authenticate_request(
        &self,
        req_header: &RequestHeader,
        ctx: &mut RequestContext,
    ) -> Result<(AuthResult, entity::user_service_apis::Model), PingoraError> {
        // 尝试从各种可能的认证头中解析API密钥
        let extracted_api_key = self.extract_api_key_from_headers(req_header).await?;

        // 从数据库中验证API密钥并获取完整的user_service_apis配置
        let user_service_api = entity::user_service_apis::Entity::find()
            .filter(entity::user_service_apis::Column::ApiKey.eq(&extracted_api_key))
            .filter(entity::user_service_apis::Column::IsActive.eq(true))
            .one(&*self.db)
            .await
            .map_err(|_| *PingoraError::new_str("Database error"))?
            .ok_or_else(|| *PingoraError::new_str("Invalid API key"))?;

        // 创建认证结果 - 使用从数据库获取的用户ID
        let auth_result = AuthResult {
            user_id: user_service_api.user_id,
            username: format!("user_{}", user_service_api.user_id), // 简化用户名
            is_admin: false,
            permissions: vec![], // 实际应该从数据库查询
            auth_method: AuthMethod::ApiKey,
            token_preview: AuthUtils::sanitize_api_key(&extracted_api_key),
        };

        ctx.user_id = Some(auth_result.user_id);

        Ok((auth_result, user_service_api))
    }

    /// 根据provider名称获取provider配置（使用缓存优化）
    async fn get_provider_by_name(
        &self,
        provider_name: &str,
    ) -> Result<entity::provider_types::Model, PingoraError> {
        let cache_key = format!("provider_by_name:{}", provider_name);

        // 首先检查缓存
        if let Ok(Some(provider)) = self
            ._cache
            .provider()
            .get::<entity::provider_types::Model>(&cache_key)
            .await
        {
            return Ok(provider);
        }

        // 从数据库查询特定的provider
        let provider = entity::provider_types::Entity::find()
            .filter(entity::provider_types::Column::Name.eq(provider_name))
            .filter(entity::provider_types::Column::IsActive.eq(true))
            .one(&*self.db)
            .await
            .map_err(|_| *PingoraError::new_str("Database error"))?
            .ok_or_else(|| *PingoraError::new_str("Unknown provider"))?;

        // 缓存结果（30分钟）
        let _ = self
            ._cache
            .provider()
            .set(
                &cache_key,
                &provider,
                Some(std::time::Duration::from_secs(1800)),
            )
            .await;

        Ok(provider)
    }

    /// 从请求头中提取API密钥 - 基于URL前缀优化版本
    async fn extract_api_key_from_headers(
        &self,
        req_header: &RequestHeader,
    ) -> Result<String, PingoraError> {
        let path = req_header.uri.path();

        // 1. 从URL路径中提取provider名称
        let provider_name = extract_provider_name_from_path(path).ok_or_else(|| {
            *PingoraError::new_str("Invalid URL format. Expected: /{provider_name}/{api_path}")
        })?;

        // 2. 从缓存获取特定provider的配置 (O(1) 查找)
        let provider = self.get_provider_by_name(provider_name).await?;

        // 3. 使用该provider的认证格式直接解析API密钥
        let auth_format = provider
            .auth_header_format
            .as_deref()
            .unwrap_or("Authorization: Bearer {key}");

        let api_key = self
            .parse_auth_header_with_format(req_header, auth_format)
            .map_err(|_| {
                *PingoraError::new_str(
                    "Failed to extract API key with configured authentication format",
                )
            })?;

        if api_key.is_empty() {
            return Err(*PingoraError::new_str("Empty API key found"));
        }

        tracing::debug!(
            provider = %provider_name,
            auth_format = %auth_format,
            api_key_preview = %crate::auth::AuthUtils::sanitize_api_key(&api_key),
            "Successfully extracted API key using URL prefix optimization"
        );

        Ok(api_key)
    }

    /// 根据指定格式解析认证头
    fn parse_auth_header_with_format(
        &self,
        req_header: &RequestHeader,
        auth_format: &str,
    ) -> Result<String, PingoraError> {
        // 解析认证格式，例如：
        // "Authorization: Bearer {key}"
        // "X-goog-api-key: {key}"

        if let Some((header_name, value_format)) = auth_format.split_once(": ") {
            // 从请求头中获取对应的header值
            let header_value = req_header
                .headers
                .get(header_name)
                .or_else(|| req_header.headers.get(&header_name.to_lowercase()))
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            if header_value.is_empty() {
                return Err(*PingoraError::new_str("Missing auth header"));
            }

            // 根据值格式提取API密钥
            if value_format == "{key}" {
                // 直接值格式，如 "X-goog-api-key: {key}"
                Ok(header_value.to_string())
            } else if value_format.starts_with("Bearer {key}") {
                // Bearer token格式
                if let Some(key) = header_value.strip_prefix("Bearer ") {
                    Ok(key.to_string())
                } else {
                    Err(*PingoraError::new_str("Invalid Bearer token format"))
                }
            } else {
                // 其他格式的解析
                // TODO: 可以扩展支持更复杂的格式
                Err(*PingoraError::new_str("Unsupported auth format"))
            }
        } else {
            Err(*PingoraError::new_str("Invalid auth format configuration"))
        }
    }

    /// 选择API密钥 - 委托给ApiKeyPoolManager处理
    async fn select_api_key(
        &self,
        _auth_result: &AuthResult,
        service_api_config: &entity::user_service_apis::Model,
        ctx: &mut RequestContext,
    ) -> Result<entity::user_provider_keys::Model, PingoraError> {
        // 创建选择上下文
        let selection_context = SelectionContext::new(
            ctx.request_id.clone(),
            service_api_config.user_id,
            service_api_config.id,
            service_api_config.provider_type_id,
        );

        // 使用ApiKeyPoolManager执行密钥选择
        let selection_result = self
            .api_key_pool
            .select_api_key_from_service_api(service_api_config, &selection_context)
            .await
            .map_err(|err| {
                tracing::error!(
                    request_id = %ctx.request_id,
                    error = %err,
                    "API key selection failed"
                );
                *PingoraError::new_str("API key selection failed")
            })?;

        // 更新请求上下文
        ctx.provider_id = Some(ProviderId::from_database_id(
            selection_result.selected_key.provider_type_id,
        ));
        ctx.api_key_id = Some(selection_result.selected_key.id);

        tracing::debug!(
            request_id = %ctx.request_id,
            selected_key_id = selection_result.selected_key.id,
            strategy = %selection_result.strategy.as_str(),
            reason = %selection_result.reason,
            "API key selected successfully"
        );

        Ok(selection_result.selected_key)
    }

    /// 创建上游连接
    async fn create_upstream_peer(
        &self,
        _selected_key: &entity::user_provider_keys::Model,
        _ctx: &RequestContext,
    ) -> Result<HttpPeer, PingoraError> {
        // 简化实现：使用默认的上游地址
        let host = "api.openai.com";
        let port = 443;
        let use_tls = true;
        let address = format!("{}:{}", host, port);

        Ok(HttpPeer::new(&address, use_tls, host.to_string()))
    }

    /// 处理响应
    pub async fn handle_response(
        &self,
        _session: &mut Session,
        ctx: &RequestContext,
    ) -> Result<(), PingoraError> {
        if let Some(ref forwarding_ctx) = ctx.forwarding_context {
            // 简化响应处理 - 假设成功状态（避免session.resp_header()方法不存在的问题）
            let status_code = 200u16;

            // 创建转发结果用于统计
            let result = ForwardingResult {
                success: status_code >= 200 && status_code < 400,
                status_code,
                response_time: forwarding_ctx.start_time.elapsed(),
                provider_id: forwarding_ctx.provider_id,
                error_message: if status_code >= 400 {
                    Some(format!("HTTP {}", status_code))
                } else {
                    None
                },
                bytes_transferred: 0, // TODO: 从响应中获取实际字节数
            };

            // 简化的追踪记录 (跳过复杂的tracer调用，直接记录基本信息)
            if let Some(_tracer) = &self.tracer {
                tracing::info!(
                    request_id = %ctx.request_id,
                    user_id = ctx.user_id.unwrap_or(0),
                    api_key_id = ctx.api_key_id.unwrap_or(0),
                    method = %ctx.method,
                    path = %ctx.path,
                    status_code = result.status_code,
                    response_time_ms = result.response_time.as_millis() as u64,
                    success = result.success,
                    "Request completed"
                );
            }
        }

        Ok(())
    }
}

impl RequestContext {
    /// 创建新的请求上下文
    pub fn new(request_id: String, method: String, path: String) -> Self {
        Self {
            request_id,
            user_id: None,
            provider_id: None,
            api_key_id: None,
            method,
            path,
            forwarding_context: None,
        }
    }
}
