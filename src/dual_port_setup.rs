/// 双端口分离架构：并发启动 Pingora 代理服务和 Axum 管理服务
use crate::{
    config::{AppConfig, ConfigManager},
    error::Result,
    auth::{service::AuthService, UnifiedAuthManager, create_unified_auth_manager},
    health::service::HealthCheckService,
    providers::manager::AdapterManager,
    scheduler::manager::LoadBalancerManager,
    statistics::service::StatisticsService,
    management::server::{ManagementServer, ManagementConfig},
    proxy::PingoraProxyServer,
    trace::{UnifiedTraceSystem, unified::UnifiedTracerConfig},
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{info, error, warn};
use clap::ArgMatches;

/// 共享服务结构体
pub struct SharedServices {
    pub auth_service: Arc<AuthService>,
    pub unified_auth_manager: Arc<UnifiedAuthManager>,
    pub health_service: Arc<HealthCheckService>,
    pub adapter_manager: Arc<AdapterManager>,
    pub load_balancer_manager: Arc<LoadBalancerManager>,
    pub statistics_service: Arc<StatisticsService>,
    pub trace_system: Option<Arc<UnifiedTraceSystem>>,
}

/// 双端口服务器启动函数
pub fn run_dual_port_servers(matches: &ArgMatches) -> Result<()> {
    info!("🚀 Starting dual-port architecture servers...");
    
    // 创建Tokio运行时
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| crate::error::ProxyError::server_init(format!("Failed to create Tokio runtime: {}", e)))?;

    rt.block_on(async {
        // 初始化共享资源
        let (config, db, shared_services) = initialize_shared_services(matches).await?;
        
        // 创建管理服务器配置 - 使用dual_port配置或默认值
        let (management_host, management_port) = if let Some(dual_port) = &config.dual_port {
            (dual_port.management.http.host.clone(), dual_port.management.http.port)
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

        info!("📊 Management server will listen on {}:{}", management_config.bind_address, management_config.port);
        info!("🔗 Proxy server will listen on {}:{}", config.server.as_ref().map_or("0.0.0.0", |s| &s.host), config.server.as_ref().map_or(8080, |s| s.port));

        // 创建管理服务器
        let management_server = ManagementServer::new(
            management_config,
            config.clone(),
            db.clone(),
            shared_services.auth_service.clone(),
            shared_services.health_service.clone(),
            shared_services.adapter_manager.clone(),
            shared_services.load_balancer_manager.clone(),
            shared_services.statistics_service.clone(),
        ).map_err(|e| crate::error::ProxyError::server_init(format!("Failed to create management server: {}", e)))?;

        // 创建代理服务器，传递数据库连接
        let proxy_server = PingoraProxyServer::new_with_db((*config).clone(), db.clone());

        info!("🎯 Starting both servers concurrently...");
        
        // 并发启动两个服务器
        tokio::select! {
            // 启动 Axum 管理服务器
            result = management_server.serve() => {
                error!("Management server exited unexpectedly: {:?}", result);
                Err(crate::error::ProxyError::server_start("Management server failed"))
            }
            // 启动 Pingora 代理服务器
            result = tokio::task::spawn_blocking(move || proxy_server.start_sync()) => {
                match result {
                    Ok(proxy_result) => {
                        if let Err(e) = proxy_result {
                            error!("Proxy server failed: {:?}", e);
                            Err(e)
                        } else {
                            error!("Proxy server exited unexpectedly");
                            Err(crate::error::ProxyError::server_start("Proxy server failed"))
                        }
                    }
                    Err(e) => {
                        error!("Failed to spawn proxy server task: {:?}", e);
                        Err(crate::error::ProxyError::server_start("Failed to spawn proxy server"))
                    }
                }
            }
        }
    })
}

/// 初始化共享服务资源
pub async fn initialize_shared_services(matches: &ArgMatches) -> Result<(Arc<AppConfig>, Arc<DatabaseConnection>, SharedServices)> {
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
            .map_err(|e| crate::error::ProxyError::server_init(format!("JWT manager init failed: {}", e)))?
    );
    let api_key_manager = Arc::new(
        crate::auth::api_key::ApiKeyManager::new(db.clone(), auth_config.clone())
    );
    let auth_service = Arc::new(
        AuthService::new(jwt_manager.clone(), api_key_manager.clone(), db.clone(), auth_config.clone())
    );
    
    let health_service = Arc::new(
        HealthCheckService::new(None)
    );
    
    // 添加一些模拟服务器到健康检查服务
    if let Err(e) = health_service.add_server(
        "api.openai.com:443".to_string(),
        crate::proxy::upstream::UpstreamType::OpenAI,
        None
    ).await {
        warn!("Failed to add OpenAI server to health check: {}", e);
    }
    
    if let Err(e) = health_service.add_server(
        "generativelanguage.googleapis.com:443".to_string(),
        crate::proxy::upstream::UpstreamType::GoogleGemini,
        None
    ).await {
        warn!("Failed to add Gemini server to health check: {}", e);
    }
    
    if let Err(e) = health_service.add_server(
        "api.anthropic.com:443".to_string(),
        crate::proxy::upstream::UpstreamType::Anthropic,
        None
    ).await {
        warn!("Failed to add Anthropic server to health check: {}", e);
    }
    
    // 启动健康检查服务
    if let Err(e) = health_service.start().await {
        warn!("Failed to start health check service: {}", e);
    }
    
    let adapter_manager = Arc::new(
        AdapterManager::new()
    );
    
    let load_balancer_manager = Arc::new(
        LoadBalancerManager::new(config_arc.clone())
            .map_err(|e| crate::error::ProxyError::server_init(format!("Load balancer init failed: {}", e)))?
    );
    
    // 初始化统一缓存管理器
    let unified_cache_manager = Arc::new(
        crate::cache::abstract_cache::UnifiedCacheManager::new(&config_arc.cache, &config_arc.redis.url)
            .map_err(|e| crate::error::ProxyError::server_init(format!("Cache manager init failed: {}", e)))?
    );
    
    // 创建统一认证管理器
    let unified_auth_manager = create_unified_auth_manager(
        jwt_manager,
        api_key_manager,
        db.clone(),
        auth_config,
        Some(unified_cache_manager.clone()),
    ).await.map_err(|e| crate::error::ProxyError::server_init(format!("Unified auth manager init failed: {}", e)))?;
    
    let statistics_service = Arc::new(
        StatisticsService::new(config_arc.clone(), unified_cache_manager.clone())
    );

    // 初始化追踪系统（如果启用）
    let trace_system = if config_arc.is_trace_enabled() {
        info!("🔍 Initializing unified trace system...");
        
        let trace_config = config_arc.get_trace_config().unwrap();
        let unified_tracer_config = UnifiedTracerConfig {
            enabled: trace_config.enabled,
            basic_sampling_rate: if trace_config.default_trace_level >= 0 { 1.0 } else { 0.0 },
            detailed_sampling_rate: if trace_config.default_trace_level >= 1 { trace_config.sampling_rate } else { 0.0 },
            full_sampling_rate: if trace_config.default_trace_level >= 2 { trace_config.sampling_rate } else { 0.1 * trace_config.sampling_rate },
            batch_size: trace_config.max_batch_size,
            batch_interval_secs: trace_config.flush_interval,
            buffer_size: trace_config.max_batch_size * 2,
            health_scoring_enabled: trace_config.enable_health_metrics,
        };
        
        let trace_system = Arc::new(UnifiedTraceSystem::new(db.clone(), unified_tracer_config));
        info!("✅ Unified trace system initialized");
        Some(trace_system)
    } else {
        info!("⚠️  Trace system disabled in configuration");
        None
    };

    info!("✅ All shared services initialized successfully");

    let shared_services = SharedServices {
        auth_service,
        unified_auth_manager,
        health_service,
        adapter_manager,
        load_balancer_manager,
        statistics_service,
        trace_system,
    };

    Ok((config_arc, db, shared_services))
}