//! # 路由配置
//!
//! 定义所有API路由和路由组织

use crate::management::server::AppState;
use axum::routing::{get, post};
use axum::Router;

/// 创建所有路由
pub fn create_routes(state: AppState) -> Router {
    Router::new()
        // 健康检查路由
        .nest("/health", health_routes())
        
        // 系统信息路由
        .nest("/system", system_routes())
        
        // 负载均衡管理路由
        .nest("/loadbalancer", loadbalancer_routes())
        
        // 适配器管理路由
        .nest("/adapters", adapter_routes())
        
        // 统计查询路由
        .nest("/statistics", statistics_routes())
        
        // 用户管理路由
        .nest("/users", user_routes())
        
        // API密钥管理路由
        .nest("/api-keys", auth_routes())
        
        // Provider密钥管理路由
        .nest("/provider-keys", provider_keys_routes())
        
        .with_state(state)
}

/// 健康检查路由
fn health_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(crate::management::server::health_check))
        .route("/detailed", get(crate::management::server::detailed_health_check))
        .route("/servers", get(crate::management::handlers::health::get_health_servers))
}

/// 系统信息路由
fn system_routes() -> Router<AppState> {
    Router::new()
        .route("/info", get(crate::management::handlers::system::get_system_info))
        .route("/metrics", get(crate::management::handlers::system::get_system_metrics))
}

/// 负载均衡管理路由
fn loadbalancer_routes() -> Router<AppState> {
    use axum::routing::patch;
    Router::new()
        .route("/status", get(crate::management::handlers::loadbalancer::get_lb_status))
        .route("/servers", get(crate::management::handlers::loadbalancer::list_servers))
        .route("/servers", post(crate::management::handlers::loadbalancer::add_server))
        .route("/servers/action", post(crate::management::handlers::loadbalancer::server_action))
        .route("/strategy", patch(crate::management::handlers::loadbalancer::change_strategy))
        .route("/metrics", get(crate::management::handlers::loadbalancer::get_lb_metrics))
}

/// 适配器管理路由
fn adapter_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(crate::management::handlers::adapters::list_adapters))
        .route("/stats", get(crate::management::handlers::adapters::get_adapter_stats))
}

/// 统计查询路由
fn statistics_routes() -> Router<AppState> {
    Router::new()
        .route("/overview", get(crate::management::handlers::statistics::get_overview))
        .route("/requests", get(crate::management::handlers::statistics::get_request_stats))
}

/// 用户管理路由
fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(crate::management::handlers::users::list_users))
        .route("/", post(crate::management::handlers::users::create_user))
        .route("/{id}", get(crate::management::handlers::users::get_user))
}
/// API密钥管理路由
fn auth_routes() -> Router<AppState> {
    use axum::routing::put;
    Router::new()
        .route("/", get(crate::management::handlers::auth::list_api_keys))
        .route("/", post(crate::management::handlers::auth::create_api_key))
        .route("/{id}", get(crate::management::handlers::auth::get_api_key))
        .route("/{id}", put(crate::management::handlers::auth::update_api_key))
        .route("/{id}/revoke", post(crate::management::handlers::auth::revoke_api_key))
}

/// 号池密钥管理路由
fn provider_keys_routes() -> Router<AppState> {
    use axum::routing::{delete, put};
    use crate::management::handlers::provider_keys;
    Router::new()
        .route("/", get(provider_keys::list_provider_keys))
        .route("/", post(provider_keys::create_provider_key))
        .route("/{id}", get(provider_keys::get_provider_key))
        .route("/{id}", put(provider_keys::update_provider_key))
        .route("/{id}", delete(provider_keys::delete_provider_key))
}
