//! # 认证管理服务
//!
//! 封装管理端认证相关的业务逻辑，避免在 HTTP handler 中重复实现。

use crate::{
    auth::{
        jwt::TokenPair,
        permissions::UserRole,
        types::{JwtClaims, UserInfo as AuthUserInfo},
    },
    error::{Context, ProxyError, Result},
    ldebug, lerror, linfo,
    logging::{LogComponent, LogStage},
    lwarn,
    management::server::ManagementState,
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
    state: &'a ManagementState,
}

impl<'a> AuthManagementService<'a> {
    #[must_use]
    pub const fn new(state: &'a ManagementState) -> Self {
        Self { state }
    }

    /// 用户登录，返回 token 对和用户信息。
    pub async fn login(&self, username: &str, password: &str) -> Result<LoginOutput> {
        if username.trim().is_empty() || password.trim().is_empty() {
            return Err(business_error("用户名和密码不能为空"));
        }

        let token_pair = self
            .state
            .auth_service()
            .login(username, password)
            .await
            .inspect_err(|err| {
                err.log();
                lwarn!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "login_fail",
                    &format!("Login failed for user {username}: {err}")
                );
            })
            .context("管理端用户登录失败")?;

        let claims = self
            .state
            .auth_service()
            .jwt_manager
            .validate_token(&token_pair.access_token)
            .inspect_err(|err| {
                err.log();
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "token_validation_fail",
                    &format!("Generated access token failed validation for user {username}: {err}")
                );
            })
            .context("管理端登录后校验 access token 失败")?;

        let user_id = claims
            .user_id()
            .inspect_err(|err| {
                err.log();
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "parse_user_id_fail",
                    &format!("Failed to parse user id from access token: {err}")
                );
            })
            .context("解析 access token 中的用户ID失败")?;

        let auth_user = self
            .state
            .auth_service()
            .get_user_info(user_id)
            .await
            .inspect_err(|err| {
                err.log();
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "load_user_info_fail",
                    &format!("Failed to load user info for {user_id}: {err}")
                );
            })
            .context("加载登录用户信息失败")?
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
        match self.state.auth_service().jwt_manager.validate_token(token) {
            Ok(claims) => Some(claims),
            Err(err) => {
                err.log();
                ldebug!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "token_validation_fail",
                    &format!("Token validation failed: {err}")
                );
                None
            }
        }
    }

    fn extract_user_id(claims: &JwtClaims) -> Option<i32> {
        match claims.user_id() {
            Ok(id) => Some(id),
            Err(err) => {
                err.log();
                lwarn!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "parse_user_id_fail",
                    &format!("Failed to parse user id from token: {err}")
                );
                None
            }
        }
    }

    async fn load_user_info(&self, user_id: i32) -> Result<Option<AuthUserInfo>> {
        match self.state.auth_service().get_user_info(user_id).await {
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
                err.log();
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
            .auth_service()
            .jwt_manager
            .validate_token(refresh_token)
            .inspect_err(|err| {
                err.log();
                lwarn!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "refresh_token_invalid",
                    &format!("Invalid refresh token: {err}")
                );
            })
            .context("刷新 token 无效")?;

        let user_id = refresh_claims
            .user_id()
            .inspect_err(|err| {
                err.log();
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "refresh_token_user_id_parse_fail",
                    &format!("Failed to parse user id from refresh token: {err}")
                );
            })
            .context("解析刷新 token 中的用户ID失败")?;

        let auth_user = self
            .state
            .auth_service()
            .get_user_info(user_id)
            .await
            .inspect_err(|err| {
                err.log();
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "refresh_token_user_info_fail",
                    &format!("Failed to load user info for {user_id}: {err}")
                );
            })
            .context("刷新 token 加载用户信息失败")?
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
            .auth_service()
            .jwt_manager
            .generate_access_token(
                user_id,
                auth_user.username.clone(),
                auth_user.is_admin,
                role,
            )
            .inspect_err(|err| {
                err.log();
                lerror!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "refresh_token_generation_fail",
                    &format!("Failed to generate new access token: {err}")
                );
            })
            .context("刷新 token 生成新的 access token 失败")?;

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
            expires_in: self.state.auth_service().jwt_manager.access_expires_in(),
        })
    }

    /// 解码 token 用于登出场景（无状态处理）。
    pub fn decode_token_for_logout(&self, token: &str) -> Result<Option<JwtClaims>> {
        let claims = self
            .state
            .auth_service()
            .jwt_manager
            .extract_claims_unsafe(token);

        if claims.is_none() {
            ldebug!(
                "system",
                LogStage::Authentication,
                LogComponent::Auth,
                "token_validation_fail_logout",
                "Token validation failed during logout"
            );
        }

        Ok(claims)
    }
}

const fn invalid_validation_result() -> ValidateTokenOutput {
    ValidateTokenOutput {
        valid: false,
        user: None,
    }
}

fn business_error(message: impl Into<String>) -> ProxyError {
    crate::error::auth::AuthError::Message(message.into()).into()
}
