//! # AI Proxy System Library
//!
//! 企业级AI服务代理平台核心库

pub mod auth;
pub mod cache;
pub mod config;
pub mod database;
pub mod dual_port_setup;
pub mod error;
pub mod health;
pub mod management;
pub mod providers;
pub mod proxy;
pub mod scheduler;
pub mod statistics;
pub mod tls;
pub mod trace;
pub mod utils;

// Re-export commonly used types
pub use config::AppConfig;
pub use error::{ProxyError, Result};
