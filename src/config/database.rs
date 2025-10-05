//! # 数据库配置

use crate::error::{ProxyError, Result};
use crate::{linfo, logging::{LogComponent, LogStage}};
use serde::{Deserialize, Serialize};
use std::path::Path;

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

impl DatabaseConfig {
    /// 确保数据库路径存在（仅对SQLite文件数据库）
    pub fn ensure_database_path(&self) -> Result<()> {
        if self.url.starts_with("sqlite://") && !self.url.contains(":memory:") {
            // 提取文件路径
            let path_str = self.url.strip_prefix("sqlite://").unwrap_or(&self.url);
            let db_path = Path::new(path_str);

            // 创建父目录
            if let Some(parent) = db_path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        ProxyError::config_with_source(
                            format!("无法创建数据库目录: {}", parent.display()),
                            e,
                        )
                    })?;

                    linfo!("system", LogStage::Startup, LogComponent::Database, "create_db_dir", &format!("创建数据库目录: {}", parent.display()));
                }
            }

            // 如果数据库文件不存在，记录将要创建的信息
            if !db_path.exists() {
                linfo!("system", LogStage::Startup, LogComponent::Database, "create_db_file_info", &format!("数据库文件将在首次连接时创建: {}", db_path.display()));
            }
        }

        Ok(())
    }

    /// 获取准备好的数据库连接字符串
    pub fn get_connection_url(&self) -> Result<String> {
        self.ensure_database_path()?;
        Ok(self.url.clone())
    }

    /// 检查是否为内存数据库
    pub fn is_memory_database(&self) -> bool {
        self.url.contains(":memory:")
    }

    /// 检查是否为SQLite数据库
    pub fn is_sqlite(&self) -> bool {
        self.url.starts_with("sqlite://")
    }
}