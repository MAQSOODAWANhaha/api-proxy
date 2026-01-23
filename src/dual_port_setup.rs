use crate::{
    app::{context::AppContext, shared_services::SharedServices, task_scheduler::TaskScheduler},
    collect::service::CollectService,
    config::{AppConfig, ConfigManager},
    error::{Context, Result},
    linfo,
    logging::{LogComponent, LogStage, log_proxy_error},
    management::server::{ManagementConfig, ManagementServer, ManagementState},
    pricing::PricingCalculatorService,
    proxy::{
        PingoraProxyServer,
        authentication_service::AuthenticationService,
        request_transform_service::RequestTransformService,
        response_transform_service::ResponseTransformService,
        state::{ProxyServices, ProxyState},
        upstream_service::UpstreamService,
    },
    trace::TraceManager,
};
use crate::{lerror, lwarn};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// 创建服务器实例
///
/// 从配置构建管理服务器和代理服务器实例
fn create_servers(
    config: &AppConfig,
    management_state: Arc<ManagementState>,
    proxy_state: Arc<ProxyState>,
) -> Result<(ManagementServer, PingoraProxyServer)> {
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
        ..Default::default()
    };

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "management_listen_info",
        &format!("[INFO] Management server will listen on {management_host}:{management_port}")
    );

    let (proxy_host, proxy_port) = config.dual_port.as_ref().map_or_else(
        || ("0.0.0.0".to_string(), 8080),
        |d| (d.proxy.http.host.clone(), d.proxy.http.port),
    );

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "proxy_listen_info",
        &format!("[INFO] Proxy server will listen on {proxy_host}:{proxy_port}")
    );

    let management_server = ManagementServer::new(management_config, management_state)
        .context("Failed to create management server")?;
    let proxy_server = PingoraProxyServer::new(proxy_state);

    Ok((management_server, proxy_server))
}

/// 等待关闭原因（Ctrl+C 或任一服务器退出），返回描述性字符串
/// 处理 Ctrl+C 信号
async fn handle_ctrl_c_signal() -> String {
    match tokio::signal::ctrl_c().await {
        Ok(()) => "Ctrl+C signal".to_string(),
        Err(e) => {
            lerror!(
                "system",
                LogStage::Shutdown,
                LogComponent::ServerSetup,
                "ctrl_c_error",
                &format!("Failed to listen for Ctrl+C: {e:?}")
            );
            "Ctrl+C handler error".to_string()
        }
    }
}

/// 处理服务器任务退出结果
fn handle_task_result(
    server_name: &str,
    result: std::result::Result<Result<()>, tokio::task::JoinError>,
) -> String {
    match result {
        Ok(Err(e)) => {
            lerror!(
                "system",
                LogStage::Shutdown,
                LogComponent::ServerSetup,
                &format!("{}_error", server_name.to_lowercase().replace(' ', "_")),
                &format!("{server_name} error: {e:?}")
            );
            format!("{server_name} error")
        }
        Err(e) => {
            lerror!(
                "system",
                LogStage::Shutdown,
                LogComponent::ServerSetup,
                &format!("{}_panic", server_name.to_lowercase().replace(' ', "_")),
                &format!("{server_name} panicked: {e:?}")
            );
            format!("{server_name} panic")
        }
        _ => format!("{server_name} exit"),
    }
}

/// 等待关闭信号（Ctrl+C 或服务器退出）
async fn await_shutdown_reason(
    management_task: &mut tokio::task::JoinHandle<Result<()>>,
    proxy_task: &mut tokio::task::JoinHandle<Result<()>>,
) -> String {
    tokio::select! {
        _ = handle_ctrl_c_signal() => {
            "Ctrl+C signal".to_string()
        },
        result = management_task => {
            handle_task_result("Management server", result)
        },
        result = proxy_task => {
            handle_task_result("Proxy server", result)
        },
    }
}

/// 双端口服务器生命周期管理器
///
/// 协调管理后台任务、管理服务器和代理服务器的启动与关闭
pub struct DualPortServerManager {
    app_context: Arc<AppContext>,
    scheduler: Arc<TaskScheduler>,
}

impl DualPortServerManager {
    #[must_use]
    pub const fn new(app_context: Arc<AppContext>, scheduler: Arc<TaskScheduler>) -> Self {
        Self {
            app_context,
            scheduler,
        }
    }

    /// 启动所有组件：后台任务 → 管理服务器 → 代理服务器
    pub async fn start_all(
        &self,
        management_server: ManagementServer,
        proxy_server: PingoraProxyServer,
    ) -> Result<(
        tokio::task::JoinHandle<Result<()>>,
        tokio::task::JoinHandle<Result<()>>,
    )> {
        // 1. 启动后台任务
        self.scheduler.start_all().await?;

        // 2. 启动管理服务器
        let management_task = tokio::spawn(async move { management_server.serve().await });

        // 3. 启动代理服务器
        let proxy_task = tokio::spawn(async move { proxy_server.start().await });

        linfo!(
            "system",
            LogStage::Startup,
            LogComponent::ServerSetup,
            "all_components_started",
            "✅ All components started"
        );

        Ok((management_task, proxy_task))
    }

