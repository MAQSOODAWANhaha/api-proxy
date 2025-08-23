/// 双端口分离架构：并发启动 Pingora 代理服务和 Axum 管理服务
use crate::{
    ProxyError,
    auth::{UnifiedAuthManager, create_unified_auth_manager, service::AuthService},
    config::{AppConfig, ConfigManager, ProviderConfigManager},
    error::Result,
    health::service::HealthCheckService,
    management::server::{ManagementConfig, ManagementServer},
    providers::DynamicAdapterManager,
    proxy::PingoraProxyServer,
    statistics::service::StatisticsService,
};
use clap::ArgMatches;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{error, info, warn};

/// 共享服务结构体
pub struct SharedServices {
    pub auth_service: Arc<AuthService>,
    pub unified_auth_manager: Arc<UnifiedAuthManager>,
    pub health_service: Arc<HealthCheckService>,
    pub adapter_manager: Arc<DynamicAdapterManager>,
    pub statistics_service: Arc<StatisticsService>,
    pub provider_config_manager: Arc<ProviderConfigManager>,
    pub provider_resolver: Arc<crate::proxy::provider_resolver::ProviderResolver>,
}

/// 双端口服务器启动函数
pub async fn run_dual_port_servers(matches: &ArgMatches) -> Result<()> {
    info!("🚀 Starting dual-port architecture servers...");

    // 初始化共享资源
    let (config, db, shared_services, trace_system) =
        initialize_shared_services(matches).await?;

    // 创建管理服务器配置 - 使用dual_port配置或默认值
    let (management_host, management_port) = if let Some(dual_port) = &config.dual_port {
        (
            dual_port.management.http.host.clone(),
            dual_port.management.http.port,
        )
    } else {
        ("127.0.0.1".to_string(), 9090)
    };

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

    info!(
        "📊 Management server will listen on {}:{}",
        management_config.bind_address, management_config.port
    );
    info!(
        "🔗 Proxy server will listen on {}:{}",
        config.server.as_ref().map_or("0.0.0.0", |s| &s.host),
        config.server.as_ref().map_or(8080, |s| s.port)
    );

    // 创建管理服务器
    let management_server = ManagementServer::new(
        management_config,
        config.clone(),
        db.clone(),
        shared_services.auth_service.clone(),
        shared_services.health_service.clone(),
        shared_services.adapter_manager.clone(),
        shared_services.statistics_service.clone(),
        shared_services.provider_resolver.clone(),
    )
    .map_err(|e| {
        ProxyError::server_init(format!("Failed to create management server: {}", e))
    })?;

    // 创建代理服务器，传递数据库连接和追踪系统
    let proxy_server =
        PingoraProxyServer::new_with_db_and_trace((*config).clone(), db.clone(), trace_system);

    info!("🎯 Starting both servers concurrently...");

    // 并发启动两个服务器
    tokio::select! {
        // 启动 Axum 管理服务器
        result = management_server.serve() => {
            error!("Management server exited unexpectedly: {:?}", result);
            Err(ProxyError::server_start("Management server failed"))
        }
        // 启动 Pingora 代理服务器
        result = tokio::task::spawn(async move {
            proxy_server.start().await
        }) => {
            match result {
                Ok(proxy_result) => {
                    if let Err(e) = proxy_result {
                        error!("Proxy server failed: {:?}", e);
                        Err(e)
                    } else {
                        error!("Proxy server exited unexpectedly");
                        Err(ProxyError::server_start("Proxy server failed"))
                    }
                }
                Err(e) => {
                    error!("Failed to spawn proxy server task: {:?}", e);
                    Err(ProxyError::server_start("Failed to spawn proxy server"))
                }
            }
        }
    }
}

