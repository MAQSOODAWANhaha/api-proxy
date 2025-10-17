//! Collect 模块聚合
//!
//! 负责请求/响应数据的采集与解析，供后续 Trace 模块使用。

pub mod field_extractor;
pub mod request;
pub mod response;
pub mod service;
pub mod types;
pub mod usage_model;
pub mod util;
