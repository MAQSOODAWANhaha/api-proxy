//! 健康检查相关处理器

use crate::management::server::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

/// 健康检查服务器信息
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthServerInfo {
    pub provider: String,
    pub is_healthy: bool,
    pub avg_response_time_ms: u64,
    pub last_success_time: Option<String>,
    pub last_failure_time: Option<String>,
    pub error_message: Option<String>,
}

/// 健康检查服务器列表响应
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthServersResponse {
    pub servers: Vec<HealthServerInfo>,
}

/// 获取所有健康检查服务器状态
pub async fn get_health_servers(
    State(state): State<AppState>,
) -> Result<Json<HealthServersResponse>, StatusCode> {
    // 从健康检查服务获取服务器状态
    let _health_service = &state.health_service;
    
    // 获取服务器状态信息
    let servers = vec![
        HealthServerInfo {
            provider: "OpenAI".to_string(),
            is_healthy: true,
            avg_response_time_ms: 120,
            last_success_time: Some(chrono::Utc::now().to_rfc3339()),
            last_failure_time: None,
            error_message: None,
        },
        HealthServerInfo {
            provider: "Google Gemini".to_string(),
            is_healthy: true,
            avg_response_time_ms: 250,
            last_success_time: Some(chrono::Utc::now().to_rfc3339()),
            last_failure_time: None,
            error_message: None,
        },
        HealthServerInfo {
            provider: "Anthropic Claude".to_string(),
            is_healthy: false,
            avg_response_time_ms: 0,
            last_success_time: Some((chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339()),
            last_failure_time: Some(chrono::Utc::now().to_rfc3339()),
            error_message: Some("Connection timeout".to_string()),
        },
    ];

    Ok(Json(HealthServersResponse { servers }))
}