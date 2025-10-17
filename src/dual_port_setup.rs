use crate::{
    app::context::AppContext,
    auth::{rate_limit_dist::DistributedRateLimiter, service::AuthService},
    config::ConfigManager,
    error::{Context, Result},
    management::server::{ManagementConfig, ManagementServer},
    proxy::PingoraProxyServer,
};
/// 双端口分离架构：并发启动 Pingora 代理服务和 Axum 管理服务
use crate::{
    lerror, linfo,
    logging::{LogComponent, LogStage},
    lwarn,
};
use std::sync::Arc;

/// 双端口服务器启动函数
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
pub async fn run_dual_port_servers() -> Result<()> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "start_servers",
        "🚀 Starting dual-port architecture servers..."
    );

    // 初始化共享资源
    let context = initialize_shared_services().await?;
    let config = context.config.clone();
    let db = context.database.clone();

    // 创建管理服务器配置 - 使用dual_port配置或默认值
    let (management_host, management_port) = config.dual_port.as_ref().map_or_else(
        || ("127.0.0.1".to_string(), 9090),
        |dual_port| {
            (
                dual_port.management.http.host.clone(),
                dual_port.management.http.port,
            )
        },
    );

    let management_config = ManagementConfig {
        bind_address: management_host.clone(),
        port: management_port,
        enable_cors: true,
        cors_origins: vec!["*".to_string()],
        allowed_ips: vec!["0.0.0.0/0".to_string()], // 默认允许所有IP
        denied_ips: vec![],
        api_prefix: "/api".to_string(),
        max_request_size: 16 * 1024 * 1024, // 16MB
        request_timeout: 30,
    };

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "management_listen_info",
        &format!(
            "📊 Management server will listen on {}:{}",
            management_config.bind_address, management_config.port
        )
    );
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "proxy_listen_info",
        &format!(
            "🔗 Proxy server will listen on {}:{}",
            config
                .dual_port
                .as_ref()
                .map_or("0.0.0.0", |d| &d.proxy.http.host),
            config
                .dual_port
                .as_ref()
                .map_or(8080, |d| d.proxy.http.port)
        )
    );

    // 创建管理服务器
    let management_server = ManagementServer::new(management_config, context.clone())
        .context("Failed to create management server")?;

    // 创建代理服务器，传递数据库连接和追踪系统
    let proxy_server = PingoraProxyServer::new(
        config.clone(),
        Some(db.clone()),
        Some(context.cache.clone()),
        context.trace_system.clone(),
    );

    // 启动OAuth token后台刷新任务
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "start_oauth_refresh_task",
        "🔄 Starting OAuth token refresh background task..."
    );
    if let Some(task) = context.oauth_token_refresh_task.as_ref() {
        if let Err(e) = task.start().await {
            lerror!(
                "system",
                LogStage::Startup,
                LogComponent::ServerSetup,
                "start_oauth_refresh_task_failed",
                &format!("Failed to start OAuth token refresh task: {e:?}")
            );
            return Err(crate::error!(
                Internal,
                "OAuth token refresh task startup failed",
                e
            ));
        }
        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "oauth_refresh_task_started",
            "✅ OAuth token refresh background task started successfully"
        );
    } else {
        lwarn!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "oauth_refresh_task_missing",
            "OAuth token refresh task not configured; skipping background startup"
        );
    }

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "start_concurrent_servers",
        "🎯 Starting both servers concurrently..."
    );

    let mut manage = Box::pin(management_server.serve());
    let mut proxy = tokio::spawn(async move { proxy_server.start().await });

    tokio::select! {
        result = &mut manage => {
            lerror!(
                "system",
                LogStage::Shutdown,
                LogComponent::ServerSetup,
                "management_server_exit",
                &format!("Management server exited unexpectedly: {result:?}")
            );
            Err(crate::error!(Internal, "Management server failed"))
        }
        result = &mut proxy => {
            match result {
                Ok(proxy_result) => {
                    if let Err(e) = proxy_result {
                        lerror!(
                            "system",
                            LogStage::Shutdown,
                            LogComponent::ServerSetup,
                            "proxy_server_fail",
                            &format!("Proxy server failed: {e:?}")
                        );
                        Err(e)
                    } else {
                        lerror!(
                            "system",
                            LogStage::Shutdown,
                            LogComponent::ServerSetup,
                            "proxy_server_exit",
                            "Proxy server exited unexpectedly"
                        );
                        Err(crate::error!(Internal, "Proxy server failed"))
                    }
                }
                Err(e) => {
                    lerror!(
                        "system",
                        LogStage::Shutdown,
                        LogComponent::ServerSetup,
                        "proxy_server_spawn_fail",
                        &format!("Failed to spawn proxy server task: {e:?}")
                    );
                    Err::<(), _>(e).context("Failed to spawn proxy server task")
                }
            }
        }
        _ = tokio::signal::ctrl_c() => {
            linfo!(
                "system",
                LogStage::Shutdown,
                LogComponent::ServerSetup,
                "shutdown_signal",
                "Received termination signal, shutting down..."
            );
            proxy.abort();
            if let Some(task) = context.oauth_token_refresh_task.as_ref()
                && let Err(e) = task.stop().await
            {
                lwarn!(
                    "system",
                    LogStage::Shutdown,
                    LogComponent::ServerSetup,
                    "oauth_task_stop_failed",
                    &format!("Failed to stop OAuth refresh task: {e:?}")
                );
            }
            Ok(())
        }
    }
}

