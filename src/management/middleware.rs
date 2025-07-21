//! # 管理API中间件
//!
//! 提供认证、授权、请求限制等中间件

use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use crate::management::server::AppState;
use crate::auth::{AuthContext};
use crate::auth::permissions::Permission;
use crate::auth::types::UserInfo;
use tracing::{warn, debug};

/// 认证中间件
pub async fn auth_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 检查Authorization头
    let auth_header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    // 直接使用已创建的认证服务
    let auth_service = &state.auth_service;
    
    let mut auth_context = AuthContext {
        auth_result: None,
        resource_path: request.uri().path().to_string(),
        method: request.method().to_string(),
        client_ip: Some(get_client_ip(&headers)),
        user_agent: headers.get("user-agent")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string()),
    };

    match auth_header {
        Some(auth) if auth.starts_with("Bearer ") => {
            let token = &auth[7..];
            
            // 验证JWT token
            match auth_service.authenticate(token, &mut auth_context).await {
                Ok(auth_result) => {
                    // 将用户信息添加到请求扩展中
                    let user_info = UserInfo {
                        id: auth_result.user_id,
                        username: auth_result.username,
                        email: "".to_string(),
                        is_admin: auth_result.is_admin,
                        is_active: true,
                        permissions: auth_result.permissions,
                        created_at: chrono::Utc::now(),
                        last_login: None,
                    };
                    request.extensions_mut().insert(user_info);
                    Ok(next.run(request).await)
                }
                Err(e) => {
                    warn!("JWT token authentication failed: {}", e);
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        }
        Some(auth) if auth.starts_with("Basic ") => {
            // 解析Basic认证
            let encoded = &auth[6..];
            
            use base64::{Engine as _, engine::general_purpose};
            match general_purpose::STANDARD.decode(encoded) {
                Ok(decoded) => {
                    let credentials = String::from_utf8_lossy(&decoded);
                    let parts: Vec<&str> = credentials.splitn(2, ':').collect();
                    
                    if parts.len() == 2 {
                        let (username, password) = (parts[0], parts[1]);
                        
                        // 调用认证服务的公共接口
                        let token_type = crate::auth::types::TokenType::Basic {
                            username: username.to_string(),
                            password: password.to_string(),
                        };
                        match auth_service.authenticate(&token_type.as_str(), &mut auth_context).await {
                            Ok(auth_result) => {
                                let user_info = UserInfo {
                                    id: auth_result.user_id,
                                    username: auth_result.username,
                                    email: "".to_string(),
                                    is_admin: auth_result.is_admin,
                                    is_active: true,
                                    permissions: auth_result.permissions,
                                    created_at: chrono::Utc::now(),
                                    last_login: None,
                                };
                                request.extensions_mut().insert(user_info);
                                Ok(next.run(request).await)
                            }
                            Err(e) => {
                                warn!("Basic authentication failed for user {}: {}", username, e);
                                Err(StatusCode::UNAUTHORIZED)
                            }
                        }
                    } else {
                        warn!("Invalid Basic authentication format");
                        Err(StatusCode::UNAUTHORIZED)
                    }
                }
                Err(_) => {
                    warn!("Invalid Basic authentication encoding");
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        }
        Some(auth) if auth.starts_with("ApiKey ") => {
            let api_key = &auth[7..];
            
            // 验证API Key
            match auth_service.authenticate(api_key, &mut auth_context).await {
                Ok(auth_result) => {
                    let user_info = UserInfo {
                        id: auth_result.user_id,
                        username: auth_result.username,
                        email: "".to_string(),
                        is_admin: auth_result.is_admin,
                        is_active: true,
                        permissions: auth_result.permissions,
                        created_at: chrono::Utc::now(),
                        last_login: None,
                    };
                    request.extensions_mut().insert(user_info);
                    Ok(next.run(request).await)
                }
                Err(e) => {
                    warn!("API Key authentication failed: {}", e);
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        }
        _ => {
            // 检查是否有X-API-Key头
            if let Some(api_key) = headers.get("x-api-key").and_then(|h| h.to_str().ok()) {
                match auth_service.authenticate(api_key, &mut auth_context).await {
                    Ok(auth_result) => {
                        let user_info = UserInfo {
                            id: auth_result.user_id,
                            username: auth_result.username,
                            email: "".to_string(),
                            is_admin: auth_result.is_admin,
                            is_active: true,
                            permissions: auth_result.permissions,
                            created_at: chrono::Utc::now(),
                            last_login: None,
                        };
                        request.extensions_mut().insert(user_info);
                        Ok(next.run(request).await)
                    }
                    Err(e) => {
                        warn!("X-API-Key authentication failed: {}", e);
                        Err(StatusCode::UNAUTHORIZED)
                    }
                }
            } else {
                warn!("Missing or invalid authorization header");
                Err(StatusCode::UNAUTHORIZED)
            }
        }
    }
}

/// 管理员权限中间件
pub async fn admin_middleware(
    State(_state): State<AppState>,
    _headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 从请求扩展中获取用户信息（假设已经通过认证中间件）
    if let Some(user) = request.extensions().get::<crate::auth::types::UserInfo>() {
        // 检查用户是否具有管理员权限
        if user.permissions.contains(&Permission::ManageServer) {
            debug!("Admin access granted for user: {}", user.username);
            Ok(next.run(request).await)
        } else {
            warn!("Insufficient permissions for admin operation: user={}", user.username);
            Err(StatusCode::FORBIDDEN)
        }
    } else {
        warn!("No user information found in request - authentication required");
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// 速率限制中间件
pub async fn rate_limit_middleware(
    State(_state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 获取客户端标识
    let _client_id = get_client_identifier(&headers);
    
    // 暂时跳过速率限制，因为AppState没有cache字段
    // TODO: 在AppState中添加cache字段后启用速率限制
    warn!("Rate limiting disabled - cache not available in AppState");
    Ok(next.run(request).await)
}

/// 请求日志中间件
pub async fn request_logging_middleware(
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start_time = std::time::Instant::now();
    
    let response = next.run(request).await;
    
    let duration = start_time.elapsed();
    let status = response.status();
    
    tracing::info!(
        method = %method,
        uri = %uri,
        status = %status,
        duration_ms = duration.as_millis(),
        "Management API request"
    );
    
    response
}

/// 获取客户端IP地址
fn get_client_ip(headers: &HeaderMap) -> String {
    // 按优先级尝试获取真实IP
    if let Some(ip) = headers.get("x-forwarded-for").and_then(|h| h.to_str().ok()) {
        // X-Forwarded-For可能包含多个IP，取第一个
        if let Some(first_ip) = ip.split(',').next() {
            return first_ip.trim().to_string();
        }
    }
    
    if let Some(ip) = headers.get("x-real-ip").and_then(|h| h.to_str().ok()) {
        return ip.to_string();
    }
    
    if let Some(ip) = headers.get("cf-connecting-ip").and_then(|h| h.to_str().ok()) {
        return ip.to_string();
    }
    
    "unknown".to_string()
}

/// 获取客户端标识符
fn get_client_identifier(headers: &HeaderMap) -> String {
    // 尝试从不同的头部获取客户端标识
    if let Some(api_key) = headers.get("x-api-key").and_then(|h| h.to_str().ok()) {
        return format!("api-key:{}", &api_key[..std::cmp::min(api_key.len(), 10)]);
    }
    
    if let Some(auth) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
        if auth.starts_with("Bearer ") {
            let token = &auth[7..];
            return format!("token:{}", &token[..std::cmp::min(token.len(), 10)]);
        }
    }
    
    if let Some(ip) = headers.get("x-forwarded-for").and_then(|h| h.to_str().ok()) {
        return format!("ip:{}", ip);
    }
    
    if let Some(ip) = headers.get("x-real-ip").and_then(|h| h.to_str().ok()) {
        return format!("ip:{}", ip);
    }
    
    "unknown".to_string()
}

// 暂时删除速率限制检查函数，等AppState添加cache字段后再实现