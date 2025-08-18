//! # 系统信息处理器

use crate::management::response;
use crate::management::server::AppState;
use axum::extract::State;
use axum::response::IntoResponse;
use chrono::Utc;
use serde::Serialize;
use std::fs;
use std::sync::OnceLock;
use std::time::Instant;

/// 全局启动时间
static START_TIME: OnceLock<Instant> = OnceLock::new();

/// 初始化启动时间
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
pub async fn get_system_info(State(state): State<AppState>) -> impl IntoResponse {
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

#[derive(Serialize)]
struct SystemMetrics {
    memory: MemoryInfo,
    cpu: CpuInfo,
    network: NetworkInfo,
    process: ProcessInfo,
    timestamp: chrono::DateTime<Utc>,
}

#[derive(Serialize)]
struct MemoryInfo {
    total: u64,
    used: u64,
    available: u64,
    usage_percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct CpuInfo {
    load_average: Vec<f64>,
    cores: usize,
    usage_percent: f64, // Placeholder
}

#[derive(Serialize)]
struct ProcessInfo {
    pid: u32,
    threads: u32,
    file_descriptors: u32,
    uptime_seconds: u64,
}

#[derive(Serialize)]
struct NetworkInfo {
    bytes_sent: u64,
    bytes_received: u64,
    connections_active: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// 获取系统指标
pub async fn get_system_metrics(State(_state): State<AppState>) -> impl IntoResponse {
    let metrics = SystemMetrics {
        memory: get_memory_info().await,
        cpu: get_cpu_info().await,
        network: get_network_info().await,
        process: get_process_info().await,
        timestamp: Utc::now(),
    };

    response::success(metrics)
}

/// 获取运行时间（秒）
fn get_uptime_seconds() -> u64 {
    if let Some(start_time) = START_TIME.get() {
        start_time.elapsed().as_secs()
    } else {
        0
    }
}

/// 获取内存信息
async fn get_memory_info() -> MemoryInfo {
    match get_memory_stats() {
        Ok((total, available)) => {
            let used = total.saturating_sub(available);
            MemoryInfo {
                total,
                used,
                available,
                usage_percent: if total > 0 {
                    (used as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
                error: None,
            }
        }
        Err(_) => MemoryInfo {
            total: 0,
            used: 0,
            available: 0,
            usage_percent: 0.0,
            error: Some("Failed to read memory info".to_string()),
        },
    }
}

/// 获取CPU信息
async fn get_cpu_info() -> CpuInfo {
    CpuInfo {
        load_average: get_load_average(),
        cores: num_cpus::get(),
        usage_percent: 0.0, // 实时CPU使用率需要更复杂的实现
    }
}

/// 获取进程信息
async fn get_process_info() -> ProcessInfo {
    ProcessInfo {
        pid: std::process::id(),
        threads: get_thread_count(),
        file_descriptors: get_fd_count(),
        uptime_seconds: get_uptime_seconds(),
    }
}

/// 获取网络信息
async fn get_network_info() -> NetworkInfo {
    match get_network_stats() {
        Ok((bytes_sent, bytes_received)) => NetworkInfo {
            bytes_sent,
            bytes_received,
            connections_active: get_active_connections(),
            error: None,
        },
        Err(_) => NetworkInfo {
            bytes_sent: 0,
            bytes_received: 0,
            connections_active: 0,
            error: Some("Failed to read network stats".to_string()),
        },
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

/// 获取线程数
fn get_thread_count() -> u32 {
    let pid = std::process::id();
    match fs::read_to_string(format!("/proc/{}/stat", pid)) {
        Ok(content) => {
            // stat文件的第20个字段是线程数
            content
                .split_whitespace()
                .nth(19)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        }
        Err(_) => 0,
    }
}

/// 获取文件描述符数量
fn get_fd_count() -> u32 {
    let pid = std::process::id();
    match fs::read_dir(format!("/proc/{}/fd", pid)) {
        Ok(entries) => entries.count() as u32,
        Err(_) => 0,
    }
}

/// 获取网络统计信息
fn get_network_stats() -> Result<(u64, u64), std::io::Error> {
    let netdev = fs::read_to_string("/proc/net/dev")?;
    let mut bytes_sent = 0;
    let mut bytes_received = 0;

    for line in netdev.lines().skip(2) {
        // 跳过头两行
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 10 && !fields[0].starts_with("lo:") {
            // 排除回环接口
            if let (Ok(rx), Ok(tx)) = (fields[1].parse::<u64>(), fields[9].parse::<u64>()) {
                bytes_received += rx;
                bytes_sent += tx;
            }
        }
    }

    Ok((bytes_sent, bytes_received))
}

/// 获取活跃连接数
fn get_active_connections() -> u32 {
    // 读取TCP连接状态
    let tcp_v4 = count_tcp_connections("/proc/net/tcp");
    let tcp_v6 = count_tcp_connections("/proc/net/tcp6");
    tcp_v4 + tcp_v6
}

/// 计算TCP连接数
fn count_tcp_connections(path: &str) -> u32 {
    match fs::read_to_string(path) {
        Ok(content) => {
            content
                .lines()
                .skip(1) // 跳过标题行
                .filter(|line| {
                    // 只计算已建立的连接 (状态 01 表示 ESTABLISHED)
                    line.split_whitespace().nth(3).map_or(false, |s| s == "01")
                })
                .count() as u32
        }
        Err(_) => 0,
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
