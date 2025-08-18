//! # 管理API模块
//!
//! 提供RESTful API接口用于系统管理和监控

pub mod server;
pub mod handlers;
pub mod routes;
pub mod middleware;
pub mod response;

pub use server::{ManagementServer, ManagementConfig};
pub use handlers::*;
pub use routes::create_routes;