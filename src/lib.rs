#![allow(missing_docs)]
#![allow(dead_code)]
//! # AI Proxy System Library
//!
//! 企业级AI服务代理平台核心库

pub mod app;
pub mod auth;
pub mod cache;
pub mod config;
pub mod database;
/// 双端口服务器设置和配置模块
pub mod dual_port_setup;
pub mod error;
pub mod logging;
pub mod management;
pub mod pricing;
pub mod proxy;
pub mod scheduler;
pub mod statistics;
pub mod trace;
pub mod types;
/// 通用工具和辅助函数模块
pub mod utils;

// Re-export commonly used types
pub use config::AppConfig;
pub use error::{ProxyError, Result};
