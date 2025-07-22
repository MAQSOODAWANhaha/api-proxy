//! # 统一认证层
//!
//! 为双端口架构提供统一的认证服务，可被Pingora代理服务和Axum管理服务共享使用

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use serde::Serialize;
use chrono::{DateTime, Utc};
use tracing::debug;

use crate::auth::{
    AuthResult, AuthMethod, AuthContext, AuthError,
    AuthService, JwtManager, ApiKeyManager,
    types::AuthConfig,
};
use crate::error::Result;

/// 统一认证管理器
/// 
/// 提供跨服务的认证功能，支持多种认证方式的统一处理
pub struct UnifiedAuthManager {
    /// 核心认证服务
    auth_service: Arc<AuthService>,
    /// 认证缓存
    auth_cache: Arc<RwLock<AuthCache>>,
    /// 认证配置
    config: Arc<AuthConfig>,
}

/// 认证缓存
/// 
/// 缓存认证结果以提高性能，避免重复的数据库查询
#[derive(Debug, Default)]
struct AuthCache {
    /// JWT令牌缓存 (token_hash -> (AuthResult, expire_time))
    jwt_cache: HashMap<String, (AuthResult, DateTime<Utc>)>,
    /// API密钥缓存 (key_hash -> (AuthResult, expire_time))
    api_key_cache: HashMap<String, (AuthResult, DateTime<Utc>)>,
    /// 黑名单缓存 (token_hash -> expire_time)
    blacklist: HashMap<String, DateTime<Utc>>,
    /// 缓存统计
    stats: CacheStats,
}

/// 缓存统计信息
#[derive(Debug, Default, Clone, Serialize)]
pub struct CacheStats {
    /// 总查询次数
    pub total_queries: u64,
    /// 缓存命中次数
    pub cache_hits: u64,
    /// 缓存未命中次数
    pub cache_misses: u64,
    /// 黑名单命中次数
    pub blacklist_hits: u64,
}

impl CacheStats {
    /// 计算缓存命中率
    pub fn hit_rate(&self) -> f64 {
        if self.total_queries == 0 {
            0.0
        } else {
            self.cache_hits as f64 / self.total_queries as f64
        }
    }
}

/// 认证请求参数
#[derive(Debug, Clone)]
pub struct AuthRequest {
    /// 授权头内容
    pub authorization: Option<String>,
    /// 客户端IP
    pub client_ip: Option<String>,
    /// 用户代理
    pub user_agent: Option<String>,
    /// 请求路径
    pub path: String,
    /// HTTP方法
    pub method: String,
    /// 额外的认证头（如API-Key）
    pub extra_headers: HashMap<String, String>,
}

impl UnifiedAuthManager {
    /// 创建新的统一认证管理器
    pub fn new(auth_service: Arc<AuthService>, config: Arc<AuthConfig>) -> Self {
        Self {
            auth_service,
            auth_cache: Arc::new(RwLock::new(AuthCache::default())),
            config,
        }
    }

    /// 统一认证接口
    /// 
    /// 支持多种认证方式：JWT、API Key、Basic Auth
    pub async fn authenticate(&self, request: AuthRequest) -> Result<AuthResult> {
        debug!("Processing authentication request for path: {}", request.path);

        // 更新统计
        {
            let mut cache = self.auth_cache.write().await;
            cache.stats.total_queries += 1;
        }

        // 尝试从Authorization头解析认证信息
        if let Some(auth_header) = &request.authorization {
            return self.authenticate_from_header(auth_header, &request).await;
        }

        // 尝试从额外头部获取API Key
        if let Some(api_key) = request.extra_headers.get("x-api-key")
            .or_else(|| request.extra_headers.get("api-key")) {
            return self.authenticate_api_key(api_key, &request).await;
        }

        // 检查是否为公开路径
        if self.is_public_path(&request.path) {
            return Ok(self.create_anonymous_auth_result());
        }

        Err(AuthError::MissingCredentials.into())
    }

