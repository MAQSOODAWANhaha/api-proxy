//! # 代理端认证适配器
//!
//! 轻量级适配器，仅负责从HTTP请求中提取认证信息
//! 所有认证逻辑委托给核心AuthService处理

use anyhow::Result;
use pingora_proxy::Session;
use std::sync::Arc;
use sea_orm::{DatabaseConnection, EntityTrait, ColumnTrait, QueryFilter};
use axum::http::Uri;

use crate::auth::{AuthUtils, AuthManager};
use crate::error::ProxyError;
use crate::proxy::ProxyContext;
use entity;

/// 认证信息来源类型
#[derive(Debug, Clone)]
pub enum AuthSource {
    Query,     // 查询参数
    Header,    // 头部
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

/// 认证结果
#[derive(Debug, Clone)]
pub struct AuthenticationResult {
    /// 用户服务API信息
    pub user_service_api: entity::user_service_apis::Model,
    /// 用户ID
    pub user_id: i32,
    /// 服务商类型ID
    pub provider_type_id: i32,
    /// 完整的服务商类型配置（新增：替代ProviderResolver功能）
    pub provider_type: entity::provider_types::Model,
    /// 认证使用的API密钥（已脱敏）
    pub api_key_preview: String,
    /// 检测到的原始认证信息（用于后续替换）
    pub detected_auth: Authorization,
    /// 选中的真实凭据（用于替换）
    pub selected_credential: String,
}

/// 代理端认证适配器
///
/// 轻量级适配器，仅提供HTTP请求解析和认证委托
/// 所有实际认证逻辑都由统一认证管理器处理
pub struct AuthenticationService {
    /// 统一认证管理器
    auth_manager: Arc<AuthManager>,
    /// 数据库连接（用于获取真实凭据）
    db: Arc<DatabaseConnection>,
}

impl AuthenticationService {
    /// 创建新的认证适配器
    pub fn new(auth_manager: Arc<AuthManager>, db: Arc<DatabaseConnection>) -> Self {
        Self { auth_manager, db }
    }

    /// 智能检测并替换客户端认证信息
    ///
    /// 新的一步式处理流程：
    /// 1. 从请求检测用户认证信息（Query参数或Headers）
    /// 2. 验证用户API并获取关联的user_provider_keys
    /// 3. 根据auth_type获取真实凭据（api_key或oauth access_token）
    /// 4. 立即在请求中替换认证信息
    /// 5. 返回认证结果
    pub async fn detect_and_replace_client_authorization(
        &self, 
        session: &mut Session,
        request_id: &str
    ) -> Result<AuthenticationResult, ProxyError> {
        tracing::debug!(
            request_id = %request_id,
            "Starting integrated authentication detection and credential replacement"
        );

        // 步骤1: 检测用户认证信息
        let user_auth = self.detect_user_auth_from_request(session)?;

        tracing::debug!(
            request_id = %request_id,
            auth_source = ?user_auth.source,
            auth_location = %user_auth.location,
            "User authentication detected"
        );

        // 步骤2: 验证用户API密钥并获取关联配置
        let proxy_auth_result = self
            .auth_manager
            .authenticate_proxy_request(&user_auth.auth_value)
            .await?;

        // 步骤3: 根据auth_type获取真实凭据并立即替换
        let selected_credential = self
            .get_real_credential_and_replace_immediately(
                session,
                &user_auth,
                &proxy_auth_result.user_api,
                request_id
            )
            .await?;

        // 步骤4: 获取完整provider配置
        let provider_type = entity::provider_types::Entity::find_by_id(proxy_auth_result.provider_type_id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| ProxyError::database(&format!("Failed to query provider_types: {}", e)))?
            .ok_or_else(|| ProxyError::internal(&format!("Provider type not found: {}", proxy_auth_result.provider_type_id)))?;

        tracing::info!(
            request_id = %request_id,
            user_id = proxy_auth_result.user_id,
            provider_type_id = proxy_auth_result.provider_type_id,
            provider_name = %provider_type.name,
            user_service_api_id = proxy_auth_result.user_api.id,
            api_key_preview = %AuthUtils::sanitize_api_key(&user_auth.auth_value),
            real_credential_preview = %AuthUtils::sanitize_api_key(&selected_credential),
            "Integrated authentication successful with immediate credential replacement"
        );

        // 步骤5: 构造认证结果
        Ok(AuthenticationResult {
            user_service_api: proxy_auth_result.user_api.clone(),
            user_id: proxy_auth_result.user_id,
            provider_type_id: proxy_auth_result.provider_type_id,
            provider_type,
            api_key_preview: AuthUtils::sanitize_api_key(&user_auth.auth_value),
            detected_auth: user_auth,
            selected_credential,
        })
    }

