use crate::{
    app::{context::AppContext, services::SharedServices},
    auth::{rate_limit_dist::DistributedRateLimiter, service::AuthService},
    cache::CacheManager,
    collect::service::CollectService,
    config::{AppConfig, ConfigManager},
    error::{Context, Result},
    key_pool::KeyPoolService,
    linfo,
    logging::{LogComponent, LogStage},
    management::server::{ManagementConfig, ManagementServer},
    pricing::PricingCalculatorService,
    proxy::{
        PingoraProxyServer, authentication_service::AuthenticationService,
        request_transform_service::RequestTransformService,
        response_transform_service::ResponseTransformService, state::ProxyState,
        upstream_service::UpstreamService,
    },
    trace::TraceManager,
};
use crate::{lerror, lwarn};
use sea_orm::DatabaseConnection;
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

    // 初始化所有共享服务和状态
    let services = initialize_shared_services().await?;
    let app_context = services.app_context;
    let proxy_state = services.proxy_state;
    let config = app_context.config.clone();

    // 创建管理服务器配置
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
        allowed_ips: vec!["0.0.0.0/0".to_string()],
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
    let management_server = ManagementServer::new(management_config, app_context.clone())
        .context("Failed to create management server")?;

    // 创建代理服务器
    let proxy_server = PingoraProxyServer::new(proxy_state);

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
            if let Err(e) = app_context.key_pool_service.stop().await {
                lwarn!(
                    "system",
                    LogStage::Shutdown,
                    LogComponent::ServerSetup,
                    "health_check_stop_failed",
                    &format!("Failed to stop key pool service: {e:?}")
                );
            }
            if let Err(e) = app_context.oauth_token_refresh_task.stop().await {
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

/// 初始化所有共享服务和状态，并启动后台任务
pub async fn initialize_shared_services() -> Result<SharedServices> {
    // 1. 基础设置：加载配置和初始化数据库
    let (config, db) = init_config_and_db().await?;

    // 2. 创建全局上下文 AppContext
    let app_context = build_app_context(config, db).await?;

    // 3. 创建代理状态 ProxyState
    let proxy_state = build_proxy_state(app_context.clone());

    // 4. 启动后台任务
    start_background_tasks(&app_context).await?;

    // 5. 返回组装好的服务容器
    Ok(SharedServices {
        app_context,
        proxy_state,
    })
}

/// 职责1：加载配置和初始化数据库
#[allow(clippy::cognitive_complexity)]
async fn init_config_and_db() -> Result<(Arc<AppConfig>, Arc<DatabaseConnection>)> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "load_config",
        "📋 Loading configuration..."
    );
    let config_manager = Arc::new(ConfigManager::new().await?);
    let config = config_manager.get_config().await;
    let config = Arc::new(config);
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "load_config_ok",
        "✅ Configuration loaded successfully"
    );

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

    Ok((config, db))
}