    /// 优雅关闭所有组件：停止服务器 → 关闭后台任务
    #[allow(clippy::cognitive_complexity)] // 关闭流程需要处理多个组件，复杂度合理
    pub async fn shutdown_all(
        &self,
        management_task: &tokio::task::JoinHandle<Result<()>>,
        proxy_task: &tokio::task::JoinHandle<Result<()>>,
        shutdown_reason: &str,
    ) -> Result<()> {
        linfo!(
            "system",
            LogStage::Shutdown,
            LogComponent::ServerSetup,
            "shutdown_initiated",
            &format!("🛑 Graceful shutdown: {shutdown_reason}")
        );

        // 停止服务器
        management_task.abort();
        proxy_task.abort();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // 关闭所有后台任务（包括周期任务和常驻服务）
        if let Err(e) = self.scheduler.shutdown().await {
            lwarn!(
                "system",
                LogStage::Shutdown,
                LogComponent::ServerSetup,
                "scheduler_shutdown_warning",
                &format!("⚠️  Scheduler shutdown warning: {e:?}")
            );
        }

        Ok(())
    }
}

/// 运行双端口服务器
///
/// 这是应用的主入口函数，负责：
/// 1. 初始化所有服务和状态
/// 2. 启动管理服务器和代理服务器
/// 3. 等待关闭信号（Ctrl+C 或服务器异常）
/// 4. 执行优雅关闭
pub async fn run_dual_port_servers() -> Result<()> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "start_servers",
        "🚀 Starting dual-port architecture servers..."
    );

    // Phase 1: 初始化服务
    let SharedServices {
        app_context,
        proxy_state,
        management_state,
    } = initialize_services().await?;
    let scheduler = app_context.tasks().scheduler();

    // Phase 2: 创建服务器实例
    let (management_server, proxy_server) =
        create_servers(&app_context.config(), management_state, proxy_state)?;

    let server_manager = DualPortServerManager::new(app_context.clone(), scheduler.clone());

    // Phase 3: 启动所有组件
    let (mut management_task, mut proxy_task) = server_manager
        .start_all(management_server, proxy_server)
        .await?;

    // Phase 4: 等待关闭信号
    let shutdown_reason = await_shutdown_reason(&mut management_task, &mut proxy_task).await;

    // Phase 5: 优雅关闭
    server_manager
        .shutdown_all(&management_task, &proxy_task, &shutdown_reason)
        .await?;

    linfo!(
        "system",
        LogStage::Shutdown,
        LogComponent::ServerSetup,
        "servers_stopped",
        "👋 All servers stopped. Goodbye!"
    );

    Ok(())
}

/// 初始化所有共享服务和状态
pub async fn initialize_services() -> Result<SharedServices> {
    let (config, db) = setup_database().await?;
    let app_context = AppContext::bootstrap(config, db).await?;
    let management_state = build_management_state(app_context.clone())?;
    let proxy_state = build_proxy_state(&app_context);

    Ok(SharedServices {
        app_context,
        proxy_state,
        management_state,
    })
}

/// 加载应用配置
fn load_config() -> Result<Arc<AppConfig>> {
    let manager = ConfigManager::new()?;
    Ok(manager.config())
}

/// 初始化数据库连接
async fn init_database(config: &AppConfig) -> Result<Arc<DatabaseConnection>> {
    let db = crate::database::init_database(&config.database.url)
        .await
        .inspect_err(|err| {
            log_proxy_error(
                "system",
                LogStage::Startup,
                LogComponent::ServerSetup,
                "init_db_fail",
                "❌ 数据库连接失败",
                err,
                &[],
            );
        })?;
    Ok(Arc::new(db))
}

/// 运行数据库迁移
async fn run_migrations(db: &DatabaseConnection) -> Result<()> {
    crate::database::run_migrations(db)
        .await
        .inspect_err(|err| {
            log_proxy_error(
                "system",
                LogStage::Startup,
                LogComponent::ServerSetup,
                "run_migrations_fail",
                "❌ 数据库迁移失败",
                err,
                &[],
            );
        })?;
    Ok(())
}

/// 加载配置并初始化数据库
async fn setup_database() -> Result<(Arc<AppConfig>, Arc<DatabaseConnection>)> {
    let config = load_config()?;
    let db = init_database(&config).await?;
    run_migrations(&db).await?;

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "database_ready",
        "✅ Database ready"
    );

    Ok((config, db))
}

/// 构建管理端状态
fn build_management_state(app_context: Arc<AppContext>) -> Result<Arc<ManagementState>> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_management_state",
        "[INIT] Initializing management services state..."
    );

    let state = ManagementState::new(app_context)?;

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_management_state_ok",
        "[OK] Management services state initialized successfully"
    );

    Ok(Arc::new(state))
}

/// 构建代理端状态
fn build_proxy_state(app_context: &Arc<AppContext>) -> Arc<ProxyState> {
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_proxy_services",
        "[INIT] Initializing proxy-specific services (ProxyState)..."
    );

    let resources = app_context.resources();
    let services_ctx = app_context.services();

    let db = resources.database();
    let auth_service = services_ctx.api_key_authentication_service();
    let cache_manager = resources.cache();
    let api_key_scheduler_service = services_ctx.api_key_scheduler_service();
    let rate_limiter = services_ctx.api_key_rate_limit_service();
    let trace_system = services_ctx.api_key_trace_service();

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
        db,
        cache_manager,
        api_key_scheduler_service.clone(),
        rate_limiter.clone(),
    ));

    let services = ProxyServices {
        auth_service: proxy_auth_service,
        collect_service,
        trace_manager,
        upstream_service,
        req_transform_service,
        resp_transform_service,
        key_scheduler_service: api_key_scheduler_service,
        rate_limiter,
    };

    let proxy_state = Arc::new(ProxyState::new(app_context.clone(), services));

    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::ServerSetup,
        "init_proxy_services_ok",
        "✅ Proxy-specific services (ProxyState) initialized successfully"
    );

    proxy_state
}