    /// 从请求中检测用户认证信息（内部辅助方法）
    fn detect_user_auth_from_request(&self, session: &Session) -> Result<Authorization, ProxyError> {
        let req_header = session.req_header();

        // 1. 检测Query参数
        if let Some(query) = req_header.uri.query() {
            for param_pair in query.split('&') {
                if let Some((key, value)) = param_pair.split_once('=') {
                    match key {
                        "key" | "access_token" | "api_key" | "apikey" => {
                            tracing::debug!(
                                param_name = key,
                                "Authentication detected in query parameter"
                            );
                            return Ok(Authorization {
                                auth_value: urlencoding::decode(value)
                                    .map_err(|e| ProxyError::authentication(&format!(
                                        "Failed to decode query parameter '{}': {}", key, e
                                    )))?
                                    .to_string(),
                                source: AuthSource::Query,
                                location: key.to_string(),
                            });
                        }
                        _ => continue,
                    }
                }
            }
        }

        // 2. 检测HTTP Headers（按优先级排序）
        let auth_headers = [
            "authorization",
            "x-api-key", 
            "x-goog-api-key",
            "x-openai-api-key"
        ];

        for header_name in &auth_headers {
            if let Some(header_value) = req_header.headers.get(*header_name) {
                if let Ok(header_str) = std::str::from_utf8(header_value.as_bytes()) {
                    let auth_value = if *header_name == "authorization" {
                        // 处理Authorization头部，可能包含Bearer前缀
                        if let Some(token) = header_str.strip_prefix("Bearer ") {
                            token.trim().to_string()
                        } else {
                            header_str.trim().to_string()
                        }
                    } else {
                        header_str.trim().to_string()
                    };

                    if !auth_value.is_empty() {
                        tracing::debug!(
                            header_name = header_name,
                            "Authentication detected in HTTP header"
                        );
                        return Ok(Authorization {
                            auth_value,
                            source: AuthSource::Header,
                            location: header_name.to_string(),
                        });
                    }
                }
            }
        }

        Err(ProxyError::authentication(
            "No authentication information found in query parameters or headers"
        ))
    }

    /// 智能认证验证和凭据替换（简化版：直接调用集成方法）
    ///
    /// 简化的认证流程：
    /// 1. 调用 detect_and_replace_client_authorization 完成所有认证和替换工作
    /// 2. 返回认证结果
    pub async fn authenticate_and_replace_credentials(
        &self,
        session: &mut Session,
        request_id: &str,
    ) -> Result<AuthenticationResult, ProxyError> {
        tracing::debug!(
            request_id = %request_id,
            "Starting simplified authentication and credential replacement"
        );

        // 一步完成：检测 + 验证 + 获取真实凭据 + 立即替换
        let auth_result = self.detect_and_replace_client_authorization(session, request_id).await?;

        tracing::info!(
            request_id = %request_id,
            user_id = auth_result.user_id,
            provider_type_id = auth_result.provider_type_id,
            provider_name = %auth_result.provider_type.name,
            user_service_api_id = auth_result.user_service_api.id,
            "Simplified authentication and credential replacement completed successfully"
        );

        Ok(auth_result)
    }

    /// 仅进行入口 API Key 认证（不做上游密钥选择、不替换凭证）
    pub async fn authenticate_entry_api(
        &self,
        session: &mut Session,
        request_id: &str,
    ) -> Result<entity::user_service_apis::Model, ProxyError> {
        tracing::debug!(request_id = %request_id, "Authenticating entry API key only");

        // 检测用户携带的认证信息（query/header）
        let user_auth = self.detect_user_auth_from_request(session)?;

        // 验证用户服务 API 密钥
        let proxy_auth_result = self
            .auth_manager
            .authenticate_proxy_request(&user_auth.auth_value)
            .await?;

        tracing::info!(
            request_id = %request_id,
            user_id = proxy_auth_result.user_id,
            provider_type_id = proxy_auth_result.provider_type_id,
            user_service_api_id = proxy_auth_result.user_api.id,
            api_key_preview = %AuthUtils::sanitize_api_key(&user_auth.auth_value),
            "Entry API authentication success (no credential replacement)"
        );

        Ok(proxy_auth_result.user_api)
    }