    /// 从Authorization头认证
    async fn authenticate_from_header(&self, auth_header: &str, request: &AuthRequest) -> Result<AuthResult> {
        if auth_header.starts_with("Bearer ") {
            // JWT认证
            let token = &auth_header[7..]; // 移除 "Bearer " 前缀
            self.authenticate_jwt(token, request).await
        } else if auth_header.starts_with("Basic ") {
            // Basic认证
            let encoded = &auth_header[6..]; // 移除 "Basic " 前缀
            self.authenticate_basic(encoded, request).await
        } else {
            // 尝试作为API Key处理
            self.authenticate_api_key(auth_header, request).await
        }
    }

    /// JWT认证
    async fn authenticate_jwt(&self, token: &str, request: &AuthRequest) -> Result<AuthResult> {
        let token_hash = self.hash_token(token);

        // 检查黑名单
        if self.is_blacklisted(&token_hash).await {
            return Err(AuthError::TokenBlacklisted.into());
        }

        // 检查缓存
        if let Some(cached_result) = self.get_cached_auth_result(&token_hash).await {
            debug!("JWT authentication cache hit");
            return Ok(cached_result);
        }

        // 使用认证服务验证
        let mut context = AuthContext::new(request.path.clone(), request.method.clone());
        context.client_ip = request.client_ip.clone();
        context.user_agent = request.user_agent.clone();

        match self.auth_service.authenticate_jwt(token, &mut context).await {
            Ok(auth_result) => {
                // 缓存结果
                self.cache_auth_result(&token_hash, &auth_result).await;
                Ok(auth_result)
            }
            Err(e) => {
                self.record_cache_miss().await;
                Err(e)
            }
        }
    }

    /// API Key认证
    async fn authenticate_api_key(&self, api_key: &str, request: &AuthRequest) -> Result<AuthResult> {
        let key_hash = self.hash_token(api_key);

        // 检查缓存
        if let Some(cached_result) = self.get_cached_auth_result(&key_hash).await {
            debug!("API Key authentication cache hit");
            return Ok(cached_result);
        }

        // 使用认证服务验证
        let mut context = AuthContext::new(request.path.clone(), request.method.clone());
        context.client_ip = request.client_ip.clone();
        context.user_agent = request.user_agent.clone();

        match self.auth_service.authenticate_api_key(api_key, &mut context).await {
            Ok(auth_result) => {
                // 缓存结果
                self.cache_auth_result(&key_hash, &auth_result).await;
                Ok(auth_result)
            }
            Err(e) => {
                self.record_cache_miss().await;
                Err(e)
            }
        }
    }

    /// Basic认证
    async fn authenticate_basic(&self, encoded: &str, request: &AuthRequest) -> Result<AuthResult> {
        // Basic认证通常不缓存，因为包含密码
        use base64::{Engine as _, engine::general_purpose};
        let decoded = general_purpose::STANDARD.decode(encoded)
            .map_err(|_| AuthError::InvalidCredentials)?;
        
        let credentials = String::from_utf8(decoded)
            .map_err(|_| AuthError::InvalidCredentials)?;
        
        let parts: Vec<&str> = credentials.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(AuthError::InvalidCredentials.into());
        }

        let mut context = AuthContext::new(request.path.clone(), request.method.clone());
        context.client_ip = request.client_ip.clone();
        context.user_agent = request.user_agent.clone();

