//! Authentication middleware
//!
//! Provides Pingora-compatible authentication middleware

use crate::auth::{AuthResult, AuthService};
use std::sync::Arc;

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
        self.skip_paths
            .iter()
            .any(|skip_path| path == skip_path || path.starts_with(&format!("{}/", skip_path)))
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
