//! # 路由配置
//!
//! 定义所有API路由和路由组织

use crate::management::server::AppState;
use axum::Router;
use axum::routing::{get, post};

/// 创建所有路由
pub fn create_routes(state: AppState) -> Router {
    Router::new()
        // 健康检查路由
        .nest("/health", health_routes())
        // 系统信息路由
        .nest("/system", system_routes())
        // 负载均衡管理路由
        .nest("/loadbalancer", loadbalancer_routes())
        // 统计查询路由
        .nest("/statistics", statistics_routes())
        .nest("/user-service", user_service_routes())
        // 用户管理路由
        .nest("/users", user_routes())
        // Provider密钥管理路由
        .nest("/provider-keys", provider_api_keys_routes())
        // Provider类型管理路由
        .nest("/provider-types", provider_type_routes())
        .with_state(state)
}

/// 健康检查路由
fn health_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(crate::management::server::health_check))
        .route(
            "/detailed",
            get(crate::management::server::detailed_health_check),
        )
        .route(
            "/servers",
            get(crate::management::handlers::health::get_health_servers),
        )
}

/// 系统信息路由
fn system_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/info",
            get(crate::management::handlers::system::get_system_info),
        )
        .route(
            "/metrics",
            get(crate::management::handlers::system::get_system_metrics),
        )
}

/// 负载均衡管理路由
fn loadbalancer_routes() -> Router<AppState> {
    use axum::routing::patch;
    Router::new()
        .route(
            "/servers",
            post(crate::management::handlers::loadbalancer::add_server),
        )
        .route(
            "/servers/action",
            post(crate::management::handlers::loadbalancer::server_action),
        )
        .route(
            "/strategy",
            patch(crate::management::handlers::loadbalancer::change_strategy),
        )
        .route(
            "/metrics",
            get(crate::management::handlers::loadbalancer::get_lb_metrics),
        )
}

/// 统计查询路由
fn statistics_routes() -> Router<AppState> {
    Router::new()
        // 新的功能分组API结构（基于docs/new.md要求）
        .nest("/today", today_stats_routes())
        .nest("/models", models_stats_routes())
        .nest("/tokens", tokens_stats_routes())
        .nest("/user-service-api-keys", user_api_keys_stats_routes())
}

/// 今日统计路由
fn today_stats_routes() -> Router<AppState> {
    Router::new().route(
        "/cards",
        get(crate::management::handlers::statistics::get_today_dashboard_cards),
    )
}

/// 模型统计路由
fn models_stats_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/rate",
            get(crate::management::handlers::statistics::get_models_usage_rate),
        )
        .route(
            "/statistics",
            get(crate::management::handlers::statistics::get_models_statistics),
        )
}

/// Token统计路由
fn tokens_stats_routes() -> Router<AppState> {
    Router::new().route(
        "/trend",
        get(crate::management::handlers::statistics::get_tokens_trend),
    )
}

/// 用户API Keys统计路由
fn user_api_keys_stats_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/request",
            get(crate::management::handlers::statistics::get_user_api_keys_request_trend),
        )
        .route(
            "/token",
            get(crate::management::handlers::statistics::get_user_api_keys_token_trend),
        )
}

/// 用户管理路由
fn user_routes() -> Router<AppState> {
    use axum::routing::put;
    Router::new()
        .route("/", get(crate::management::handlers::users::list_users))
        .route("/", post(crate::management::handlers::users::create_user))
        .route("/{id}", get(crate::management::handlers::users::get_user))
        .route(
            "/profile",
            get(crate::management::handlers::users::get_user_profile),
        )
        .route(
            "/profile",
            put(crate::management::handlers::users::update_user_profile),
        )
        .route(
            "/password",
            post(crate::management::handlers::users::change_password),
        )
        .route(
            "/auth/login",
            post(crate::management::handlers::auth::login),
        )
        .route(
            "/auth/logout",
            post(crate::management::handlers::auth::logout),
        )
        .route(
            "/auth/validate",
            get(crate::management::handlers::auth::validate_token),
        )
}

/// Provider API密钥路由（内部密钥池管理）- 核心功能
fn provider_api_keys_routes() -> Router<AppState> {
    Router::new().route(
        "/keys",
        get(crate::management::handlers::provider_keys::get_user_provider_keys),
    )
}

/// 用户服务API路由（对外API服务管理）
fn user_service_routes() -> Router<AppState> {
    use axum::routing::{delete, put};
    Router::new()
        // 用户API Keys卡片展示
        .route(
            "/cards",
            get(crate::management::handlers::service_apis::get_user_service_cards),
        )
        // 用户API Keys列表
        .route(
            "/keys",
            get(crate::management::handlers::service_apis::list_user_service_keys),
        )
        // 新增API Key
        .route(
            "/keys",
            post(crate::management::handlers::service_apis::create_user_service_key),
        )
        // 获取API Key详情
        .route(
            "/keys/{id}",
            get(crate::management::handlers::service_apis::get_user_service_key),
        )
        // 编辑API Key
        .route(
            "/keys/{id}",
            put(crate::management::handlers::service_apis::update_user_service_key),
        )
        // 删除API Key
        .route(
            "/keys/{id}",
            delete(crate::management::handlers::service_apis::delete_user_service_key),
        )
        // API Key使用统计
        .route(
            "/keys/{id}/usage",
            get(crate::management::handlers::service_apis::get_user_service_key_usage),
        )
        // 重新生成API Key
        .route(
            "/keys/{id}/regenerate",
            post(crate::management::handlers::service_apis::regenerate_user_service_key),
        )
        // 启用/禁用API Key
        .route(
            "/keys/{id}/status",
            put(crate::management::handlers::service_apis::update_user_service_key_status),
        )
}

/// Provider类型管理路由
fn provider_type_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/providers",
            get(crate::management::handlers::provider_types::list_provider_types),
        )
        .route(
            "/scheduling-strategies",
            get(crate::management::handlers::provider_types::get_scheduling_strategies),
        )
}
