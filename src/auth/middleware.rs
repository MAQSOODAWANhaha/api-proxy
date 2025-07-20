//! Authentication middleware
//!
//! Provides Pingora-compatible authentication middleware

use std::sync::Arc;
use crate::auth::{AuthService, AuthResult};

/// Authentication middleware for Pingora
pub struct AuthMiddleware {
    /// Authentication service
    auth_service: Arc<AuthService>,
    /// Skip authentication for certain paths
    skip_paths: Vec<String>,
}

impl AuthMiddleware {
    /// Create new authentication middleware
    pub fn new(auth_service: Arc<AuthService>) -> Self {
        Self {
            auth_service,
            skip_paths: vec![
                "/health".to_string(),
                "/metrics".to_string(),
                "/ping".to_string(),
            ],
        }
    }

    /// Add path to skip authentication
    pub fn skip_path(mut self, path: String) -> Self {
        self.skip_paths.push(path);
        self
    }

    /// Check if path should skip authentication
    pub fn should_skip_auth(&self, path: &str) -> bool {
        self.skip_paths.iter().any(|skip_path| {
            path == skip_path || path.starts_with(&format!("{}/", skip_path))
        })
    }

    /// Get authentication service
    pub fn auth_service(&self) -> &Arc<AuthService> {
        &self.auth_service
    }
}

/// Authentication result for Pingora context
#[derive(Debug, Clone)]
pub struct AuthenticationResult {
    /// Whether request is authenticated
    pub is_authenticated: bool,
    /// User ID if authenticated
    pub user_id: Option<i32>,
    /// Username if authenticated
    pub username: Option<String>,
    /// Whether user is admin
    pub is_admin: bool,
    /// Authentication method used
    pub auth_method: Option<String>,
    /// Sanitized token for logging
    pub token_preview: Option<String>,
}

impl Default for AuthenticationResult {
    fn default() -> Self {
        Self {
            is_authenticated: false,
            user_id: None,
            username: None,
            is_admin: false,
            auth_method: None,
            token_preview: None,
        }
    }
}

impl From<AuthResult> for AuthenticationResult {
    fn from(auth_result: AuthResult) -> Self {
        Self {
            is_authenticated: true,
            user_id: Some(auth_result.user_id),
            username: Some(auth_result.username),
            is_admin: auth_result.is_admin,
            auth_method: Some(format!("{:?}", auth_result.auth_method)),
            token_preview: Some(auth_result.token_preview),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::helpers::init_test_env;

    #[test]
    fn test_authentication_result_conversion() {
        init_test_env();
        
        let auth_result = AuthResult {
            user_id: 123,
            username: "testuser".to_string(),
            is_admin: true,
            permissions: vec![],
            auth_method: crate::auth::AuthMethod::Jwt,
            token_preview: "sk-abc***xyz".to_string(),
        };
        
        let auth_result_converted: AuthenticationResult = auth_result.into();
        
        assert!(auth_result_converted.is_authenticated);
        assert_eq!(auth_result_converted.user_id, Some(123));
        assert_eq!(auth_result_converted.username, Some("testuser".to_string()));
        assert!(auth_result_converted.is_admin);
        assert_eq!(auth_result_converted.auth_method, Some("Jwt".to_string()));
        assert_eq!(auth_result_converted.token_preview, Some("sk-abc***xyz".to_string()));
    }

    #[test]
    fn test_default_authentication_result() {
        init_test_env();
        
        let default_result = AuthenticationResult::default();
        
        assert!(!default_result.is_authenticated);
        assert_eq!(default_result.user_id, None);
        assert_eq!(default_result.username, None);
        assert!(!default_result.is_admin);
        assert_eq!(default_result.auth_method, None);
        assert_eq!(default_result.token_preview, None);
    }
}