//! Authentication service
//!
//! Provides unified authentication and authorization services

use bcrypt::verify;
use chrono::Utc;
use entity::{users, users::Entity as Users};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

use crate::auth::{
    AuthContext, AuthError, AuthMethod, AuthResult,
    api_key::ApiKeyManager,
    jwt::{JwtManager, TokenPair},
    permissions::{Permission, PermissionChecker},
    types::{AuditEventType, AuditLogEntry, AuditResult, AuthConfig, TokenType, UserInfo},
};
use crate::cache::abstract_cache::UnifiedCacheManager;
use crate::cache::keys::CacheKeyBuilder;
use crate::error::Result;

/// Authentication service error types
#[derive(Debug, Error)]
pub enum AuthServiceError {
    #[error("Authentication failed")]
    /// 认证失败
    AuthenticationFailed,
    #[error("Authorization failed")]
    /// 授权失败
    AuthorizationFailed,
    #[error("Invalid credentials")]
    /// 凭证无效
    InvalidCredentials,
    #[error("Service unavailable: {0}")]
    /// 服务不可用
    ServiceUnavailable(String),
}

impl From<AuthServiceError> for AuthError {
    fn from(service_error: AuthServiceError) -> Self {
        match service_error {
            AuthServiceError::AuthenticationFailed => AuthError::InvalidToken,
            AuthServiceError::AuthorizationFailed => AuthError::InsufficientPermissions,
            AuthServiceError::InvalidCredentials => AuthError::InvalidPassword,
            AuthServiceError::ServiceUnavailable(msg) => AuthError::InternalError(msg),
        }
    }
}

/// Authentication service
pub struct AuthService {
    /// JWT manager
    jwt_manager: Arc<JwtManager>,
    /// API key manager
    api_key_manager: Arc<ApiKeyManager>,
    /// Database connection
    db: Arc<DatabaseConnection>,
    /// Authentication configuration
    #[allow(dead_code)]
    config: Arc<AuthConfig>,
    /// Cache manager for token blacklist
    cache_manager: Option<Arc<UnifiedCacheManager>>,
    /// Audit log cache
    audit_cache: tokio::sync::RwLock<Vec<AuditLogEntry>>,
}

impl AuthService {
    /// Create new authentication service
    pub fn new(
        jwt_manager: Arc<JwtManager>,
        api_key_manager: Arc<ApiKeyManager>,
        db: Arc<DatabaseConnection>,
        config: Arc<AuthConfig>,
    ) -> Self {
        Self {
            jwt_manager,
            api_key_manager,
            db,
            config,
            cache_manager: None,
            audit_cache: tokio::sync::RwLock::new(Vec::new()),
        }
    }

    /// Create authentication service with cache manager
    pub fn with_cache(
        jwt_manager: Arc<JwtManager>,
        api_key_manager: Arc<ApiKeyManager>,
        db: Arc<DatabaseConnection>,
        config: Arc<AuthConfig>,
        cache_manager: Arc<UnifiedCacheManager>,
    ) -> Self {
        Self {
            jwt_manager,
            api_key_manager,
            db,
            config,
            cache_manager: Some(cache_manager),
            audit_cache: tokio::sync::RwLock::new(Vec::new()),
        }
    }

    /// Authenticate request using various methods
    pub async fn authenticate(
        &self,
        auth_header: &str,
        context: &mut AuthContext,
    ) -> Result<AuthResult> {
        // Parse authentication token
        let token_type =
            TokenType::from_auth_header(auth_header).ok_or(AuthServiceError::InvalidCredentials)?;

        let auth_result = match token_type {
            TokenType::Bearer(token) => self.authenticate_jwt(&token, context).await,
            TokenType::ApiKey(api_key) => self.authenticate_api_key(&api_key, context).await,
            TokenType::Basic { username, password } => {
                self.authenticate_basic(&username, &password, context).await
            }
        }?;

        // Log successful authentication
        self.log_audit_event(
            context,
            AuditEventType::ApiCall,
            AuditResult::Success,
            Some(format!(
                "Authentication successful via {:?}",
                auth_result.auth_method
            )),
        )
        .await;

        Ok(auth_result)
    }

    /// Authenticate using JWT token
    pub async fn authenticate_jwt(
        &self,
        token: &str,
        _context: &AuthContext,
    ) -> Result<AuthResult> {
        let claims = self.jwt_manager.validate_token(token)?;

        let user_id = claims
            .user_id()
            .map_err(|_| AuthServiceError::InvalidCredentials)?;

        let permissions = claims
            .permissions
            .iter()
            .filter_map(|p| crate::auth::permissions::Permission::from_str(p))
            .collect();

        Ok(AuthResult {
            user_id,
            username: claims.username,
            is_admin: claims.is_admin,
            permissions,
            auth_method: AuthMethod::Jwt,
            token_preview: self.sanitize_token(token),
            token_info: None, // JWT认证不需要OAuth token信息
            expires_at: Some(chrono::DateTime::from_timestamp(claims.exp, 0).unwrap_or_else(chrono::Utc::now)),
            session_info: None,
        })
    }

