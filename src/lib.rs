//! # AI Proxy System Library
//!
//! 企业级AI服务代理平台核心库

pub mod config;
pub mod database;
pub mod auth;
pub mod proxy;
pub mod management;
pub mod scheduler;
pub mod health;
pub mod statistics;
pub mod tls;
pub mod providers;
pub mod cache;
pub mod trace;
pub mod utils;
pub mod error;
pub mod dual_port_setup;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

// Re-export commonly used types
pub use error::{ProxyError, Result};
pub use config::AppConfig;