    /// 根据auth_type获取真实凭据并立即在请求中替换
    ///
    /// 关键改进：区别处理API Key和OAuth认证类型
    /// - api_key: 直接使用user_provider_keys.api_key
    /// - oauth: 使用user_provider_keys.api_key作为session_id查询oauth_client_sessions.access_token
    async fn get_real_credential_and_replace_immediately(
        &self,
        session: &mut Session,
        detected_auth: &Authorization,
        user_api: &entity::user_service_apis::Model,
        request_id: &str,
    ) -> Result<String, ProxyError> {
        // 步骤1: 解析user_provider_keys_ids JSON数组
        let provider_key_ids: Vec<i32> = user_api.user_provider_keys_ids
            .as_array()
            .ok_or_else(|| ProxyError::internal("user_provider_keys_ids is not a JSON array"))?
            .iter()
            .filter_map(|v| v.as_i64().map(|i| i as i32))
            .collect();

        if provider_key_ids.is_empty() {
            return Err(ProxyError::internal("No provider keys configured for this user service API"));
        }

        tracing::debug!(
            request_id = %request_id,
            user_api_id = user_api.id,
            provider_key_ids = ?provider_key_ids,
            "Fetching real credentials from user_provider_keys table with auth_type handling"
        );

        // 步骤2: 查询所有可用的user_provider_keys
        let provider_keys = entity::user_provider_keys::Entity::find()
            .filter(entity::user_provider_keys::Column::Id.is_in(provider_key_ids))
            .filter(entity::user_provider_keys::Column::IsActive.eq(true))
            .all(self.db.as_ref())
            .await
            .map_err(|e| ProxyError::database(&format!("Failed to query user_provider_keys: {}", e)))?;

        if provider_keys.is_empty() {
            return Err(ProxyError::authentication("No active provider keys found"));
        }

        // 步骤3: 根据调度策略选择最优凭据
        let selected_key = self.select_credential_by_strategy(user_api, &provider_keys).await?;

        tracing::debug!(
            request_id = %request_id,
            selected_key_id = selected_key.id,
            auth_type = %selected_key.auth_type,
            "Selected credential by strategy"
        );

        // 步骤4: 根据认证类型获取真实凭据（核心逻辑）
        let real_credential = match selected_key.auth_type.as_str() {
            "api_key" => {
                tracing::debug!(
                    request_id = %request_id,
                    key_id = selected_key.id,
                    "Using API key authentication"
                );
                // 直接使用API密钥
                selected_key.api_key.clone()
            }
            "oauth" => {
                tracing::debug!(
                    request_id = %request_id,
                    key_id = selected_key.id,
                    session_id = %selected_key.api_key,
                    "Using OAuth authentication, fetching access token"
                );
                // OAuth类型：api_key字段存储的是session_id，需要查询oauth_client_sessions表
                self.get_oauth_access_token(&selected_key.api_key, request_id).await?
            }
            _ => {
                return Err(ProxyError::internal(&format!(
                    "Unsupported auth type '{}' for key_id={}", selected_key.auth_type, selected_key.id
                )));
            }
        };

        // 步骤5: 验证凭据非空
        if real_credential.is_empty() {
            return Err(ProxyError::authentication(&format!(
                "Retrieved credential is empty for auth_type='{}', key_id={}", 
                selected_key.auth_type, selected_key.id
            )));
        }

        tracing::debug!(
            request_id = %request_id,
            selected_key_id = selected_key.id,
            auth_type = %selected_key.auth_type,
            credential_preview = %AuthUtils::sanitize_api_key(&real_credential),
            "Real credential retrieved successfully"
        );

        // 步骤6: 立即在请求中替换认证信息
        self.replace_auth_immediately(session, detected_auth, &real_credential, request_id)?;

        Ok(real_credential)
    }

