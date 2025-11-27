//! JWT token management
//!
//! Provides JWT token generation, validation and refresh functionality

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
};
use serde::{Deserialize, Serialize};

use crate::auth::permissions::UserRole;
use crate::auth::types::{AuthConfig, JwtClaims};
use crate::error::{Result, auth::AuthError};

/// JWT token manager
pub struct JwtManager {
    /// Encoding key
    encoding_key: EncodingKey,
    /// Decoding key
    decoding_key: DecodingKey,
    /// Validation configuration
    validation: Validation,
    /// Access token 过期时间（秒）
    access_expires_in: i64,
    /// Refresh token 过期时间（秒）
    refresh_expires_in: i64,
}

impl JwtManager {
    /// Create new JWT manager
    pub fn new(config: &AuthConfig) -> Result<Self> {
        let encoding_key = EncodingKey::from_secret(config.jwt_secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(config.jwt_secret.as_bytes());

        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&["ai-proxy"]);
        validation.set_audience(&["ai-proxy-users"]);
        validation.validate_exp = true;
        validation.validate_nbf = false;
        validation.leeway = 30; // 30 seconds tolerance

        Ok(Self {
            encoding_key,
            decoding_key,
            validation,
            access_expires_in: config.jwt_expires_in,
            refresh_expires_in: config.refresh_expires_in,
        })
    }

    /// Generate access token
    pub fn generate_access_token(
        &self,
        user_id: i32,
        username: String,
        is_admin: bool,
        role: UserRole,
    ) -> Result<String> {
        let permissions = vec![role.as_str().to_string()];
        let claims = JwtClaims::new(
            user_id,
            username,
            is_admin,
            permissions,
            self.access_expires_in,
        );

        let header = Header::new(Algorithm::HS256);

        match encode(&header, &claims, &self.encoding_key) {
            Ok(token) => Ok(token),
            Err(e) => {
                Err(AuthError::Message(format!("Failed to generate access token: {e}")).into())
            }
        }
    }

    /// Generate refresh token
    pub fn generate_refresh_token(&self, user_id: i32, username: String) -> Result<String> {
        let claims = JwtClaims::new(
            user_id,
            username,
            false,  // Refresh tokens don't include admin permissions
            vec![], // Refresh tokens don't include specific permissions
            self.refresh_expires_in,
        );

        let header = Header::new(Algorithm::HS256);

        match encode(&header, &claims, &self.encoding_key) {
            Ok(token) => Ok(token),
            Err(e) => {
                Err(AuthError::Message(format!("Failed to generate refresh token: {e}")).into())
            }
        }
    }

    /// Validate and parse token
    pub fn validate_token(&self, token: &str) -> Result<JwtClaims> {
        use jsonwebtoken::errors::ErrorKind;

        let token_data: TokenData<JwtClaims> =
            match decode(token, &self.decoding_key, &self.validation) {
                Ok(data) => data,
                Err(e) => {
                    let auth_err = match e.kind() {
                        ErrorKind::ExpiredSignature => {
                            AuthError::Message("认证令牌已过期".to_string())
                        }
                        _ => AuthError::Message(format!("Token validation failed: {e}")),
                    };
                    return Err(auth_err.into());
                }
            };

        let claims = token_data.claims;

        // Additional check for token expiration
        if claims.is_expired() {
            crate::bail!(crate::error::auth::AuthError::Message(
                "认证令牌已过期".to_string()
            ));
        }

        Ok(claims)
    }

    /// Refresh access token
    pub fn refresh_access_token(
        &self,
        refresh_token: &str,
        role: UserRole,
        is_admin: bool,
    ) -> Result<String> {
        // Validate refresh token
        let claims = self.validate_token(refresh_token)?;

        // Check if user ID is valid
        let user_id = claims.user_id()?;

        // Generate new access token
        self.generate_access_token(user_id, claims.username, is_admin, role)
    }

    /// Extract user info from token (unsafe - doesn't verify signature)
    #[must_use]
    pub fn extract_claims_unsafe(&self, token: &str) -> Option<JwtClaims> {
        use jsonwebtoken::dangerous::insecure_decode;

        insecure_decode::<JwtClaims>(token)
            .map(|token_data| token_data.claims)
            .ok()
    }

    /// Get remaining token TTL
    #[must_use]
    pub fn get_token_ttl(&self, token: &str) -> Option<Duration> {
        self.extract_claims_unsafe(token).and_then(|claims| {
            let exp_time = DateTime::<Utc>::from_timestamp(claims.exp, 0)?;
            let now = Utc::now();
            if exp_time > now {
                Some(exp_time - now)
            } else {
                None
            }
        })
    }

    /// Check if token is expiring soon
    #[must_use]
    pub fn is_token_expiring_soon(&self, token: &str, threshold_seconds: i64) -> bool {
        self.get_token_ttl(token)
            .is_none_or(|ttl| ttl.num_seconds() < threshold_seconds)
    }

