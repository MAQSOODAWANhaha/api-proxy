//! # 认证管理处理器

use crate::auth::types::UserInfo as AuthUserInfo;
use crate::error::ProxyError;
use crate::logging::{LogComponent, LogStage};
use crate::management::services::auth::{AuthManagementService, LoginOutput};
use crate::management::{response, server::ManagementState};
use crate::{linfo, lwarn};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Json;
use serde::{Deserialize, Serialize};

fn business_error(message: impl Into<String>) -> ProxyError {
    crate::error::auth::AuthError::Message(message.into()).into()
}

/// 登录请求
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// 用户名
    pub username: String,
    /// 密码  
    pub password: String,
}

/// 登录响应
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    /// JWT token
    pub token: String,
    /// 刷新token
    pub refresh_token: String,
    /// 用户信息
    pub user: UserInfo,
}

/// 用户信息
#[derive(Debug, Serialize)]
pub struct UserInfo {
    /// 用户ID
    pub id: i32,
    /// 用户名
    pub username: String,
    /// 邮箱
    pub email: String,
    /// 是否为管理员
    pub is_admin: bool,
}

impl From<AuthUserInfo> for UserInfo {
    fn from(user: AuthUserInfo) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            is_admin: user.is_admin,
        }
    }
}

/// 用户登录（完整密码验证版本）
#[allow(clippy::cognitive_complexity)]
pub async fn login(
    State(state): State<ManagementState>,
    Json(request): Json<LoginRequest>,
) -> axum::response::Response {
    let service = AuthManagementService::new(&state);
    match service
        .login(&request.username, &request.password)
        .await
        .map(|LoginOutput { token_pair, user }| LoginResponse {
            token: token_pair.access_token,
            refresh_token: token_pair.refresh_token,
            user: UserInfo::from(user),
        }) {
        Ok(response_body) => response::success_with_message(response_body, "Login successful"),
        Err(err) => {
            err.log();
            crate::management::response::app_error(err)
        }
    }
}

/// 验证token响应
#[derive(Debug, Serialize)]
pub struct ValidateTokenResponse {
    /// token是否有效
    pub valid: bool,
    /// 用户信息（如果token有效）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserInfo>,
}

/// 刷新token请求
#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    /// 刷新token
    pub refresh_token: String,
}

/// 刷新token响应
#[derive(Debug, Serialize)]
pub struct RefreshTokenResponse {
    /// 新的access token
    pub access_token: String,
    /// token类型
    pub token_type: String,
    /// 过期时间（秒）
    pub expires_in: i64,
}

/// 用户登出
#[allow(clippy::cognitive_complexity)]
pub async fn logout(
    State(state): State<ManagementState>,
    headers: HeaderMap,
) -> axum::response::Response {
    // 从Authorization头中提取token
    let auth_header = if let Some(header) = headers.get("Authorization") {
        match header.to_str() {
            Ok(header_str) => header_str,
            Err(err) => {
                lwarn!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "invalid_auth_header",
                    &format!("Invalid Authorization header format: {err}")
                );
                return crate::management::response::app_error(business_error(
                    "Invalid Authorization header format",
                ));
            }
        }
    } else {
        lwarn!(
            "system",
            LogStage::Authentication,
            LogComponent::Auth,
            "no_auth_header_logout",
            "No Authorization header found in logout request"
        );
        return crate::management::response::app_error(business_error(
            "No Authorization header found",
        ));
    };

    // 检查Bearer前缀
    if !auth_header.starts_with("Bearer ") {
        return crate::management::response::app_error(business_error(
            "Invalid Authorization header format",
        ));
    }

    let token = &auth_header[7..]; // 移除"Bearer "前缀

    let service = AuthManagementService::new(&state);
    match service.decode_token_for_logout(token) {
        Ok(Some(claims)) => {
            linfo!(
                "system",
                LogStage::Authentication,
                LogComponent::Auth,
                "logout_success",
                &format!("User {} logged out successfully", claims.username)
            );
            response::success_without_data("Logout successful")
        }
        Ok(None) => response::success_without_data("Logout successful"),
        Err(err) => {
            err.log();
            crate::management::response::app_error(err)
        }
    }
}

/// 验证JWT Token
#[allow(clippy::cognitive_complexity)]
pub async fn validate_token(
    State(state): State<ManagementState>,
    headers: HeaderMap,
) -> axum::response::Response {
    linfo!(
        "system",
        LogStage::Authentication,
        LogComponent::Auth,
        "validate_token_headers",
        "Validate token request headers:"
    );
    for (name, value) in &headers {
        if let Ok(value_str) = value.to_str() {
            linfo!(
                "system",
                LogStage::Authentication,
                LogComponent::Auth,
                "validate_token_header",
                &format!("  {name}: {value_str}")
            );
        }
    }

    let auth_header = if let Some(header) = headers.get("Authorization") {
        match header.to_str() {
            Ok(value) => value,
            Err(err) => {
                lwarn!(
                    "system",
                    LogStage::Authentication,
                    LogComponent::Auth,
                    "invalid_auth_header",
                    &format!("Invalid Authorization header format: {err}")
                );
                return response::success(ValidateTokenResponse {
                    valid: false,
                    user: None,
                });
            }
        }
    } else {
        lwarn!(
            "system",
            LogStage::Authentication,
            LogComponent::Auth,
            "no_auth_header",
            "No Authorization header found in request"
        );
        return response::success(ValidateTokenResponse {
            valid: false,
            user: None,
        });
    };

    if !auth_header.starts_with("Bearer ") {
        return response::success(ValidateTokenResponse {
            valid: false,
            user: None,
        });
    }

    let token = &auth_header[7..];
    let service = AuthManagementService::new(&state);
    match service.validate_token(token).await {
        Ok(output) => response::success(ValidateTokenResponse {
            valid: output.valid,
            user: output.user.map(UserInfo::from),
        }),
        Err(err) => {
            err.log();
            crate::management::response::app_error(err)
        }
    }
}

/// 刷新access token
#[allow(clippy::cognitive_complexity)]
pub async fn refresh_token(
    State(state): State<ManagementState>,
    Json(request): Json<RefreshTokenRequest>,
) -> axum::response::Response {
    let service = AuthManagementService::new(&state);
    match service.refresh_token(&request.refresh_token).await {
        Ok(output) => {
            let response_body = RefreshTokenResponse {
                access_token: output.access_token,
                token_type: output.token_type,
                expires_in: output.expires_in,
            };
            response::success_with_message(response_body, "Token refreshed successfully")
        }
        Err(err) => {
            err.log();
            crate::management::response::app_error(err)
        }
    }
}
