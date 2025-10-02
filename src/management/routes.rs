//! # 路由配置
//!
//! 定义所有API路由和路由组织

use crate::management::middleware::auth::auth;
use crate::management::server::AppState;
use axum::Router;
use axum::middleware;
use axum::routing::{get, post};

/// 创建所有路由
pub fn create_routes(state: AppState) -> Router {
    let public_routes = Router::new()
        .route(
            "/ping",
            get(crate::management::handlers::system::ping_handler),
        )
        .route(
            "/users/auth/login",
            post(crate::management::handlers::auth::login),
        )
        .with_state(state.clone());

    let protected_routes = Router::new()
        // 健康检查路由（需要认证）
        .nest("/health", health_routes())
        // 系统信息路由（需要认证）
        .nest("/system", system_routes())
        // 统计查询路由（需要认证）
        .nest("/statistics", statistics_routes())
        // 用户服务API路由（需要认证）
        .nest("/user-service", user_service_routes())
        // 用户管理路由（需要认证，但不包含login）
        .nest("/users", user_routes())
        // Provider密钥管理路由（需要认证）
        .nest("/provider-keys", provider_api_keys_routes())
        // Provider类型管理路由（需要认证）
        .nest("/provider-types", provider_type_routes())
        // 日志管理路由（需要认证）
        .nest("/logs", logs_routes())
        // OAuth认证路由（需要认证）
        .nest("/oauth", oauth_v2_routes())
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state.clone(), auth));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(state)
}

/// 健康检查路由
fn health_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(crate::management::handlers::health::health_check))
        .route(
            "/detailed",
            get(crate::management::handlers::health::detailed_health_check),
        )
        .route(
            "/api-keys",
            get(crate::management::handlers::health::get_api_keys_health),
        )
        .route(
            "/stats",
            get(crate::management::handlers::health::get_health_stats),
        )
        .route(
            "/check/{key_id}",
            post(crate::management::handlers::health::trigger_key_health_check),
        )
        .route(
            "/mark-unhealthy/{key_id}",
            post(crate::management::handlers::health::mark_key_unhealthy),
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
    use axum::routing::{delete, patch, put};
    Router::new()
        // 用户CRUD基础接口
        .route("/", get(crate::management::handlers::users::list_users))
        .route("/", post(crate::management::handlers::users::create_user))
        .route(
            "/",
            delete(crate::management::handlers::users::batch_delete_users),
        )
        .route("/{id}", get(crate::management::handlers::users::get_user))
        .route(
            "/{id}",
            put(crate::management::handlers::users::update_user),
        )
        .route(
            "/{id}",
            delete(crate::management::handlers::users::delete_user),
        )
        // 用户状态管理
        .route(
            "/{id}/toggle-status",
            patch(crate::management::handlers::users::toggle_user_status),
        )
        .route(
            "/{id}/reset-password",
            patch(crate::management::handlers::users::reset_user_password),
        )
        // 用户个人资料管理
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
        // 认证相关接口（需要认证，但不包括login）
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
    use axum::routing::{delete, post, put};
    Router::new()
        // 获取提供商密钥卡片统计数据
        .route(
            "/dashboard-stats",
            get(crate::management::handlers::provider_keys::get_provider_keys_dashboard_stats),
        )
        // 获取提供商密钥列表（完整版，支持分页搜索过滤）
        .route(
            "/keys",
            get(crate::management::handlers::provider_keys::get_provider_keys_list),
        )
        // 获取简单提供商密钥列表（用于下拉选择）
        .route(
            "/simple",
            get(crate::management::handlers::provider_keys::get_simple_provider_keys_list),
        )
        // 创建提供商密钥
        .route(
            "/keys",
            post(crate::management::handlers::provider_keys::create_provider_key),
        )
        // 获取提供商密钥详情
        .route(
            "/keys/{id}",
            get(crate::management::handlers::provider_keys::get_provider_key_detail),
        )
        // 更新提供商密钥
        .route(
            "/keys/{id}",
            put(crate::management::handlers::provider_keys::update_provider_key),
        )
        // 删除提供商密钥
        .route(
            "/keys/{id}",
            delete(crate::management::handlers::provider_keys::delete_provider_key),
        )
        // 获取密钥统计信息
        .route(
            "/keys/{id}/stats",
            get(crate::management::handlers::provider_keys::get_provider_key_stats),
        )
        // 获取密钥趋势数据
        .route(
            "/keys/{id}/trends",
            get(crate::management::handlers::provider_keys::get_provider_key_trends),
        )
        // 执行健康检查
        .route(
            "/keys/{id}/health-check",
            post(crate::management::handlers::provider_keys::health_check_provider_key),
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
        // API Key趋势数据
        .route(
            "/keys/{id}/trends",
            get(crate::management::handlers::provider_keys::get_user_service_api_trends),
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

/// 日志管理路由
fn logs_routes() -> Router<AppState> {
    Router::new()
        // 获取日志仪表板统计数据
        .route(
            "/dashboard-stats",
            get(crate::management::handlers::logs::get_dashboard_stats),
        )
        // 获取日志列表
        .route(
            "/traces",
            get(crate::management::handlers::logs::get_traces_list),
        )
        // 获取日志详情
        .route(
            "/traces/{id}",
            get(crate::management::handlers::logs::get_trace_detail),
        )
        // 获取日志统计分析
        .route(
            "/analytics",
            get(crate::management::handlers::logs::get_logs_analytics),
        )
}

// OAuth认证路由已迁移到oauth_v2_routes

/// OAuth v2客户端路由
fn oauth_v2_routes() -> Router<AppState> {
    use axum::routing::delete;
    Router::new()
        // 开始OAuth授权流程
        .route(
            "/authorize",
            post(crate::management::handlers::oauth_v2::start_authorization),
        )
        // 轮询OAuth会话状态
        .route(
            "/poll",
            get(crate::management::handlers::oauth_v2::poll_session),
        )
        // 交换授权码获取令牌
        .route(
            "/exchange",
            post(crate::management::handlers::oauth_v2::exchange_token),
        )
        // 获取用户会话列表
        .route(
            "/sessions",
            get(crate::management::handlers::oauth_v2::list_sessions),
        )
        // 删除会话
        .route(
            "/sessions/{session_id}",
            delete(crate::management::handlers::oauth_v2::delete_session),
        )
        // 刷新令牌
        .route(
            "/sessions/{session_id}/refresh",
            post(crate::management::handlers::oauth_v2::refresh_token),
        )
        // 获取统计信息
        .route(
            "/statistics",
            get(crate::management::handlers::oauth_v2::get_statistics),
        )
        // 清理过期会话（管理员接口）
        .route(
            "/cleanup",
            post(crate::management::handlers::oauth_v2::cleanup_expired_sessions),
        )
        // 获取支持的提供商列表
        .route(
            "/providers",
            get(crate::management::handlers::oauth_v2::list_providers),
        )
}