/// 初始化共享服务资源
#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]
pub async fn initialize_shared_services() -> Result<Arc<AppContext>> {
    // 加载配置
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "load_config",
        "📋 Loading configuration..."
    );
    let config_manager = Arc::new(ConfigManager::new().await?);
    let config = config_manager.get_config().await;

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "load_config_ok",
        "✅ Configuration loaded successfully"
    );

    // 初始化数据库连接
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_db",
        "🗄️  Initializing database connection..."
    );
    let db = match crate::database::init_database(&config.database.url).await {
        Ok(db) => {
            linfo!(
                "system",
                LogStage::Startup,
                LogComponent::ServerSetup,
                "init_db_ok",
                "✅ Database connection established"
            );
            Arc::new(db)
        }
        Err(e) => {
            lerror!(
                "system",
                LogStage::Startup,
                LogComponent::ServerSetup,
                "init_db_fail",
                &format!("❌ Database connection failed: {e:?}")
            );
            return Err(e.into());
        }
    };

    // 运行数据库迁移
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "run_migrations",
        "🔄 Running database migrations..."
    );
    if let Err(e) = crate::database::run_migrations(&db).await {
        lerror!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "run_migrations_fail",
            &format!("❌ Database migration failed: {e:?}")
        );
        return Err(e.into());
    }
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "run_migrations_ok",
        "✅ Database migrations completed"
    );

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "ensure_data",
        "🔍 Ensuring default model pricing data..."
    );
    if let Err(e) = crate::database::ensure_model_pricing_data(&db).await {
        lerror!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "ensure_data_fail",
            &format!("❌ Failed to ensure model pricing data: {e:?}")
        );
        return Err(e);
    }
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "ensure_data_ok",
        "✅ Model pricing data is up to date"
    );

    let config_arc = Arc::new(config);

    // 初始化所有共享服务
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_services",
        "🛠️  Initializing shared services..."
    );

    // 初始化认证系统组件
    let auth_config = Arc::new(crate::auth::types::AuthConfig::default());
    let jwt_manager = Arc::new(
        crate::auth::jwt::JwtManager::new(auth_config.clone())
            .context("JWT manager init failed")?,
    );

    // 初始化统一缓存管理器
    let cache_manager = Arc::new(
        crate::cache::abstract_cache::CacheManager::new(&config_arc.cache)
            .context("Cache manager init failed")?,
    );

    let rate_limiter = Arc::new(DistributedRateLimiter::new(
        cache_manager.clone(),
        db.clone(),
    ));

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "warmup_rate_limit_cache",
        "🔁 Warming up daily usage cache..."
    );
    if let Err(e) = rate_limiter.warmup_daily_usage_cache().await {
        lwarn!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "warmup_rate_limit_cache_failed",
            &format!("Failed to warm up daily usage cache: {e}")
        );
    } else {
        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "warmup_rate_limit_cache_ok",
            "✅ Daily usage cache warmup completed"
        );
    }

    let api_key_manager = Arc::new(crate::auth::api_key::ApiKeyManager::new(
        db.clone(),
        auth_config.clone(),
        cache_manager.clone(),
        Arc::new(config_arc.cache.clone()),
    ));
    // 注意：认证服务在后续会统一创建一次

    // Note: 旧的服务器健康检查已移除，现在使用API密钥健康检查系统
    // 参见: src/key_pool/api_key_health.rs

    // 创建认证服务
    let auth_service = Arc::new(AuthService::new(
        jwt_manager,
        api_key_manager,
        db.clone(),
        auth_config.clone(),
    ));

    // 创建统一认证管理器

    // 初始化统一追踪系统 - 这是关键的缺失组件!
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_trace_system",
        "🔍 Initializing unified trace system..."
    );
    let tracer_config = crate::trace::immediate::ImmediateTracerConfig::default();
    let trace_system = Arc::new(crate::trace::TraceSystem::new_immediate(
        db.clone(),
        tracer_config,
    ));
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_trace_system_ok",
        "✅ Unified trace system initialized successfully"
    );

    // 初始化API密钥健康检查器
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_health_checker",
        "🏥 Initializing API key health checker..."
    );
    let api_key_health_checker = Arc::new(
        crate::key_pool::api_key_health::ApiKeyHealthChecker::new(db.clone(), None),
    );
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_health_checker_ok",
        "✅ API key health checker initialized successfully"
    );

    // 初始化OAuth客户端
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_oauth_client",
        "🔐 Initializing OAuth client..."
    );
    let oauth_client = Arc::new(crate::auth::oauth_client::OAuthClient::new(db.clone()));
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_oauth_client_ok",
        "✅ OAuth client initialized successfully"
    );

    // 初始化OAuth token刷新服务
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_oauth_refresh_service",
        "🔄 Initializing OAuth token refresh service..."
    );
    let oauth_refresh_service = Arc::new(
        crate::auth::oauth_token_refresh_service::OAuthTokenRefreshService::new(
            db.clone(),
            oauth_client.clone(),
        ),
    );
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_oauth_refresh_service_ok",
        "✅ OAuth token refresh service initialized successfully"
    );

    // 初始化智能API密钥提供者
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_smart_provider",
        "🧠 Initializing smart API key provider..."
    );
    let smart_api_key_provider = Arc::new(
        crate::auth::smart_api_key_provider::SmartApiKeyProvider::new(
            db.clone(),
            oauth_client.clone(),
            Arc::clone(&oauth_refresh_service),
        ),
    );
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_smart_provider_ok",
        "✅ Smart API key provider initialized successfully"
    );

    // 初始化OAuth token刷新任务
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_oauth_task",
        "⏰ Initializing OAuth token refresh task..."
    );
    let oauth_token_refresh_task = Arc::new(
        crate::auth::oauth_token_refresh_task::OAuthTokenRefreshTask::new(oauth_refresh_service),
    );
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_oauth_task_ok",
        "✅ OAuth token refresh task initialized successfully"
    );

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_services_ok",
        "✅ All shared services initialized successfully"
    );

    let context = Arc::new(AppContext::new(
        config_arc,
        db,
        cache_manager,
        auth_service,
        rate_limiter,
        Some(trace_system),
        Some(api_key_health_checker),
        Some(oauth_client),
        Some(smart_api_key_provider),
        Some(oauth_token_refresh_task),
    ));

    Ok(context)
}