    /// 立即在请求中替换认证信息
    fn replace_auth_immediately(
        &self,
        session: &mut Session,
        detected_auth: &Authorization,
        real_credential: &str,
        request_id: &str,
    ) -> Result<(), ProxyError> {
        tracing::debug!(
            request_id = %request_id,
            auth_source = ?detected_auth.source,
            auth_location = %detected_auth.location,
            "Immediately replacing authentication credential in request"
        );

        match &detected_auth.source {
            AuthSource::Query => {
                self.replace_query_param(session, &detected_auth.location, real_credential)?;
            }
            AuthSource::Header => {
                self.replace_header_value(session, &detected_auth.location, real_credential)?;
            }
        }

        tracing::info!(
            request_id = %request_id,
            auth_source = ?detected_auth.source,
            auth_location = %detected_auth.location,
            real_credential_preview = %AuthUtils::sanitize_api_key(real_credential),
            "Authentication credential replaced immediately in request"
        );

        Ok(())
    }


    /// 根据调度策略选择凭据
    async fn select_credential_by_strategy(
        &self,
        user_api: &entity::user_service_apis::Model,
        provider_keys: &[entity::user_provider_keys::Model],
    ) -> Result<entity::user_provider_keys::Model, ProxyError> {
        let strategy = user_api.scheduling_strategy
            .as_deref()
            .unwrap_or("round_robin");

        match strategy {
            "round_robin" => {
                // 轮询调度：选择第一个可用的（简化实现）
                // TODO: 实现真正的轮询调度算法
                Ok(provider_keys[0].clone())
            }
            "weighted" => {
                // 权重调度：根据weight字段选择
                self.select_by_weight(provider_keys)
            }
            "health_best" => {
                // 健康度最佳：选择health_status为healthy的第一个
                let healthy_keys: Vec<_> = provider_keys.iter()
                    .filter(|key| key.health_status == "healthy")
                    .collect();
                
                if healthy_keys.is_empty() {
                    // 如果没有健康的密钥，选择第一个可用的
                    Ok(provider_keys[0].clone())
                } else {
                    Ok(healthy_keys[0].clone())
                }
            }
            _ => {
                tracing::warn!(
                    strategy = strategy,
                    "Unknown scheduling strategy, using first available credential"
                );
                Ok(provider_keys[0].clone())
            }
        }
    }

    /// 根据权重选择凭据
    fn select_by_weight(
        &self,
        provider_keys: &[entity::user_provider_keys::Model],
    ) -> Result<entity::user_provider_keys::Model, ProxyError> {
        // 计算权重总和
        let total_weight: i32 = provider_keys.iter()
            .map(|key| key.weight.unwrap_or(1))
            .sum();

        if total_weight <= 0 {
            return Ok(provider_keys[0].clone());
        }

        // 简化实现：随机选择（基于权重）
        use fastrand;
        let mut random_weight = fastrand::i32(1..=total_weight);
        
        for key in provider_keys {
            let weight = key.weight.unwrap_or(1);
            if random_weight <= weight {
                return Ok(key.clone());
            }
            random_weight -= weight;
        }

        // 默认选择第一个
        Ok(provider_keys[0].clone())
    }

