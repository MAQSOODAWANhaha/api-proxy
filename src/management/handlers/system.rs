//! # 系统信息处理器

use crate::management::response;
use crate::management::server::AppState;
use axum::extract::State;
use chrono::Utc;
use serde::Serialize;
use std::fs;
use std::sync::OnceLock;
use std::time::Instant;

/// 全局启动时间
static START_TIME: OnceLock<Instant> = OnceLock::new();

/// 初始化启动时间（现在不需要手动调用）
pub fn init_start_time() {
    START_TIME.set(Instant::now()).ok();
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
    server_port: u16,
    https_port: u16,
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
            server_port: state.config.server.as_ref().map_or(8080, |s| s.port),
            https_port: state.config.server.as_ref().map_or(0, |s| s.https_port),
            workers: state.config.server.as_ref().map_or(1, |s| s.workers),
            database_url: mask_sensitive_info(&state.config.database.url),
        },
    };

    response::success(system_info)
}

/// 系统监控指标 - 匹配API文档格式
#[derive(Serialize)]
struct SystemMetrics {
    cpu_usage: f64,
    memory: MemoryMetrics,
    disk: DiskMetrics,
    uptime: String,
}

#[derive(Serialize)]
struct MemoryMetrics {
    total_mb: u64,
    used_mb: u64, 
    usage_percentage: f64,
}

#[derive(Serialize)]
struct DiskMetrics {
    total_gb: u64,
    used_gb: u64,
    usage_percentage: f64,
}

/// 获取系统指标 - 匹配API文档格式
pub async fn get_system_metrics(State(_state): State<AppState>) -> axum::response::Response {
    let metrics = SystemMetrics {
        cpu_usage: get_cpu_usage().await,
        memory: get_memory_metrics().await,
        disk: get_disk_metrics().await,
        uptime: format_uptime(get_uptime_seconds()),
    };

    response::success(metrics)
}

/// 获取程序运行时间（秒）- 自动初始化
fn get_uptime_seconds() -> u64 {
    // 第一次调用时自动初始化启动时间
    let start_time = START_TIME.get_or_init(|| Instant::now());
    start_time.elapsed().as_secs()
}

/// 获取CPU使用率
async fn get_cpu_usage() -> f64 {
    // 基于load_average计算近似CPU使用率
    let load_avg = get_load_average();
    if !load_avg.is_empty() {
        let cores = num_cpus::get() as f64;
        let load_1min = load_avg[0];
        // 将负载平均值转换为百分比（相对于核心数）
        ((load_1min / cores) * 100.0).min(100.0)
    } else {
        0.0
    }
}

/// 获取内存指标 - 转换为MB单位
async fn get_memory_metrics() -> MemoryMetrics {
    match get_memory_stats() {
        Ok((total, available)) => {
            let used = total.saturating_sub(available);
            let total_mb = total / (1024 * 1024); // 字节转MB
            let used_mb = used / (1024 * 1024);
            
            MemoryMetrics {
                total_mb,
                used_mb,
                usage_percentage: if total > 0 {
                    (used as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
            }
        }
        Err(_) => MemoryMetrics {
            total_mb: 0,
            used_mb: 0,
            usage_percentage: 0.0,
        },
    }
}

/// 获取磁盘指标
async fn get_disk_metrics() -> DiskMetrics {
    match get_disk_stats() {
        Ok((total, used)) => {
            let total_gb = total / (1024 * 1024 * 1024); // 字节转GB
            let used_gb = used / (1024 * 1024 * 1024);
            
            DiskMetrics {
                total_gb,
                used_gb,
                usage_percentage: if total > 0 {
                    (used as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
            }
        }
        Err(_) => DiskMetrics {
            total_gb: 0,
            used_gb: 0,
            usage_percentage: 0.0,
        },
    }
}

/// 格式化运行时间为可读字符串
fn format_uptime(uptime_seconds: u64) -> String {
    let days = uptime_seconds / 86400;
    let hours = (uptime_seconds % 86400) / 3600;
    let minutes = (uptime_seconds % 3600) / 60;
    
    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

/// 获取内存统计信息
fn get_memory_stats() -> Result<(u64, u64), std::io::Error> {
    let meminfo = fs::read_to_string("/proc/meminfo")?;
    let mut total = 0;
    let mut available = 0;

    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            if let Some(value) = extract_memory_value(line) {
                total = value * 1024; // 转换为字节
            }
        } else if line.starts_with("MemAvailable:") {
            if let Some(value) = extract_memory_value(line) {
                available = value * 1024; // 转换为字节
            }
        }
    }

    Ok((total, available))
}

/// 提取内存数值
fn extract_memory_value(line: &str) -> Option<u64> {
    line.split_whitespace().nth(1).and_then(|s| s.parse().ok())
}

/// 获取磁盘使用统计信息 - 安全版本
fn get_disk_stats() -> Result<(u64, u64), std::io::Error> {
    // 尝试读取 /proc/mounts 找到根分区
    match fs::read_to_string("/proc/mounts") {
        Ok(mounts_content) => {
            // 查找根分区或第一个真实文件系统
            for line in mounts_content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let mount_point = parts[1];
                    let fs_type = parts[2];
                    
                    // 跳过虚拟文件系统
                    if mount_point == "/" 
                        && !["tmpfs", "proc", "sysfs", "devpts", "devtmpfs"].contains(&fs_type) {
                        // 尝试通过读取 /proc/diskstats 获取磁盘使用信息
                        return get_disk_usage_from_diskstats();
                    }
                }
            }
            // 如果没找到合适的分区，使用回退方案
            get_disk_stats_simulation()
        }
        Err(_) => get_disk_stats_simulation(),
    }
}

/// 从 /proc/diskstats 读取磁盘使用信息
fn get_disk_usage_from_diskstats() -> Result<(u64, u64), std::io::Error> {
    // 在实际环境中，这里需要更复杂的逻辑来解析diskstats
    // 由于这涉及复杂的系统调用和计算，我们暂时使用模拟数据
    // TODO: 实现真实的磁盘使用率检测
    get_disk_stats_simulation()
}

/// 磁盘统计信息模拟方案 - 用于开发阶段
fn get_disk_stats_simulation() -> Result<(u64, u64), std::io::Error> {
    // 模拟一个合理的磁盘使用情况
    let total_bytes = 500_000_000_000u64; // 500GB
    let used_bytes = 250_000_000_000u64;  // 250GB (50% 使用率)
    Ok((total_bytes, used_bytes))
}

/// 获取负载平均值
fn get_load_average() -> Vec<f64> {
    match fs::read_to_string("/proc/loadavg") {
        Ok(content) => content
            .split_whitespace()
            .take(3)
            .filter_map(|s| s.parse().ok())
            .collect(),
        Err(_) => vec![0.0, 0.0, 0.0],
    }
}


/// 掩盖敏感信息
fn mask_sensitive_info(url: &str) -> String {
    if url.contains("://") {
        if let Some(at_pos) = url.find('@') {
            if let Some(scheme_end) = url.find("://") {
                let scheme = &url[..scheme_end + 3];
                let after_at = &url[at_pos + 1..];
                format!("{}***:***@{}", scheme, after_at)
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