/// 初始化共享服务资源
pub async fn initialize_shared_services(
    matches: &ArgMatches,
) -> Result<(
    Arc<AppConfig>,
    Arc<DatabaseConnection>,
    SharedServices,
    Arc<crate::trace::UnifiedTraceSystem>,
)> {
    // 加载配置
    info!("📋 Loading configuration...");
    let config_manager = ConfigManager::new().await?;
    let mut config = config_manager.get_config().await;

    // 应用命令行参数覆盖
    if let Some(server) = config.server.as_mut() {
        if let Some(port) = matches.get_one::<u16>("port") {
            info!("🔧 Overriding proxy port from CLI: {}", port);
            server.port = *port;
        }

        if let Some(host) = matches.get_one::<String>("host") {
            info!("🔧 Overriding proxy host from CLI: {}", host);
            server.host = host.clone();
        }

        if let Some(https_port) = matches.get_one::<u16>("https_port") {
            info!("🔧 Overriding HTTPS port from CLI: {}", https_port);
            server.https_port = *https_port;
        }

        if let Some(workers) = matches.get_one::<u16>("workers") {
            info!("🔧 Overriding worker count from CLI: {}", workers);
            server.workers = *workers as usize;
        }
    }

    if let Some(database_url) = matches.get_one::<String>("database_url") {
        info!("🔧 Overriding database URL from CLI");
        config.database.url = database_url.clone();
    }

    info!("✅ Configuration loaded successfully");

    // 初始化数据库连接
    info!("🗄️  Initializing database connection...");
    let db = match crate::database::init_database(&config.database.url).await {
        Ok(db) => {
            info!("✅ Database connection established");
            Arc::new(db)
        }
        Err(e) => {
            error!("❌ Database connection failed: {:?}", e);
            return Err(e.into());
        }
    };

    // 运行数据库迁移
    info!("🔄 Running database migrations...");
    if let Err(e) = crate::database::run_migrations(&db).await {
        error!("❌ Database migration failed: {:?}", e);
        return Err(e.into());
    }
    info!("✅ Database migrations completed");

    let config_arc = Arc::new(config);

    // 初始化所有共享服务
    info!("🛠️  Initializing shared services...");

    // 初始化认证系统组件
    let auth_config = Arc::new(crate::auth::types::AuthConfig::default());
    let jwt_manager = Arc::new(
        crate::auth::jwt::JwtManager::new(auth_config.clone())
            .map_err(|e| ProxyError::server_init(format!("JWT manager init failed: {}", e)))?,
    );
    let api_key_manager = Arc::new(crate::auth::api_key::ApiKeyManager::new(
        db.clone(),
        auth_config.clone(),
    ));
    let auth_service = Arc::new(AuthService::new(
        jwt_manager.clone(),
        api_key_manager.clone(),
        db.clone(),
        auth_config.clone(),
    ));

    // 初始化统一缓存管理器
    let unified_cache_manager = Arc::new(
        crate::cache::abstract_cache::UnifiedCacheManager::new(
            &config_arc.cache,
            &config_arc.redis.url,
        )
        .map_err(|e| ProxyError::server_init(format!("Cache manager init failed: {}", e)))?,
    );

    // 初始化服务商配置管理器
    info!("🔧 Initializing provider configuration manager...");
    let provider_config_manager = Arc::new(ProviderConfigManager::new(
        db.clone(),
        unified_cache_manager.clone(),
    ));

    let health_service = Arc::new(HealthCheckService::new(None));

    // 使用动态配置添加服务器到健康检查服务
    info!("🏥 Adding dynamic provider servers to health check service...");
    let providers = provider_config_manager
        .get_active_providers()
        .await
        .map_err(|e| {
            error!(
                "❌ Failed to load provider configurations for health check: {}",
                e
            );
            ProxyError::server_init("Failed to load provider configurations")
        })?;

    for provider in providers {
        // 使用数据库中的提供商ID而不是硬编码映射
        let provider_id = crate::proxy::types::ProviderId::from_database_id(provider.id);

        if let Err(e) = health_service
            .add_server(provider.upstream_address.clone(), provider_id, None)
            .await
        {
            warn!(
                "Failed to add {} server ({}) to health check: {}",
                provider.display_name, provider.upstream_address, e
            );
        } else {
            info!(
                "✅ Added {} server ({}) to health check",
                provider.display_name, provider.upstream_address
            );
        }
    }

    // 启动健康检查服务
    if let Err(e) = health_service.start().await {
        warn!("Failed to start health check service: {}", e);
    }

    let adapter_manager = Arc::new(DynamicAdapterManager::new(
        db.clone(),
        provider_config_manager.clone(),
    ));

    // 创建提供商解析服务
    info!("🔍 Initializing provider resolver...");
    let provider_resolver = Arc::new(crate::proxy::provider_resolver::ProviderResolver::new(
        db.clone(),
    ));


    // 创建统一认证管理器
    let unified_auth_manager = create_unified_auth_manager(
        jwt_manager,
        api_key_manager,
        db.clone(),
        auth_config,
        Some(unified_cache_manager.clone()),
    )
    .await
    .map_err(|e| {
        crate::error::ProxyError::server_init(format!("Unified auth manager init failed: {}", e))
    })?;

    let statistics_service = Arc::new(StatisticsService::new(
        config_arc.clone(),
        unified_cache_manager.clone(),
    ));

    // 初始化统一追踪系统 - 这是关键的缺失组件!
    info!("🔍 Initializing unified trace system...");
    let tracer_config = crate::trace::immediate::ImmediateTracerConfig::default();
    let trace_system = Arc::new(crate::trace::UnifiedTraceSystem::new_immediate(
        db.clone(),
        tracer_config,
    ));
    info!("✅ Unified trace system initialized successfully");

    info!("✅ All shared services initialized successfully");

    let shared_services = SharedServices {
        auth_service,
        unified_auth_manager,
        health_service,
        adapter_manager,
        statistics_service,
        provider_config_manager,
        provider_resolver,
    };

    Ok((config_arc, db, shared_services, trace_system))
}
