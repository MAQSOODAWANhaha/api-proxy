//! # 智能API密钥提供者
//!
//! 为代理端提供统一的API密钥获取接口，透明处理传统API密钥和OAuth token刷新
//! 支持双重刷新机制：被动刷新（使用时检查）+ 主动刷新（后台任务）

use chrono::{DateTime, Duration, Utc};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::{ldebug, lerror, linfo, lwarn, logging::{LogComponent, LogStage}};
use crate::auth::oauth_client::OAuthClient;
use crate::auth::oauth_token_refresh_service::OAuthTokenRefreshService;
use crate::error::{ProxyError, Result};
use entity::user_provider_keys;

/// 智能API密钥提供者
///
/// 核心职责：
/// 1. 统一API密钥/OAuth token获取接口
/// 2. 被动刷新：使用时自动检查并刷新过期token
/// 3. 缓存管理：内存缓存有效token避免频繁数据库查询
/// 4. 错误处理：token刷新失败时的降级策略
pub struct SmartApiKeyProvider {
    db: Arc<DatabaseConnection>,
    oauth_client: Arc<OAuthClient>,

    /// OAuth token智能刷新服务
    refresh_service: Arc<OAuthTokenRefreshService>,

    /// 内存缓存：provider_key_id -> CachedCredential
    credential_cache: Arc<RwLock<HashMap<i32, CachedCredential>>>,

    /// 刷新锁：防止并发刷新同一个token
    refresh_locks: Arc<RwLock<HashMap<i32, Arc<tokio::sync::Mutex<()>>>>>,
}

/// 缓存的凭证信息
#[derive(Debug, Clone)]
struct CachedCredential {
    /// 实际的API密钥或访问token
    credential: String,

    /// 凭证类型
    auth_type: AuthCredentialType,

    /// 缓存时间
    cached_at: DateTime<Utc>,

    /// 过期时间（OAuth token才有）
    expires_at: Option<DateTime<Utc>>,

    /// 是否正在刷新中
    refreshing: bool,
}

/// 认证凭证类型
#[derive(Debug, Clone, PartialEq)]
pub enum AuthCredentialType {
    /// 传统API密钥（不会过期）
    ApiKey,

    /// OAuth访问token（会过期，需要刷新）
    OAuthToken { session_id: String },
}

/// 获取凭证的结果
#[derive(Debug, Clone)]
pub struct CredentialResult {
    /// 有效的API密钥或访问token
    pub credential: String,

    /// 凭证类型
    pub auth_type: AuthCredentialType,

    /// 是否是刚刷新的（用于监控统计）
    pub refreshed: bool,
}

