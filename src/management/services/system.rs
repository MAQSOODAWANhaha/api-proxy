//! # 系统信息服务
//!
//! 提供管理端系统信息、运行指标等业务逻辑，供 handler 复用。

use std::{
    sync::{Mutex, OnceLock},
    time::Instant,
};

use chrono::Utc;
use chrono_tz::Tz;
use serde::Serialize;
use sysinfo::{Disks, System};
use tokio::task;

use crate::error::Result;
use crate::logging::{LogComponent, LogStage};
use crate::lwarn;
use crate::management::server::ManagementState;
use crate::types::timezone_utils;

use super::shared::metrics::ratio_as_percentage;

/// 启动时间及系统信息的全局缓存
static START_TIME: OnceLock<Instant> = OnceLock::new();
static SYS_INFO: OnceLock<Mutex<System>> = OnceLock::new();

#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub service: ServiceInfo,
    pub runtime: RuntimeInfo,
    pub configuration: ConfigurationInfo,
}

#[derive(Debug, Serialize)]
pub struct ServiceInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub build_time: &'static str,
    pub git_commit: &'static str,
}

#[derive(Debug, Serialize)]
pub struct RuntimeInfo {
    pub uptime_seconds: u64,
    pub rust_version: &'static str,
    pub target: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ConfigurationInfo {
    pub management_port: u16,
    pub proxy_port: u16,
    pub workers: usize,
    pub database_url: String,
}

#[derive(Debug, Serialize)]
pub struct SystemMetrics {
    pub cpu_usage: f32,
    pub memory: MemoryMetrics,
    pub disk: DiskMetrics,
    pub uptime: String,
}

#[derive(Debug, Serialize)]
pub struct MemoryMetrics {
    pub total_mb: u64,
    pub used_mb: u64,
    pub usage_percentage: f64,
}

#[derive(Debug, Serialize)]
pub struct DiskMetrics {
    pub total_gb: u64,
    pub used_gb: u64,
    pub usage_percentage: f64,
}

/// 初始化启动时间缓存。
pub fn init_start_time() {
    START_TIME.set(Instant::now()).ok();
}

/// 构建系统信息，供 handler 返回。
#[must_use]
pub fn build_system_info(state: &ManagementState) -> SystemInfo {
    SystemInfo {
        service: ServiceInfo {
            name: "AI Proxy",
            version: env!("CARGO_PKG_VERSION"),
            build_time: option_env!("BUILD_TIME").unwrap_or("unknown"),
            git_commit: option_env!("GIT_COMMIT").unwrap_or("unknown"),
        },
        runtime: RuntimeInfo {
            uptime_seconds: uptime_seconds(),
            rust_version: option_env!("CARGO_PKG_RUST_VERSION").unwrap_or("unknown"),
            target: std::env::consts::ARCH,
        },
        configuration: ConfigurationInfo {
            management_port: state.config.get_management_port(),
            proxy_port: state.config.get_proxy_port(),
            workers: state.config.dual_port.as_ref().map_or(1, |d| d.workers),
            database_url: mask_sensitive_info(&state.config.database.url),
        },
    }
}

/// 收集系统运行指标。
pub async fn collect_system_metrics() -> Result<SystemMetrics> {
    task::spawn_blocking(|| {
        let mut sys = get_sys().lock().expect("system info mutex poisoned");
        sys.refresh_cpu_all();
        sys.refresh_memory();

        let cpu_usage = sys.global_cpu_usage();
        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();

        // 释放锁，避免磁盘信息阻塞其它线程。
        drop(sys);

        let memory = MemoryMetrics {
            total_mb: total_memory / 1024 / 1024,
            used_mb: used_memory / 1024 / 1024,
            usage_percentage: ratio_as_percentage(used_memory, total_memory),
        };

        let disks = Disks::new_with_refreshed_list();
        let (total_disk, used_disk) = disks
            .iter()
            .find(|d| d.mount_point() == std::path::Path::new("/"))
            .map_or_else(
                || {
                    disks.iter().fold((0, 0), |(total, used), disk| {
                        (
                            total + disk.total_space(),
                            used + (disk.total_space() - disk.available_space()),
                        )
                    })
                },
                |d| (d.total_space(), d.total_space() - d.available_space()),
            );

        let disk = DiskMetrics {
            total_gb: total_disk / 1024 / 1024 / 1024,
            used_gb: used_disk / 1024 / 1024 / 1024,
            usage_percentage: ratio_as_percentage(used_disk, total_disk),
        };

        SystemMetrics {
            cpu_usage,
            memory,
            disk,
            uptime: format_uptime(uptime_seconds()),
        }
    })
    .await
    .map_err(|err| {
        lwarn!(
            "system",
            LogStage::Internal,
            LogComponent::Main,
            "system_metrics_collect_join_fail",
            &format!("Failed to join system metrics task: {err}")
        );
        crate::error!(Internal, "Failed to collect system metrics")
    })
}

/// 构建管理根信息。
#[must_use]
pub fn build_root_metadata(timezone: &Tz) -> serde_json::Value {
    serde_json::json!({
        "success": true,
        "message": "AI Proxy Management API",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": timezone_utils::format_utc_for_response(&Utc::now(), timezone)
    })
}

fn get_sys() -> &'static Mutex<System> {
    SYS_INFO.get_or_init(|| Mutex::new(System::new()))
}

fn uptime_seconds() -> u64 {
    let start = START_TIME.get_or_init(Instant::now);
    start.elapsed().as_secs()
}

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

fn mask_sensitive_info(url: &str) -> String {
    if !url.contains("://") {
        return "***".to_string();
    }

    url.find('@').map_or_else(
        || url.to_string(),
        |at_pos| {
            url.find("://").map_or_else(
                || url.to_string(),
                |scheme_end| {
                    let scheme = &url[..scheme_end + 3];
                    let after_at = &url[at_pos + 1..];
                    format!("{scheme}***:***@{after_at}")
                },
            )
        },
    )
}