        self.auth_service.authenticate_basic(parts[0], parts[1], &mut context).await
    }

    /// 检查是否为公开路径
    fn is_public_path(&self, path: &str) -> bool {
        // 定义公开路径模式
        let public_patterns = [
            "/health",
            "/metrics",
            "/api/health",
            "/api/version",
        ];

        public_patterns.iter().any(|pattern| path.starts_with(pattern))
    }

    /// 创建匿名认证结果
    fn create_anonymous_auth_result(&self) -> AuthResult {
        AuthResult {
            user_id: 0,
            username: "anonymous".to_string(),
            is_admin: false,
            permissions: vec![],
            auth_method: AuthMethod::Internal,
            token_preview: "anonymous".to_string(),
        }
    }

    /// 令牌黑名单管理
    pub async fn blacklist_token(&self, token: &str, expire_time: DateTime<Utc>) {
        let token_hash = self.hash_token(token);
        let mut cache = self.auth_cache.write().await;
        cache.blacklist.insert(token_hash.clone(), expire_time);
        
        // 从认证缓存中移除
        cache.jwt_cache.remove(&token_hash);
        cache.api_key_cache.remove(&token_hash);
    }

    /// 检查令牌是否在黑名单中
    async fn is_blacklisted(&self, token_hash: &str) -> bool {
        let cache = self.auth_cache.read().await;
        if let Some(expire_time) = cache.blacklist.get(token_hash) {
            if Utc::now() < *expire_time {
                return true;
            }
        }
        false
    }

    /// 获取缓存的认证结果
    async fn get_cached_auth_result(&self, token_hash: &str) -> Option<AuthResult> {
        let cache = self.auth_cache.read().await;
        
        // 检查JWT缓存
        if let Some((auth_result, expire_time)) = cache.jwt_cache.get(token_hash) {
            if Utc::now() < *expire_time {
                let result = auth_result.clone();
                // 更新缓存命中统计
                drop(cache);
                self.record_cache_hit().await;
                return Some(result);
            }
        }

        // 检查API Key缓存
        if let Some((auth_result, expire_time)) = cache.api_key_cache.get(token_hash) {
            if Utc::now() < *expire_time {
                let result = auth_result.clone();
                drop(cache);
                self.record_cache_hit().await;
                return Some(result);
            }
        }

        None
    }

    /// 缓存认证结果
    async fn cache_auth_result(&self, token_hash: &str, auth_result: &AuthResult) {
        let expire_time = Utc::now() + chrono::Duration::minutes(self.config.cache_ttl_minutes as i64);
        let mut cache = self.auth_cache.write().await;
        
        match auth_result.auth_method {
            AuthMethod::Jwt => {
                cache.jwt_cache.insert(token_hash.to_string(), (auth_result.clone(), expire_time));
            }
            AuthMethod::ApiKey => {
                cache.api_key_cache.insert(token_hash.to_string(), (auth_result.clone(), expire_time));
            }
            _ => {
                // Basic认证等不缓存
            }
        }
    }

    /// 记录缓存命中
    async fn record_cache_hit(&self) {
        let mut cache = self.auth_cache.write().await;
        cache.stats.cache_hits += 1;
    }

    /// 记录缓存未命中
    async fn record_cache_miss(&self) {
        let mut cache = self.auth_cache.write().await;
        cache.stats.cache_misses += 1;
    }

    /// 哈希令牌
    fn hash_token(&self, token: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// 清理过期缓存
    pub async fn cleanup_expired_cache(&self) {
        let mut cache = self.auth_cache.write().await;
        let now = Utc::now();

        // 清理JWT缓存
        cache.jwt_cache.retain(|_, (_, expire_time)| now < *expire_time);
        
        // 清理API Key缓存
        cache.api_key_cache.retain(|_, (_, expire_time)| now < *expire_time);
        
        // 清理黑名单
        cache.blacklist.retain(|_, expire_time| now < *expire_time);

        debug!("Cleaned up expired auth cache entries");
    }

    /// 获取缓存统计
    pub async fn get_cache_stats(&self) -> CacheStats {
        let cache = self.auth_cache.read().await;
        cache.stats.clone()
    }

    /// 清空缓存
    pub async fn clear_cache(&self) {
        let mut cache = self.auth_cache.write().await;
        cache.jwt_cache.clear();
        cache.api_key_cache.clear();
        cache.blacklist.clear();
        cache.stats = CacheStats::default();
        debug!("Cleared all auth cache");
    }

    /// 获取认证服务引用
    pub fn get_auth_service(&self) -> &Arc<AuthService> {
        &self.auth_service
    }
}

/// 为双端口架构创建统一认证管理器的工厂函数
pub async fn create_unified_auth_manager(
    jwt_manager: Arc<JwtManager>,
    api_key_manager: Arc<ApiKeyManager>,
    db: Arc<sea_orm::DatabaseConnection>,
    config: Arc<AuthConfig>,
    cache_manager: Option<Arc<crate::cache::abstract_cache::UnifiedCacheManager>>,
) -> Result<Arc<UnifiedAuthManager>> {
    let auth_service = if let Some(cache) = cache_manager {
        Arc::new(AuthService::with_cache(
            jwt_manager,
            api_key_manager,
            db,
            config.clone(),
            cache,
        ))
    } else {
        Arc::new(AuthService::new(
            jwt_manager,
            api_key_manager,
            db,
            config.clone(),
        ))
    };

    Ok(Arc::new(UnifiedAuthManager::new(auth_service, config)))
}