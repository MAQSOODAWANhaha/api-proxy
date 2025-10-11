//! # 系统信息处理器

use crate::management::response;
use crate::management::server::AppState;
use axum::extract::State;
use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use sysinfo::{Disks, System};

/// 全局启动时间
static START_TIME: OnceLock<Instant> = OnceLock::new();
/// 全局系统信息
static SYS_INFO: OnceLock<Mutex<System>> = OnceLock::new();

/// 初始化启动时间
pub fn init_start_time() {
    START_TIME.set(Instant::now()).ok();
}

/// 获取系统信息实例
fn get_sys() -> &'static Mutex<System> {
    SYS_INFO.get_or_init(|| Mutex::new(System::new()))
}

#[derive(Serialize)]
struct SystemInfo {
    service: ServiceInfo,
    runtime: RuntimeInfo,
    configuration: ConfigurationInfo,
}

#[derive(Serialize)]
struct ServiceInfo {
    name: &'static str,
    version: &'static str,
    build_time: &'static str,
    git_commit: &'static str,
}

#[derive(Serialize)]
struct RuntimeInfo {
    uptime_seconds: u64,
    rust_version: &'static str,
    target: &'static str,
}

#[derive(Serialize)]
struct ConfigurationInfo {
    management_port: u16,
    proxy_port: u16,
    workers: usize,
    database_url: String,
}

/// 获取系统信息
pub async fn get_system_info(State(state): State<AppState>) -> axum::response::Response {
    let system_info = SystemInfo {
        service: ServiceInfo {
            name: "AI Proxy",
            version: env!("CARGO_PKG_VERSION"),
            build_time: option_env!("BUILD_TIME").unwrap_or("unknown"),
            git_commit: option_env!("GIT_COMMIT").unwrap_or("unknown"),
        },
        runtime: RuntimeInfo {
            uptime_seconds: get_uptime_seconds(),
            rust_version: option_env!("CARGO_PKG_RUST_VERSION").unwrap_or("unknown"),
            target: std::env::consts::ARCH,
        },
        configuration: ConfigurationInfo {
            management_port: state.config.get_management_port(),
            proxy_port: state.config.get_proxy_port(),
            workers: state.config.dual_port.as_ref().map_or(1, |d| d.workers),
            database_url: mask_sensitive_info(&state.config.database.url),
        },
    };

    response::success(system_info)
}

/// 系统监控指标 - 匹配API文档格式
#[derive(Serialize)]
struct SystemMetrics {
    cpu_usage: f32,
    memory: MemoryMetrics,
    disk: DiskMetrics,
    uptime: String,
}

#[derive(Serialize)]
struct MemoryMetrics {
    total_mb: u64,
    used_mb: u64,
    usage_percentage: f32,
}

#[derive(Serialize)]
struct DiskMetrics {
    total_gb: u64,
    used_gb: u64,
    usage_percentage: f32,
}

/// 获取系统指标 - 匹配API文档格式
pub async fn get_system_metrics(State(_state): State<AppState>) -> axum::response::Response {
    // The metrics gathering can be blocking, so it's run in a blocking task.
    let metrics = tokio::task::spawn_blocking(move || {
        let mut sys = get_sys().lock().unwrap();

        // Refresh CPU and memory information.
        // Note: The first call to `global_cpu_usage` will be 0.
        sys.refresh_cpu_all();
        sys.refresh_memory();

        // CPU
        let cpu_usage = sys.global_cpu_usage();

        // Memory
        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let memory = MemoryMetrics {
            total_mb: total_memory / 1024 / 1024,
            used_mb: used_memory / 1024 / 1024,
            usage_percentage: if total_memory == 0 {
                0.0
            } else {
                (used_memory as f32 / total_memory as f32) * 100.0
            },
        };

        // Disk: Handled by the separate `Disks` struct
        let disks = Disks::new_with_refreshed_list();
        let (total_disk, used_disk) = disks
            .iter()
            .find(|d| d.mount_point() == std::path::Path::new("/"))
            .map(|d| (d.total_space(), d.total_space() - d.available_space()))
            .unwrap_or_else(|| {
                disks.iter().fold((0, 0), |(total, used), disk| {
                    (
                        total + disk.total_space(),
                        used + (disk.total_space() - disk.available_space()),
                    )
                })
            });

        let disk = DiskMetrics {
            total_gb: total_disk / 1024 / 1024 / 1024,
            used_gb: used_disk / 1024 / 1024 / 1024,
            usage_percentage: if total_disk == 0 {
                0.0
            } else {
                (used_disk as f32 / total_disk as f32) * 100.0
            },
        };

        SystemMetrics {
            cpu_usage,
            memory,
            disk,
            uptime: format_uptime(get_uptime_seconds()),
        }
    })
    .await
    .unwrap();

    response::success(metrics)
}

/// 根路径处理器（管理API信息）
pub async fn root_handler() -> axum::response::Response {
    response::success(serde_json::json!({
        "success": true,
        "message": "AI Proxy Management API",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Ping 处理器
pub async fn ping_handler() -> &'static str {
    "pong"
}

/// 获取程序运行时间（秒）- 自动初始化
fn get_uptime_seconds() -> u64 {
    // 第一次调用时自动初始化启动时间
    let start_time = START_TIME.get_or_init(Instant::now);
    start_time.elapsed().as_secs()
}

/// 格式化运行时间为可读字符串
fn format_uptime(uptime_seconds: u64) -> String {
    let days = uptime_seconds / 86_400;
    let hours = (uptime_seconds % 86_400) / 3_600;
    let minutes = (uptime_seconds % 3_600) / 60;
    let seconds = uptime_seconds % 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m {seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

/// 掩盖敏感信息
fn mask_sensitive_info(url: &str) -> String {
    if url.contains("://") {
        if let Some(at_pos) = url.find('@') {
            if let Some(scheme_end) = url.find("://") {
                let scheme = &url[..scheme_end + 3];
                let after_at = &url[at_pos + 1..];
                format!("{scheme}***:***@{after_at}")
            } else {
                url.to_string()
            }
        } else {
            url.to_string()
        }
    } else {
        "***".to_string()
    }
}
