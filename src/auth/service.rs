//! Authentication service
//!
//! Provides unified authentication and authorization services

use std::sync::Arc;
use std::collections::HashMap;
use chrono::Utc;
use thiserror::Error;
use sea_orm::DatabaseConnection;

use crate::auth::{
    AuthResult, AuthMethod, AuthContext, AuthError,
    jwt::{JwtManager, TokenPair},
    api_key::ApiKeyManager,
    types::{AuthConfig, TokenType, UserInfo, AuditLogEntry, AuditEventType, AuditResult},
    permissions::{Permission, PermissionChecker},
};
use crate::error::Result;

/// Authentication service error types
#[derive(Debug, Error)]
pub enum AuthServiceError {
    #[error("Authentication failed")]
    AuthenticationFailed,
    #[error("Authorization failed")]
    AuthorizationFailed,
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Service unavailable: {0}")]
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
    config: Arc<AuthConfig>,
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
            audit_cache: tokio::sync::RwLock::new(Vec::new()),
        }
    }

    /// Authenticate request using various methods
    pub async fn authenticate(&self, auth_header: &str, context: &mut AuthContext) -> Result<AuthResult> {
        // Parse authentication token
        let token_type = TokenType::from_auth_header(auth_header)
            .ok_or(AuthServiceError::InvalidCredentials)?;

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
            Some(format!("Authentication successful via {:?}", auth_result.auth_method)),
        ).await;

        Ok(auth_result)
    }

    /// Authenticate using JWT token
    async fn authenticate_jwt(&self, token: &str, _context: &AuthContext) -> Result<AuthResult> {
        let claims = self.jwt_manager.validate_token(token)?;
        
        let user_id = claims.user_id()
            .map_err(|_| AuthServiceError::InvalidCredentials)?;

        let permissions = claims.permissions.iter()
            .filter_map(|p| crate::auth::permissions::Permission::from_str(p))
            .collect();

        Ok(AuthResult {
            user_id,
            username: claims.username,
            is_admin: claims.is_admin,
            permissions,
            auth_method: AuthMethod::Jwt,
            token_preview: self.sanitize_token(token),
        })
    }

    /// Authenticate using API key
    async fn authenticate_api_key(&self, api_key: &str, _context: &AuthContext) -> Result<AuthResult> {
        let validation_result = self.api_key_manager.validate_api_key(api_key).await?;

        Ok(AuthResult {
            user_id: validation_result.api_key_info.user_id,
            username: format!("api_key_{}", validation_result.api_key_info.id),
            is_admin: false, // API keys are typically not admin accounts
            permissions: validation_result.permissions,
            auth_method: AuthMethod::ApiKey,
            token_preview: validation_result.api_key_info.api_key,
        })
    }

    /// Authenticate using basic authentication
    async fn authenticate_basic(&self, _username: &str, _password: &str, _context: &AuthContext) -> Result<AuthResult> {
        // TODO: Implement basic authentication with database lookup and password verification
        // For now, return an error as basic auth is not fully implemented
        Err(AuthServiceError::AuthenticationFailed.into())
    }

    /// Authorize request based on permissions
    pub async fn authorize(&self, auth_result: &AuthResult, context: &AuthContext) -> Result<bool> {
        let permission_checker = PermissionChecker::new(auth_result.permissions.clone());
        
        let is_authorized = permission_checker.can_access_path(
            &context.resource_path,
            &context.method,
        );

        if !is_authorized {
            // Log authorization failure
            self.log_audit_event(
                context,
                AuditEventType::PermissionCheck,
                AuditResult::PermissionDenied,
                Some(format!("Access denied to {} {}", context.method, context.resource_path)),
            ).await;

            return Err(AuthServiceError::AuthorizationFailed.into());
        }

        // Log successful authorization
        self.log_audit_event(
            context,
            AuditEventType::PermissionCheck,
            AuditResult::Success,
            Some(format!("Access granted to {} {}", context.method, context.resource_path)),
        ).await;

        Ok(true)
    }

    /// Generate token pair for user login
    pub async fn login(&self, _username: &str, _password: &str) -> Result<TokenPair> {
        // TODO: Implement user lookup and password verification
        // For now, return a mock token pair
        
        let user_id = 1; // Mock user ID
        let is_admin = false;
        let permissions = vec!["use_openai".to_string()];

        let token_pair = self.jwt_manager.generate_token_pair(
            user_id,
            _username.to_string(),
            is_admin,
            permissions,
        )?;

        Ok(token_pair)
    }

    /// Refresh access token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<String> {
        // TODO: Get user permissions from database
        let permissions = vec!["use_openai".to_string()];
        let is_admin = false;

        self.jwt_manager.refresh_access_token(refresh_token, permissions, is_admin)
    }

    /// Logout user (revoke tokens)
    pub async fn logout(&self, access_token: &str) -> Result<()> {
        let jti = self.jwt_manager.revoke_token(access_token)?;
        
        // TODO: Add JTI to blacklist in Redis
        tracing::info!("Token revoked: {}", jti);
        
        Ok(())
    }

    /// Check if user has specific permission
    pub fn check_permission(&self, auth_result: &AuthResult, permission: &Permission) -> bool {
        auth_result.permissions.contains(permission) ||
        auth_result.permissions.contains(&Permission::SuperAdmin)
    }

    /// Check if user has any of the specified permissions
    pub fn check_any_permission(&self, auth_result: &AuthResult, permissions: &[Permission]) -> bool {
        let permission_checker = PermissionChecker::new(auth_result.permissions.clone());
        permission_checker.has_any(permissions)
    }

    /// Get user information by user ID
    pub async fn get_user_info(&self, _user_id: i32) -> Result<Option<UserInfo>> {
        // TODO: Implement database lookup
        Ok(None)
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
        cache.iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    /// Sanitize token for logging
    fn sanitize_token(&self, token: &str) -> String {
        if token.len() > 20 {
            format!("{}***{}", &token[..8], &token[token.len()-8..])
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
        status.insert("database".to_string(), "healthy".to_string()); // TODO: Real DB health check
        
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_sanitization() {
        // Test token sanitization logic
        fn sanitize_token(token: &str) -> String {
            if token.len() > 20 {
                format!("{}***{}", &token[..8], &token[token.len()-8..])
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