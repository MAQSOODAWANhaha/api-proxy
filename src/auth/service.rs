//! Authentication service
//!
//! Provides unified authentication and authorization services

use bcrypt::verify;
use chrono::Utc;
use entity::{users, users::Entity as Users};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::sync::Arc;

use crate::auth::api_key_manager::ApiKeyManager;
use crate::auth::jwt::{JwtManager, TokenPair};
use crate::auth::permissions::UserRole;
use crate::auth::types::{
    AuthContext, AuthMethod, AuthResult, Authentication, TokenType, UserInfo,
};
use crate::error::{ProxyError, Result};

/// Authentication service
pub struct ApiKeyAuthenticationService {
    /// JWT manager
    pub jwt_manager: Arc<JwtManager>,
    /// API key manager
    api_key_manager: Arc<ApiKeyManager>,
    /// Database connection
    db: Arc<DatabaseConnection>,
}

impl ApiKeyAuthenticationService {
    /// Create new authentication service
    #[must_use]
    pub const fn new(
        jwt_manager: Arc<JwtManager>,
        api_key_manager: Arc<ApiKeyManager>,
        db: Arc<DatabaseConnection>,
    ) -> Self {
        Self {
            jwt_manager,
            api_key_manager,
            db,
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
            TokenType::from_auth_header(auth_header).ok_or_else(invalid_credentials_error)?;

        let auth_result = match token_type {
            TokenType::Bearer(token) => self.authenticate_jwt(&token, context),
            TokenType::ApiKey(api_key) => self.authenticate_api_key(&api_key, context).await,
        }?;

        Ok(auth_result)
    }

    /// Authenticate using JWT token
    pub fn authenticate_jwt(&self, token: &str, _context: &AuthContext) -> Result<AuthResult> {
        let claims = self.jwt_manager.validate_token(token)?;

        let user_id = claims.user_id()?;

        // 确定用户角色
        let role = if claims.is_admin {
            UserRole::Admin
        } else {
            UserRole::RegularUser
        };

        Ok(AuthResult {
            user_id,
            username: claims.username,
            is_admin: claims.is_admin,
            role,
            auth_method: AuthMethod::Jwt,
            token_preview: Self::sanitize_token(token),
            token_info: None, // JWT认证不需要OAuth token信息
            expires_at: Some(
                chrono::DateTime::from_timestamp(claims.exp, 0).unwrap_or_else(chrono::Utc::now),
            ),
            session_info: None,
        })
    }

    /// Authenticate using API key
    pub async fn authenticate_api_key(
        &self,
        api_key: &str,
        _context: &AuthContext,
    ) -> Result<AuthResult> {
        let api_key_info = self.api_key_manager.validate_api_key(api_key).await?;

        Ok(AuthResult {
            user_id: api_key_info.user_id,
            username: format!("api_key_{}", api_key_info.id),
            is_admin: false, // API keys are typically not admin accounts
            role: UserRole::RegularUser,
            auth_method: AuthMethod::ApiKey,
            token_preview: api_key_info.api_key,
            token_info: None, // API key认证不需要OAuth token信息
            expires_at: None, // API密钥通常无过期时间
            session_info: Some(serde_json::json!({
                "api_key_id": api_key_info.id,
                "provider_type": "unknown"
            })),
        })
    }

    /// Authenticate user service API key - `直接返回user_service_apis模型`
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
            .map_err(|e| crate::error!(Database, format!("Database error: {}", e)))?
            .ok_or_else(invalid_credentials_error)?;

        // 检查API密钥是否过期
        if let Some(expires_at) = user_api.expires_at
            && expires_at < chrono::Utc::now().naive_utc()
        {
            return Err(invalid_credentials_error());
        }

        Ok(user_api)
    }

    /// 代理端 API Key 认证入口（向后兼容）
    pub async fn authenticate_proxy_request(&self, api_key: &str) -> Result<Authentication> {
        let user_api = self.authenticate_user_service_api(api_key).await?;

        Ok(Authentication {
            user_id: user_api.user_id,
            provider_type_id: user_api.provider_type_id,
            user_api,
        })
    }

    /// Generate token pair for user login
    pub async fn login(&self, username: &str, password: &str) -> Result<TokenPair> {
        // Query user from database
        let user = Users::find()
            .filter(users::Column::Username.eq(username))
            .filter(users::Column::IsActive.eq(true))
            .one(self.db.as_ref())
            .await
            .map_err(|e| crate::error!(Database, format!("Database error: {}", e)))?;

        let user = user.ok_or_else(invalid_credentials_error)?;

        // Verify password
        let password_valid = verify(password, &user.password_hash)
            .map_err(|e| crate::error!(Internal, "Password verification error", e))?;

        if !password_valid {
            return Err(invalid_credentials_error());
        }

        // 确定用户角色
        let role = if user.is_admin {
            UserRole::Admin
        } else {
            UserRole::RegularUser
        };

        let token_pair = self.jwt_manager.generate_token_pair(
            user.id,
            user.username.clone(),
            user.is_admin,
            role,
        )?;

        // Update last login time
        self.update_last_login(user.id).await?;

        Ok(token_pair)
    }

    /// Refresh access token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<String> {
        // Validate refresh token and get user ID
        let claims = self.jwt_manager.validate_token(refresh_token)?;
        let user_id = claims.user_id()?;

        // Get user from database
        let user = Users::find_by_id(user_id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| crate::error!(Database, format!("Database error: {}", e)))?
            .ok_or_else(invalid_credentials_error)?;

        if !user.is_active {
            return Err(invalid_credentials_error());
        }

        // 确定用户角色
        let role = if user.is_admin {
            UserRole::Admin
        } else {
            UserRole::RegularUser
        };

        self.jwt_manager
            .refresh_access_token(refresh_token, role, user.is_admin)
    }

    /// Get user information by user ID
    pub async fn get_user_info(&self, user_id: i32) -> Result<Option<UserInfo>> {
        let user = Users::find_by_id(user_id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| crate::error!(Database, format!("Database error: {}", e)))?;

        if let Some(user) = user {
            let role = if user.is_admin {
                UserRole::Admin
            } else {
                UserRole::RegularUser
            };

            Ok(Some(UserInfo {
                id: user.id,
                username: user.username,
                email: user.email,
                is_admin: user.is_admin,
                is_active: user.is_active,
                permissions: vec![role], // 简化权限列表
                created_at: user.created_at.and_utc(),
                last_login: user.last_login.map(|dt| dt.and_utc()),
            }))
        } else {
            Ok(None)
        }
    }

    /// Sanitize token for logging
    fn sanitize_token(token: &str) -> String {
        if token.len() > 20 {
            format!("{}***{}", &token[..8], &token[token.len() - 8..])
        } else if token.len() > 8 {
            format!("{}***", &token[..4])
        } else {
            "***".to_string()
        }
    }

    /// Update user's last login time
    async fn update_last_login(&self, user_id: i32) -> Result<()> {
        use sea_orm::ActiveModelTrait;

        // Find the user
        let user = Users::find_by_id(user_id)
            .one(self.db.as_ref())
            .await
            .map_err(|e| crate::error!(Database, format!("Database error: {}", e)))?
            .ok_or_else(invalid_credentials_error)?;

        // Update last login time
        let mut user: users::ActiveModel = user.into();
        user.last_login = sea_orm::Set(Some(Utc::now().naive_utc()));

        user.update(self.db.as_ref())
            .await
            .map_err(|e| crate::error!(Database, format!("Database error: {}", e)))?;

        Ok(())
    }
}

fn invalid_credentials_error() -> ProxyError {
    crate::error!(Authentication, "无效的认证凭据")
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
