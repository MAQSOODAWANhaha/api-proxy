//! # 管理端认证工具模块
//!
//! 提供管理端专用的认证工具函数，使用共享的AuthUtils基础组件

use crate::auth::AuthUtils;
use crate::management::response;
use axum::http::HeaderMap;
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};

/// JWT Claims 结构体
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// 用户ID
    pub sub: String,
    /// 用户名
    pub username: String,
    /// 是否为管理员
    pub is_admin: bool,
    /// 过期时间
    pub exp: usize,
    /// 签发时间
    pub iat: usize,
}

/// 从请求头中提取用户ID
/// 解析JWT token并返回当前认证用户的ID
pub fn extract_user_id_from_headers(headers: &HeaderMap) -> Result<i32, axum::response::Response> {
    // 使用共享的AuthUtils提取Authorization头
    let auth_header = match AuthUtils::extract_authorization_header(headers) {
        Some(header) => header,
        None => {
            tracing::warn!("Missing Authorization header");
            return Err(response::error(
                axum::http::StatusCode::UNAUTHORIZED,
                "AUTH_ERROR",
                "Authorization header required",
            ));
        }
    };

    // 使用共享的AuthUtils提取Bearer token
    let token = match AuthUtils::extract_bearer_token(&auth_header) {
        Some(token) => token,
        None => {
            tracing::warn!("Invalid Authorization header format - not a Bearer token");
            return Err(response::error(
                axum::http::StatusCode::UNAUTHORIZED,
                "AUTH_ERROR",
                "Invalid Authorization header format - Bearer token required",
            ));
        }
    };

    // 从环境变量获取JWT密钥
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "change-me-in-production-jwt-secret-key".to_string());

    // 验证并解码JWT token
    let validation = Validation::default();
    let token_data = match decode::<Claims>(
        &token,
        &DecodingKey::from_secret(jwt_secret.as_ref()),
        &validation,
    ) {
        Ok(data) => data,
        Err(err) => {
            tracing::warn!("JWT token validation failed: {}", err);
            return Err(response::error(
                axum::http::StatusCode::UNAUTHORIZED,
                "AUTH_ERROR",
                "Invalid or expired token",
            ));
        }
    };

    // 解析用户ID
    let user_id: i32 = match token_data.claims.sub.parse() {
        Ok(id) => id,
        Err(_) => {
            tracing::error!(
                "Failed to parse user ID from JWT token: {}",
                token_data.claims.sub
            );
            return Err(response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Invalid user ID in token",
            ));
        }
    };

    Ok(user_id)
}