    /// Revoke token (add to blacklist)
    pub fn revoke_token(&self, token: &str) -> Result<String> {
        if let Some(claims) = self.extract_claims_unsafe(token) {
            // Return JTI for blacklist storage
            Ok(claims.jti)
        } else {
            Err(crate::error::auth::AuthError::Message("认证令牌格式无效".to_string()).into())
        }
    }

    /// Get configuration reference
    #[must_use]
    pub const fn access_expires_in(&self) -> i64 {
        self.access_expires_in
    }

    /// 获取刷新令牌有效期
    #[must_use]
    pub const fn refresh_expires_in(&self) -> i64 {
        self.refresh_expires_in
    }

    /// Generate token pair (access + refresh tokens)
    pub fn generate_token_pair(
        &self,
        user_id: i32,
        username: String,
        is_admin: bool,
        role: UserRole,
    ) -> Result<TokenPair> {
        let access_token = self.generate_access_token(user_id, username.clone(), is_admin, role)?;

        let refresh_token = self.generate_refresh_token(user_id, username)?;

        Ok(TokenPair {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.access_expires_in,
        })
    }
}

/// Token pair structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    /// Access token
    pub access_token: String,
    /// Refresh token
    pub refresh_token: String,
    /// Token type
    pub token_type: String,
    /// Expires in seconds
    pub expires_in: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::types::AuthConfig;

    fn create_test_manager() -> JwtManager {
        let config = AuthConfig::test();
        JwtManager::new(&config).unwrap()
    }

    #[test]
    fn test_token_generation_and_validation() {
        let manager = create_test_manager();

        let token = manager
            .generate_access_token(1, "testuser".to_string(), false, UserRole::RegularUser)
            .unwrap();

        let claims = manager.validate_token(&token).unwrap();
        assert_eq!(claims.user_id().unwrap(), 1);
        assert_eq!(claims.username, "testuser");
        assert!(!claims.is_admin);
        assert_eq!(claims.permissions, vec!["regular_user"]);
    }

    #[test]
    fn test_refresh_token_flow() {
        let manager = create_test_manager();

        // Generate refresh token
        let refresh_token = manager
            .generate_refresh_token(1, "testuser".to_string())
            .unwrap();

        // Use refresh token to generate new access token
        let new_access_token = manager
            .refresh_access_token(&refresh_token, UserRole::RegularUser, false)
            .unwrap();

        let claims = manager.validate_token(&new_access_token).unwrap();
        assert_eq!(claims.user_id().unwrap(), 1);
        assert_eq!(claims.username, "testuser");
    }

    #[test]
    fn test_token_pair_generation() {
        let manager = create_test_manager();

        let token_pair = manager
            .generate_token_pair(1, "testuser".to_string(), true, UserRole::Admin)
            .unwrap();

        // Validate access token
        let access_claims = manager.validate_token(&token_pair.access_token).unwrap();
        assert_eq!(access_claims.user_id().unwrap(), 1);
        assert!(access_claims.is_admin);

        // Validate refresh token
        let refresh_claims = manager.validate_token(&token_pair.refresh_token).unwrap();
        assert_eq!(refresh_claims.user_id().unwrap(), 1);
        assert!(!refresh_claims.is_admin); // Refresh tokens don't contain admin permissions
    }

    #[test]
    fn test_token_expiration_checking() {
        let manager = create_test_manager();

        let token = manager
            .generate_access_token(1, "testuser".to_string(), false, UserRole::RegularUser)
            .unwrap();

        // Check if token is expiring soon (should not be, since just generated)
        assert!(!manager.is_token_expiring_soon(&token, 60));

        // Check with large threshold
        assert!(manager.is_token_expiring_soon(&token, 7200)); // 2 hours
    }

    #[test]
    fn test_token_ttl() {
        let manager = create_test_manager();

        let token = manager
            .generate_access_token(1, "testuser".to_string(), false, UserRole::RegularUser)
            .unwrap();

        let ttl = manager.get_token_ttl(&token);
        assert!(ttl.is_some());
        assert!(ttl.unwrap().num_seconds() > 3500);
    }

    #[test]
    fn test_invalid_token() {
        let manager = create_test_manager();

        // Test invalid token
        let result = manager.validate_token("invalid-token");
        assert!(result.is_err());

        // Test empty token
        let result = manager.validate_token("");
        assert!(result.is_err());
    }

    #[test]
    fn test_token_revocation() {
        let manager = create_test_manager();

        let token = manager
            .generate_access_token(1, "testuser".to_string(), false, UserRole::RegularUser)
            .unwrap();

        let jti = manager.revoke_token(&token).unwrap();
        assert!(!jti.is_empty());

        // Test revoking invalid token
        let result = manager.revoke_token("invalid-token");
        assert!(result.is_err());
    }

    #[test]
    fn test_unsafe_claims_extraction() {
        let manager = create_test_manager();

        let token = manager
            .generate_access_token(1, "testuser".to_string(), false, UserRole::RegularUser)
            .unwrap();

        let claims = manager.extract_claims_unsafe(&token);
        assert!(claims.is_some());

        let claims = claims.unwrap();
        assert_eq!(claims.user_id().unwrap(), 1);
        assert_eq!(claims.username, "testuser");
    }
}
