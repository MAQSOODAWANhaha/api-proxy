//! # 数据库配置

use serde::{Deserialize, Serialize};

/// 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// 数据库URL
    pub url: String,
    /// 最大连接数
    pub max_connections: u32,
    /// 连接超时时间（秒）
    pub connect_timeout: u64,
    /// 查询超时时间（秒）
    pub query_timeout: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://./data/api_proxy.db".to_string(),
            max_connections: 10,
            connect_timeout: 30,
            query_timeout: 60,
        }
    }
}