//! # 认证中间件
//!
//! 从请求头中提取JWT，验证并将其解析的用户信息注入到请求扩展中。

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::auth::AuthUtils;
use crate::management::server::ManagementState;

/// 包含认证用户信息的上下文
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: i32,
    pub is_admin: bool,
}

/// Axum认证中间件
pub async fn auth(
    State(state): State<ManagementState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 从请求头中提取 `Authorization`
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok());

    let Some(auth_header) = auth_header else {
        // 如果没有 Authorization 头，直接拒绝
        return Err(StatusCode::UNAUTHORIZED);
    };

    // 提取 Bearer Token
    let Some(token) = AuthUtils::extract_bearer_token(auth_header) else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    // 使用 JwtManager 验证 Token
    match state.auth_service().jwt_manager.validate_token(&token) {
        Ok(claims) => {
            // Token有效，将用户信息注入到请求扩展中
            let user_id = claims
                .user_id()
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let auth_context = Arc::new(AuthContext {
                user_id,
                is_admin: claims.is_admin,
            });
            request.extensions_mut().insert(auth_context);
            // 将请求传递给下一个中间件或处理器
            Ok(next.run(request).await)
        }
        Err(_) => {
            // Token无效或过期，返回401
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}