    /// 获取OAuth访问令牌
    async fn get_oauth_access_token(
        &self, 
        session_id: &str, 
        request_id: &str
    ) -> Result<String, ProxyError> {
        tracing::debug!(
            request_id = %request_id,
            session_id = %session_id,
            "Querying OAuth session for access token"
        );

        // 查询oauth_client_sessions表
        let oauth_session = entity::oauth_client_sessions::Entity::find()
            .filter(entity::oauth_client_sessions::Column::SessionId.eq(session_id))
            .one(self.db.as_ref())
            .await
            .map_err(|e| {
                tracing::error!(
                    request_id = %request_id,
                    session_id = %session_id,
                    error = %e,
                    "Database error while querying oauth_client_sessions"
                );
                ProxyError::database(&format!("Failed to query oauth_client_sessions: {}", e))
            })?;

        // 验证session存在
        let session = match oauth_session {
            Some(session) => session,
            None => {
                tracing::error!(
                    request_id = %request_id,
                    session_id = %session_id,
                    "OAuth session not found in oauth_client_sessions table"
                );
                return Err(ProxyError::authentication(&format!(
                    "OAuth session not found: {}", session_id
                )));
            }
        };

        tracing::debug!(
            request_id = %request_id,
            session_id = %session_id,
            session_status = %session.status,
            provider_name = %session.provider_name,
            user_id = session.user_id,
            "OAuth session found"
        );

        // 验证session状态
        if session.status != "completed" {
            tracing::error!(
                request_id = %request_id,
                session_id = %session_id,
                session_status = %session.status,
                "OAuth session is not in completed status"
            );
            return Err(ProxyError::authentication(&format!(
                "OAuth session {} is not completed, current status: {}", 
                session_id, session.status
            )));
        }

        // 验证access_token存在
        let access_token = match &session.access_token {
            Some(token) => token,
            None => {
                tracing::error!(
                    request_id = %request_id,
                    session_id = %session_id,
                    "OAuth session has no access_token field"
                );
                return Err(ProxyError::authentication(&format!(
                    "OAuth session {} has no access_token", session_id
                )));
            }
        };

        // 验证access_token非空
        if access_token.is_empty() {
            tracing::error!(
                request_id = %request_id,
                session_id = %session_id,
                "OAuth session has empty access_token"
            );
            return Err(ProxyError::authentication(&format!(
                "OAuth session {} has empty access_token", session_id
            )));
        }

        // 检查令牌是否过期
        let now = chrono::Utc::now().naive_utc();
        if session.expires_at <= now {
            tracing::error!(
                request_id = %request_id,
                session_id = %session_id,
                expires_at = %session.expires_at,
                current_time = %now,
                "OAuth access token has expired"
            );
            return Err(ProxyError::authentication(&format!(
                "OAuth access token has expired for session {}, expired at: {}", 
                session_id, session.expires_at
            )));
        }

        tracing::info!(
            request_id = %request_id,
            session_id = %session_id,
            provider_name = %session.provider_name,
            expires_at = %session.expires_at,
            access_token_preview = %AuthUtils::sanitize_api_key(access_token),
            "OAuth access token retrieved successfully"
        );

        Ok(access_token.clone())
    }



    /// 替换查询参数中的认证信息
    fn replace_query_param(
        &self,
        session: &mut Session,
        param_name: &str,
        new_value: &str,
    ) -> Result<(), ProxyError> {
        let req_header = session.req_header_mut();
        let uri = req_header.uri.clone();
        
        // 获取原始路径和查询字符串
        let path = uri.path();
        let query = uri.query().unwrap_or("");
        
        // 解析查询参数并替换目标参数
        let mut params: Vec<(String, String)> = Vec::new();
        let mut replaced = false;
        
        if !query.is_empty() {
            for param_pair in query.split('&') {
                if let Some((key, value)) = param_pair.split_once('=') {
                    if key == param_name {
                        // 替换找到的参数
                        params.push((key.to_string(), urlencoding::encode(new_value).into_owned()));
                        replaced = true;
                    } else {
                        params.push((key.to_string(), value.to_string()));
                    }
                } else {
                    // 处理没有值的参数
                    params.push((param_pair.to_string(), String::new()));
                }
            }
        }
        
        // 如果没有找到参数，添加新参数
        if !replaced {
            params.push((param_name.to_string(), urlencoding::encode(new_value).into_owned()));
        }
        
        // 重新构建查询字符串
        let new_query = if params.is_empty() {
            String::new()
        } else {
            params.iter()
                .map(|(k, v)| if v.is_empty() { k.clone() } else { format!("{}={}", k, v) })
                .collect::<Vec<_>>()
                .join("&")
        };
        
        // 构建新的URI
        let new_uri_str = if new_query.is_empty() {
            path.to_string()
        } else {
            format!("{}?{}", path, new_query)
        };
        
        // 解析并设置新的URI
        let new_uri = new_uri_str.parse::<Uri>()
            .map_err(|e| ProxyError::internal(&format!("Failed to parse new URI: {}", e)))?;
        
        req_header.set_uri(new_uri);
        
        tracing::debug!(
            param_name = param_name,
            new_value_preview = %AuthUtils::sanitize_api_key(new_value),
            new_query = %new_query,
            "Query parameter replaced successfully"
        );
        
        Ok(())
    }

