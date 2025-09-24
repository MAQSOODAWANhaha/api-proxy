//! # OAuth Token智能刷新服务
//!
//! 实现OAuth token的智能刷新逻辑，支持主动和被动两种刷新策略：
//! - 被动刷新：在获取token时检查过期状态并自动刷新
//! - 主动刷新：后台定期检查即将过期的token并提前刷新

use chrono::{DateTime, Duration, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::auth::oauth_client::OAuthClient;
use crate::auth::types::AuthStatus;
use crate::error::{ProxyError, Result};
use entity::oauth_client_sessions;

/// OAuth Token智能刷新服务
///
/// 核心职责：
/// 1. 被动刷新：使用时检查token是否过期并自动刷新
/// 2. 主动刷新：后台任务定期检查即将过期的token并提前刷新
/// 3. 刷新锁：防止并发刷新同一个token
/// 4. 失败重试：token刷新失败时的智能重试机制
pub struct OAuthTokenRefreshService {
    db: Arc<DatabaseConnection>,
    oauth_client: Arc<OAuthClient>,

    /// 刷新锁：session_id -> Mutex，防止并发刷新同一个token
    refresh_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,

    /// 刷新统计信息
    refresh_stats: Arc<RwLock<RefreshStats>>,

    /// 配置
    config: RefreshServiceConfig,
}

/// 刷新服务配置
#[derive(Debug, Clone)]
pub struct RefreshServiceConfig {
    /// 提前刷新时间（分钟），在token过期前多久开始刷新
    pub refresh_buffer_minutes: i64,

    /// 主动刷新检查间隔（分钟）
    pub active_refresh_interval_minutes: i64,

    /// 最大重试次数
    pub max_retry_attempts: u32,

    /// 重试间隔（秒）
    pub retry_interval_seconds: u64,

    /// 失败token的冷却时间（分钟），失败后多久再次尝试刷新
    pub failure_cooldown_minutes: i64,
}

impl Default for RefreshServiceConfig {
    fn default() -> Self {
        Self {
            refresh_buffer_minutes: 5,           // 提前5分钟刷新
            active_refresh_interval_minutes: 10, // 每10分钟检查一次
            max_retry_attempts: 3,               // 最多重试3次
            retry_interval_seconds: 30,          // 重试间隔30秒
            failure_cooldown_minutes: 30,        // 失败后冷却30分钟
        }
    }
}

/// 刷新统计信息
#[derive(Debug, Default, Clone)]
pub struct RefreshStats {
    /// 总刷新次数
    pub total_refreshes: u64,

    /// 成功刷新次数
    pub successful_refreshes: u64,

    /// 失败刷新次数
    pub failed_refreshes: u64,

    /// 被动刷新次数（使用时触发）
    pub passive_refreshes: u64,

    /// 主动刷新次数（后台任务触发）
    pub active_refreshes: u64,

    /// 最后刷新时间
    pub last_refresh_time: Option<DateTime<Utc>>,

    /// 最后失败时间
    pub last_failure_time: Option<DateTime<Utc>>,

    /// 当前正在刷新的token数量
    pub refreshing_tokens: u32,
}

/// Token刷新结果
#[derive(Debug, Clone)]
pub struct TokenRefreshResult {
    /// 是否成功刷新
    pub success: bool,

    /// 新的访问token（如果刷新成功）
    pub new_access_token: Option<String>,

    /// 新的过期时间（如果刷新成功）
    pub new_expires_at: Option<DateTime<Utc>>,

    /// 错误信息（如果刷新失败）
    pub error_message: Option<String>,

    /// 是否应该重试
    pub should_retry: bool,

    /// 刷新类型
    pub refresh_type: RefreshType,
}

/// 刷新类型
#[derive(Debug, Clone, PartialEq)]
pub enum RefreshType {
    /// 被动刷新：使用时检查过期并刷新
    Passive,
    /// 主动刷新：后台任务提前刷新
    Active,
}

impl OAuthTokenRefreshService {
    /// 创建新的OAuth Token智能刷新服务
    pub fn new(
        db: Arc<DatabaseConnection>,
        oauth_client: Arc<OAuthClient>,
        config: RefreshServiceConfig,
    ) -> Self {
        Self {
            db,
            oauth_client,
            refresh_locks: Arc::new(RwLock::new(HashMap::new())),
            refresh_stats: Arc::new(RwLock::new(RefreshStats::default())),
            config,
        }
    }

    /// 使用默认配置创建刷新服务
    pub fn new_with_defaults(db: Arc<DatabaseConnection>, oauth_client: Arc<OAuthClient>) -> Self {
        Self::new(db, oauth_client, RefreshServiceConfig::default())
    }

    /// 被动刷新：检查token是否需要刷新，如果需要则刷新
    ///
    /// 这个方法通常在SmartApiKeyProvider中使用时调用
    pub async fn passive_refresh_if_needed(&self, session_id: &str) -> Result<TokenRefreshResult> {
        debug!("Checking passive refresh for session_id: {}", session_id);

        // 检查是否需要刷新
        if !self.should_refresh_token(session_id).await? {
            debug!("Token for session_id {} does not need refresh", session_id);
            return Ok(TokenRefreshResult {
                success: true,
                new_access_token: None,
                new_expires_at: None,
                error_message: None,
                should_retry: false,
                refresh_type: RefreshType::Passive,
            });
        }

        // 执行被动刷新
        self.refresh_token_with_lock(session_id, RefreshType::Passive)
            .await
    }

    /// 主动刷新：后台任务调用，刷新所有即将过期的token
    pub async fn active_refresh_expiring_tokens(&self) -> Result<Vec<TokenRefreshResult>> {
        info!("Starting active refresh for expiring tokens");

        // 查询即将过期的OAuth sessions
        let expiring_sessions = self.find_expiring_oauth_sessions().await?;
        info!("Found {} expiring OAuth sessions", expiring_sessions.len());

        let mut results = Vec::new();

        for session in expiring_sessions {
            debug!("Processing expiring session: {}", session.session_id);

            // 执行主动刷新
            match self
                .refresh_token_with_lock(&session.session_id, RefreshType::Active)
                .await
            {
                Ok(result) => {
                    if result.success {
                        info!(
                            "Successfully refreshed token for session: {}",
                            session.session_id
                        );
                    } else {
                        warn!(
                            "Failed to refresh token for session: {}, error: {:?}",
                            session.session_id, result.error_message
                        );
                    }
                    results.push(result);
                }
                Err(e) => {
                    error!(
                        "Error refreshing token for session {}: {:?}",
                        session.session_id, e
                    );
                    results.push(TokenRefreshResult {
                        success: false,
                        new_access_token: None,
                        new_expires_at: None,
                        error_message: Some(format!("Refresh error: {:?}", e)),
                        should_retry: true,
                        refresh_type: RefreshType::Active,
                    });
                }
            }
        }

        info!(
            "Active refresh completed, processed {} sessions",
            results.len()
        );
        Ok(results)
    }

    /// 检查token是否需要刷新
    async fn should_refresh_token(&self, session_id: &str) -> Result<bool> {
        let session = oauth_client_sessions::Entity::find()
            .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
            .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
            .one(&*self.db)
            .await
            .map_err(|e| {
                ProxyError::database_with_source(
                    format!("Failed to find OAuth session: {:?}", e),
                    e,
                )
            })?
            .ok_or_else(|| {
                ProxyError::authentication(format!("OAuth session not found: {}", session_id))
            })?;

        // 检查是否有有效的访问token
        if session.access_token.is_none() {
            debug!("Session {} has no access token", session_id);
            return Ok(false); // 没有token，无需刷新
        }

        // 检查过期时间
        let now = Utc::now();
        let buffer = Duration::minutes(self.config.refresh_buffer_minutes);
        let expires_at_utc = DateTime::<Utc>::from_naive_utc_and_offset(session.expires_at, Utc);
        let should_refresh = now + buffer >= expires_at_utc;

        debug!(
            "Session {} expires at {:?}, should refresh: {}",
            session_id, session.expires_at, should_refresh
        );

        Ok(should_refresh)
    }

    /// 使用锁进行token刷新，防止并发刷新
    async fn refresh_token_with_lock(
        &self,
        session_id: &str,
        refresh_type: RefreshType,
    ) -> Result<TokenRefreshResult> {
        // 获取刷新锁
        let refresh_lock = self.get_refresh_lock(session_id).await;
        let _guard = refresh_lock.lock().await;

        // 获得锁后再次检查是否需要刷新（可能其他线程已经刷新了）
        if refresh_type == RefreshType::Passive && !self.should_refresh_token(session_id).await? {
            debug!(
                "Token already refreshed by another thread for session: {}",
                session_id
            );
            return Ok(TokenRefreshResult {
                success: true,
                new_access_token: None,
                new_expires_at: None,
                error_message: None,
                should_retry: false,
                refresh_type,
            });
        }

        // 更新统计信息
        self.increment_refreshing_count().await;

        // 执行实际的token刷新
        let result = self
            .perform_token_refresh(session_id, refresh_type.clone())
            .await;

        // 更新统计信息
        self.decrement_refreshing_count().await;
        if let Ok(ref refresh_result) = result {
            self.update_refresh_stats(refresh_result).await;
        }

        result
    }

    /// 执行实际的token刷新
    async fn perform_token_refresh(
        &self,
        session_id: &str,
        refresh_type: RefreshType,
    ) -> Result<TokenRefreshResult> {
        debug!(
            "Performing token refresh for session: {}, type: {:?}",
            session_id, refresh_type
        );

        // 使用OAuth client进行token刷新
        match self.oauth_client.get_valid_access_token(session_id).await {
            Ok(Some(new_access_token)) => {
                info!("Successfully refreshed token for session: {}", session_id);

                // 获取新的过期时间
                let new_expires_at = self.get_token_expires_at(session_id).await;

                Ok(TokenRefreshResult {
                    success: true,
                    new_access_token: Some(new_access_token),
                    new_expires_at,
                    error_message: None,
                    should_retry: false,
                    refresh_type,
                })
            }

            Ok(None) => {
                warn!("No valid access token returned for session: {}", session_id);
                Ok(TokenRefreshResult {
                    success: false,
                    new_access_token: None,
                    new_expires_at: None,
                    error_message: Some("No valid access token available".to_string()),
                    should_retry: true,
                    refresh_type,
                })
            }

            Err(e) => {
                error!(
                    "Failed to refresh token for session {}: {:?}",
                    session_id, e
                );
                Ok(TokenRefreshResult {
                    success: false,
                    new_access_token: None,
                    new_expires_at: None,
                    error_message: Some(format!("OAuth client error: {:?}", e)),
                    should_retry: self.should_retry_refresh(&e),
                    refresh_type,
                })
            }
        }
    }

    /// 查找即将过期的OAuth sessions
    async fn find_expiring_oauth_sessions(&self) -> Result<Vec<oauth_client_sessions::Model>> {
        let now = Utc::now();
        let buffer = Duration::minutes(self.config.refresh_buffer_minutes);
        let expiry_threshold = now + buffer;

        oauth_client_sessions::Entity::find()
            .filter(oauth_client_sessions::Column::Status.eq(AuthStatus::Authorized.to_string()))
            .filter(oauth_client_sessions::Column::ExpiresAt.lte(expiry_threshold))
            .filter(oauth_client_sessions::Column::AccessToken.is_not_null())
            .filter(oauth_client_sessions::Column::RefreshToken.is_not_null())
            .all(&*self.db)
            .await
            .map_err(|e| {
                ProxyError::database_with_source(
                    format!("Failed to find expiring OAuth sessions: {:?}", e),
                    e,
                )
            })
    }

    /// 获取token的过期时间
    async fn get_token_expires_at(&self, session_id: &str) -> Option<DateTime<Utc>> {
        match oauth_client_sessions::Entity::find()
            .filter(oauth_client_sessions::Column::SessionId.eq(session_id))
            .one(&*self.db)
            .await
        {
            Ok(Some(session)) => Some(DateTime::<Utc>::from_naive_utc_and_offset(
                session.expires_at,
                Utc,
            )),
            _ => None,
        }
    }

    /// 判断是否应该重试刷新
    fn should_retry_refresh(&self, _error: &crate::auth::oauth_client::OAuthError) -> bool {
        // 根据错误类型判断是否应该重试
        // TODO: 根据实际的OAuthError类型来判断
        true // 暂时总是重试
    }

    /// 获取刷新锁
    async fn get_refresh_lock(&self, session_id: &str) -> Arc<Mutex<()>> {
        let mut locks = self.refresh_locks.write().await;
        locks
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// 增加正在刷新的计数
    async fn increment_refreshing_count(&self) {
        let mut stats = self.refresh_stats.write().await;
        stats.refreshing_tokens += 1;
    }

    /// 减少正在刷新的计数
    async fn decrement_refreshing_count(&self) {
        let mut stats = self.refresh_stats.write().await;
        if stats.refreshing_tokens > 0 {
            stats.refreshing_tokens -= 1;
        }
    }

    /// 更新刷新统计信息
    async fn update_refresh_stats(&self, result: &TokenRefreshResult) {
        let mut stats = self.refresh_stats.write().await;

        stats.total_refreshes += 1;
        stats.last_refresh_time = Some(Utc::now());

        if result.success {
            stats.successful_refreshes += 1;
        } else {
            stats.failed_refreshes += 1;
            stats.last_failure_time = Some(Utc::now());
        }

        match result.refresh_type {
            RefreshType::Passive => stats.passive_refreshes += 1,
            RefreshType::Active => stats.active_refreshes += 1,
        }
    }

    /// 获取刷新统计信息
    pub async fn get_refresh_stats(&self) -> RefreshStats {
        self.refresh_stats.read().await.clone()
    }

    /// 清理过期的刷新锁
    pub async fn cleanup_expired_locks(&self) {
        let mut locks = self.refresh_locks.write().await;

        // 清理长时间未使用的锁（简单策略：清理所有锁）
        // 在生产环境中可以实现更复杂的清理策略
        if locks.len() > 1000 {
            // 避免锁过多
            locks.clear();
            debug!("Cleaned up expired refresh locks");
        }
    }

    /// 强制刷新指定session的token
    pub async fn force_refresh_token(&self, session_id: &str) -> Result<TokenRefreshResult> {
        info!("Force refreshing token for session: {}", session_id);
        self.refresh_token_with_lock(session_id, RefreshType::Passive)
            .await
    }
}

/// 刷新服务构建器
pub struct OAuthTokenRefreshServiceBuilder {
    config: RefreshServiceConfig,
}

impl OAuthTokenRefreshServiceBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            config: RefreshServiceConfig::default(),
        }
    }

    /// 设置提前刷新时间
    pub fn refresh_buffer_minutes(mut self, minutes: i64) -> Self {
        self.config.refresh_buffer_minutes = minutes;
        self
    }

    /// 设置主动刷新检查间隔
    pub fn active_refresh_interval_minutes(mut self, minutes: i64) -> Self {
        self.config.active_refresh_interval_minutes = minutes;
        self
    }

    /// 设置最大重试次数
    pub fn max_retry_attempts(mut self, attempts: u32) -> Self {
        self.config.max_retry_attempts = attempts;
        self
    }

    /// 设置重试间隔
    pub fn retry_interval_seconds(mut self, seconds: u64) -> Self {
        self.config.retry_interval_seconds = seconds;
        self
    }

    /// 设置失败冷却时间
    pub fn failure_cooldown_minutes(mut self, minutes: i64) -> Self {
        self.config.failure_cooldown_minutes = minutes;
        self
    }

    /// 构建刷新服务
    pub fn build(
        self,
        db: Arc<DatabaseConnection>,
        oauth_client: Arc<OAuthClient>,
    ) -> OAuthTokenRefreshService {
        OAuthTokenRefreshService::new(db, oauth_client, self.config)
    }
}

impl Default for OAuthTokenRefreshServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}