    /// Authenticate using API key
    pub async fn authenticate_api_key(
        &self,
        api_key: &str,
        _context: &AuthContext,
    ) -> Result<AuthResult> {
        let validation_result = self.api_key_manager.validate_api_key(api_key).await?;

        Ok(AuthResult {
            user_id: validation_result.api_key_info.user_id,
            username: format!("api_key_{}", validation_result.api_key_info.id),
            is_admin: false, // API keys are typically not admin accounts
            permissions: validation_result.permissions,
            auth_method: AuthMethod::ApiKey,
            token_preview: validation_result.api_key_info.api_key,
            token_info: None, // API key认证不需要OAuth token信息
            expires_at: None, // API密钥通常无过期时间
            session_info: Some(serde_json::json!({
                "api_key_id": validation_result.api_key_info.id,
                "provider_type": "unknown"
            })),
        })
    }

    /// Authenticate user service API key - 直接返回user_service_apis模型
    pub async fn authenticate_user_service_api(
        &self,
        api_key: &str,
    ) -> Result<entity::user_service_apis::Model> {
        // 从数据库查询user_service_apis
        let user_api = entity::user_service_apis::Entity::find()
            .filter(entity::user_service_apis::Column::ApiKey.eq(api_key))
            .filter(entity::user_service_apis::Column::IsActive.eq(true))
            .one(&*self.db)
            .await
            .map_err(|e| AuthServiceError::ServiceUnavailable(format!("Database error: {}", e)))?
            .ok_or(AuthServiceError::InvalidCredentials)?;

        // 检查API密钥是否过期
        if let Some(expires_at) = user_api.expires_at {
            if expires_at < chrono::Utc::now().naive_utc() {
                return Err(AuthServiceError::InvalidCredentials.into());
            }
        }

        Ok(user_api)
    }

