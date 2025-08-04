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
        
        // 用户中心路由（前端兼容性）
        .nest("/user", user_center_routes())
        
        // 认证路由
        .nest("/auth", auth_routes())
        
        // API密钥管理路由  
        .nest("/api-keys", api_keys_routes())
        
        // Provider类型管理路由
        .nest("/provider-types", provider_type_routes())
        
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
        // TODO: 系统配置管理相关功能暂时不实现
        // .route("/config", get(crate::management::handlers::system::get_system_config))
        // .route("/config", put(crate::management::handlers::system::update_system_config))
        // .route("/config/reload", post(crate::management::handlers::system::reload_config))
        // 日志管理
        // .route("/logs", get(crate::management::handlers::system::get_system_logs))
        // .route("/logs/download", get(crate::management::handlers::system::download_system_logs))
        // 系统操作
        // .route("/restart", post(crate::management::handlers::system::restart_system))
        // .route("/cache/clear", post(crate::management::handlers::system::clear_cache))
        // .route("/gc", post(crate::management::handlers::system::garbage_collect))
        // 备份与恢复
        // .route("/backup", post(crate::management::handlers::system::create_backup))
        // .route("/backups", get(crate::management::handlers::system::list_backups))
        // .route("/backups/{id}/download", get(crate::management::handlers::system::download_backup))
        // .route("/restore/{id}", post(crate::management::handlers::system::restore_backup))
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
        // Dashboard相关接口
        .nest("/dashboard", dashboard_routes())
        // 其他核心统计接口
        .route("/logs", get(crate::management::handlers::statistics::get_request_logs))
        .route("/logs/{id}", get(crate::management::handlers::statistics::get_request_log_detail))
        .route("/realtime", get(crate::management::handlers::statistics::get_realtime_stats))
        .route("/tokens", get(crate::management::handlers::statistics::get_token_stats))
        // 新增的高级统计接口
        .route("/response-time", get(crate::management::handlers::statistics::get_response_time_analysis))
        .route("/errors", get(crate::management::handlers::statistics::get_error_statistics))
}

/// Dashboard统计路由
fn dashboard_routes() -> Router<AppState> {
    Router::new()
        .route("/cards", get(crate::management::handlers::statistics::get_dashboard_cards))
        .route("/trend", get(crate::management::handlers::statistics::get_dashboard_trend))
        .route("/provider-distribution", get(crate::management::handlers::statistics::get_provider_distribution))
}

/// 用户管理路由
fn user_routes() -> Router<AppState> {
    use axum::routing::put;
    Router::new()
        .route("/", get(crate::management::handlers::users::list_users))
        .route("/", post(crate::management::handlers::users::create_user))
        .route("/{id}", get(crate::management::handlers::users::get_user))
        .route("/profile", get(crate::management::handlers::users::get_user_profile))
        .route("/profile", put(crate::management::handlers::users::update_user_profile))
        .route("/password", post(crate::management::handlers::users::change_password))
}

/// 用户中心路由（为前端兼容性提供 /user/* 路径）
fn user_center_routes() -> Router<AppState> {
    use axum::routing::{put, post};
    Router::new()
        .route("/profile", get(crate::management::handlers::users::get_user_profile))
        .route("/profile", put(crate::management::handlers::users::update_user_profile))
        .route("/password", post(crate::management::handlers::users::change_password))
        // TODO: 高级用户功能暂时不实现
        // .route("/avatar", post(crate::management::handlers::users::upload_avatar))
        // .route("/security", get(crate::management::handlers::users::get_security_settings))
        // .route("/security", put(crate::management::handlers::users::update_security_settings))
        // .route("/2fa/enable", post(crate::management::handlers::users::enable_2fa))
        // .route("/2fa/disable", post(crate::management::handlers::users::disable_2fa))
        // .route("/login-history", get(crate::management::handlers::users::get_login_history))
}

/// 认证路由
fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(crate::management::handlers::auth::login))
        .route("/logout", post(crate::management::handlers::auth::logout))
        .route("/validate", get(crate::management::handlers::auth::validate_token))
        // TODO: 其他认证功能暂时不实现，专注于核心功能
        // .route("/register", post(crate::management::handlers::auth::register))
        // .route("/profile", get(crate::management::handlers::auth::get_profile))
        // .route("/refresh", post(crate::management::handlers::auth::refresh_token))
        // .route("/password", put(crate::management::handlers::auth::change_password))
}

/// API密钥管理路由
fn api_keys_routes() -> Router<AppState> {
    use axum::routing::put;
    Router::new()
        // 传统API密钥管理（兼容现有功能）
        .route("/", get(crate::management::handlers::auth::list_api_keys))
        .route("/", post(crate::management::handlers::auth::create_api_key))
        .route("/{id}", get(crate::management::handlers::auth::get_api_key))
        .route("/{id}", put(crate::management::handlers::auth::update_api_key))
        .route("/{id}/revoke", post(crate::management::handlers::auth::revoke_api_key))
        
        // Provider密钥管理（内部API密钥池）- 核心功能
        .nest("/provider", provider_api_keys_routes())
        
        // Service API管理（对外API服务）
        .nest("/service", service_api_keys_routes())
        
        // 服务商类型查询
        .route("/provider-types", get(crate::management::handlers::provider_keys::get_provider_types))
        
        // 调度策略查询
        .route("/scheduling-strategies", get(crate::management::handlers::service_apis::get_scheduling_strategies))
        
        // Provider Keys健康监控接口
        .route("/health", get(crate::management::handlers::provider_keys::get_provider_keys_health_status))
}

/// Provider API密钥路由（内部密钥池管理）- 核心功能
fn provider_api_keys_routes() -> Router<AppState> {
    use axum::routing::{delete, put, patch};
    Router::new()
        .route("/", get(crate::management::handlers::provider_keys::list_provider_keys))
        .route("/", post(crate::management::handlers::provider_keys::create_provider_key))
        .route("/{id}", get(crate::management::handlers::provider_keys::get_provider_key))
        .route("/{id}", put(crate::management::handlers::provider_keys::update_provider_key))
        .route("/{id}", delete(crate::management::handlers::provider_keys::delete_provider_key))
        .route("/{id}/status", patch(crate::management::handlers::provider_keys::toggle_provider_key_status))
        .route("/{id}/usage", get(crate::management::handlers::provider_keys::get_provider_key_usage))
        .route("/{id}/test", post(crate::management::handlers::provider_keys::test_provider_key))
        .route("/{id}/health-check", post(crate::management::handlers::provider_keys::trigger_provider_key_health_check))
}

/// Service API密钥路由（对外API服务管理）
fn service_api_keys_routes() -> Router<AppState> {
    use axum::routing::{delete, put};
    Router::new()
        .route("/", get(crate::management::handlers::service_apis::list_service_apis))
        .route("/", post(crate::management::handlers::service_apis::create_service_api))
        .route("/{id}", get(crate::management::handlers::service_apis::get_service_api))
        .route("/{id}", put(crate::management::handlers::service_apis::update_service_api))
        .route("/{id}", delete(crate::management::handlers::service_apis::delete_service_api))
        .route("/{id}/regenerate", post(crate::management::handlers::service_apis::regenerate_service_api_key))
        .route("/{id}/revoke", post(crate::management::handlers::service_apis::revoke_service_api))
}

/// Provider类型管理路由
fn provider_type_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(crate::management::handlers::auth::list_provider_types))
}

// 已删除重复的provider_keys_routes函数
// 功能已合并到api_keys_routes中的provider_api_keys_routes子路由
