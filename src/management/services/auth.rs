//! # 认证管理服务
//!
//! 封装管理端认证相关的业务逻辑，避免在 HTTP handler 中重复实现。

use crate::{
    auth::{
        jwt::TokenPair,
        permissions::UserRole,
        types::{JwtClaims, UserInfo as AuthUserInfo},
    },
    error::{ProxyError, Result},
    ldebug, lerror, linfo,
    logging::{LogComponent, LogStage},
    lwarn,
    management::server::AppState,
};

/// 登录响应
#[derive(Debug, Clone)]
pub struct LoginOutput {
    pub token_pair: TokenPair,
    pub user: AuthUserInfo,
}

/// 验证 token 响应
#[derive(Debug, Clone)]
pub struct ValidateTokenOutput {
    pub valid: bool,
    pub user: Option<AuthUserInfo>,
}

/// 刷新 token 响应
#[derive(Debug, Clone)]
pub struct RefreshTokenOutput {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

pub struct AuthManagementService<'a> {
    state: &'a AppState,
}

impl<'a> AuthManagementService<'a> {
    #[must_use]
    pub const fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    /// 用户登录，返回 token 对和用户信息。
    pub async fn login(&self, username: &str, password: &str) -> Result<LoginOutput> {
        if username.trim().is_empty() || password.trim().is_empty() {
            return Err(business_error("用户名和密码不能为空"));
        }

        let token_pair = self
            .state
            .auth_service
            .login(username, password)
            .await
            .map_err(|err| {
                lwarn!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "login_fail",
                    &format!("Login failed for user {username}: {err}")
                );
                err
            })?;

        let claims = self
            .state
            .auth_service
            .jwt_manager
            .validate_token(&token_pair.access_token)
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "token_validation_fail",
                    &format!("Generated access token failed validation for user {username}: {err}")
                );
                err
            })?;

        let user_id = claims.user_id().map_err(|err| {
            lerror!(
                "system",
                LogStage::Authentication,
                LogComponent::Auth,
                "parse_user_id_fail",
                &format!("Failed to parse user id from access token: {err}")
            );
            err
        })?;

        let auth_user = self
            .state
            .auth_service
            .get_user_info(user_id)
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "load_user_info_fail",
                    &format!("Failed to load user info for {user_id}: {err}")
                );
                err
            })?
            .ok_or_else(|| {
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "user_not_found_after_login",
                    &format!("User {user_id} not found after successful login")
                );
                business_error("用户名或密码错误")
            })?;

        linfo!(
            "system",
            LogStage::Authentication,
            LogComponent::Auth,
            "login_success",
            &format!("User {username} logged in successfully")
        );

        Ok(LoginOutput {
            token_pair,
            user: auth_user,
        })
    }

    /// 验证 access token 并返回用户信息（若存在）。
    pub async fn validate_token(&self, token: &str) -> Result<ValidateTokenOutput> {
        let Some(claims) = self.decode_token(token) else {
            return Ok(invalid_validation_result());
        };

        let Some(user_id) = Self::extract_user_id(&claims) else {
            return Ok(invalid_validation_result());
        };

        let output =
            self.load_user_info(user_id)
                .await?
                .map_or_else(invalid_validation_result, |user| ValidateTokenOutput {
                    valid: true,
                    user: Some(user),
                });
        Ok(output)
    }

    fn decode_token(&self, token: &str) -> Option<JwtClaims> {
        self.state
            .auth_service
            .jwt_manager
            .validate_token(token)
            .map_err(|err| {
                ldebug!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "token_validation_fail",
                    &format!("Token validation failed: {err}")
                );
                err
            })
            .ok()
    }

    fn extract_user_id(claims: &JwtClaims) -> Option<i32> {
        claims
            .user_id()
            .map_err(|err| {
                lwarn!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "parse_user_id_fail",
                    &format!("Failed to parse user id from token: {err}")
                );
                err
            })
            .ok()
    }

    async fn load_user_info(&self, user_id: i32) -> Result<Option<AuthUserInfo>> {
        match self.state.auth_service.get_user_info(user_id).await {
            Ok(Some(user)) => Ok(Some(user)),
            Ok(None) => {
                lwarn!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "user_not_found_in_db",
                    &format!("Token valid but user {user_id} not found in database")
                );
                Ok(None)
            }
            Err(err) => {
                lerror!(
                    "system",
                    LogStage::Db,
                    LogComponent::Auth,
                    "db_error_token_validation",
                    &format!("Database error during token validation: {err}")
                );
                Ok(None)
            }
        }
    }

    /// 刷新 access token。
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<RefreshTokenOutput> {
        let refresh_claims = self
            .state
            .auth_service
            .jwt_manager
            .validate_token(refresh_token)
            .map_err(|err| {
                lwarn!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "refresh_token_invalid",
                    &format!("Invalid refresh token: {err}")
                );
                err
            })?;

        let user_id = refresh_claims.user_id().map_err(|err| {
            lerror!(
                "system",
                LogStage::Authentication,
                LogComponent::Auth,
                "refresh_token_user_id_parse_fail",
                &format!("Failed to parse user id from refresh token: {err}")
            );
            err
        })?;

        let auth_user = self
            .state
            .auth_service
            .get_user_info(user_id)
            .await
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "refresh_token_user_info_fail",
                    &format!("Failed to load user info for {user_id}: {err}")
                );
                err
            })?
            .ok_or_else(|| {
                lwarn!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "refresh_token_user_not_found",
                    &format!("User {user_id} not found during token refresh")
                );
                business_error("用户不存在")
            })?;

        let role = if auth_user.is_admin {
            UserRole::Admin
        } else {
            UserRole::RegularUser
        };

        let new_access_token = self
            .state
            .auth_service
            .jwt_manager
            .generate_access_token(
                user_id,
                auth_user.username.clone(),
                auth_user.is_admin,
                role,
            )
            .map_err(|err| {
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "refresh_token_generation_fail",
                    &format!("Failed to generate new access token: {err}")
                );
                err
            })?;

        linfo!(
            "system",
            LogStage::Authentication,
            LogComponent::Auth,
            "token_refresh_success",
            &format!("Access token refreshed for user {}", auth_user.username)
        );

        Ok(RefreshTokenOutput {
            access_token: new_access_token,
            token_type: "Bearer".to_string(),
            expires_in: self
                .state
                .auth_service
                .jwt_manager
                .get_config()
                .jwt_expires_in,
        })
    }

    /// 解码 token 用于登出场景（无状态处理）。
    pub fn decode_token_for_logout(&self, token: &str) -> Result<Option<JwtClaims>> {
        let jwt_secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "change-me-in-production-jwt-secret-key".to_string());

        let validation = jsonwebtoken::Validation::default();
        match jsonwebtoken::decode::<JwtClaims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(jwt_secret.as_ref()),
            &validation,
        ) {
            Ok(data) => Ok(Some(data.claims)),
            Err(err) => {
                ldebug!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "token_validation_fail_logout",
                    &format!("Token validation failed during logout: {err}")
                );
                Ok(None)
            }
        }
    }
}

const fn invalid_validation_result() -> ValidateTokenOutput {
    ValidateTokenOutput {
        valid: false,
        user: None,
    }
}

fn business_error(message: impl Into<String>) -> ProxyError {
    crate::error!(Authentication, message.into())
}
