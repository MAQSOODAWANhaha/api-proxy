//! # 管理API模块
//!
//! `提供RESTful` API接口用于系统管理和监控

pub mod handlers;
pub mod middleware;
pub mod response;
pub mod routes;
pub mod server;
pub mod services;

pub use handlers::*;
pub use routes::create_routes;
pub use server::{ManagementConfig, ManagementServer};