impl SmartApiKeyProvider {
    /// 创建新的智能API密钥提供者
    pub fn new(
        db: Arc<DatabaseConnection>,
        oauth_client: Arc<OAuthClient>,
        refresh_service: Arc<OAuthTokenRefreshService>,
    ) -> Self {
        Self {
            db,
            oauth_client,
            refresh_service,
            credential_cache: Arc::new(RwLock::new(HashMap::new())),
            refresh_locks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取有效的API凭证
    ///
    /// 这是核心方法，代理端使用此方法获取API密钥/token：
    /// 1. 检查内存缓存
    /// 2. 对于OAuth token，检查是否即将过期
    /// 3. 如果需要，执行token刷新
    /// 4. 返回有效凭证
    pub async fn get_valid_credential(&self, provider_key_id: i32) -> Result<CredentialResult> {
        ldebug!(
            "system",
            LogStage::Authentication,
            LogComponent::Auth,
            "get_credential",
            &format!("Getting valid credential for provider_key_id: {}", provider_key_id)
        );

        // 1. 先检查缓存
        if let Some(cached) = self.get_cached_credential(provider_key_id).await? {
            ldebug!(
                "system",
                LogStage::Cache,
                LogComponent::Auth,
                "cache_hit",
                &format!("Found cached credential for provider_key_id: {}", provider_key_id)
            );
            return Ok(CredentialResult {
                credential: cached.credential,
                auth_type: cached.auth_type,
                refreshed: false,
            });
        }

        // 2. 缓存中没有，从数据库加载
        let provider_key = self.load_provider_key_from_db(provider_key_id).await?;

        // 3. 根据认证类型处理
        match provider_key.auth_type.as_str() {
            "api_key" => {
                // 传统API密钥，直接返回并缓存
                let credential_type = AuthCredentialType::ApiKey;
                let result = CredentialResult {
                    credential: provider_key.api_key.clone(),
                    auth_type: credential_type.clone(),
                    refreshed: false,
                };

                // 缓存API密钥（永不过期）
                self.cache_credential(
                    provider_key_id,
                    CachedCredential {
                        credential: provider_key.api_key,
                        auth_type: credential_type,
                        cached_at: Utc::now(),
                        expires_at: None,
                        refreshing: false,
                    },
                )
                .await;

                Ok(result)
            }

            // OAuth认证类型处理
            "oauth" => {
                // OAuth token，使用智能刷新服务处理
                self.handle_oauth_credential_with_refresh_service(provider_key_id, &provider_key)
                    .await
            }

            auth_type => {
                lerror!("system", LogStage::Authentication, LogComponent::Auth, "unsupported_auth_type", &format!("Unsupported auth_type: {}", auth_type));
                Err(crate::proxy_err!(
                    auth,
                    "Unsupported auth_type: {}",
                    auth_type
                ))
            }
        }
    }

    /// 使用智能刷新服务处理OAuth凭证获取
    async fn handle_oauth_credential_with_refresh_service(
        &self,
        provider_key_id: i32,
        provider_key: &user_provider_keys::Model,
    ) -> Result<CredentialResult> {
        let session_id = &provider_key.api_key;

        ldebug!(
            "system",
            LogStage::Authentication,
            LogComponent::Auth,
            "handle_oauth_credential",
            &format!("Handling OAuth credential with refresh service for session_id: {}", session_id)
        );

        // 获取刷新锁，防止并发刷新
        let refresh_lock = self.get_refresh_lock(provider_key_id).await;
        let _guard = refresh_lock.lock().await;

        // 再次检查缓存（可能在等待锁的过程中其他线程已刷新）
        if let Some(cached) = self.get_cached_credential(provider_key_id).await? {
            ldebug!(
                "system",
                LogStage::Cache,
                LogComponent::Auth,
                "cache_hit_after_lock",
                &format!("Found cached credential after lock for provider_key_id: {}", provider_key_id)
            );
            return Ok(CredentialResult {
                credential: cached.credential,
                auth_type: cached.auth_type,
                refreshed: false,
            });
        }

        // 使用智能刷新服务进行被动刷新检查
        match self
            .refresh_service
            .passive_refresh_if_needed(session_id)
            .await
        {
            Ok(refresh_result) => {
                if refresh_result.success {
                    // 刷新成功或token仍然有效
                    if let Some(new_access_token) = refresh_result.new_access_token {
                        // 有新token，更新缓存
                        linfo!(
                            "system",
                            LogStage::Authentication,
                            LogComponent::Auth,
                            "token_refreshed",
                            &format!("Got refreshed OAuth access token for provider_key_id: {}", provider_key_id)
                        );

                        let credential_type = AuthCredentialType::OAuthToken {
                            session_id: session_id.clone(),
                        };

                        let result = CredentialResult {
                            credential: new_access_token.clone(),
                            auth_type: credential_type.clone(),
                            refreshed: true,
                        };

                        // 缓存新token
                        self.cache_credential(
                            provider_key_id,
                            CachedCredential {
                                credential: new_access_token,
                                auth_type: credential_type,
                                cached_at: Utc::now(),
                                expires_at: refresh_result.new_expires_at,
                                refreshing: false,
                            },
                        )
                        .await;

                        Ok(result)
                    } else {
                        // token仍然有效，从OAuth client获取当前token
                        match self.oauth_client.get_valid_access_token(session_id).await {
                            Ok(Some(access_token)) => {
                                ldebug!(
                                    "system",
                                    LogStage::Authentication,
                                    LogComponent::Auth,
                                    "using_current_token",
                                    &format!("Using current valid OAuth access token for provider_key_id: {}", provider_key_id)
                                );

                                let credential_type = AuthCredentialType::OAuthToken {
                                    session_id: session_id.clone(),
                                };

                                let result = CredentialResult {
                                    credential: access_token.clone(),
                                    auth_type: credential_type.clone(),
                                    refreshed: false,
                                };

                                // 缓存当前token
                                self.cache_credential(
                                    provider_key_id,
                                    CachedCredential {
                                        credential: access_token,
                                        auth_type: credential_type,
                                        cached_at: Utc::now(),
                                        expires_at: self.get_token_expiry_time(session_id).await,
                                        refreshing: false,
                                    },
                                )
                                .await;

                                Ok(result)
                            }
                            Ok(None) => {
                                lwarn!(
                                    "system",
                                    LogStage::Authentication,
                                    LogComponent::Auth,
                                    "no_valid_token",
                                    &format!("No valid OAuth access token available for provider_key_id: {}", provider_key_id)
                                );
                                self.fallback_to_api_key(provider_key)
                            }
                            Err(e) => {
                                lerror!(
                                    "system",
                                    LogStage::Authentication,
                                    LogComponent::Auth,
                                    "get_token_failed",
                                    &format!("Failed to get OAuth access token for provider_key_id: {}: {:?}", provider_key_id, e)
                                );
                                self.fallback_to_api_key(provider_key)
                            }
                        }
                    }
                } else {
                    // 刷新失败，尝试降级
                    lwarn!(
                        "system",
                        LogStage::Authentication,
                        LogComponent::Auth,
                        "token_refresh_failed",
                        &format!("OAuth token refresh failed for provider_key_id: {}, error: {:?}", provider_key_id, refresh_result.error_message)
                    );
                    self.fallback_to_api_key(provider_key)
                }
            }
            Err(e) => {
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "refresh_service_error",
                    &format!("OAuth refresh service error for provider_key_id: {}: {:?}", provider_key_id, e)
                );
                self.fallback_to_api_key(provider_key)
            }
        }
    }

    /// 降级到使用存储的API密钥
    fn fallback_to_api_key(
        &self,
        provider_key: &user_provider_keys::Model,
    ) -> Result<CredentialResult> {
        if !provider_key.api_key.is_empty() {
            lwarn!(
                "system",
                LogStage::Authentication,
                LogComponent::Auth,
                "fallback_to_api_key",
                &format!("Falling back to stored api_key for provider_key_id: {}", provider_key.id)
            );
            Ok(CredentialResult {
                credential: provider_key.api_key.clone(),
                auth_type: AuthCredentialType::ApiKey,
                refreshed: false,
            })
        } else {
            Err(crate::proxy_err!(
                auth,
                "OAuth token refresh failed and no fallback API key available"
            ))
        }
    }

    /// 检查缓存中的凭证是否有效
    async fn get_cached_credential(
        &self,
        provider_key_id: i32,
    ) -> Result<Option<CachedCredential>> {
        let cache = self.credential_cache.read().await;

        if let Some(cached) = cache.get(&provider_key_id) {
            // 检查缓存是否过期
            match &cached.auth_type {
                AuthCredentialType::ApiKey => {
                    // API密钥不过期，但检查缓存时间（避免长期缓存过旧数据）
                    if Utc::now().signed_duration_since(cached.cached_at) < Duration::hours(1) {
                        return Ok(Some(cached.clone()));
                    }
                }

                AuthCredentialType::OAuthToken { .. } => {
                    // OAuth token需要检查过期时间
                    if let Some(expires_at) = cached.expires_at {
                        // 提前5分钟认为过期，确保有足够时间刷新
                        let buffer_time = Duration::minutes(5);
                        if Utc::now() + buffer_time < expires_at {
                            return Ok(Some(cached.clone()));
                        } else {
                            ldebug!(
                                "system",
                                LogStage::Cache,
                                LogComponent::Auth,
                                "token_expired",
                                &format!("Cached OAuth token expired or expiring soon for provider_key_id: {}", provider_key_id)
                            );
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// 从数据库加载provider key
    async fn load_provider_key_from_db(
        &self,
        provider_key_id: i32,
    ) -> Result<user_provider_keys::Model> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        user_provider_keys::Entity::find()
            .filter(user_provider_keys::Column::Id.eq(provider_key_id))
            .filter(user_provider_keys::Column::IsActive.eq(true))
            .one(&*self.db)
            .await
            .map_err(|e| {
                ProxyError::database_with_source(format!("Failed to load provider key: {:?}", e), e)
            })?
            .ok_or_else(|| {
                crate::proxy_err!(
                    auth,
                    "Provider key not found or inactive: {}",
                    provider_key_id
                )
            })
    }

    /// 获取token过期时间
    async fn get_token_expiry_time(&self, _session_id: &str) -> Option<DateTime<Utc>> {
        // 从oauth_client_sessions表查询expires_at
        // TODO: 这里需要根据实际的oauth_client_sessions entity来实现
        // 暂时返回None，后续完善
        None
    }

    /// 缓存凭证
    async fn cache_credential(&self, provider_key_id: i32, credential: CachedCredential) {
        let mut cache = self.credential_cache.write().await;
        cache.insert(provider_key_id, credential);

        // 清理过期的缓存项（简单的LRU策略）
        if cache.len() > 1000 {
            // 限制缓存大小
            let now = Utc::now();
            cache.retain(|_, cached| match cached.expires_at {
                Some(expires_at) => now < expires_at,
                None => now.signed_duration_since(cached.cached_at) < Duration::hours(24),
            });
        }
    }

    /// 获取刷新锁
    async fn get_refresh_lock(&self, provider_key_id: i32) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.refresh_locks.write().await;
        locks
            .entry(provider_key_id)
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// 清除指定provider key的缓存
    pub async fn invalidate_cache(&self, provider_key_id: i32) {
        let mut cache = self.credential_cache.write().await;
        if cache.remove(&provider_key_id).is_some() {
            ldebug!("system", LogStage::Cache, LogComponent::Auth, "invalidate_cache", &format!("Invalidated cache for provider_key_id: {}", provider_key_id));
        }
    }

    /// 清除所有缓存
    pub async fn clear_all_cache(&self) {
        let mut cache = self.credential_cache.write().await;
        let count = cache.len();
        cache.clear();
        ldebug!("system", LogStage::Cache, LogComponent::Auth, "clear_cache", &format!("Cleared all cached credentials, count: {}", count));
    }

    /// 获取缓存统计信息
    pub async fn get_cache_stats(&self) -> HashMap<String, u64> {
        let cache = self.credential_cache.read().await;
        let mut stats = HashMap::new();

        stats.insert("total_cached".to_string(), cache.len() as u64);

        let mut api_key_count = 0;
        let mut oauth_token_count = 0;
        let mut expired_count = 0;

        let now = Utc::now();
        for cached in cache.values() {
            match &cached.auth_type {
                AuthCredentialType::ApiKey => api_key_count += 1,
                AuthCredentialType::OAuthToken { .. } => {
                    oauth_token_count += 1;
                    if let Some(expires_at) = cached.expires_at {
                        if now >= expires_at {
                            expired_count += 1;
                        }
                    }
                }
            }
        }

        stats.insert("api_key_cached".to_string(), api_key_count);
        stats.insert("oauth_token_cached".to_string(), oauth_token_count);
        stats.insert("expired_cached".to_string(), expired_count);

        stats
    }
}

/// 智能API密钥提供者的配置
#[derive(Debug, Clone)]
pub struct SmartApiKeyProviderConfig {
    /// 缓存TTL（小时）
    pub cache_ttl_hours: i64,

    /// OAuth token提前刷新时间（分钟）
    pub refresh_buffer_minutes: i64,

    /// 最大缓存项数
    pub max_cache_items: usize,
}

impl Default for SmartApiKeyProviderConfig {
    fn default() -> Self {
        Self {
            cache_ttl_hours: 1,
            refresh_buffer_minutes: 5,
            max_cache_items: 1000,
        }
    }
}