    /// 替换请求头中的认证信息
    fn replace_header_value(
        &self,
        session: &mut Session,
        header_name: &str,
        new_value: &str,
    ) -> Result<(), ProxyError> {
        let req_header = session.req_header_mut();
        
        // 对于Authorization头部，需要保持原有格式
        let final_value = if header_name == "authorization" {
            // 检查原值是否有Bearer前缀
            if let Some(original_value) = req_header.headers.get(header_name) {
                if let Ok(original_str) = std::str::from_utf8(original_value.as_bytes()) {
                    if original_str.starts_with("Bearer ") {
                        // 保持Bearer前缀
                        format!("Bearer {}", new_value)
                    } else {
                        // 直接使用新值
                        new_value.to_string()
                    }
                } else {
                    new_value.to_string()
                }
            } else {
                new_value.to_string()
            }
        } else {
            // 其他头部直接使用新值
            new_value.to_string()
        };
        
        // 使用pingora的insert_header方法替换头部
        let header_name_owned = header_name.to_string();
        if let Err(e) = session.req_header_mut().insert_header(header_name_owned, &final_value) {
            return Err(ProxyError::internal(&format!(
                "Failed to insert header '{}': {}", header_name, e
            )));
        }
        
        tracing::debug!(
            header_name = header_name,
            new_value_preview = %AuthUtils::sanitize_api_key(&final_value),
            "Header value replaced successfully"
        );
        
        Ok(())
    }




    /// 将认证结果应用到上下文（增强版：包含完整provider配置）
    pub fn apply_auth_result_to_context(
        &self,
        ctx: &mut ProxyContext,
        auth_result: &AuthenticationResult,
    ) {
        ctx.user_service_api = Some(auth_result.user_service_api.clone());
        ctx.provider_type = Some(auth_result.provider_type.clone());
        
        // 设置超时配置，优先级：用户配置 > provider默认配置
        ctx.timeout_seconds = Some(
            auth_result.user_service_api.timeout_seconds
                .or(auth_result.provider_type.timeout_seconds)
                .unwrap_or(30) // 默认30秒
        );
        
        tracing::debug!(
            user_id = auth_result.user_id,
            provider_name = %auth_result.provider_type.name,
            provider_base_url = %auth_result.provider_type.base_url,
            timeout_seconds = ctx.timeout_seconds.unwrap_or(30),
            "Authentication result applied to context with complete provider configuration"
        );
    }

    /// 检查速率限制
    ///
    /// 基于用户和服务API的速率限制配置进行检查
    pub async fn check_rate_limit(&self, ctx: &ProxyContext) -> Result<(), ProxyError> {
        // TODO: 实现基于Redis的速率限制检查
        // 这里应该检查:
        // 1. 每分钟请求数限制
        // 2. 每天请求数限制
        // 3. 每天token使用量限制

        tracing::debug!(
            request_id = %ctx.request_id,
            user_service_api_id = ctx.user_service_api.as_ref().map(|api| api.id),
            "Rate limit check passed (placeholder implementation)"
        );

        Ok(())
    }

    /// 验证API密钥格式
    ///
    /// 快速格式验证，避免无效密钥的数据库查询
    pub fn validate_api_key_format(&self, api_key: &str) -> bool {
        self.auth_manager.validate_proxy_api_key_format(api_key)
    }

    /// 为上游AI服务商构建出站认证头（出站认证 - 代理→AI服务商）
    ///
    /// 根据服务商类型自动选择适合的认证头格式
    /// 用途：构建发送给AI服务商的HTTP认证头，确保上游服务商收到正确格式的认证信息
    pub fn build_outbound_auth_headers_for_upstream(
        &self,
        provider: &entity::provider_types::Model,
        api_key: &str,
    ) -> Result<Vec<(String, String)>, ProxyError> {
        let mut auth_headers = Vec::new();
        
        // 根据服务商类型选择适合的认证头格式
        match provider.name.as_str() {
            "gemini" | "custom_gemini" => {
                // Gemini支持两种认证方式
                auth_headers.push(("Authorization".to_string(), format!("Bearer {}", api_key)));
                auth_headers.push(("X-goog-api-key".to_string(), api_key.to_string()));
            }
            _ => {
                // 其他服务商使用标准Bearer认证
                auth_headers.push(("Authorization".to_string(), format!("Bearer {}", api_key)));
            }
        }

        tracing::debug!(
            provider_name = %provider.name,
            generated_headers = ?auth_headers.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            "Generated authentication headers using provider-specific logic"
        );

        Ok(auth_headers)
    }
}
