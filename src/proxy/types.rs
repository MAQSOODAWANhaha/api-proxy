//! # 代理服务类型定义
//!
//! 提供代理服务中使用的核心类型定义

use serde::{Deserialize, Serialize};

/// 提供商标识符 - 基于数据库主键的动态标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderId(pub i32);

impl ProviderId {
    /// 从数据库ID创建提供商标识
    pub fn from_database_id(id: i32) -> Self {
        Self(id)
    }

    /// 获取数据库ID
    pub fn id(&self) -> i32 {
        self.0
    }

    /// 转换为字符串形式（用于日志等）
    pub fn as_string(&self) -> String {
        format!("provider_{}", self.0)
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "provider_{}", self.0)
    }
}

impl From<i32> for ProviderId {
    fn from(id: i32) -> Self {
        Self(id)
    }
}

impl From<ProviderId> for i32 {
    fn from(provider_id: ProviderId) -> Self {
        provider_id.0
    }
}

/// 简化的请求转发结果
#[derive(Debug, Clone)]
pub struct ForwardingResult {
    /// 请求是否成功
    pub success: bool,
    /// 响应状态码
    pub status_code: u16,
    /// 响应时间
    pub response_time: std::time::Duration,
    /// 使用的提供商ID
    pub provider_id: ProviderId,
    /// 错误信息（如果有）
    pub error_message: Option<String>,
    /// 传输的字节数
    pub bytes_transferred: u64,
}

/// 简化的转发上下文
#[derive(Debug, Clone)]
pub struct ForwardingContext {
    /// 请求ID
    pub request_id: String,
    /// 提供商ID
    pub provider_id: ProviderId,
    /// 请求开始时间
    pub start_time: std::time::Instant,
}

impl ForwardingContext {
    /// 创建新的转发上下文
    pub fn new(request_id: String, provider_id: ProviderId) -> Self {
        Self {
            request_id,
            provider_id,
            start_time: std::time::Instant::now(),
        }
    }
}