    /// Authenticate using basic authentication
    pub async fn authenticate_basic(
        &self,
        username: &str,
        password: &str,
        _context: &AuthContext,
    ) -> Result<AuthResult> {
        // Query user from database
        let user = Users::find()
            .filter(users::Column::Username.eq(username))
            .filter(users::Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await
            .map_err(|e| AuthServiceError::ServiceUnavailable(format!("Database error: {}", e)))?;

        let user = user.ok_or(AuthServiceError::InvalidCredentials)?;

        // Verify password
        let password_valid = verify(password, &user.password_hash).map_err(|e| {
            AuthServiceError::ServiceUnavailable(format!("Password verification error: {}", e))
        })?;

        if !password_valid {
            return Err(AuthServiceError::InvalidCredentials.into());
        }

        // Get user permissions based on admin status
        let permissions = if user.is_admin {
            vec![Permission::SuperAdmin]
        } else {
            vec![Permission::UseApi] // 基本权限
        };

        Ok(AuthResult {
            user_id: user.id,
            username: user.username.clone(),
            is_admin: user.is_admin,
            permissions,
            auth_method: AuthMethod::BasicAuth,
            token_preview: self.sanitize_token(&format!("{}:{}", username, "***")),
            token_info: None, // Basic认证不需要OAuth token信息
            expires_at: None, // Basic认证通常无过期时间
            session_info: Some(serde_json::json!({
                "username": username,
                "auth_type": "basic"
            })),
        })
    }

    /// Authorize request based on permissions
    pub async fn authorize(&self, auth_result: &AuthResult, context: &AuthContext) -> Result<bool> {
        let permission_checker = PermissionChecker::new(auth_result.permissions.clone());

        let is_authorized =
            permission_checker.can_access_path(&context.resource_path, &context.method);

        if !is_authorized {
            // Log authorization failure
            self.log_audit_event(
                context,
                AuditEventType::PermissionCheck,
                AuditResult::PermissionDenied,
                Some(format!(
                    "Access denied to {} {}",
                    context.method, context.resource_path
                )),
            )
            .await;

            return Err(AuthServiceError::AuthorizationFailed.into());
        }

        // Log successful authorization
        self.log_audit_event(
            context,
            AuditEventType::PermissionCheck,
            AuditResult::Success,
            Some(format!(
                "Access granted to {} {}",
                context.method, context.resource_path
            )),
        )
        .await;

        Ok(true)
    }

    /// Generate token pair for user login
    pub async fn login(&self, username: &str, password: &str) -> Result<TokenPair> {
        // Query user from database
        let user = Users::find()
            .filter(users::Column::Username.eq(username))
            .filter(users::Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await
            .map_err(|e| AuthServiceError::ServiceUnavailable(format!("Database error: {}", e)))?;

        let user = user.ok_or(AuthServiceError::InvalidCredentials)?;

        // Verify password
        let password_valid = verify(password, &user.password_hash).map_err(|e| {
            AuthServiceError::ServiceUnavailable(format!("Password verification error: {}", e))
        })?;

        if !password_valid {
            return Err(AuthServiceError::InvalidCredentials.into());
        }

        // Get user permissions based on admin status and user configuration
        let permissions = self.get_user_permissions(&user).await;

        let token_pair = self.jwt_manager.generate_token_pair(
            user.id,
            user.username.clone(),
            user.is_admin,
            permissions,
        )?;

        // Update last login time
        self.update_last_login(user.id).await?;

        Ok(token_pair)
    }

    /// Refresh access token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<String> {
        // Validate refresh token and get user ID
        let claims = self.jwt_manager.validate_token(refresh_token)?;
        let user_id = claims
            .user_id()
            .map_err(|_| AuthServiceError::InvalidCredentials)?;

        // Get user from database
        let user = Users::find_by_id(user_id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| AuthServiceError::ServiceUnavailable(format!("Database error: {}", e)))?
            .ok_or(AuthServiceError::InvalidCredentials)?;

        if !user.is_active {
            return Err(AuthServiceError::InvalidCredentials.into());
        }

        // Get current user permissions
        let permissions = self.get_user_permissions(&user).await;

        self.jwt_manager
            .refresh_access_token(refresh_token, permissions, user.is_admin)
    }

    /// Logout user (revoke tokens)
    pub async fn logout(&self, access_token: &str) -> Result<()> {
        let jti = self.jwt_manager.revoke_token(access_token)?;

        // Add JTI to blacklist in Redis
        if let Some(cache_manager) = &self.cache_manager {
            let blacklist_key = CacheKeyBuilder::auth_token(&jti);
            let blacklist_data = serde_json::json!({
                "revoked_at": Utc::now(),
                "token_type": "access_token",
                "reason": "user_logout"
            });

            if let Err(e) = cache_manager
                .set_with_strategy(&blacklist_key, &blacklist_data)
                .await
            {
                tracing::warn!("Failed to add token to blacklist cache: {}", e);
            } else {
                tracing::debug!("Token added to blacklist: {}", jti);
            }
        }

        tracing::info!("Token revoked: {}", jti);
        Ok(())
    }

    /// Check if token is blacklisted
    pub async fn is_token_blacklisted(&self, jti: &str) -> bool {
        if let Some(cache_manager) = &self.cache_manager {
            let blacklist_key = CacheKeyBuilder::auth_token(jti);
            match cache_manager.exists(&blacklist_key.build()).await {
                Ok(exists) => {
                    if exists {
                        tracing::debug!("Token found in blacklist: {}", jti);
                        true
                    } else {
                        false
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to check token blacklist: {}", e);
                    // 在缓存不可用时，为了安全起见，不允许访问
                    false
                }
            }
        } else {
            // 没有缓存管理器时，无法检查黑名单
            false
        }
    }

    /// Check if user has specific permission
    pub fn check_permission(&self, auth_result: &AuthResult, permission: &Permission) -> bool {
        auth_result.permissions.contains(permission)
            || auth_result.permissions.contains(&Permission::SuperAdmin)
    }

    /// Check if user has any of the specified permissions
    pub fn check_any_permission(
        &self,
        auth_result: &AuthResult,
        permissions: &[Permission],
    ) -> bool {
        let permission_checker = PermissionChecker::new(auth_result.permissions.clone());
        permission_checker.has_any(permissions)
    }

    /// Get user information by user ID
    pub async fn get_user_info(&self, user_id: i32) -> Result<Option<UserInfo>> {
        let user = Users::find_by_id(user_id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| AuthServiceError::ServiceUnavailable(format!("Database error: {}", e)))?;

        if let Some(user) = user {
            let permission_strings = self.get_user_permissions(&user).await;
            let permissions: Vec<Permission> = permission_strings
                .iter()
                .filter_map(|s| Permission::from_str(s))
                .collect();

            Ok(Some(UserInfo {
                id: user.id,
                username: user.username,
                email: user.email,
                is_admin: user.is_admin,
                is_active: user.is_active,
                permissions,
                created_at: user.created_at.and_utc(),
                last_login: user.last_login.map(|dt| dt.and_utc()),
            }))
        } else {
            Ok(None)
        }
    }

    /// Log audit event
    async fn log_audit_event(
        &self,
        context: &AuthContext,
        event_type: AuditEventType,
        result: AuditResult,
        message: Option<String>,
    ) {
        let audit_entry = AuditLogEntry {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: context.get_user_id(),
            username: context.get_username().map(|s| s.to_string()),
            event_type,
            resource_path: context.resource_path.clone(),
            method: context.method.clone(),
            client_ip: context.client_ip.clone(),
            user_agent: context.user_agent.clone(),
            result,
            error_message: message,
            metadata: HashMap::new(),
            timestamp: Utc::now(),
        };

        // Add to cache (in production, should write to database immediately)
        {
            let mut cache = self.audit_cache.write().await;
            cache.push(audit_entry);

            // Keep only last 1000 entries in memory
            if cache.len() > 1000 {
                let drain_count = cache.len() - 1000;
                cache.drain(0..drain_count);
            }
        }
    }

    /// Get recent audit logs
    pub async fn get_audit_logs(&self, limit: usize) -> Vec<AuditLogEntry> {
        let cache = self.audit_cache.read().await;
        cache.iter().rev().take(limit).cloned().collect()
    }

    /// Sanitize token for logging
    fn sanitize_token(&self, token: &str) -> String {
        if token.len() > 20 {
            format!("{}***{}", &token[..8], &token[token.len() - 8..])
        } else if token.len() > 8 {
            format!("{}***", &token[..4])
        } else {
            "***".to_string()
        }
    }

    /// Health check for authentication service
    pub async fn health_check(&self) -> HashMap<String, String> {
        let mut status = HashMap::new();

        status.insert("jwt_manager".to_string(), "healthy".to_string());
        status.insert("api_key_manager".to_string(), "healthy".to_string());
        // Real database health check
        let db_status = match self.test_database_connection().await {
            Ok(_) => "healthy",
            Err(_) => "unhealthy",
        };
        status.insert("database".to_string(), db_status.to_string());

        // Check cache stats
        let cache_stats = self.api_key_manager.get_cache_stats().await;
        status.insert("cache_keys".to_string(), cache_stats.total_keys.to_string());

        // Check audit log cache
        let audit_count = self.audit_cache.read().await.len();
        status.insert("audit_entries".to_string(), audit_count.to_string());

        status
    }

    /// Cleanup expired resources
    pub async fn cleanup(&self) {
        // Cleanup API key cache
        self.api_key_manager.cleanup_expired_cache().await;

        // Cleanup old audit logs
        let mut cache = self.audit_cache.write().await;
        let one_day_ago = Utc::now() - chrono::Duration::days(1);
        cache.retain(|entry| entry.timestamp > one_day_ago);
    }

    /// Get user permissions based on database configuration
    async fn get_user_permissions(&self, user: &users::Model) -> Vec<String> {
        if user.is_admin {
            vec![
                "super_admin".to_string(),
                "use_openai".to_string(),
                "use_claude".to_string(),
                "use_gemini".to_string(),
                "manage_users".to_string(),
                "view_stats".to_string(),
            ]
        } else {
            // Default user permissions - could be extended to query from user_permissions table
            vec![
                "use_openai".to_string(),
                "use_claude".to_string(),
                "use_gemini".to_string(),
            ]
        }
    }

    /// Update user's last login time
    async fn update_last_login(&self, user_id: i32) -> Result<()> {
        use sea_orm::ActiveModelTrait;

        // Find the user
        let user = Users::find_by_id(user_id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| AuthServiceError::ServiceUnavailable(format!("Database error: {}", e)))?
            .ok_or(AuthServiceError::InvalidCredentials)?;

        // Update last login time
        let mut user: users::ActiveModel = user.into();
        user.last_login = sea_orm::Set(Some(Utc::now().naive_utc()));

        user.update(self.db.as_ref())
            .await
            .map_err(|e| AuthServiceError::ServiceUnavailable(format!("Database error: {}", e)))?;

        Ok(())
    }

    /// Test database connection
    async fn test_database_connection(&self) -> Result<()> {
        // Simple query to test database connectivity
        Users::find()
            .limit(1)
            .one(self.db.as_ref())
            .await
            .map_err(|e| {
                AuthServiceError::ServiceUnavailable(format!(
                    "Database connection test failed: {}",
                    e
                ))
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_token_sanitization() {
        // Test token sanitization logic
        fn sanitize_token(token: &str) -> String {
            if token.len() > 20 {
                format!("{}***{}", &token[..8], &token[token.len() - 8..])
            } else if token.len() > 8 {
                format!("{}***", &token[..4])
            } else {
                "***".to_string()
            }
        }

        let long_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ";
        let sanitized = sanitize_token(long_token);
        assert!(sanitized.contains("***"));
        assert!(sanitized.len() < long_token.len());

        let short_token = "short";
        let sanitized_short = sanitize_token(short_token);
        assert_eq!(sanitized_short, "***");
    }
}