/// 职责2：创建全局上下文 `AppContext`
async fn build_app_context(
    config: Arc<AppConfig>,
    db: Arc<DatabaseConnection>,
) -> Result<Arc<AppContext>> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_global_services",
        "🛠️  Initializing global shared services (AppContext)..."
    );

    let auth_config = Arc::new(crate::auth::types::AuthConfig::default());
    let jwt_manager = Arc::new(
        crate::auth::jwt::JwtManager::new(auth_config.clone())
            .context("JWT manager init failed")?,
    );
    let cache_manager =
        Arc::new(CacheManager::new(&config.cache).context("Cache manager init failed")?);
    let rate_limiter = Arc::new(DistributedRateLimiter::new(
        cache_manager.clone(),
        db.clone(),
    ));

    let api_key_manager = Arc::new(crate::auth::api_key::ApiKeyManager::new(
        db.clone(),
        auth_config.clone(),
        cache_manager.clone(),
        Arc::new(config.cache.clone()),
    ));

    let auth_service = Arc::new(AuthService::new(
        jwt_manager,
        api_key_manager,
        db.clone(),
        auth_config,
    ));

    let tracer_config = crate::trace::immediate::ImmediateTracerConfig::default();
    let trace_system = Arc::new(crate::trace::TraceSystem::new_immediate(
        db.clone(),
        tracer_config,
    ));

    let api_key_health_checker = Arc::new(
        crate::key_pool::api_key_health::ApiKeyHealthChecker::new(db.clone(), None),
    );
    let key_pool_service = Arc::new(KeyPoolService::new(db.clone(), api_key_health_checker));

    let oauth_client = Arc::new(crate::auth::oauth_client::OAuthClient::new(db.clone()));
    let oauth_refresh_service = Arc::new(
        crate::auth::oauth_token_refresh_service::OAuthTokenRefreshService::new(
            db.clone(),
            oauth_client.clone(),
        ),
    );
    let smart_api_key_provider = Arc::new(
        crate::auth::smart_api_key_provider::SmartApiKeyProvider::new(
            db.clone(),
            oauth_client.clone(),
            Arc::clone(&oauth_refresh_service),
        ),
    );
    key_pool_service
        .set_smart_provider(smart_api_key_provider.clone())
        .await;

    let oauth_token_refresh_task = Arc::new(
        crate::auth::oauth_token_refresh_task::OAuthTokenRefreshTask::new(oauth_refresh_service),
    );

    let app_context = Arc::new(
        AppContext::builder()
            .with_config(config)
            .with_database(db)
            .with_cache(cache_manager)
            .with_auth_service(auth_service)
            .with_rate_limiter(rate_limiter)
            .with_key_pool_service(key_pool_service)
            .with_oauth_token_refresh_task(oauth_token_refresh_task)
            .with_trace_system(trace_system)
            .with_oauth_client(oauth_client)
            .with_smart_api_key_provider(smart_api_key_provider)
            .build()?,
    );

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_global_services_ok",
        "✅ Global shared services (AppContext) initialized successfully"
    );

    Ok(app_context)
}

/// 职责3：创建代理状态 `ProxyState`
fn build_proxy_state(app_context: Arc<AppContext>) -> Arc<ProxyState> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_proxy_services",
        "🔧 Initializing proxy-specific services (ProxyState)..."
    );

    let db = app_context.database.clone();
    let auth_service = app_context.auth_service.clone();
    let cache_manager = app_context.cache.clone();
    let key_pool_service = app_context.key_pool_service.clone();
    let rate_limiter = app_context.rate_limiter.clone();
    let trace_system = app_context.trace_system.clone();

    let pricing_calculator = Arc::new(PricingCalculatorService::new(db.clone()));
    let collect_service = Arc::new(CollectService::new(pricing_calculator));
    let trace_manager = Arc::new(TraceManager::new(
        trace_system.immediate_tracer(),
        rate_limiter.clone(),
    ));
    let upstream_service = Arc::new(UpstreamService::new(db.clone()));
    let req_transform_service = Arc::new(RequestTransformService::new(db.clone()));
    let resp_transform_service = Arc::new(ResponseTransformService::new());

    let proxy_auth_service = Arc::new(AuthenticationService::new(
        auth_service,
        db.clone(),
        cache_manager,
        key_pool_service.clone(),
        rate_limiter.clone(),
    ));

    let proxy_state = Arc::new(ProxyState {
        context: app_context,
        db,
        auth_service: proxy_auth_service,
        collect_service,
        trace_manager,
        upstream_service,
        req_transform_service,
        resp_transform_service,
        key_pool_service,
        rate_limiter,
    });

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_proxy_services_ok",
        "✅ Proxy-specific services (ProxyState) initialized successfully"
    );

    proxy_state
}

/// 职责4：启动后台任务
#[allow(clippy::cognitive_complexity)]
async fn start_background_tasks(app_context: &Arc<AppContext>) -> Result<()> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "start_background_tasks",
        "🏃 Starting background tasks..."
    );

    if let Err(e) = app_context.rate_limiter.warmup_daily_usage_cache().await {
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

    if let Err(e) = app_context.key_pool_service.start().await {
        lerror!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "start_health_check_failed",
            &format!("Failed to start key pool service: {e:?}")
        );
        return Err(crate::error!(
            Internal,
            "Key pool service startup failed",
            e
        ));
    }
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "health_check_task_started",
        "✅ Key pool service bootstrapped successfully"
    );

    if let Err(e) = app_context.oauth_token_refresh_task.start().await {
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

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "start_background_tasks_ok",
        "✅ All background tasks started successfully"
    );

    Ok(())
}
