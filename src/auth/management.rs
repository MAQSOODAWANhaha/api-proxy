//! # 管理端认证工具模块
//!
//! 提供管理端专用的认证工具函数，使用共享的AuthUtils基础组件

use crate::auth::AuthUtils;
use crate::auth::jwt::JwtManager;
use axum::http::HeaderMap;

/// 从请求头中检查用户是否为管理员
/// 基于已有的JWT token解析逻辑，返回管理员状态
pub fn check_is_admin_from_headers(
    headers: &HeaderMap,
    jwt_manager: &JwtManager,
) -> Result<bool, axum::response::Response> {
    // 使用共享的AuthUtils提取Authorization头
    let auth_header = match AuthUtils::extract_authorization_header(headers) {
        Some(header) => header,
        None => return Ok(false), // 无认证头，默认非管理员
    };

    // 使用共享的AuthUtils提取Bearer token
    let token = match AuthUtils::extract_bearer_token(&auth_header) {
        Some(token) => token,
        None => return Ok(false), // token格式错误，默认非管理员
    };

    // 使用JwtManager进行验证
    let claims = match jwt_manager.validate_token(&token) {
        Ok(claims) => claims,
        Err(_) => return Ok(false), // token无效，默认非管理员
    };

    Ok(claims.is_admin)
}

/// 从请求头中提取用户ID
/// 解析JWT token并返回当前认证用户的ID
pub fn extract_user_id_from_headers(
    headers: &HeaderMap,
    jwt_manager: &JwtManager,
) -> Result<i32, axum::response::Response> {
    // 使用共享的AuthUtils提取Authorization头
    let auth_header = match AuthUtils::extract_authorization_header(headers) {
        Some(header) => header,
        None => {
            tracing::warn!("Missing Authorization header");
            return Err(crate::manage_error!(crate::proxy_err!(
                auth,
                "Authorization header required"
            )));
        }
    };

    // 使用共享的AuthUtils提取Bearer token
    let token = match AuthUtils::extract_bearer_token(&auth_header) {
        Some(token) => token,
        None => {
            tracing::warn!("Invalid Authorization header format - not a Bearer token");
            return Err(crate::manage_error!(crate::proxy_err!(
                business,
                "Invalid Authorization header format - Bearer token required"
            )));
        }
    };

    // 使用JwtManager进行验证
    let claims = match jwt_manager.validate_token(&token) {
        Ok(claims) => claims,
        Err(err) => {
            tracing::warn!("JWT token validation failed: {}", err);
            return Err(crate::manage_error!(crate::proxy_err!(
                auth,
                "Invalid or expired token"
            )));
        }
    };

    // 从claims中安全地获取user_id
    claims.user_id().map_err(|err| {
        tracing::error!("Failed to parse user ID from JWT token: {}", err);
        crate::manage_error!(crate::proxy_err!(internal, "Invalid user ID in token"))
    })
}
