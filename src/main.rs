//! # AI Proxy 主程序
//!
//! 企业级 AI 服务代理平台 - 基于 Pingora 的高性能代理服务

use api_proxy::{
    Result, dual_port_setup, linfo,
    logging::{self, LogComponent, LogStage},
};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志系统
    logging::init_optimized_logging(None);

    // 初始化管理端系统启动时间（用于 /api/system/metrics uptime）
    // 确保在进程启动时即记录，而非在首次 API 调用时懒初始化
    api_proxy::management::handlers::system::init_start_time();

    // 启动服务
    linfo!(
        "system",
        LogStage::Startup,
        LogComponent::Main,
        "service_starting",
        "服务启动"
    );
    dual_port_setup::run_dual_port_servers().await?;

    linfo!(
        "system",
        LogStage::Shutdown,
        LogComponent::Main,
        "service_shutdown",
        "服务正常关闭"
    );
    Ok(())
}
